//! Boot / lifecycle API exposed to the Flutter frontend.
//!
//! Native builds acquire the single-instance lock
//! (`platform::instance_lock`) and drive host discovery
//! (`platform::host_bootstrap`). Wasm builds return the stub `NoHost`
//! result because a browser cannot host a daemon nor take an OS file
//! lock.
//!
//! `init_app` is the one-shot boot the Flutter side awaits before
//! rendering the first frame. `shutdown` releases the lock so Drop
//! cleanup runs even when the process exits through a Dart-side path
//! that would otherwise skip the static destructor lane.

pub use upeg_pegboard_ui::deep_link::DesktopLaunch;
use upeg_pegboard_ui::deep_link::{desktop_launch_from_env, desktop_launch_from_env_maybe};

/// Report returned by [`init_app`] so the Flutter side can render
/// the initial UI state in one round-trip.
///
/// `host_state` carries the bootstrap outcome (A07/A08) so the Dart
/// side can distinguish attached vs embedded vs no-host without
/// re-running discovery.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct AppInitReport {
    pub launch: DesktopLaunchDto,
    pub host_state: HostStateDto,
    pub version: String,
}

/// Non-opaque DTO mirror of [`DesktopLaunch`]. The source struct
/// holds `&'static str` fields (board/tool ids) which FRB cannot
/// ship as borrows; the owned-string copy crosses the wire instead.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct DesktopLaunchDto {
    pub board: String,
    pub tool: Option<String>,
    pub input: Option<String>,
}

impl From<DesktopLaunch> for DesktopLaunchDto {
    fn from(launch: DesktopLaunch) -> Self {
        Self {
            board: launch.board.to_string(),
            tool: launch.tool.map(str::to_string),
            input: launch.input,
        }
    }
}

/// Non-opaque enum mirror of [`HostState`]. Lives next to the source
/// struct so the FRB layer ships a concrete sealed Dart class
/// instead of an opaque RustAutoOpaque handle.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum HostStateDto {
    Attached { endpoint: String },
    Embedded { endpoint: String },
    NoHost,
}

impl From<HostState> for HostStateDto {
    fn from(state: HostState) -> Self {
        match state {
            HostState::Attached { endpoint } => Self::Attached { endpoint },
            HostState::Embedded { endpoint } => Self::Embedded { endpoint },
            HostState::NoHost => Self::NoHost,
        }
    }
}

/// Snapshot of the upeg host process state at app boot.
///
/// Sealed enum (Rust → Dart sealed class via flutter_rust_bridge):
/// the Dart side gets a `switch` that the compiler enforces is
/// exhaustive over these variants.
#[derive(Debug, Clone)]
pub enum HostState {
    /// Attached to an already-running upeg host process (CLI/daemon).
    Attached { endpoint: String },
    /// Embedded a host inside this process (desktop binary owns it).
    Embedded { endpoint: String },
    /// No host running, and the `Local HTTP host` preference is off.
    NoHost,
}

/// Domain error type the entire FRB API surface returns. Variants are
/// enumerated up-front so Dart sealed-class matching is exhaustive
/// rather than relying on a string discriminator.
#[derive(Debug, Clone, thiserror::Error)]
pub enum FrbError {
    #[error("already running (pid {pid})")]
    AlreadyRunning { pid: u32 },
    #[error("host unavailable")]
    HostUnavailable,
    #[error("project manifest changed: {path}; restart UPeg and reconnect the agent's MCP server")]
    ProjectManifestChanged { path: String },
    #[error("validation: {field}: {reason}")]
    Validation { field: String, reason: String },
    #[error("i/o: {message}")]
    Io { message: String },
    #[error("internal: {message}")]
    Internal { message: String },
}

/// One-shot boot: acquires the OS-level instance lock + runs host
/// discovery (attach existing / embed new / no host).
///
/// The bootstrap result is forwarded to the host-state broadcaster
/// in `api/events.rs` so any subscriber attached after `init_app`
/// returns immediately receives the cached snapshot on subscribe.
///
/// Intentionally NOT `#[frb(sync)]`: native host discovery can block for
/// up to `EMBED_READY_TIMEOUT_SECS` waiting on the embedded HTTP host, and
/// runtime-source loading touches the filesystem. A sync FRB call runs on
/// the Dart UI isolate and would freeze the splash; the async surface runs
/// it on an FRB worker so the splash stays responsive.
pub fn init_app() -> Result<AppInitReport, FrbError> {
    upeg_tools::register_all();

    // User-provided runtime sources must be registered before host
    // discovery and before the launch intent is resolved, so a cold-boot
    // `upeg://open?...&tool=<runtime tool>` deep link can find its tool.
    #[cfg(not(target_arch = "wasm32"))]
    load_local_runtime_sources();

    #[cfg(not(target_arch = "wasm32"))]
    let host_state = native_host_bootstrap()?;
    #[cfg(target_arch = "wasm32")]
    let host_state = HostState::NoHost;

    super::events::host_state_broadcast(super::events::HostStateEvent::from(host_state.clone()));

    Ok(AppInitReport {
        launch: DesktopLaunchDto::from(desktop_launch_from_env()),
        host_state: HostStateDto::from(host_state),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn native_host_bootstrap() -> Result<HostState, FrbError> {
    use crate::platform::{host_bootstrap, instance_lock};

    match instance_lock::acquire_global() {
        Ok(()) => {}
        Err(instance_lock::AcquireError::AlreadyRunning { pid }) => {
            return Err(FrbError::AlreadyRunning { pid });
        }
        Err(instance_lock::AcquireError::Io(err)) => {
            return Err(FrbError::Io {
                message: err.to_string(),
            });
        }
    }
    Ok(host_bootstrap::ensure_host_for_desktop())
}

/// Load user-provided runtime tool sources (declarative toolkits, the
/// project manifest, local WASM toolkits) from the environment so they are
/// registered in the Flutter surface. This step is mandatory at startup:
/// without it imported tools never appear in palette/list APIs and dispatch
/// fails for them. Wasm builds skip this — a browser has no local
/// filesystem to scan.
#[cfg(not(target_arch = "wasm32"))]
fn load_local_runtime_sources() {
    let report =
        upeg_sources::load_local_runtime_sources(&upeg_sources::RuntimeSourceConfig::from_env());
    log_runtime_source_report(&report);
}

/// Which local source declared a tool the host then dropped. Named
/// rather than inlined so the boot summary says *where* to go fix it.
#[cfg(not(target_arch = "wasm32"))]
const TOOLKITS_DIR_SOURCE: &str = "toolkits dir";
#[cfg(not(target_arch = "wasm32"))]
const PROJECT_MANIFEST_SOURCE: &str = "project manifest";

/// One tool the desktop host will not show, and where it was declared.
///
/// Built as a value before anything is logged so the *decision* — which
/// sources get walked, and what each skip is called — is something a
/// test can assert on. It has to be: a skip only happens on a host that
/// lacks the capability (`upeg-loader`'s `HostCapabilities`), so on a
/// developer's Linux box no `pty = true` tool is ever skipped and an
/// end-to-end assertion would silently test nothing.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, PartialEq, Eq)]
struct SkippedToolNote {
    source: &'static str,
    tool: String,
    reason: String,
}

/// Every tool the local runtime sources dropped on this host, in the
/// order the sources are loaded.
#[cfg(not(target_arch = "wasm32"))]
fn skipped_tool_notes(report: &upeg_sources::RuntimeSourceReport) -> Vec<SkippedToolNote> {
    let project_skips = report
        .project_manifest
        .iter()
        .flat_map(|project| project.outcome.skipped.iter())
        .map(|skipped| (PROJECT_MANIFEST_SOURCE, skipped));
    report
        .toolkits_outcome
        .skipped
        .iter()
        .map(|skipped| (TOOLKITS_DIR_SOURCE, skipped))
        .chain(project_skips)
        .map(|(source, skipped)| SkippedToolNote {
            source,
            tool: skipped.id.clone(),
            reason: skipped.reason.to_string(),
        })
        .collect()
}

/// Boot summary for the desktop host: every source that did not fully
/// land.
///
/// Skips are logged alongside the failures rather than left silent. A
/// `pty = true` tool on a build without pseudoterminal support parses
/// cleanly, registers nothing, and would otherwise simply not be in the
/// palette — a hole with no name attached. `warn`, not `error`: the
/// manifest is legal and the rest of it loaded.
#[cfg(not(target_arch = "wasm32"))]
fn log_runtime_source_report(report: &upeg_sources::RuntimeSourceReport) {
    for (path, err) in &report.toolkits_outcome.failed {
        tracing::warn!(path = %path.display(), error = %err, "failed to load declarative Toolkit");
    }
    if let Some(project) = &report.project_manifest {
        for (path, err) in &project.outcome.failed {
            tracing::warn!(
                manifest = %project.path.display(),
                path = %path.display(),
                error = %err,
                "failed to load project manifest Toolkit"
            );
        }
    }
    for note in skipped_tool_notes(report) {
        tracing::warn!(
            source = note.source,
            tool = note.tool,
            reason = note.reason,
            "skipped tool this host cannot run"
        );
    }
    for (path, err) in &report.wasm_failures {
        tracing::warn!(path = %path.display(), error = %err, "failed to load WASM Toolkit");
    }
}

/// Re-read the launch intent without re-running the whole boot.
/// Returns `None` when argv carries no supported `upeg://open` argument
/// so the Dart side can distinguish "no deep link" from an explicit
/// default-board deep link.
///
/// The Dart `app.dart` calls this once after `initApp()` resolves and
/// seeds the result into `launchIntentProvider`, which the
/// `LaunchIntentApplier` observer drains on the first frame.
#[flutter_rust_bridge::frb(sync)]
pub fn current_launch_intent() -> Option<crate::api::deep_link::LaunchIntentDto> {
    let launch = desktop_launch_from_env_maybe()?;
    Some(crate::api::deep_link::LaunchIntentDto {
        board: Some(launch.board.to_string()),
        tool: launch.tool.map(|t| t.to_string()),
        input_json: launch.input,
    })
}

/// Hand the host back to the OS: drops the instance lock so
/// `~/.upeg/desktop.lock` is removed even when the process exits via
/// a Dart-side teardown path. The embedded HTTP host thread keeps
/// running until process exit; its discovery `Drop` guard handles
/// cleanup. There is no explicit host-stop signal yet.
#[flutter_rust_bridge::frb(sync)]
pub fn shutdown() -> Result<(), FrbError> {
    #[cfg(not(target_arch = "wasm32"))]
    crate::platform::instance_lock::drop_global();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A report whose only local sources are two skipped tools — one per
    /// source — so the boot summary's traversal can be asserted on any
    /// host, including the ones that never skip anything themselves.
    #[cfg(not(target_arch = "wasm32"))]
    fn report_with_only_skipped_tools() -> upeg_sources::RuntimeSourceReport {
        fn skipped(id: &str) -> upeg_loader::SkippedTool {
            upeg_loader::SkippedTool {
                id: id.to_string(),
                reason: upeg_loader::LoadError::PtyUnsupportedOnHost,
            }
        }
        upeg_sources::RuntimeSourceReport {
            toolkits: upeg_sources::DirectoryStatus::Unconfigured,
            toolkits_outcome: upeg_loader::LoadOutcome {
                skipped: vec![skipped("t.terminal")],
                ..Default::default()
            },
            project_manifest: Some(upeg_sources::ProjectManifestLoad {
                path: std::path::PathBuf::from("/w/upeg.toml"),
                origin: upeg_sources::project::ProjectManifestOrigin::Detected,
                outcome: upeg_loader::LoadOutcome {
                    skipped: vec![skipped("p.shell")],
                    ..Default::default()
                },
            }),
            wasm: upeg_sources::DirectoryStatus::Unconfigured,
            wasm_failures: Vec::new(),
            mcp_imports: upeg_sources::DirectoryStatus::Unconfigured,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn boot_summary_collects_skipped_tools_from_all_local_sources_with_origin() {
        let notes = skipped_tool_notes(&report_with_only_skipped_tools());

        let reason = upeg_loader::LoadError::PtyUnsupportedOnHost.to_string();
        assert_eq!(
            notes,
            vec![
                SkippedToolNote {
                    source: TOOLKITS_DIR_SOURCE,
                    tool: "t.terminal".to_string(),
                    reason: reason.clone(),
                },
                SkippedToolNote {
                    source: PROJECT_MANIFEST_SOURCE,
                    tool: "p.shell".to_string(),
                    reason,
                },
            ],
            "both sources must be swept so palette-missing tools get named"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn boot_summary_leaves_no_lines_when_no_tools_skipped() {
        let mut report = report_with_only_skipped_tools();
        report.toolkits_outcome.skipped.clear();
        report
            .project_manifest
            .as_mut()
            .expect("the fixed report has a project manifest")
            .outcome
            .skipped
            .clear();

        assert!(skipped_tool_notes(&report).is_empty());
    }

    #[test]
    fn host_state_dto_maps_all_three_variants() {
        // A07/A08 — the FRB-facing DTO must cover all three native
        // bootstrap outcomes so the Dart side can pattern-match
        // exhaustively (sealed Dart class).
        assert_eq!(HostStateDto::from(HostState::NoHost), HostStateDto::NoHost);
        assert_eq!(
            HostStateDto::from(HostState::Attached {
                endpoint: "127.0.0.1:9001".to_string()
            }),
            HostStateDto::Attached {
                endpoint: "127.0.0.1:9001".to_string()
            }
        );
        assert_eq!(
            HostStateDto::from(HostState::Embedded {
                endpoint: "127.0.0.1:9002".to_string()
            }),
            HostStateDto::Embedded {
                endpoint: "127.0.0.1:9002".to_string()
            }
        );
    }

    #[test]
    fn desktop_launch_dto_converts_to_owned_strings() {
        // FRB cannot ship `&'static str` borrows over FFI; the DTO
        // owns every field so the Dart side sees concrete Strings.
        let launch = DesktopLaunch {
            board: "trading",
            tool: Some("num.hex_to_decimal"),
            input: Some("{\"hex\":\"0xff\"}".to_string()),
        };
        let dto = DesktopLaunchDto::from(launch);
        assert_eq!(dto.board, "trading");
        assert_eq!(dto.tool.as_deref(), Some("num.hex_to_decimal"));
        assert_eq!(dto.input.as_deref(), Some("{\"hex\":\"0xff\"}"));
    }

    // ─── Task A1: `wasm-plugin` feature wiring ──────────────────────
    //
    // `init_app` (line ~117) always calls `load_local_runtime_sources`
    // (line ~165), which forwards to `upeg_sources::load_wasm_dir`. That
    // function only hosts real WASM Toolkits when `upeg-sources` itself
    // is compiled with its own `wasm-plugin` feature — which upeg-frb's
    // `wasm-plugin` feature (Cargo.toml) now forwards to. These tests
    // exercise the exact `RuntimeSourceConfig`-driven path `init_app`
    // uses, then dispatch through the desktop tools surface
    // (`api::tools::dispatch_tool`) to prove the whole boot → dispatch
    // chain reaches a hosted WASM plugin end to end — not just that the
    // loader function compiles.
    //
    // upeg-cli/Cargo.toml made `wasm-plugin` a *default* upeg-cli
    // feature (DX item: shipped binaries load plugins out of the box).
    // upeg-frb unconditionally depends on upeg-cli on native targets
    // (`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`
    // above), and upeg-cli's own default features apply whenever it is
    // compiled at all — independent of upeg-frb's own `wasm-plugin`
    // feature flag. So `upeg-loader`'s real (non-stub) `Wasm` invoker
    // dispatcher is now unconditionally unified into every native build
    // that includes upeg-frb, including plain `cargo test -p upeg_frb`
    // with upeg-frb's own `wasm-plugin` feature left off. The stub path
    // this module's tests used to cover with upeg-frb's feature off is
    // no longer reachable from any such build; that regression-guard
    // test was removed rather than left permanently red.

    /// Fixture shared by `upeg-wasm`/`upeg-loader`/`upeg-cli`'s own
    /// wasm-plugin tests: a precompiled extism module declaring
    /// `test.wasm.echo` / `test.wasm.shout`. Reused here rather than
    /// rebuilding `examples/plugins/greet` so this test doesn't need a
    /// wasm32 toolchain at `cargo test` time.
    #[cfg(feature = "wasm-plugin")]
    fn wasm_fixture_path() -> std::path::PathBuf {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../upeg-wasm/tests/fixtures/test_plugin.wasm");
        assert!(
            path.exists(),
            "wasm fixture missing at {} — expected alongside upeg-wasm/upeg-loader/upeg-cli's own wasm-plugin tests",
            path.display()
        );
        path
    }

    #[cfg(feature = "wasm-plugin")]
    #[test]
    fn wasm_plugin_tool_registers_via_runtime_source_loading_and_runs_via_dispatch_tool() {
        let dir =
            std::env::temp_dir().join(format!("upeg_frb_boot_wasm_plugin_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create wasm fixture dir");
        std::fs::copy(wasm_fixture_path(), dir.join("test_plugin.wasm"))
            .expect("copy wasm fixture into place");

        let config = upeg_sources::RuntimeSourceConfig {
            toolkits_dir: None,
            project_manifest: None,
            wasm_dir: Some(dir.clone()),
            mcp_import_dir: None,
        };
        let report = upeg_sources::load_local_runtime_sources(&config);
        assert!(
            report.wasm_failures.is_empty(),
            "wasm plugin load reported failures: {:?}",
            report.wasm_failures
        );
        assert_eq!(
            report.wasm.count(),
            2,
            "expected both fixture tools (echo/shout) registered from {}",
            dir.display()
        );

        let outcome = crate::api::tools::dispatch_tool(
            "test.wasm.echo".to_string(),
            serde_json::json!({ "input": "boot-경로" }).to_string(),
            None,
        );
        assert!(outcome.ok, "dispatch_tool failed: {outcome:?}");
        assert!(
            outcome.outputs.iter().any(|entry| matches!(
                &entry.value,
                crate::api::tools::CanonicalOutputValue::String { value }
                    if value == "echoed: boot-경로"
            )),
            "unexpected outputs: {:?}",
            outcome.outputs
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Writes a declarative Toolkit whose sole tool uses the `Wasm`
    /// invoker + `wasm_path` pointing at the shared fixture, mirroring
    /// the pattern `upeg-loader`'s own dispatcher tests use
    /// (`declarative_wasm_dispatcher_loads_and_runs_module_tool`).
    ///
    /// The wrapping toolkit/tool ids (`test` / `wasm.echo`) are not
    /// arbitrary — they must match the ids the fixture's own `manifest`
    /// export declares (`test.wasm.echo`). `wasm_dispatcher_for`'s lazy
    /// closure loads the module and then re-looks-up its own
    /// `parsed.id` through `try_runtime_dispatch` to run it; if that id
    /// doesn't match anything the freshly-loaded module registered, the
    /// registry still holds the *same lazy closure* under that id (the
    /// module registered a different one), so the lookup calls back
    /// into itself — infinite recursion / stack overflow, not a clean
    /// "not found" error. Returns the toolkit directory and the tool's
    /// fully-qualified id (`{toolkit}.{local_id}`, per `upeg-loader`'s
    /// `parse.rs`).
    ///
    /// Only used by the `#[cfg(feature = "wasm-plugin")]` test below —
    /// the stub-path counterpart that used to call this with the
    /// feature off was removed (see the module-level comment above).
    #[cfg(feature = "wasm-plugin")]
    fn write_toml_wasm_path_toolkit(label: &str) -> (std::path::PathBuf, String) {
        let dir = std::env::temp_dir().join(format!(
            "upeg_frb_boot_toml_wasm_path_{}_{label}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create toolkit dir");
        let wasm = wasm_fixture_path();
        let wasm = wasm.canonicalize().unwrap_or(wasm);
        std::fs::write(
            dir.join("frb_wasm_path.toml"),
            format!(
                r#"
id = "test"
tags = ["wasm"]

[[tools]]
id = "wasm.echo"
pegboard_units = "U1"
invoker = "Wasm"
wasm_path = "{}"
"#,
                wasm.display()
            ),
        )
        .expect("write declarative toolkit toml");
        (dir, "test.wasm.echo".to_string())
    }

    #[cfg(feature = "wasm-plugin")]
    #[test]
    fn toml_wasm_path_invoker_runs_for_real_under_wasm_plugin_feature() {
        // Enabling `upeg-sources/wasm-plugin` also flips `upeg-loader`'s
        // own `wasm` feature on (`upeg-sources/wasm-plugin` forwards to
        // `upeg-loader/wasm` — see upeg-sources/Cargo.toml), which swaps
        // `upeg-loader/src/dispatcher.rs`'s `wasm_dispatcher_for` from
        // the `not(feature = "wasm")` stub arm (a fixed "requires the
        // `wasm` cargo feature" failure) to the real extism-backed one.
        let (dir, tool_id) = write_toml_wasm_path_toolkit("live");

        let config = upeg_sources::RuntimeSourceConfig {
            toolkits_dir: Some(dir.clone()),
            project_manifest: None,
            wasm_dir: None,
            mcp_import_dir: None,
        };
        let report = upeg_sources::load_local_runtime_sources(&config);
        assert!(
            report.toolkits_outcome.failed.is_empty(),
            "toolkit load reported failures: {:?}",
            report.toolkits_outcome.failed
        );

        let outcome = crate::api::tools::dispatch_tool(
            tool_id,
            serde_json::json!({ "input": "toml-경로" }).to_string(),
            None,
        );
        assert!(
            outcome.ok,
            "expected the real wasm dispatch to run, not the stub: {outcome:?}"
        );
        assert!(
            outcome.outputs.iter().any(|entry| matches!(
                &entry.value,
                crate::api::tools::CanonicalOutputValue::String { value }
                    if value == "echoed: toml-경로"
            )),
            "unexpected outputs: {:?}",
            outcome.outputs
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // A `toml_wasm_path_invoker_returns_stub_error_when_wasm_plugin_off`
    // stub-path regression guard used to live here, gated
    // `#[cfg(not(feature = "wasm-plugin"))]`. It asserted that with
    // upeg-frb's own `wasm-plugin` feature off, the `Wasm` invoker's
    // `wasm_path` dispatch failed through `upeg-loader`'s fixed stub
    // message. That precondition can no longer occur (see the
    // module-level comment above `wasm_fixture_path`), so the test was
    // removed rather than left permanently red.
}
