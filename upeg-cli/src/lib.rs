//! upeg CLI crate boundary.
//!
//! The binary entrypoint stays small: parse CLI args, load runtime sources,
//! then hand command orchestration to [`app::run`]. Transport code lives under
//! [`surfaces`]; filesystem/client/project glue lives under internal adapters.

// Tests routinely use unwrap/expect/panic/indexing — restriction lints
// configured for prod code at workspace level are noise in tests.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::get_unwrap,
        clippy::tests_outside_test_module,
        clippy::print_stdout,
        clippy::unreachable,
        clippy::string_add,
        clippy::manual_let_else,
        reason = "tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
    )
)]

pub(crate) mod adapters;
mod app;
pub(crate) mod domain;
pub(crate) mod error;
pub(crate) mod i18n;
pub(crate) mod infrastructure;
pub mod inventory;
pub(crate) mod surfaces;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod toolkit_schema_command;

pub use app::board_agent;
pub use app::run;
pub use domain::execution::dispatch::{Outcome, dispatch_tool, text_success};
pub use error::CliError;
pub use infrastructure::paths::{
    config_root, credentials_path, env, execution_log_path, http_log_path, mcp_import_dir,
    server_json_path, toolkits_dir, wasm_dir,
};
pub use infrastructure::pause::{is_paused, set_paused, toggle_paused};
/// Cross-platform process-liveness probe. Single owner for the
/// workspace: the discovery reaper (`server.json` staleness) and the
/// desktop single-instance lock in `upeg-frb` both call this one
/// implementation instead of keeping private copies that drift.
pub use infrastructure::process::pid_alive;
#[cfg(feature = "wasm-plugin")]
pub use surfaces::cli::WasmAction;
pub use surfaces::cli::{
    BoardAction, Cli, Command, CredentialAction, DiagnosticAction, HostAction, HttpAction,
    ProjectAction, ProjectChoiceArg, StorageAction, TagAction, ToolAction, ToolkitAction,
    TriggerAction,
};
pub use surfaces::http::router as http_router;
pub use surfaces::mcp::handle as handle_mcp_message;
// Board-scoped sibling of `handle_mcp_message` ("board = server", `upeg
// mcp --board <b>`). Exposed so cross-surface parity tests (CLI board
// list / HTTP `/v1/boards/{b}` / MCP board-scoped `tools/list`) can
// drive all three surfaces from one process without spawning a real
// stdio MCP server.
pub use surfaces::mcp::handle_with_board as handle_mcp_message_with_board;
pub use upeg_sources::mcp_import::{
    ImportError as McpImportError, ImportOutcome as McpImportOutcome,
    Registration as McpImportRegistration, SkipReason as McpImportSkipReason,
    SkippedTool as McpImportSkippedTool, UpstreamConfig as UpstreamMcpConfig,
    UpstreamServer as UpstreamMcpServer, register_dir as register_mcp_import_dir,
    register_server as register_upstream_mcp_server,
};

/// Re-exported so embedders can name [`current_host`]'s return type —
/// `upeg-frb`'s `classify_reachable_host` matches on `pid`/`origin` to
/// tell "this process's own embed" apart from a separate host process
/// (PRD §5.9).
pub use infrastructure::discovery::{HostOrigin, ServerInfo};

/// PRD §5.2 — return the currently-running host, if any. Used by
/// embedders (Flutter desktop via upeg-frb, future `upeg call`
/// auto-attach) to decide
/// "host" vs "client" mode at startup. The returned info is only
/// produced when the recorded endpoint actually answered `/healthz`.
pub fn current_host() -> Option<ServerInfo> {
    infrastructure::discovery::read_reachable()
}

/// Inspect native External prerequisites without running a command.
///
/// Embedders supply the optional Board key so its declared PATH override is
/// applied consistently with a Board-scoped connection.
pub fn inspect_local_tool_readiness(
    tool_id: &str,
    board_key: Option<&str>,
) -> Result<Option<upeg_runtime::readiness::ToolReadiness>, CliError> {
    let board = app::tool_readiness::parse_board_key(board_key)?;
    app::tool_readiness::inspect_local_tool_readiness(tool_id, board.as_ref())
}

/// PRD §5.9 — embed the HTTP host inside another binary (`upeg-desktop`
/// being the first consumer). Blocks the current thread; the caller
/// should spawn a worker thread and use `ready` to learn the bound
/// endpoint. On `Drop`, the discovery file is cleaned up automatically
/// (RAII guard inside `serve_with_options`).
///
/// `notifications_enabled` flows into [`surfaces::http::ServerOptions::notifications_enabled`]
/// — pass `true` from a desktop tray host, `false` for the same silent
/// behavior the CLI `--daemon` lane has by default. The fire-vs-skip
/// gate itself lives inside [`notify::fire_if`].
pub fn embedded_http_with_ready(
    notifications_enabled: bool,
    ready: std::sync::mpsc::Sender<std::io::Result<String>>,
) -> std::io::Result<()> {
    let _storage = upeg_core::paths::StorageLease::acquire().map_err(std::io::Error::other)?;
    let opts = surfaces::http::ServerOptions::loopback_ephemeral()?
        .with_notifications(notifications_enabled);
    surfaces::http::serve_with_ready(opts, ready)
}

/// Register every declared upstream MCP server into this process's
/// Toolbox, on a background thread. Long-lived server processes call
/// this once at startup; one-shot commands and the TUI never do.
///
/// It returns immediately by design: the desktop embedder must not
/// delay its splash by an unreachable upstream's bounded retries,
/// which is why this is a separate public entry point instead of being
/// folded into [`embedded_http_with_ready`] — see
/// `infrastructure::mcp_imports`'s module docs. The resulting "host is
/// serving but the imports are not in yet" window is observable: the
/// load phase moves
/// to [`McpImportPhase::Loading`] before this call returns, and the
/// host publishes it as `importsPending` on `/healthz`.
pub fn spawn_mcp_imports_for_host() {
    infrastructure::mcp_imports::spawn_detached_load_for_host();
}

/// Declare that this process is about to import, BEFORE it can be
/// observed as a host.
///
/// The desktop embedder needs this because its listener starts
/// answering `/healthz` from inside a worker thread, while the boot path
/// is still waiting for the ready signal — so scheduling the load only
/// after the host is reachable leaves a window in which the host
/// truthfully-but-uselessly answers `importsPending: false` and a client
/// takes the (importless) `tools/list` it just read as final.
///
/// Pair it with [`clear_mcp_imports_pending`] on the paths where the
/// host never comes up. Calling [`spawn_mcp_imports_for_host`] after
/// this is safe — it re-stamps the same phase.
pub fn mark_mcp_imports_pending() {
    infrastructure::mcp_imports::mark_pending();
}

/// Give back a mark taken by [`mark_mcp_imports_pending`] when no load
/// followed, so nothing waits on imports that were never scheduled. A
/// load that already finished keeps its published tally.
pub fn clear_mcp_imports_pending() {
    infrastructure::mcp_imports::clear_pending();
}

/// Typed MCP-import load phase of THIS process, for embedders that
/// render it (the Flutter status bar via `upeg-frb`). A process that
/// never imports — one-shot CLI, TUI, attach-only desktop — honestly
/// reports [`McpImportPhase::NotStarted`].
pub use infrastructure::mcp_imports::{
    McpImportPhase, McpImportTally, import_phase as mcp_import_phase,
};
pub use surfaces::tui::{Action, Key, State, View, apply_outcome, handle_key};

pub(crate) use domain::tools::display::display_id;
#[cfg(test)]
pub(crate) use domain::tools::display::truncate_for_display;
pub(crate) use domain::tools::suggest::{unknown_tool_hint, warn_unknown_filter_value};

pub mod mcp_import {
    pub use upeg_sources::mcp_import::*;
}

#[cfg(test)]
pub(crate) use adapters::credentials;
#[cfg(test)]
pub(crate) use app::{parse_log_since, run_no_command_with_terminal, v21_single_tool_toml};
#[cfg(test)]
pub(crate) use domain::tools::suggest::suggest_tool_ids;
#[cfg(test)]
pub(crate) use surfaces::cli::args::parse_kv_arg;
#[cfg(test)]
pub(crate) use surfaces::cli::formatters::format_tool_list_filtered;

pub fn load_detected_project_manifest() -> Option<(std::path::PathBuf, upeg_loader::LoadOutcome)> {
    upeg_sources::project::load_detected_project_manifest()
}

#[cfg(test)]
mod tests;
