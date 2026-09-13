//! Process detach for `upeg http --daemon`. PRD §5.5.
//!
//! Cross-platform without `unsafe`:
//!   - Unix: re-spawn `self` with stdio attached to the log file,
//!     then exit the parent. The child inherits the new session via
//!     normal kernel reparenting when the parent disappears — good
//!     enough for the desktop/server cases the PRD targets. (`setsid`
//!     would require `pre_exec`, which the workspace forbids.)
//!   - Windows: `CommandExt::creation_flags(DETACHED_PROCESS |
//!     CREATE_NEW_PROCESS_GROUP)` does the same job natively.
//!
//! The detach helper writes nothing user-facing; the caller prints
//! "started (pid N, endpoint …)" once `server.json` shows up.

#![allow(
    clippy::exit,
    reason = "process-detach by design exits the parent after the child takes over"
)]

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Marker env var. The respawned child sees this set and skips
/// re-detaching (would fork-bomb otherwise).
pub const DETACH_MARKER_ENV: &str = "UPEG_HTTP_DETACHED";

/// Re-spawn the current executable with the same CLI args, detached
/// from this terminal, with stdout/stderr pointed at `log_path`. On
/// success this function does **not** return — it calls
/// `std::process::exit(0)` after the child is spawned.
///
/// `extra_args` are appended to whatever produced the current
/// invocation; pass `--addr`, `--token`, etc. that the foreground
/// branch would otherwise consume.
pub fn detach_and_exit(log_path: PathBuf, extra_args: &[String]) -> std::io::Result<()> {
    if std::env::var_os(DETACH_MARKER_ENV).is_some() {
        // Already detached — caller's foreground path should run.
        return Ok(());
    }
    let exe = std::env::current_exe()?;

    let log_for_stdout = super::log_file::open_rotated(&log_path)?;
    let log_for_stderr = log_for_stdout.try_clone()?;

    let mut cmd = Command::new(exe);
    cmd.arg("http");
    for arg in extra_args {
        cmd.arg(arg);
    }
    cmd.env(DETACH_MARKER_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_for_stdout))
        .stderr(Stdio::from(log_for_stderr));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    let child = cmd.spawn()?;
    eprintln!(
        "upeg http: detached daemon started (pid {}, log {})",
        child.id(),
        log_path.display()
    );
    std::process::exit(0);
}

/// Whether the running process is the detached child. Used by the
/// startup path to swap stderr formatting (the child's stderr is the
/// log file, so banner lines belong as plain text without color).
pub fn is_detached_child() -> bool {
    is_detached_from(std::env::var_os(DETACH_MARKER_ENV).as_deref())
}

/// Pure helper for [`is_detached_child`]. Allows tests to drive the
/// detection without mutating process env.
pub(crate) const fn is_detached_from(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn 환경변수_값이_있으면_is_detached_from은_참을_반환한다() {
        assert!(is_detached_from(Some(OsStr::new("1"))));
        // Any value — even empty — is treated as present (var_os
        // returns Some for an empty environment variable too).
        assert!(is_detached_from(Some(OsStr::new(""))));
    }

    #[test]
    fn 환경변수_값이_없으면_is_detached_from은_거짓을_반환한다() {
        assert!(!is_detached_from(None));
    }
}
