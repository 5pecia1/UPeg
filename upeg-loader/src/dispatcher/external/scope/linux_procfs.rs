use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};

use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};

use super::super::error::ExternalProcessError;
use super::super::pty::{ChildDevice, DeviceIdentity};
use super::ScopeDeadline;

const PROCESS_SCAN_LIMIT: usize = 128 * 1024;
const FD_SCAN_LIMIT: usize = 64 * 1024;
const PROCESS_RESOURCE: &str = "processes";
const FILE_DESCRIPTOR_RESOURCE: &str = "file descriptors";
const FDINFO_FLAGS_FIELD: &str = "flags:";
const ACCESS_MODE_MASK: u32 = 0o3;
const READ_ONLY_ACCESS: u32 = 0;
/// `/proc/<pid>/fd/N` symlink shape for a pipe, keyed by inode.
const PIPE_LINK_PREFIX: &str = "pipe:[";
const PIPE_LINK_SUFFIX: &str = "]";

/// How a still-running descendant is recognised as holding one of this
/// invocation's output streams open.
///
/// Two shapes, because the two capture paths give the child two
/// different kinds of object. Both are identified the same way — a cheap
/// `readlink` prefilter over `/proc/<pid>/fd`, then a `stat` that
/// confirms the identity the link only claimed — and they differ in what
/// "the same object" means and in which descriptors count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HolderIdentity {
    /// One end of an ordinary pipe, identified by device + inode.
    Pipe {
        device: u64,
        inode: u64,
        proc_link_target: OsString,
    },
    /// The child side of a pseudoterminal, identified by the file it is.
    /// Stable for the whole invocation because upeg holds the terminal
    /// side open, so the kernel cannot hand this pts number to anybody
    /// else.
    ///
    /// The identity is the inode pair, not the character-device number
    /// alone: `/dev/pts/7` in another devpts instance carries the very
    /// same `rdev`, and killing whoever holds *that* would be killing a
    /// stranger. See [`DeviceIdentity`].
    PtyDevice {
        identity: DeviceIdentity,
        proc_link_target: OsString,
    },
}

impl HolderIdentity {
    pub(super) fn from_pipe(fd: &impl AsFd) -> Result<Self, ExternalProcessError> {
        let stat = rustix::fs::fstat(fd)
            .map_err(|source| ExternalProcessError::ScopeInspect { source })?;
        Ok(Self::Pipe {
            device: stat.st_dev,
            inode: stat.st_ino,
            proc_link_target: format!("{PIPE_LINK_PREFIX}{}{PIPE_LINK_SUFFIX}", stat.st_ino).into(),
        })
    }

    /// Infallible, unlike [`Self::from_pipe`]: the child side was
    /// `stat`ed while it was open (see [`DeviceIdentity`]), so there is
    /// nothing left to look up — and nothing left to look up *wrongly*,
    /// which re-resolving the path here would risk.
    pub(super) fn from_pty_device(device: &ChildDevice) -> Self {
        Self::PtyDevice {
            identity: device.identity(),
            proc_link_target: device.path().to_os_string(),
        }
    }

    /// Whether a `stat` of a candidate descriptor confirms it is this
    /// very object.
    fn matches(&self, metadata: &fs::Metadata) -> bool {
        match self {
            Self::Pipe { device, inode, .. } => {
                *device == metadata.dev() && *inode == metadata.ino()
            }
            Self::PtyDevice { identity, .. } => {
                identity.rdev == metadata.rdev()
                    && identity.device == metadata.dev()
                    && identity.inode == metadata.ino()
                    && metadata.file_type().is_char_device()
            }
        }
    }

    fn matches_proc_link(&self, target: &OsStr) -> bool {
        self.proc_link_target() == target
    }

    fn proc_link_target(&self) -> &OsStr {
        match self {
            Self::Pipe {
                proc_link_target, ..
            }
            | Self::PtyDevice {
                proc_link_target, ..
            } => proc_link_target,
        }
    }

    /// Whether a descriptor opened in this access mode is one that keeps
    /// the stream from reporting end-of-file.
    ///
    /// For a pipe only writers do: a reader cannot hold the read end
    /// open against itself. For a pty, *any* open child-side descriptor
    /// does — the terminal side reports `EIO` only when the last one
    /// closes, whatever it was opened for.
    fn holds_open(&self, pid: Pid, descriptor: &OsStr) -> bool {
        match self {
            Self::Pipe { .. } => descriptor_is_writable(pid, descriptor),
            Self::PtyDevice { .. } => true,
        }
    }
}

pub(super) fn scan_and_terminate(
    holders: &[HolderIdentity],
    deadline: &ScopeDeadline,
) -> Result<usize, ExternalProcessError> {
    let processes =
        fs::read_dir("/proc").map_err(|source| ExternalProcessError::ScopeDiscovery { source })?;
    let current_pid = i32::try_from(std::process::id()).ok();
    let mut scanned = 0;
    let mut matched = 0;
    for entry in processes {
        deadline.ensure_remaining()?;
        let Ok(entry) = entry else {
            continue;
        };
        let Some(pid) = parse_pid(&entry.file_name()) else {
            continue;
        };
        if Some(pid.as_raw_pid()) == current_pid {
            continue;
        }
        scanned += 1;
        if scanned > PROCESS_SCAN_LIMIT {
            return Err(ExternalProcessError::ScopeDiscoveryLimit {
                resource: PROCESS_RESOURCE,
                max: PROCESS_SCAN_LIMIT,
            });
        }
        if !process_holds_stream(holders, pid, deadline)? {
            continue;
        }
        let pidfd = match pidfd_open(pid, PidfdFlags::empty()) {
            Ok(pidfd) => pidfd,
            Err(rustix::io::Errno::SRCH) => continue,
            Err(source) => {
                return Err(ExternalProcessError::ScopePidfd {
                    pid: pid.as_raw_pid(),
                    source,
                });
            }
        };
        // PID 재사용과 fd 교체 사이의 TOCTOU를 막기 위해 pidfd로
        // 프로세스를 고정한 뒤 같은 stream을 여전히 보유하는지 재확인한다.
        if !process_holds_stream(holders, pid, deadline)? {
            continue;
        }
        match pidfd_send_signal(&pidfd, Signal::KILL) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => matched += 1,
            Err(source) => {
                return Err(ExternalProcessError::ScopeTerminate {
                    pid: pid.as_raw_pid(),
                    source,
                });
            }
        }
    }
    Ok(matched)
}

fn process_holds_stream(
    holders: &[HolderIdentity],
    pid: Pid,
    deadline: &ScopeDeadline,
) -> Result<bool, ExternalProcessError> {
    let fd_path = format!("/proc/{}/fd", pid.as_raw_pid());
    let descriptors = match fs::read_dir(fd_path) {
        Ok(descriptors) => descriptors,
        Err(source)
            if matches!(
                source.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            return Ok(false);
        }
        Err(_) => return Ok(false),
    };
    for (index, descriptor) in descriptors.enumerate() {
        deadline.ensure_remaining()?;
        if index >= FD_SCAN_LIMIT {
            return Err(ExternalProcessError::ScopeDiscoveryLimit {
                resource: FILE_DESCRIPTOR_RESOURCE,
                max: FD_SCAN_LIMIT,
            });
        }
        let Ok(descriptor) = descriptor else {
            continue;
        };
        let Ok(link_target) = fs::read_link(descriptor.path()) else {
            continue;
        };
        let Some(holder) = holders
            .iter()
            .find(|holder| holder.matches_proc_link(link_target.as_os_str()))
        else {
            continue;
        };
        if !holder.holds_open(pid, &descriptor.file_name()) {
            continue;
        }
        let Ok(metadata) = fs::metadata(descriptor.path()) else {
            continue;
        };
        if holder.matches(&metadata) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_pid(name: &OsStr) -> Option<Pid> {
    name.to_str()
        .and_then(|name| name.parse::<i32>().ok())
        .and_then(Pid::from_raw)
}

fn descriptor_is_writable(pid: Pid, descriptor: &OsStr) -> bool {
    let Some(descriptor) = descriptor.to_str() else {
        return false;
    };
    let fdinfo_path = format!("/proc/{}/fdinfo/{descriptor}", pid.as_raw_pid());
    fs::read_to_string(fdinfo_path)
        .ok()
        .is_some_and(|fdinfo| fdinfo_is_writable(&fdinfo))
}

fn fdinfo_is_writable(fdinfo: &str) -> bool {
    fdinfo
        .lines()
        .find_map(|line| line.strip_prefix(FDINFO_FLAGS_FIELD))
        .and_then(|flags| u32::from_str_radix(flags.trim(), 8).ok())
        .is_some_and(|flags| flags & ACCESS_MODE_MASK != READ_ONLY_ACCESS)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::super::pty::{DeviceIdentity, Pty};
    use super::{HolderIdentity, fdinfo_is_writable};

    #[test]
    fn pty_보유자는_열려_있던_자식_쪽의_식별자를_그대로_쓴다() {
        // Given: an open pseudoterminal.
        let terminal = Pty::open().expect("pty를 연다").into_master();
        let device = terminal.child_device();

        // Then: the sweep identity is the one recorded at open time —
        // not a fresh `stat` of the path, which by sweep time may name
        // a different file altogether.
        assert_eq!(
            HolderIdentity::from_pty_device(device),
            HolderIdentity::PtyDevice {
                identity: device.identity(),
                proc_link_target: device.path().to_os_string(),
            }
        );
    }

    #[test]
    fn rdev가_같아도_inode가_다르면_같은_터미널이_아니다() {
        // Given: a real child side and the way `/proc` stats it.
        let terminal = Pty::open().expect("pty를 연다").into_master();
        let device = terminal.child_device();
        let metadata = fs::metadata(device.path()).expect("자식 쪽 장치를 stat한다");
        let 실제 = device.identity();
        let 보유자 = |identity| HolderIdentity::PtyDevice {
            identity,
            proc_link_target: device.path().to_os_string(),
        };

        // Then
        assert!(
            보유자(실제).matches(&metadata),
            "자기 자신은 언제나 같은 터미널이다"
        );
        // A second devpts instance — a container's own `/dev/pts` — has
        // a `/dev/pts/N` with the very same character-device number on a
        // different superblock. rdev-only identity would have upeg kill
        // whoever holds that stranger's terminal.
        assert!(
            !보유자(DeviceIdentity {
                device: 실제.device.wrapping_add(1),
                ..실제
            })
            .matches(&metadata),
            "rdev가 같아도 다른 devpts 인스턴스의 터미널은 남의 것이다"
        );
        assert!(
            !보유자(DeviceIdentity {
                inode: 실제.inode.wrapping_add(1),
                ..실제
            })
            .matches(&metadata),
            "같은 인스턴스라도 pts 번호가 다르면 다른 터미널이다"
        );
    }

    #[test]
    fn fdinfo의_read_only_descriptor는_writer가_아니다() {
        assert!(!fdinfo_is_writable("pos:\t0\nflags:\t00\nmnt_id:\t1\n"));
    }

    #[test]
    fn fdinfo의_write_descriptor는_writer다() {
        assert!(fdinfo_is_writable(
            "pos:\t0\nflags:\t02000001\nmnt_id:\t1\n"
        ));
        assert!(fdinfo_is_writable("pos:\t0\nflags:\t02\nmnt_id:\t1\n"));
    }
}
