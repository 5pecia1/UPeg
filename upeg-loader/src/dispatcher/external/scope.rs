//! Containment for descendants that outlive their process group.
//!
//! The child is spawned into its own process group, so terminating that
//! group covers everything that stayed in it. What it does not cover is
//! a descendant that called `setsid` — a background daemon a build
//! script started, say — and kept the child's output stream open. Until
//! that holder is gone the drain thread cannot see end-of-file, and the
//! invocation would hang long after the tool it was asked to run had
//! finished.
//!
//! On Linux this module answers that by identifying descendants through
//! the **stream they hold**, and in both capture paths that identity is
//! an inode: the pipe's for the ordinary path, the child side's for the
//! `pty = true` path (`/dev/pts/N` is only the cheap prefilter — the
//! `DeviceIdentity` recorded in [`super::pty`] is the proof).
//! Everywhere else this is a no-op — the process-group termination is
//! the whole guarantee.

#[cfg(target_os = "linux")]
mod linux_procfs;

use super::child::ChildOutput;
use super::error::ExternalProcessError;

#[cfg(target_os = "linux")]
const SCOPE_TERMINATION_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

#[cfg(target_os = "linux")]
struct ScopeDeadline {
    expires_at: std::time::Instant,
}

#[cfg(target_os = "linux")]
impl ScopeDeadline {
    fn new() -> Self {
        Self {
            expires_at: std::time::Instant::now() + SCOPE_TERMINATION_TIMEOUT,
        }
    }

    fn ensure_remaining(&self) -> Result<(), ExternalProcessError> {
        if std::time::Instant::now() >= self.expires_at {
            return Err(ExternalProcessError::ScopeTimeout {
                timeout_ms: u64::try_from(SCOPE_TERMINATION_TIMEOUT.as_millis())
                    .unwrap_or(u64::MAX),
            });
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    #[cfg(test)]
    use std::cell::Cell;
    use std::thread;
    use std::time::Duration;

    use super::super::child::ChildOutput;
    use super::super::error::ExternalProcessError;
    use super::ScopeDeadline;
    use super::linux_procfs::{self, HolderIdentity};

    const EMPTY_SCAN_TARGET: u8 = 2;
    const SCOPE_SCAN_INTERVAL: Duration = Duration::from_millis(10);

    #[cfg(test)]
    std::thread_local! {
        static SCOPE_SCAN_COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) struct LinuxInvocationScope {
        /// One entry per stream the child was given: two pipes, or the
        /// single pty device both of its streams share.
        holders: Vec<HolderIdentity>,
    }

    impl LinuxInvocationScope {
        pub(super) fn new(output: &ChildOutput) -> Result<Self, ExternalProcessError> {
            let holders = match output {
                ChildOutput::Pipes { stdout, stderr } => vec![
                    HolderIdentity::from_pipe(stdout)?,
                    HolderIdentity::from_pipe(stderr)?,
                ],
                ChildOutput::Merged { terminal } => {
                    vec![HolderIdentity::from_pty_device(terminal.child_device())]
                }
            };
            Ok(Self { holders })
        }

        pub(super) fn terminate_stream_holders(&self) -> Result<(), ExternalProcessError> {
            #[cfg(test)]
            SCOPE_SCAN_COUNT.set(SCOPE_SCAN_COUNT.get() + 1);

            let deadline = ScopeDeadline::new();
            let mut empty_scans = 0;
            while empty_scans < EMPTY_SCAN_TARGET {
                let matched = linux_procfs::scan_and_terminate(&self.holders, &deadline)?;
                if matched == 0 {
                    empty_scans += 1;
                } else {
                    empty_scans = 0;
                }
                if empty_scans < EMPTY_SCAN_TARGET {
                    thread::sleep(SCOPE_SCAN_INTERVAL);
                }
            }
            Ok(())
        }
    }

    #[cfg(test)]
    pub(crate) fn reset_scope_scan_count() {
        SCOPE_SCAN_COUNT.set(0);
    }

    #[cfg(test)]
    pub(crate) fn scope_scan_count() -> usize {
        SCOPE_SCAN_COUNT.get()
    }
}

#[cfg(target_os = "linux")]
pub(super) struct InvocationScope {
    inner: linux::LinuxInvocationScope,
}

#[cfg(target_os = "linux")]
impl InvocationScope {
    pub(super) fn new(output: &ChildOutput) -> Result<Self, ExternalProcessError> {
        linux::LinuxInvocationScope::new(output).map(|inner| Self { inner })
    }

    pub(super) fn terminate_stream_holders(&self) -> Result<(), ExternalProcessError> {
        self.inner.terminate_stream_holders()
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(super) use linux::{reset_scope_scan_count, scope_scan_count};

#[cfg(not(target_os = "linux"))]
pub(super) struct InvocationScope;

#[cfg(not(target_os = "linux"))]
impl InvocationScope {
    pub(super) const fn new(_output: &ChildOutput) -> Result<Self, ExternalProcessError> {
        Ok(Self)
    }

    pub(super) const fn terminate_stream_holders(&self) -> Result<(), ExternalProcessError> {
        Ok(())
    }
}
