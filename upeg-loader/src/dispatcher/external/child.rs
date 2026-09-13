use std::process::{ChildStderr, ChildStdout, Command, ExitStatus, Stdio};

#[cfg(any(unix, windows))]
use std::io;

use super::error::{ExternalProcessError, OutputStream};
use super::pty::TerminalMode;
#[cfg(unix)]
use super::pty::{Pty, PtyMaster};

#[cfg(any(unix, windows))]
type ManagedChild = command_group::GroupChild;
#[cfg(not(any(unix, windows)))]
type ManagedChild = std::process::Child;

/// What the spawned child writes into, from the parent's side.
///
/// Two pipes or one pseudoterminal — the difference the whole `pty`
/// feature comes down to, expressed once so the capture layer can be
/// written in terms of "however many streams there are" instead of
/// "stdout and stderr".
pub(super) enum ChildOutput {
    /// The default: separate stdout and stderr, drained independently.
    Pipes {
        stdout: ChildStdout,
        stderr: ChildStderr,
    },
    /// `pty = true`: the terminal side of a pseudoterminal, carrying
    /// both of the child's streams interleaved as it wrote them.
    #[cfg(unix)]
    Merged { terminal: PtyMaster },
}

pub(super) struct ChildGuard {
    child: ManagedChild,
    direct_status: Option<ExitStatus>,
    reaped: bool,
}

impl ChildGuard {
    /// Spawn the child into its own process group with its output
    /// connected as `terminal` asks.
    ///
    /// Returns the guard *and* the parent's side of that output,
    /// because the two are created together and neither is usable
    /// alone: a pty's child-side descriptor has to be released between
    /// the spawn and the first read, and the pipes have to be taken out
    /// of the child before anything drains them.
    pub(super) fn spawn(
        command: &mut Command,
        terminal: TerminalMode,
    ) -> Result<(Self, ChildOutput), ExternalProcessError> {
        // stdin is null, never inherited, and never the pty either. An
        // inherited stdin lets `cat`, an interactive `git` credential
        // prompt, or `bash -l` block the calling surface forever — and
        // upeg has no terminal to hand them anyway, since a dispatch can
        // come from the daemon, MCP, or a GUI pin just as easily as from
        // a shell.
        command.stdin(Stdio::null());
        match terminal {
            TerminalMode::Pipes => Self::spawn_piped(command),
            #[cfg(unix)]
            TerminalMode::Pty => Self::spawn_on_pty(command),
        }
    }

    fn spawn_piped(command: &mut Command) -> Result<(Self, ChildOutput), ExternalProcessError> {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut guard = Self::spawn_contained(command)?;
        let stdout =
            guard
                .direct_child()
                .stdout
                .take()
                .ok_or(ExternalProcessError::MissingPipe {
                    stream: OutputStream::Stdout,
                })?;
        let stderr =
            guard
                .direct_child()
                .stderr
                .take()
                .ok_or(ExternalProcessError::MissingPipe {
                    stream: OutputStream::Stderr,
                })?;
        Ok((guard, ChildOutput::Pipes { stdout, stderr }))
    }

    /// `pty = true`: both child streams point at the child side of one
    /// pseudoterminal.
    ///
    /// The parent has to stop holding the child side **twice over**, and
    /// both releases happen here.
    ///
    /// A spawn *duplicates* the [`Stdio`] values it was handed into the
    /// child instead of consuming them, and [`Command`] keeps its own
    /// copies for the next spawn — so the two child-side descriptors
    /// installed above are still open in this process once the child is
    /// running. [`release_child_side`] drops them; [`Pty::into_master`]
    /// drops the one the [`Pty`] itself holds. While *any* of the three
    /// is open the terminal side can never report end-of-file, so
    /// missing either release turns every pty run into a wait for the
    /// escaped-descendant grace period instead of a read that ends when
    /// the child does.
    #[cfg(unix)]
    fn spawn_on_pty(command: &mut Command) -> Result<(Self, ChildOutput), ExternalProcessError> {
        let pty = Pty::open()?;
        command
            .stdout(pty.child_stdio()?)
            .stderr(pty.child_stdio()?);
        // Released on the failure path too: a `Command` the caller still
        // owns must not carry this invocation's terminal away with it.
        let spawned = Self::spawn_contained(command);
        release_child_side(command);
        Ok((
            spawned?,
            ChildOutput::Merged {
                terminal: pty.into_master(),
            },
        ))
    }

    fn spawn_contained(command: &mut Command) -> Result<Self, ExternalProcessError> {
        #[cfg(any(unix, windows))]
        let child = {
            use command_group::CommandGroup as _;
            command
                .group_spawn()
                .map_err(|source| ExternalProcessError::ContainmentSpawn { source })?
        };
        #[cfg(not(any(unix, windows)))]
        let child = command
            .spawn()
            .map_err(|source| ExternalProcessError::Spawn { source })?;
        Ok(Self {
            child,
            direct_status: None,
            reaped: false,
        })
    }

    pub(super) fn poll_direct_exit(&mut self) -> Result<bool, ExternalProcessError> {
        if self.direct_status.is_some() {
            return Ok(true);
        }
        self.direct_status = self
            .direct_child()
            .try_wait()
            .map_err(|source| ExternalProcessError::Wait { source })?;
        Ok(self.direct_status.is_some())
    }

    pub(super) fn wait(&mut self) -> Result<ExitStatus, ExternalProcessError> {
        let waited = self
            .child
            .wait()
            .map_err(|source| ExternalProcessError::Wait { source })?;
        self.reaped = true;
        Ok(self.direct_status.unwrap_or(waited))
    }

    pub(super) fn terminate_and_reap(&mut self) -> Result<(), ExternalProcessError> {
        #[cfg(any(unix, windows))]
        {
            match self.child.kill() {
                Ok(()) => {}
                Err(source) if containment_is_empty(&source) => {}
                Err(source) => {
                    let _ = self.direct_child().kill();
                    let _ = self.wait();
                    return Err(ExternalProcessError::ContainmentTerminate { source });
                }
            }
            self.wait().map(|_| ())
        }
        #[cfg(not(any(unix, windows)))]
        {
            if self
                .child
                .try_wait()
                .map_err(|source| ExternalProcessError::Wait { source })?
                .is_none()
            {
                self.child
                    .kill()
                    .map_err(|source| ExternalProcessError::Terminate { source })?;
            }
            self.wait().map(|_| ())
        }
    }

    fn direct_child(&mut self) -> &mut std::process::Child {
        #[cfg(any(unix, windows))]
        {
            self.child.inner()
        }
        #[cfg(not(any(unix, windows)))]
        {
            &mut self.child
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Give up the parent's copies of whatever the child's streams were
/// connected to.
///
/// `Stdio::null()` is a marker rather than an open descriptor, so this
/// costs nothing and replaces — hence drops — the owned descriptors the
/// `Command` was holding.
#[cfg(unix)]
fn release_child_side(command: &mut Command) {
    command.stdout(Stdio::null()).stderr(Stdio::null());
}

#[cfg(any(unix, windows))]
fn containment_is_empty(error: &io::Error) -> bool {
    let empty_kind = matches!(
        error.kind(),
        io::ErrorKind::InvalidInput | io::ErrorKind::NotFound
    );
    #[cfg(unix)]
    let empty_code = error.raw_os_error() == Some(rustix::io::Errno::SRCH.raw_os_error());
    #[cfg(not(unix))]
    let empty_code = false;
    empty_kind || empty_code
}
