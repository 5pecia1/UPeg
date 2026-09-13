//! `pty = true`: give an External child a real terminal instead of two
//! pipes.
//!
//! Everything upeg captures normally goes through a pair of pipes, and a
//! well-behaved CLI seeing a pipe on fd 1 turns its color — and often
//! its whole human-oriented rendering — off. [`super::color`] answers
//! that with the environment convention, which is enough for `cargo`,
//! `gh`, and every chalk-based tool. It is *not* enough for a program
//! that calls `isatty(3)` and nothing else: `git`, `ls`, `grep`, and
//! anything drawing a TUI box measured against `TIOCGWINSZ`.
//!
//! This module is the other answer. It opens a pseudoterminal, hands the
//! child side to the child as fd 1 and fd 2, and keeps the terminal side
//! as the single reader the capture layer drains.
//!
//! Three consequences the rest of the External stack has to live with,
//! and does:
//!
//!   * **One stream, not two.** A pty has one buffer; stdout and stderr
//!     arrive interleaved exactly as the child wrote them. The capture
//!     layer drains it as [`super::error::OutputStream::Stdout`], so the
//!     merged text is what `stdout` carries and `stderr` is empty.
//!   * **EIO is EOF.** When the last child-side descriptor closes, Linux
//!     reports `EIO` on the terminal side rather than a zero-length
//!     read. [`PtyMaster`] translates it, so the drain loop stays
//!     ignorant of the difference.
//!   * **Containment still works.** An escaped descendant is found by
//!     the child side it holds open instead of by pipe inode — by that
//!     terminal's own inode, with `/dev/pts/N` serving as the prefilter.
//!     See [`DeviceIdentity`] and [`super::scope`].
//!
//! stdin is deliberately **not** the terminal. It stays `/dev/null`, as
//! it is for every other External tool: upeg is never the human behind
//! the terminal side, so a child that read from a tty stdin would block
//! on a prompt nobody can answer. `isatty(1)` and `isatty(2)` are true;
//! `isatty(0)` is false.

/// Whether this host can open a pseudoterminal for an External child.
///
/// Read at load time (`crate::parse::HostCapabilities`) so a `pty = true`
/// tool is skipped where it is declared instead of failing at its first
/// dispatch — and only that tool: the manifest around it is correct and
/// loads. Windows would need ConPTY and wasm has no processes at all;
/// neither is implemented.
pub(crate) const HOST_SUPPORTS_PTY: bool = cfg!(unix);

/// What the child's stdout and stderr are connected to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum TerminalMode {
    /// Two pipes, drained separately — the default for every tool that
    /// does not declare `pty`.
    #[default]
    Pipes,
    /// One pseudoterminal, drained as a single merged stream.
    ///
    /// Only constructible on a host where [`HOST_SUPPORTS_PTY`] holds:
    /// the loader skips a tool that asks for a terminal everywhere else,
    /// and this variant does not exist there to be reached by accident.
    #[cfg(unix)]
    Pty,
}

impl TerminalMode {
    /// The mode a manifest's `pty` declaration asks for.
    pub(super) const fn from_declaration(pty: bool) -> Self {
        #[cfg(unix)]
        {
            if pty { Self::Pty } else { Self::Pipes }
        }
        #[cfg(not(unix))]
        {
            let _ = pty;
            Self::Pipes
        }
    }
}

#[cfg(unix)]
mod unix {
    use std::ffi::{OsStr, OsString};
    use std::fs::File;
    use std::io::{self, Read};
    use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
    use std::os::unix::ffi::OsStringExt as _;
    use std::process::Stdio;

    use rustix::fs::{Mode, OFlags};
    use rustix::io::FdFlags;
    use rustix::pty::OpenptFlags;
    use rustix::termios::{LocalModes, OptionalActions, OutputModes};

    use super::super::error::ExternalProcessError;

    /// Named so a failure says which step of the handshake broke —
    /// `posix_openpt` failing on a host out of pty slots reads very
    /// differently from `tcsetattr` failing.
    const OPEN_TERMINAL: &str = "open terminal side";
    const UNLOCK: &str = "unlock";
    const GRANT: &str = "grant";
    const NAME: &str = "name";
    const OPEN_CHILD: &str = "open child side";
    const READ_MODES: &str = "read terminal modes";
    const SET_MODES: &str = "set terminal modes";
    const CLOSE_ON_EXEC: &str = "set close-on-exec";
    const IDENTIFY: &str = "identify child side";

    /// `ptsname` writes into a caller buffer; this is the one it gets.
    /// `/dev/pts/1048575` is 17 bytes, so nothing realistic grows it.
    const PTS_NAME_CAPACITY: usize = 64;

    fn pty_error(operation: &'static str, source: rustix::io::Errno) -> ExternalProcessError {
        ExternalProcessError::Pty { operation, source }
    }

    /// An open pseudoterminal, before the child has been spawned.
    ///
    /// Holds both sides: the terminal side upeg will read, and the child
    /// side it hands over as fd 1 and fd 2. The parent's own child-side
    /// descriptor must be dropped once the child owns its copies — until
    /// it is, the terminal side can never report end-of-file, because
    /// upeg itself is still a holder. [`Self::into_master`] is that
    /// drop.
    pub(crate) struct Pty {
        master: PtyMaster,
        slave: OwnedFd,
    }

    impl Pty {
        /// Allocate a pseudoterminal and put its child side into the
        /// mode a captured run wants.
        pub(crate) fn open() -> Result<Self, ExternalProcessError> {
            let master = rustix::pty::openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)
                .map_err(|source| pty_error(OPEN_TERMINAL, source))?;
            rustix::io::fcntl_setfd(&master, FdFlags::CLOEXEC)
                .map_err(|source| pty_error(CLOSE_ON_EXEC, source))?;
            rustix::pty::grantpt(&master).map_err(|source| pty_error(GRANT, source))?;
            rustix::pty::unlockpt(&master).map_err(|source| pty_error(UNLOCK, source))?;
            let name = rustix::pty::ptsname(&master, Vec::with_capacity(PTS_NAME_CAPACITY))
                .map_err(|source| pty_error(NAME, source))?;
            let slave = rustix::fs::open(
                name.as_c_str(),
                OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| pty_error(OPEN_CHILD, source))?;
            configure_child_side(&slave)?;
            let identity = DeviceIdentity::of(&slave)?;
            Ok(Self {
                master: PtyMaster {
                    terminal: File::from(master),
                    child_device: ChildDevice {
                        path: OsString::from_vec(name.into_bytes()),
                        identity,
                    },
                },
                slave,
            })
        }

        /// One child-side descriptor for the `Command` to install as a
        /// standard stream. Called once per stream, because
        /// [`Stdio`] takes ownership of what it is given.
        pub(crate) fn child_stdio(&self) -> Result<Stdio, ExternalProcessError> {
            self.slave
                .try_clone()
                .map(Stdio::from)
                .map_err(|source| ExternalProcessError::PtyChildStdio { source })
        }

        /// Give up the parent's own child-side descriptor and keep only
        /// the terminal side.
        ///
        /// Must be called *after* the spawn: the child inherits its
        /// copies during the spawn, and until every other holder is gone
        /// the terminal side never reaches end-of-file.
        pub(crate) fn into_master(self) -> PtyMaster {
            drop(self.slave);
            self.master
        }
    }

    /// Put the child side into "captured terminal" mode.
    ///
    /// `OPOST` off is the load-bearing one: with output post-processing
    /// on, the line discipline rewrites every `\n` the child writes into
    /// `\r\n`, and upeg would capture — and every surface would render —
    /// a carriage return the program never emitted. Off, the terminal
    /// side sees exactly the bytes that were written, which is the same
    /// promise the pipe path makes.
    ///
    /// `ECHO` off matters because a pty echoes its input back out; upeg
    /// never writes to the terminal side, so this only guards against a
    /// surprise, but a surprise that would land in captured output.
    fn configure_child_side(slave: &OwnedFd) -> Result<(), ExternalProcessError> {
        let mut modes =
            rustix::termios::tcgetattr(slave).map_err(|source| pty_error(READ_MODES, source))?;
        modes.output_modes -= OutputModes::OPOST;
        modes.local_modes -= LocalModes::ECHO;
        rustix::termios::tcsetattr(slave, OptionalActions::Now, &modes)
            .map_err(|source| pty_error(SET_MODES, source))
    }

    /// Exactly which file the child side is, as a `stat` reports it.
    ///
    /// `rdev` alone does not answer that. A devpts mount is a superblock
    /// of its own, so a container with its own `/dev/pts` has a
    /// `/dev/pts/3` whose character-device number is identical to the
    /// host's — same `rdev`, different terminal. `(st_dev, st_ino)` is
    /// the pair that is unique across every instance, and it is the same
    /// pair the pipe path already matches on; `rdev` stays as the cheap
    /// prefilter that rejects most descriptors without a second look.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct DeviceIdentity {
        /// Superblock the child side lives in — one per devpts instance.
        pub(crate) device: rustix::fs::Dev,
        /// Inode within that superblock, i.e. the pts number.
        pub(crate) inode: u64,
        /// Character-device number, the prefilter.
        pub(crate) rdev: rustix::fs::Dev,
    }

    impl DeviceIdentity {
        /// Read straight off the open child side, before anyone else can
        /// have it. Taking it from the *path* instead would mean
        /// re-resolving `/dev/pts/7` later, which is the very ambiguity
        /// this type exists to remove.
        fn of(slave: &OwnedFd) -> Result<Self, ExternalProcessError> {
            let stat = rustix::fs::fstat(slave).map_err(|source| pty_error(IDENTIFY, source))?;
            Ok(Self {
                device: stat.st_dev,
                inode: stat.st_ino,
                rdev: stat.st_rdev,
            })
        }
    }

    /// The device the child holds open, as a process scanning `/proc`
    /// would see it.
    ///
    /// Recorded while the pty is allocated because the parent drops its
    /// own child-side descriptor at spawn time — but the escaped-process
    /// sweep needs to recognise that device long afterwards. Stable for
    /// the whole invocation: the terminal side stays open in upeg, so
    /// the kernel cannot hand this pts number to anyone else.
    #[derive(Clone, Debug)]
    pub(crate) struct ChildDevice {
        path: OsString,
        identity: DeviceIdentity,
    }

    impl ChildDevice {
        /// Filesystem path of the device — `/dev/pts/7`, which is also
        /// exactly what `/proc/<pid>/fd/N` links to for a holder. A
        /// prefilter only: the path is what a *candidate* claims to be,
        /// and [`Self::identity`] is what it has to prove.
        pub(crate) fn path(&self) -> &OsStr {
            &self.path
        }

        pub(crate) const fn identity(&self) -> DeviceIdentity {
            self.identity
        }
    }

    /// The terminal side of the pty: the single reader that carries
    /// everything the child wrote to either stream.
    pub(crate) struct PtyMaster {
        terminal: File,
        child_device: ChildDevice,
    }

    impl PtyMaster {
        pub(crate) const fn child_device(&self) -> &ChildDevice {
            &self.child_device
        }
    }

    impl Read for PtyMaster {
        /// End-of-file on a pty is spelled `EIO`.
        ///
        /// When the last child-side descriptor closes, Linux fails the
        /// next read with `EIO` instead of returning zero bytes. Every
        /// caller in the capture layer treats a zero-length read as "the
        /// child is done", so the translation belongs here — one place,
        /// rather than an `EIO` special case in the drain loop, the
        /// progress tap, and the timeout path.
        ///
        /// `EAGAIN` is deliberately *not* translated: the drain loop
        /// sets the terminal side non-blocking and relies on
        /// `WouldBlock` to poll for cancellation.
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            match self.terminal.read(buffer) {
                Err(source) if is_end_of_file(&source) => Ok(0),
                other => other,
            }
        }
    }

    impl AsFd for PtyMaster {
        fn as_fd(&self) -> BorrowedFd<'_> {
            self.terminal.as_fd()
        }
    }

    fn is_end_of_file(source: &io::Error) -> bool {
        source.raw_os_error() == Some(rustix::io::Errno::IO.raw_os_error())
    }
}

#[cfg(unix)]
pub(super) use unix::{ChildDevice, DeviceIdentity, Pty, PtyMaster};
