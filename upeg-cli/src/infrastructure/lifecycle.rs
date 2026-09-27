//! `upeg http status / stop / restart / logs`. PRD §5.5.
//!
//! All commands route through the discovery file as the single source
//! of truth. PID signalling delegates to the `kill` / `taskkill`
//! binaries that ship on every supported OS so the workspace's
//! no-`unsafe` posture (`unsafe_code = "deny"`) is preserved.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use upeg_sources::{DirectoryStatus, RuntimeSourceConfig, RuntimeSourceDirs};

use super::{discovery, log_file, mcp_imports, paths};

const STOP_GRACE: Duration = Duration::from_secs(5);

/// Labels for the runtime source directories `upeg host status`
/// reports. Named constants so the text and JSON renderings can never
/// drift apart.
const TOOLKITS_SOURCE_LABEL: &str = "toolkits";
const WASM_SOURCE_LABEL: &str = "wasm";
const MCP_IMPORTS_SOURCE_LABEL: &str = "mcp-imports";

/// Heading (text) and object key (JSON) for the project-manifest block
/// of `upeg host status`.
///
/// Both say "client" out loud on purpose. `upeg host status` runs in a
/// SEPARATE one-shot process from the host, and project-manifest
/// detection is per process working directory
/// (`upeg_sources::project` module docs) — so this block reports what
/// THIS process resolves, which is not necessarily the manifest the
/// running host loaded from wherever it was started. Reporting it under
/// a bare `Project manifest` heading read as if it were the host's.
const CLIENT_PROJECT_MANIFEST_HEADING: &str = "Project manifest (this process, not the host)";
const CLIENT_PROJECT_MANIFEST_JSON_KEY: &str = "client";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKind {
    /// `server.json` exists and `/healthz` answered.
    Running,
    /// The recorded process is gone and its stale record was removed.
    Stale,
    /// The record remains because the process is alive or its state is unknown.
    Unreachable,
    /// No `server.json` at all.
    NotRunning,
}

/// Render the current host state. `--json` uses [`status_json`].
pub fn status_text() -> String {
    let (kind, info) = probe();
    let label = paths::server_json_path()
        .map_or_else(|| "(unavailable)".into(), |p| p.display().to_string());
    let mut out = format_status_text(&kind, info.as_ref(), &label);
    out.push_str(&format_source_dirs_text(&survey_source_dirs()));
    out.push_str(&format_project_manifest_text(
        &upeg_sources::project::project_manifest_status(),
    ));
    out
}

pub fn status_json() -> Value {
    let (kind, info) = probe();
    let mut out = format_status_json(&kind, info.as_ref());
    if let Some(object) = out.as_object_mut() {
        object.insert(
            "sources".to_string(),
            source_dirs_json(&survey_source_dirs()),
        );
        object.insert(
            "projectManifest".to_string(),
            project_manifest_json(&upeg_sources::project::project_manifest_status()),
        );
        // The one block that reports the HOST's live state instead of
        // this process's declarations: the running host's MCP-import
        // phase, read back off the `/healthz` document `probe()` just
        // proved answers (infrastructure::mcp_imports).
        //
        // Emitted UNCONDITIONALLY. The mcp_imports module docs promise
        // `{"state": "unknown"}` when the host does not answer, and a
        // consumer that has to tell "absent key" from "unknown" gets the
        // same non-answer twice in two shapes — so a stale/not-running
        // host reports `unknown` like any other host that did not
        // answer, instead of dropping the block.
        object.insert(
            mcp_imports::MCP_IMPORTS_FIELD.to_string(),
            mcp_imports_json(&kind, info.as_ref()),
        );
    }
    out
}

/// The `mcpImports` block for one probe result — always a block.
///
/// Only a host that is actually answering can report its live phase;
/// everything else (no `server.json`, a stale one, a running host whose
/// `/healthz` did not come back) is the same fact — we do not know —
/// and reports `unknown`.
fn mcp_imports_json(kind: &HostKind, info: Option<&discovery::ServerInfo>) -> Value {
    match (kind, info) {
        (HostKind::Running, Some(info)) => mcp_imports::host_imports_json(info),
        _ => mcp_imports::unknown_imports_json(),
    }
}

/// Where tools can come from, without loading any of them. A host
/// process registers MCP imports at startup, but `upeg host status`
/// runs in a separate one-shot process — so it reports *declarations*
/// (which directory, how many files), never live registrations.
fn survey_source_dirs() -> RuntimeSourceDirs {
    upeg_sources::survey_runtime_source_dirs(&RuntimeSourceConfig::from_env())
}

/// Pure formatter for the runtime-source block appended to
/// [`status_text`].
pub(crate) fn format_source_dirs_text(dirs: &RuntimeSourceDirs) -> String {
    let mut out = String::from("Sources (declared, not loaded)\n");
    for (label, status) in labelled(dirs) {
        out.push_str(&format!(
            "  {label:<12} {}\n",
            source_dir_summary_text(status)
        ));
    }
    out
}

/// Pure formatter for the runtime-source block in [`status_json`].
pub(crate) fn source_dirs_json(dirs: &RuntimeSourceDirs) -> Value {
    Value::Array(
        labelled(dirs)
            .into_iter()
            .map(|(label, status)| {
                json!({
                    "source": label,
                    "state": source_dir_state(status),
                    "path": status.path().map(|p| p.display().to_string()),
                    "declared": status.count(),
                })
            })
            .collect(),
    )
}

const fn labelled(dirs: &RuntimeSourceDirs) -> [(&'static str, &DirectoryStatus); 3] {
    [
        (TOOLKITS_SOURCE_LABEL, &dirs.toolkits),
        (WASM_SOURCE_LABEL, &dirs.wasm),
        (MCP_IMPORTS_SOURCE_LABEL, &dirs.mcp_imports),
    ]
}

const fn source_dir_state(status: &DirectoryStatus) -> &'static str {
    match status {
        DirectoryStatus::Unconfigured => "unconfigured",
        DirectoryStatus::Missing(_) => "missing",
        DirectoryStatus::Loaded { .. } => "present",
    }
}

fn source_dir_summary_text(status: &DirectoryStatus) -> String {
    match status {
        DirectoryStatus::Unconfigured => "unconfigured".to_string(),
        DirectoryStatus::Missing(path) => format!("missing  {}", path.display()),
        DirectoryStatus::Loaded { path, count } => {
            format!("{count} file(s)  {}", path.display())
        }
    }
}

/// C-7: `upeg host status` text block for project-manifest detection —
/// the resolved path (or `none`) plus the
/// [`upeg_sources::project::ProjectManifestOverride`] that produced it,
/// under the [`CLIENT_PROJECT_MANIFEST_HEADING`] that says whose
/// resolution this is. Pure over an already-resolved `status` (env/cwd
/// reads happen once, at the [`status_text`] call site) so it's
/// testable without touching real process state.
pub(crate) fn format_project_manifest_text(
    status: &upeg_sources::project::ProjectManifestStatus,
) -> String {
    let label = upeg_sources::project::project_manifest_override_label(&status.override_state);
    let resolved = status.lookup.as_ref().map_or_else(
        || "none".to_string(),
        |lookup| lookup.path.display().to_string(),
    );
    format!("{CLIENT_PROJECT_MANIFEST_HEADING}\n  {resolved} (override: {label})\n")
}

/// JSON form of [`format_project_manifest_text`], reusing the same
/// typed status/label so the text and JSON renderings can never drift.
/// Nested under [`CLIENT_PROJECT_MANIFEST_JSON_KEY`] for the same
/// reason the heading names the client: this is the status process's
/// own detection, not the host's.
pub(crate) fn project_manifest_json(
    status: &upeg_sources::project::ProjectManifestStatus,
) -> Value {
    json!({
        CLIENT_PROJECT_MANIFEST_JSON_KEY: {
            "path": status.lookup.as_ref().map(|l| l.path.display().to_string()),
            "override":
                upeg_sources::project::project_manifest_override_label(&status.override_state),
        }
    })
}

/// Pure formatting helper for [`status_text`]. Extracted for unit
/// tests — the path of `server.json` and the `(kind, info)` pair are
/// the only inputs.
pub(crate) fn format_status_text(
    kind: &HostKind,
    info: Option<&discovery::ServerInfo>,
    discovery_path_label: &str,
) -> String {
    match (kind, info) {
        (HostKind::Running, Some(info)) => {
            let age = age_seconds(info.started_at_ms);
            format!(
                "Running\n  endpoint:   {}\n  mcp:        {}\n  pid:        {}\n  started:    {} ms (uptime ≈ {}s)\n  discovery:  {}\n",
                info.endpoint,
                info.mcp_endpoint,
                info.pid,
                info.started_at_ms,
                age,
                discovery_path_label,
            )
        }
        (HostKind::Stale, Some(info)) => format!(
            "Stale (recorded process gone; server.json cleaned up)\n  recorded pid: {}\n",
            info.pid
        ),
        (HostKind::Unreachable, Some(info)) => format!(
            "Unreachable (server.json retained; retry when the host responds)\n  recorded pid: {}\n",
            info.pid
        ),
        _ => "Not running\n".to_string(),
    }
}

/// Pure formatting helper for [`status_json`].
pub(crate) fn format_status_json(kind: &HostKind, info: Option<&discovery::ServerInfo>) -> Value {
    match (kind, info) {
        (HostKind::Running, Some(info)) => json!({
            "running": true,
            "endpoint": info.endpoint,
            "mcp_endpoint": info.mcp_endpoint,
            "pid": info.pid,
            "started_at_ms": info.started_at_ms,
            "uptime_seconds": age_seconds(info.started_at_ms),
        }),
        _ => json!({ "running": false }),
    }
}

/// Name of the desktop Settings switch that owns the embedded host's
/// lifecycle (`Tweaks.local_http_host`,
/// `upeg_cli::infrastructure::attach` module docs).
const DESKTOP_LOCAL_HOST_SWITCH: &str = "Local HTTP host";

/// What `upeg host stop` may do to the host `server.json` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopDecision {
    /// A host the user started from the CLI (`upeg host start`, foreground
    /// or `--daemon`): its pid is a plain `upeg` process, signal it.
    Signal(u32),
    /// The desktop app's in-process host: the pid IS the GUI, so the CLI
    /// never signals it and points the user at the desktop control.
    RefuseEmbedded(u32),
}

fn stop_decision(info: &discovery::ServerInfo) -> StopDecision {
    match info.origin {
        discovery::HostOrigin::Explicit => StopDecision::Signal(info.pid),
        discovery::HostOrigin::Embedded => StopDecision::RefuseEmbedded(info.pid),
    }
}

fn embedded_host_stop_refused(pid: u32) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!(
            "the running host is embedded in the upeg desktop app (pid {pid}); \
             `upeg host stop` does not signal the desktop process — turn off \
             `{DESKTOP_LOCAL_HOST_SWITCH}` in the desktop app's Settings (or quit the app) instead"
        ),
    )
}

/// Stop the host. PRD §5.5 grace policy: SIGTERM, wait up to 5s for
/// `server.json` to disappear (its RAII guard removes it on clean
/// exit), then escalate to SIGKILL only if `force` is true. A host the
/// desktop app embeds in its own process is never signalled — see
/// [`StopDecision`].
pub fn stop(force: bool) -> Result<String, std::io::Error> {
    stop_with(force, signal_pid)
}

/// [`stop`] with the pid signaller injected, so tests can prove which
/// pids get signalled (and that an embedded host's never does) without
/// touching real processes.
fn stop_with(
    force: bool,
    mut signal: impl FnMut(u32, bool) -> std::io::Result<()>,
) -> Result<String, std::io::Error> {
    let Some(info) = discovery::read() else {
        return Ok("Not running\n".into());
    };
    let pid = match stop_decision(&info) {
        StopDecision::Signal(pid) => pid,
        StopDecision::RefuseEmbedded(pid) => return Err(embedded_host_stop_refused(pid)),
    };

    signal(pid, false)?;
    if wait_for_shutdown(STOP_GRACE) {
        // Defensive: clean up just in case the guard didn't (e.g. host
        // crashed mid-shutdown).
        let _ = discovery::clear();
        return Ok(format!("Stopped pid {pid}\n"));
    }

    if force {
        signal(pid, true)?;
        let _ = wait_for_shutdown(STOP_GRACE);
        let _ = discovery::clear();
        return Ok(format!("Force-killed pid {pid}\n"));
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        format!(
            "pid {pid} did not exit within {}s — retry with --force",
            STOP_GRACE.as_secs()
        ),
    ))
}

/// Stop the host (if any) and return — caller spawns the new instance.
/// We don't fork the new server from inside this function because the
/// caller has already-parsed CLI args and the right entry-point.
///
/// Propagates `stop` errors verbatim: a `TimedOut` from a graceful
/// shutdown without `--force` must surface so the caller doesn't race
/// a still-living host with a fresh start on a different port.
pub fn prepare_restart(force: bool) -> Result<(), std::io::Error> {
    stop(force).map(|_| ())
}

pub fn logs_tail(lines: usize) -> std::io::Result<Vec<String>> {
    let Some(path) = paths::http_log_path() else {
        return Ok(Vec::new());
    };
    log_file::tail(&path, lines)
}

pub fn log_path() -> Option<PathBuf> {
    paths::http_log_path()
}

fn probe() -> (HostKind, Option<discovery::ServerInfo>) {
    let Some(info) = discovery::read() else {
        return (HostKind::NotRunning, None);
    };
    match discovery::read_reachable() {
        Some(reachable) => (HostKind::Running, Some(reachable)),
        None => match discovery::read() {
            Some(current) => (HostKind::Unreachable, Some(current)),
            None => (HostKind::Stale, Some(info)),
        },
    }
}

/// Cross-platform PID signalling without `unsafe`. Unix uses
/// `/bin/kill` (POSIX-mandated to be on every system), Windows uses
/// `taskkill`. Both produce a non-zero exit on "no such pid" which
/// surfaces as `Err` to the caller.
fn signal_pid(pid: u32, force: bool) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let sig = if force { "-KILL" } else { "-TERM" };
        let status = std::process::Command::new("kill")
            .args([sig, &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "kill {sig} {pid} failed (exit {status})"
            )));
        }
    }
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string()]);
        if force {
            cmd.arg("/F");
        }
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let status = cmd.status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "taskkill /PID {pid} failed (exit {status})"
            )));
        }
    }
    Ok(())
}

fn wait_for_shutdown(grace: Duration) -> bool {
    let deadline = std::time::Instant::now() + grace;
    while std::time::Instant::now() < deadline {
        if discovery::read().is_none() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    discovery::read().is_none()
}

fn age_seconds(started_at_ms: u64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    now.saturating_sub(started_at_ms) / 1000
}

#[cfg(test)]
mod tests {
    use super::super::discovery::{HostOrigin, ServerInfo};
    use super::*;

    /// Obviously fake credential for fixtures. Never a real secret.
    const FAKE_TOKEN: &str = "not-a-real-token";

    fn fake_info() -> ServerInfo {
        ServerInfo {
            endpoint: "http://127.0.0.1:49317".into(),
            mcp_endpoint: "http://127.0.0.1:49317/mcp".into(),
            token: FAKE_TOKEN.into(),
            pid: 1234,
            started_at_ms: 1_700_000_000_000,
            origin: HostOrigin::default(),
        }
    }

    /// Write `info` as the discovery file of the current (isolated)
    /// `UPEG_HOME` and return its path. Callers run inside
    /// [`crate::test_support::with_seeded_pegboard_home`].
    fn write_discovery(info: &ServerInfo) -> std::path::PathBuf {
        let path = paths::server_json_path().expect("server.json path under UPEG_HOME");
        std::fs::write(&path, serde_json::to_string(info).expect("serialize")).expect("write");
        path
    }

    #[test]
    fn stop_decision_signals_explicit_hosts_and_refuses_embedded_ones() {
        let explicit = ServerInfo {
            origin: HostOrigin::Explicit,
            ..fake_info()
        };
        let embedded = ServerInfo {
            origin: HostOrigin::Embedded,
            ..fake_info()
        };
        assert_eq!(stop_decision(&explicit), StopDecision::Signal(1234));
        assert_eq!(stop_decision(&embedded), StopDecision::RefuseEmbedded(1234));
    }

    /// `HostOrigin::Embedded` means the pid is the desktop GUI itself: a
    /// SIGTERM would close the user's app. The CLI must refuse without
    /// signalling anything and point at the desktop's own control.
    #[test]
    fn stop_never_signals_an_embedded_host_and_points_at_the_desktop_control() {
        crate::test_support::with_seeded_pegboard_home(
            "lifecycle-stop-embedded",
            |_| {},
            || {
                let path = write_discovery(&ServerInfo {
                    origin: HostOrigin::Embedded,
                    pid: 99_999_999,
                    ..fake_info()
                });
                let mut signalled: Vec<(u32, bool)> = Vec::new();

                let err = stop_with(true, |pid, force| {
                    signalled.push((pid, force));
                    Ok(())
                })
                .expect_err("an embedded host must not be stopped from the CLI");

                assert!(
                    signalled.is_empty(),
                    "no signal may reach the desktop process, not even with --force: {signalled:?}"
                );
                assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
                assert!(
                    err.to_string().contains(DESKTOP_LOCAL_HOST_SWITCH),
                    "must guide the user to the desktop control; got {err}"
                );
                assert!(
                    path.exists(),
                    "the embedded host's discovery file must be left in place"
                );
            },
        );
    }

    #[test]
    fn stop_signals_an_explicit_host_once_and_reports_its_pid() {
        crate::test_support::with_seeded_pegboard_home(
            "lifecycle-stop-explicit",
            |_| {},
            || {
                let path = write_discovery(&ServerInfo {
                    origin: HostOrigin::Explicit,
                    pid: 4242,
                    ..fake_info()
                });
                let mut signalled: Vec<(u32, bool)> = Vec::new();

                let out = stop_with(false, |pid, force| {
                    signalled.push((pid, force));
                    // Stand in for the host's RAII cleanup so the grace wait
                    // returns at once.
                    let _ = std::fs::remove_file(&path);
                    Ok(())
                })
                .expect("an explicit host stops");

                assert_eq!(signalled, vec![(4242, false)], "exactly one SIGTERM");
                assert_eq!(out, "Stopped pid 4242\n");
            },
        );
    }

    #[test]
    fn format_status_text_not_running() {
        let out = format_status_text(&HostKind::NotRunning, None, "(unavailable)");
        assert_eq!(out, "Not running\n");
    }

    #[test]
    fn format_status_text_running_includes_endpoint_and_pid() {
        let info = fake_info();
        let out = format_status_text(&HostKind::Running, Some(&info), "/tmp/server.json");
        assert!(out.starts_with("Running\n"));
        assert!(out.contains("http://127.0.0.1:49317"));
        assert!(out.contains("1234"));
        assert!(out.contains("/tmp/server.json"));
    }

    #[test]
    fn format_status_text_stale_mentions_staleness_and_pid() {
        let info = fake_info();
        let out = format_status_text(&HostKind::Stale, Some(&info), "/tmp/server.json");
        assert!(out.starts_with("Stale "));
        assert!(out.contains("1234"));
    }

    #[test]
    fn format_status_text_unreachable_says_record_is_retained() {
        let info = fake_info();
        let out = format_status_text(&HostKind::Unreachable, Some(&info), "/tmp/server.json");
        assert!(out.starts_with("Unreachable "));
        assert!(out.contains("retained"));
        assert!(out.contains("1234"));
    }

    #[test]
    fn format_status_text_running_without_info_reads_as_not_running() {
        // Defensive: probe() shouldn't produce this combination, but
        // the formatter must not panic if it does.
        let out = format_status_text(&HostKind::Running, None, "/tmp/server.json");
        assert_eq!(out, "Not running\n");
    }

    #[test]
    fn format_status_json_not_running_returns_only_false() {
        let v = format_status_json(&HostKind::NotRunning, None);
        assert_eq!(v["running"], false);
        assert!(v.get("endpoint").is_none());
    }

    #[test]
    fn format_status_json_running_has_the_full_payload() {
        let info = fake_info();
        let v = format_status_json(&HostKind::Running, Some(&info));
        assert_eq!(v["running"], true);
        assert_eq!(v["endpoint"], "http://127.0.0.1:49317");
        assert_eq!(v["mcp_endpoint"], "http://127.0.0.1:49317/mcp");
        assert_eq!(v["pid"], 1234);
        assert_eq!(v["started_at_ms"], 1_700_000_000_000_u64);
        assert!(v.get("uptime_seconds").is_some());
    }

    /// `upeg host status --json` promises an `mcpImports` block in
    /// every answer (see `mcp_imports` module docs): a missing key and
    /// `"state": "unknown"` are the same non-answer, and shipping both
    /// shapes makes every consumer handle two.
    #[test]
    fn mcp_imports_block_is_unknown_even_without_a_host() {
        for (kind, info) in [
            (HostKind::NotRunning, None),
            (HostKind::Stale, Some(fake_info())),
        ] {
            let block = mcp_imports_json(&kind, info.as_ref());
            assert_eq!(
                block["state"], "unknown",
                "the block must be present in state {kind:?}: {block}"
            );
        }
    }

    #[test]
    fn format_status_json_stale_is_treated_as_not_running() {
        let info = fake_info();
        let v = format_status_json(&HostKind::Stale, Some(&info));
        assert_eq!(v["running"], false);
    }

    fn sample_source_dirs() -> RuntimeSourceDirs {
        RuntimeSourceDirs {
            toolkits: DirectoryStatus::Loaded {
                path: std::path::PathBuf::from("/home/u/.upeg/toolkits"),
                count: 3,
            },
            wasm: DirectoryStatus::Unconfigured,
            mcp_imports: DirectoryStatus::Missing(std::path::PathBuf::from(
                "/home/u/.upeg/mcp-imports",
            )),
        }
    }

    #[test]
    fn source_dirs_text_reports_all_three_directories() {
        let out = format_source_dirs_text(&sample_source_dirs());

        assert!(out.starts_with("Sources (declared, not loaded)\n"));
        assert!(out.contains("toolkits"));
        assert!(out.contains("3 file(s)"));
        assert!(out.contains("wasm"));
        assert!(out.contains("unconfigured"));
        assert!(out.contains("mcp-imports"));
        assert!(out.contains("missing"));
    }

    #[test]
    fn source_dirs_json_carries_state_and_declared_count() {
        let value = source_dirs_json(&sample_source_dirs());

        let rows = value.as_array().expect("array");
        assert_eq!(rows.len(), 3);
        let toolkits = &rows[0];
        assert_eq!(toolkits["source"], "toolkits");
        assert_eq!(toolkits["state"], "present");
        assert_eq!(toolkits["declared"], 3);
        assert_eq!(rows[1]["state"], "unconfigured");
        assert!(rows[1]["path"].is_null());
        assert_eq!(rows[2]["source"], "mcp-imports");
        assert_eq!(rows[2]["state"], "missing");
        assert_eq!(rows[2]["declared"], 0);
    }

    #[test]
    fn project_manifest_text_shows_detected_path_and_override_state() {
        let status = upeg_sources::project::ProjectManifestStatus {
            lookup: Some(upeg_sources::project::ProjectManifestLookup {
                path: std::path::PathBuf::from("/home/user/project/upeg.toml"),
                origin: upeg_sources::project::ProjectManifestOrigin::Detected,
            }),
            override_state: upeg_sources::project::ProjectManifestOverride::Detect,
        };

        let text = format_project_manifest_text(&status);

        assert!(
            text.starts_with(&format!("{CLIENT_PROJECT_MANIFEST_HEADING}\n")),
            "the heading must say this is this process's detection: {text}"
        );
        assert!(text.contains("/home/user/project/upeg.toml"));
        assert!(text.contains("override: detect"));
    }

    /// `host status` runs in a different process from the host — this
    /// block must not read as the manifest the host loaded.
    #[test]
    fn project_manifest_heading_says_it_is_not_the_hosts() {
        assert!(
            CLIENT_PROJECT_MANIFEST_HEADING.contains("this process, not the host"),
            "the heading must name its owner: {CLIENT_PROJECT_MANIFEST_HEADING}"
        );
    }

    #[test]
    fn project_manifest_text_shows_none_and_off_when_absent() {
        let status = upeg_sources::project::ProjectManifestStatus {
            lookup: None,
            override_state: upeg_sources::project::ProjectManifestOverride::Disabled,
        };

        let text = format_project_manifest_text(&status);

        assert!(text.contains("none"));
        assert!(text.contains("override: off"));
    }

    #[test]
    fn project_manifest_json_carries_path_and_override_as_fields() {
        let explicit_path = std::path::PathBuf::from("/explicit/upeg.toml");
        let status = upeg_sources::project::ProjectManifestStatus {
            lookup: Some(upeg_sources::project::ProjectManifestLookup {
                path: explicit_path.clone(),
                origin: upeg_sources::project::ProjectManifestOrigin::EnvOverride,
            }),
            override_state: upeg_sources::project::ProjectManifestOverride::Explicit(
                explicit_path.clone(),
            ),
        };

        let json = project_manifest_json(&status);

        let client = &json[CLIENT_PROJECT_MANIFEST_JSON_KEY];
        assert_eq!(client["path"], explicit_path.display().to_string());
        assert_eq!(client["override"], "explicit");
        assert!(
            json.get("path").is_none(),
            "the path must live only under `client`: {json}"
        );
    }

    #[test]
    fn project_manifest_json_has_null_path_when_absent() {
        let status = upeg_sources::project::ProjectManifestStatus {
            lookup: None,
            override_state: upeg_sources::project::ProjectManifestOverride::Detect,
        };

        let json = project_manifest_json(&status);

        let client = &json[CLIENT_PROJECT_MANIFEST_JSON_KEY];
        assert!(client["path"].is_null());
        assert_eq!(client["override"], "detect");
    }

    /// Source-pin: `prepare_restart` must propagate `stop` errors so a
    /// `TimedOut` (graceful stop without `--force`) surfaces to the CLI
    /// instead of silently letting `run_http_start` race a still-living
    /// host on a fresh ephemeral port. Pre-fix the body discarded the
    /// stop result with a throwaway bind before returning Ok.
    #[test]
    fn prepare_restart_propagates_stop_failures_as_errors() {
        const SRC: &str = include_str!("lifecycle.rs"); // Use format!() so the assertion-message literal itself
        // can't satisfy contains() — same meta-bug guard pattern as
        // popup.rs.
        let positive = format!("{}.map(|_| ())", "stop(force)");
        assert!(
            SRC.contains(&positive),
            "prepare_restart must propagate stop result via map(|_| ())"
        );
        let negative = format!("let _ = {};", "stop(force)");
        assert!(
            !SRC.contains(&negative),
            "prepare_restart must not swallow stop errors via discarding bind"
        );
    }
}
