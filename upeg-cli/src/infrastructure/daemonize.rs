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
/// invocation; pass `--addr`, `--token-file`, etc. that the foreground
/// branch would otherwise consume. An explicit `--token` value travels
/// as `operator_token` instead and reaches the child through its
/// environment — see [`detached_command`].
pub fn detach_and_exit(
    log_path: PathBuf,
    extra_args: &[String],
    operator_token: Option<&str>,
) -> std::io::Result<()> {
    if std::env::var_os(DETACH_MARKER_ENV).is_some() {
        // Already detached — caller's foreground path should run.
        return Ok(());
    }
    let exe = std::env::current_exe()?;

    let log_for_stdout = super::log_file::open_rotated(&log_path)?;
    let log_for_stderr = log_for_stdout.try_clone()?;

    let mut cmd = detached_command(exe, extra_args, operator_token);
    cmd.stdin(Stdio::null())
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

/// The child's command line and environment, minus stdio wiring.
///
/// Secrets travel through the environment, never through argv: a command
/// line is world-readable in `ps` output on every platform upeg targets,
/// while a process environment is readable only by its own user
/// (docs/architecture/host-topology.md). The child resolves
/// `UPEG_HTTP_TOKEN` exactly as an operator-exported variable, so it
/// still sees the token as `TokenSource::Provided`.
fn detached_command(exe: PathBuf, extra_args: &[String], operator_token: Option<&str>) -> Command {
    let mut cmd = Command::new(exe);
    cmd.arg("http").args(extra_args).env(DETACH_MARKER_ENV, "1");
    if let Some(token) = operator_token {
        cmd.env(super::paths::env::HTTP_TOKEN, token);
    }
    cmd
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

    /// Obviously fake credential for the argv/env assertions. Never a
    /// real secret.
    const FAKE_TOKEN: &str = "not-a-real-token";

    #[test]
    fn is_detached_from_is_true_when_the_marker_has_any_value() {
        assert!(is_detached_from(Some(OsStr::new("1"))));
        // Any value — even empty — is treated as present (var_os
        // returns Some for an empty environment variable too).
        assert!(is_detached_from(Some(OsStr::new(""))));
    }

    #[test]
    fn is_detached_from_is_false_when_the_marker_is_absent() {
        assert!(!is_detached_from(None));
    }

    #[test]
    fn detached_child_receives_the_operator_token_through_env_not_argv() {
        let extra = vec!["--addr".to_string(), "127.0.0.1:7173".to_string()];

        let cmd = detached_command(PathBuf::from("upeg"), &extra, Some(FAKE_TOKEN));

        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert_eq!(
            args,
            vec![
                OsStr::new("http"),
                OsStr::new("--addr"),
                OsStr::new("127.0.0.1:7173")
            ],
            "argv must carry only non-secret flags"
        );
        assert!(
            !args.iter().any(|arg| *arg == OsStr::new(FAKE_TOKEN)),
            "the token must never appear in the child's command line"
        );
        let env_token = cmd
            .get_envs()
            .find(|(key, _)| *key == OsStr::new(super::super::paths::env::HTTP_TOKEN))
            .and_then(|(_, value)| value);
        assert_eq!(env_token, Some(OsStr::new(FAKE_TOKEN)));
        assert!(
            cmd.get_envs()
                .any(|(key, value)| key == OsStr::new(DETACH_MARKER_ENV) && value.is_some()),
            "the child must still see the detach marker"
        );
    }

    #[test]
    fn detached_child_without_an_explicit_token_inherits_no_token_override() {
        let cmd = detached_command(PathBuf::from("upeg"), &[], None);

        assert!(
            cmd.get_envs()
                .all(|(key, _)| key != OsStr::new(super::super::paths::env::HTTP_TOKEN)),
            "no --token means no UPEG_HTTP_TOKEN injected into the child"
        );
    }
}
