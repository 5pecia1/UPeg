use std::io;
use std::process::ExitStatus;

#[derive(Clone, Copy, Debug)]
pub(super) struct CaptureLimit(u64);

impl CaptureLimit {
    pub(super) const fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    pub(super) const fn bytes(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputStream {
    Stdout,
    Stderr,
}

impl std::fmt::Display for OutputStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stdout => formatter.write_str("stdout"),
            Self::Stderr => formatter.write_str("stderr"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ExternalProcessError {
    #[cfg(not(any(unix, windows)))]
    #[error("failed to spawn process: {source}")]
    Spawn {
        #[source]
        source: io::Error,
    },
    #[cfg(any(unix, windows))]
    #[error("failed to spawn contained process group: {source}")]
    ContainmentSpawn {
        #[source]
        source: io::Error,
    },
    #[cfg(any(unix, windows))]
    #[error("failed to terminate contained process group: {source}")]
    ContainmentTerminate {
        #[source]
        source: io::Error,
    },
    #[error("spawned process did not expose piped {stream}")]
    MissingPipe { stream: OutputStream },
    #[cfg(unix)]
    #[error("failed to {operation} for the external process pty: {source}")]
    Pty {
        operation: &'static str,
        #[source]
        source: rustix::io::Errno,
    },
    #[cfg(unix)]
    #[error("failed to hand the external process its pty stream: {source}")]
    PtyChildStdio {
        #[source]
        source: io::Error,
    },
    #[error("failed to read process {stream}: {source}")]
    Read {
        stream: OutputStream,
        #[source]
        source: io::Error,
    },
    #[error("process {stream} exceeds the byte limit of {max}")]
    StreamLimitExceeded { stream: OutputStream, max: u64 },
    #[error("combined process stdout and stderr exceed the byte limit of {max}")]
    AggregateLimitExceeded { max: u64 },
    #[error("failed to reserve bounded process {stream} storage: {detail}")]
    Allocation {
        stream: OutputStream,
        detail: String,
    },
    #[error("failed to wait for process: {source}")]
    Wait {
        #[source]
        source: io::Error,
    },
    #[cfg(not(any(unix, windows)))]
    #[error("failed to terminate process after capture failure: {source}")]
    Terminate {
        #[source]
        source: io::Error,
    },
    #[error("process {stream} drain thread stopped unexpectedly")]
    DrainThreadStopped { stream: OutputStream },
    #[error("failed to cancel process output drains: {source}")]
    DrainCancel {
        #[source]
        source: io::Error,
    },
    #[cfg(target_os = "linux")]
    #[error("failed to inspect external process pipe identity: {source}")]
    ScopeInspect {
        #[source]
        source: rustix::io::Errno,
    },
    #[cfg(target_os = "linux")]
    #[error("failed to discover escaped external processes: {source}")]
    ScopeDiscovery {
        #[source]
        source: io::Error,
    },
    #[cfg(target_os = "linux")]
    #[error("external process scope {resource} scan exceeds the limit of {max}")]
    ScopeDiscoveryLimit { resource: &'static str, max: usize },
    #[cfg(target_os = "linux")]
    #[error("failed to open pidfd for external process {pid}: {source}")]
    ScopePidfd {
        pid: i32,
        #[source]
        source: rustix::io::Errno,
    },
    #[cfg(target_os = "linux")]
    #[error("failed to terminate escaped external process {pid}: {source}")]
    ScopeTerminate {
        pid: i32,
        #[source]
        source: rustix::io::Errno,
    },
    #[cfg(target_os = "linux")]
    #[error("escaped external process cleanup exceeded {timeout_ms} ms")]
    ScopeTimeout { timeout_ms: u64 },
}

/// How a captured External run ended.
///
/// A timeout is not an [`ExternalProcessError`]: upeg still holds
/// everything the child managed to write before it was terminated, and
/// those partial diagnostics are exactly what an operator needs. Making
/// it a completion variant keeps that output on the success path of the
/// capture layer and off the "capture itself broke" path.
#[derive(Debug)]
pub(crate) enum CaptureCompletion {
    /// The child exited on its own.
    Exited(ExitStatus),
    /// The declared budget elapsed; upeg terminated the process group.
    TimedOut { timeout_ms: u64 },
    /// The ambient [`upeg_runtime::CancellationToken`] fired while the
    /// child was still running; upeg terminated the process group.
    ///
    /// Its own variant rather than a flavour of `TimedOut` because the
    /// two answer different operator questions — "your budget was too
    /// small" versus "you (or your disconnected client) asked for this"
    /// — and only one of them is a bug in the manifest.
    Cancelled,
}

#[derive(Debug)]
pub(crate) struct CapturedOutput {
    pub(crate) completion: CaptureCompletion,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}
