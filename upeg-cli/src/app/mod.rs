//! upeg CLI — second consumer of the universal Toolbox (PRD v2.1 §6.5).
//!
//! Output principles (PRD §6.5):
//!   - Minimal one-line result for direct tool calls (`upeg num hex-to-decimal 0xff` → `255`).
//!   - Tabular for discovery (`upeg tool list`) — pipe-friendly, no decorations.
//!   - No args → TUI when stdout is interactive; compact help when piped.
//!
//! All routing happens in `run`, which returns a `Result<String, CliError>`.
//! `main.rs` is a thin wrapper: print stdout + exit-code translation.

use std::io::IsTerminal;

use upeg_core::Surface;
use upeg_runtime::toolbox_tool;

pub(crate) mod args;
pub mod board_agent;
mod board_scope;
mod call_output;
mod dynamic_route;
mod file_output;
mod plugin_command;

use crate::domain::execution::dispatch;
use crate::domain::execution::dispatch::LiveOutput;
use call_output::{CallOutputMode, CallOutputPlan, run_dispatch_outcome};
use dynamic_route::run_dynamic_tool_command;
use file_output::{FileOutputOptions, write_file_output};

use crate::adapters::execution_log;
use crate::adapters::{credentials, triggers};
use crate::error::CliError;
use crate::infrastructure::{auth, daemonize, discovery, lifecycle, paths};
use crate::inventory::command::run_interface_command;
#[cfg(feature = "wasm-plugin")]
use crate::surfaces::cli::WasmAction;
use crate::surfaces::cli::completion::generate_completion;
use crate::surfaces::cli::doctor::{format_doctor, format_doctor_json};
use crate::surfaces::cli::formatters::{
    format_board_list, format_board_show, format_tag_list, format_tag_show,
    format_tool_list_filtered, format_tool_show, format_tool_show_json, format_tool_validate,
    format_toolkit_list, format_toolkit_show, format_toolkit_validate,
};
use crate::surfaces::cli::{
    BoardAction, Cli, Command, CredentialAction, HostAction, HttpAction, TagAction, ToolAction,
    ToolkitAction, TriggerAction,
};
use crate::surfaces::{http, mcp, tui};

fn normalize_cli_tool_token(s: &str) -> String {
    s.trim().replace('-', "_")
}

/// Dash→underscore normalization per `.`-separated segment, preserving
/// the dot. Unlike [`normalize_cli_tool_token`], this does NOT trim
/// whitespace: padded ids like `" num.hex_to_decimal "` must keep
/// being rejected as UnknownTool.
fn normalize_tool_id_segments(tool_id: &str) -> String {
    tool_id
        .split('.')
        .map(|segment| segment.replace('-', "_"))
        .collect::<Vec<_>>()
        .join(".")
}

/// Resolve the execution context for a CLI-entered call: `--board` (or a
/// board subcommand) scopes the call to that board, merging the pin's
/// saved args preset when the tool is pinned there. No board → global.
///
/// Permissive on membership by design: the `--board` flag annotates
/// context for any board key (an unpinned tool simply has no preset).
/// The strict "must be pinned" gate belongs to `upeg board <b> call`
/// ([`resolve_board_scoped_context`]).
pub(crate) fn cli_execution_context_for(
    surface: Surface,
    active_board: Option<&str>,
    tool_id: &str,
) -> Result<ExecutionContext, CliError> {
    let Some(board) = active_board.map(str::trim).filter(|b| !b.is_empty()) else {
        return Ok(ExecutionContext::global(surface));
    };
    let board = upeg_core::BoardKey::parse(board)
        .map_err(|e| CliError::tool_failed(format!("--board: {e}")))?;
    let state = upeg_sources::pegboard::load_state();
    let preset = upeg_sources::pegboard::placement_in(&state, board.as_str(), tool_id)
        .and_then(|placement| placement.args_preset.clone());
    Ok(ExecutionContext::for_optional_board(
        surface,
        Some(board),
        preset,
    ))
}

pub(crate) fn parse_log_since(value: &str) -> Result<u64, CliError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::tool_failed("log --since cannot be empty"));
    }
    if let Ok(epoch_ms) = value.parse::<u64>() {
        return Ok(epoch_ms);
    }
    let (amount, unit) = value.split_at(value.len().saturating_sub(1));
    let n = amount.trim().parse::<u64>().map_err(|_| {
        CliError::tool_failed("log --since must be epoch milliseconds or a window like 1h")
    })?;
    let multiplier_ms = match unit {
        "s" | "S" => 1_000,
        "m" | "M" => 60_000,
        "h" | "H" => 3_600_000,
        "d" | "D" => 86_400_000,
        _ => {
            return Err(CliError::tool_failed(
                "log --since unit must be one of s, m, h, d",
            ));
        }
    };
    let window = n.saturating_mul(multiplier_ms);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    Ok(now.saturating_sub(window))
}

/// Parse `upeg call`-shaped arguments — the `-a name=value` pairs or
/// the raw-JSON positional — into the args object. PARSING ONLY: no
/// requiredness, no unknown-key rejection.
///
/// Validation is deliberately a separate step ([`args::validate_call_args`],
/// run for every path by [`call_args_for_context`]) because a board
/// pin's [`upeg_core::ArgsPreset`] may supply *required* inputs, so
/// requiredness can only be judged after the preset merge. Callers
/// supply the target's [`args::ReservedInputs`] so a Chain tool's
/// `approve` survives both arg syntaxes (E-3/B-2).
fn parse_call_args(
    args: String,
    arg: Vec<(String, serde_json::Value)>,
    input_spec: Option<&upeg_core::InputSpec>,
    reserved: args::ReservedInputs,
) -> Result<serde_json::Value, CliError> {
    if arg.is_empty() {
        return args::read_call_args_json(args, input_spec, reserved);
    }
    match input_spec {
        Some(spec) => args::named_args_from_cli(spec, reserved, arg),
        // No registered spec → pass args through unvalidated. Reached
        // either from `--dry-run` on a tool that isn't registered yet
        // (a deliberate `upeg call <missing> --dry-run` lint check) or
        // from a real call that will hit UnknownTool downstream.
        None => Ok(serde_json::Value::Object(arg.into_iter().collect())),
    }
}

/// The pin preset a resolved [`ExecutionContext`] contributes as
/// argument defaults. A board-scoped context always carries one
/// (possibly empty); a global (boardless) call has none, and merging
/// is then skipped entirely so a plain `upeg call` never reshapes the
/// caller's args.
fn context_preset(context: &ExecutionContext) -> Option<&upeg_core::ArgsPreset> {
    match context {
        ExecutionContext::Global { .. } => None,
        ExecutionContext::Board { preset, .. } => Some(preset),
    }
}

/// THE args pipeline for every CLI call path (`upeg call`, `upeg
/// --board B call`, `board <b> call`, `trigger fire`): parse → merge
/// the board pin's preset → validate, in exactly that order.
///
/// The order is the contract (C-4 + the pin-preset rule): the preset
/// supplies defaults the caller may override, so unknown keys AND
/// requiredness must both be judged on the merged object. Validating
/// first would reject `upeg --board B call TOOL -a partial` for a
/// required input the pin already supplies. `upeg call` without a
/// board resolves to [`ExecutionContext::Global`], where the merge is
/// a no-op and this collapses to plain parse-then-validate.
fn call_args_for_context(
    args: String,
    arg: Vec<(String, serde_json::Value)>,
    input_spec: Option<&upeg_core::InputSpec>,
    reserved: args::ReservedInputs,
    context: &ExecutionContext,
) -> Result<serde_json::Value, CliError> {
    let parsed = parse_call_args(args, arg, input_spec, reserved)?;
    ensure_object_or_null(&parsed)?;
    let merged = match context_preset(context) {
        Some(preset) => upeg_runtime::merge_args_with_preset(parsed, preset),
        None => parsed,
    };
    if let Some(spec) = input_spec {
        args::validate_call_args(spec, reserved, &merged)?;
    }
    Ok(merged)
}

/// The target's [`args::ReservedInputs`], or `NONE` for an unregistered
/// id. One place decides "does this tool accept `approve`?" so `upeg
/// call`, `board <b> call`, and `trigger fire` cannot drift apart.
pub(crate) fn reserved_inputs_for(tool_id: &str) -> args::ReservedInputs {
    toolbox_tool(tool_id).map_or(args::ReservedInputs::NONE, |meta| {
        args::ReservedInputs::for_invoker(meta.invoker)
    })
}

fn ensure_object_or_null(args: &serde_json::Value) -> Result<(), CliError> {
    if !args.is_null() && !args.is_object() {
        return Err(CliError::tool_failed(
            "args must be a JSON object or null (zero-arg tools accept either)".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn tools_list_json_for_surface(surface: Surface) -> serde_json::Value {
    upeg_runtime::tools_list_json_for_surface(surface, "name")
}

pub(crate) use crate::domain::execution::context::{
    ExecutionContext, dispatch_tool_call, dispatch_tool_on_surface, prepare_tool_args,
};

pub(crate) fn list_credentials() -> std::io::Result<Vec<credentials::CredentialRecord>> {
    credentials::list_references()
}

/// Whether a tool call may auto-attach to a running host (PRD §5.2 L4)
/// or must stay in-process. `--local` on `upeg call` and the dynamic
/// route selects `LocalOnly`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostAttachPolicy {
    Auto,
    LocalOnly,
}

impl HostAttachPolicy {
    pub(crate) const fn from_local_flag(local: bool) -> Self {
        if local { Self::LocalOnly } else { Self::Auto }
    }
}

/// One-line stderr notice emitted when a call auto-attaches, so the
/// user can tell where the run landed without polluting stdout (which
/// stays the tool's payload).
fn attach_notice_line(endpoint: &str) -> String {
    format!("attached to host {endpoint}")
}

/// Dispatch one tool call either through a live host or in-process.
/// Shared by `upeg call` and the dynamic `{toolkit} {tool}` route so
/// both surfaces keep identical attach semantics.
///
/// PRD §5.2 L4 attach: host running → dispatch through HTTP so the
/// call shows up in the host's shared state (execution log, live
/// clients panel). The host's HTTP surface gate enforces visibility,
/// so the local CLI surface check is skipped on the attach path.
fn dispatch_local_or_attached(
    tool_id: &str,
    args: serde_json::Value,
    active_board: Option<&str>,
    policy: HostAttachPolicy,
    live: LiveOutput,
) -> Result<crate::domain::execution::dispatch::Outcome, CliError> {
    // D-1: a Project Manifest tool is defined by THIS process's cwd. A
    // host started elsewhere resolved a different `upeg.toml` (or none),
    // so attaching would ask it for a tool it has never heard of. Such a
    // tool stays in-process even when a host is reachable.
    if policy == HostAttachPolicy::Auto
        && !crate::domain::execution::context::requires_local_dispatch(tool_id)
        && let Some(host) = crate::infrastructure::attach::current_host()
    {
        // D-3: the surface is `cli` on both sides of the wire. The host
        // re-stamps `_upeg.surface` from the origin-surface header this
        // client sends (`infrastructure::attach`), so a terminal keeps
        // being a terminal when a host happens to be up — otherwise a
        // chain's approval gate refuses the person typing the command.
        let context = cli_execution_context_for(Surface::Cli, active_board, tool_id)?;
        let args = prepare_tool_args(args, &context, None);
        // D-2: without this the host runs External tools from the
        // daemon's working directory, silently resolving relative paths
        // against the wrong tree.
        let args = crate::domain::execution::context::with_caller_cwd(args);
        eprintln!("{}", attach_notice_line(&host.endpoint));
        return crate::infrastructure::attach::dispatch_tool(&host, tool_id, &args, Surface::Cli)
            .map_err(|e| CliError::tool_failed(format!("attach: {e}")));
    }

    let context = cli_execution_context_for(Surface::Cli, active_board, tool_id)?;
    let args = prepare_tool_args(args, &context, None);
    ensure_cli_surface(tool_id)?;
    // Live child output belongs to the in-process lane only. The attach
    // lane above returns the host's final envelope over HTTP and has no
    // channel to stream through — see the attach note in
    // docs/architecture/http-api.md.
    Ok(dispatch::with_live_output(live, || {
        dispatch_tool_on_surface(tool_id, &args, Surface::Cli)
    }))
}

/// Library entry point. Given parsed CLI arguments, returns the stdout
/// text or an error. `main.rs` is a thin wrapper around this.
///
/// Not pure: command branches own their I/O. The notable side-effecting
/// branches are `Mcp` (takes over stdin/stdout for JSON-RPC), `Http`
/// (binds a listener and blocks), the no-command interactive case
/// (launches the TUI event loop on a terminal stdout), and every
/// dispatch path (writes to the execution log via the runtime).
///
/// Tests that need a deterministic, side-effect-light slice should call
/// the granular helpers — for instance
/// [`run_no_command_with_terminal`] with `stdout_is_terminal=false`
/// exercises the no-command path without spawning the TUI.
pub fn run(cli: Cli) -> Result<String, CliError> {
    let active_board = cli.board.as_deref();
    let active_tui_tag = cli.tui_tag.as_deref();
    match cli.command {
        None => run_no_command(active_board, active_tui_tag),
        Some(Command::Interface { action }) => run_interface_command(action),
        Some(Command::Tool { action }) => run_tool_action(action),
        Some(Command::Toolkit { action }) => run_toolkit_action(action),
        Some(Command::Tag { action }) => run_tag_action(action),
        Some(Command::Board { action }) => run_board_action(action),
        Some(Command::Log {
            tool,
            surface,
            status,
            trigger,
            since,
            limit,
            json,
        }) => run_log_command(tool, surface, status, trigger, since, limit, json),
        Some(Command::Credential { action }) => run_credential_action(action),
        Some(Command::Trigger { action }) => run_trigger_action(action, active_board),
        Some(Command::Mcp { board }) => run_mcp_server(board.as_deref()),
        Some(Command::Http {
            action,
            addr,
            daemon,
            token,
            token_file,
            log_file,
            cors_origin,
        }) => run_http_command(
            action,
            addr,
            daemon,
            token,
            token_file,
            log_file,
            cors_origin,
        ),
        Some(Command::Host { action }) => run_host_command(action),
        Some(Command::Completions { shell }) => Ok(generate_completion(shell)),
        Some(Command::Doctor { json }) => Ok(if json {
            format_doctor_json()
        } else {
            format_doctor()
        }),
        // B-5: `upeg tui` is an explicit alias for the no-command
        // interactive entry point — same terminal gate, same
        // `--board`/`--tui-tag` handling.
        Some(Command::Tui) => run_no_command(active_board, active_tui_tag),
        #[cfg(feature = "wasm-plugin")]
        Some(Command::Wasm { action }) => run_wasm_action(action),
        Some(Command::Plugin { action }) => plugin_command::run_plugin_command(action),
        Some(Command::Call {
            tool_id,
            args,
            arg,
            dry_run,
            json,
            field,
            pretty,
            local,
            out,
            force,
        }) => run_call_command(
            tool_id,
            args,
            arg,
            dry_run,
            CallOutputPlan {
                mode: CallOutputMode::from_flags(json, field, pretty),
                file: FileOutputOptions { out, force },
            },
            HostAttachPolicy::from_local_flag(local),
            active_board,
        ),
        Some(Command::External(argv)) => run_dynamic_tool_command(argv, active_board),
    }
}

fn run_no_command(
    active_board: Option<&str>,
    active_tui_tag: Option<&str>,
) -> Result<String, CliError> {
    run_no_command_with_terminal(
        active_board,
        active_tui_tag,
        std::io::stdout().is_terminal(),
    )
}

pub(crate) fn run_no_command_with_terminal(
    active_board: Option<&str>,
    active_tui_tag: Option<&str>,
    stdout_is_terminal: bool,
) -> Result<String, CliError> {
    if stdout_is_terminal {
        tui::serve_with_filters(active_board, active_tui_tag)
            .map_err(|e| CliError::tool_failed(format!("tui: {e}")))?;
        return Ok(String::new());
    }
    let mut selected = Vec::new();
    if let Some(board) = active_board.map(str::trim).filter(|v| !v.is_empty()) {
        selected.push(format!("Board `{board}`"));
    }
    if let Some(tag) = active_tui_tag.map(str::trim).filter(|v| !v.is_empty()) {
        selected.push(format!("Tag `{tag}`"));
    }
    Ok(if selected.is_empty() {
        "upeg — pin once, call anywhere. Try `upeg --help`.\n".into()
    } else {
        format!(
            "upeg — pin once, call anywhere. {} selected. Try `upeg --help`.\n",
            selected.join(" + ")
        )
    })
}

fn run_tool_action(action: ToolAction) -> Result<String, CliError> {
    match action {
        ToolAction::List {
            tag,
            surface,
            board,
            pin,
            json,
        } => format_tool_list_filtered(
            tag.as_deref(),
            surface.as_deref(),
            board.as_deref(),
            pin.as_deref(),
            json,
        ),
        ToolAction::Show { id, json } => {
            if json {
                format_tool_show_json(&id)
            } else {
                format_tool_show(&id)
            }
        }
        ToolAction::Validate {
            path,
            resolve_chain,
        } => format_tool_validate(&path, resolve_chain),
    }
}

fn run_toolkit_action(action: ToolkitAction) -> Result<String, CliError> {
    match action {
        ToolkitAction::List { json } => format_toolkit_list(json),
        ToolkitAction::Show { id, json } => format_toolkit_show(&id, json),
        ToolkitAction::Validate { dir } => format_toolkit_validate(dir.as_deref()),
    }
}

fn run_tag_action(action: TagAction) -> Result<String, CliError> {
    match action {
        TagAction::List { json } => format_tag_list(json),
        TagAction::Show { tag, json } => format_tag_show(&tag, json),
    }
}

fn run_board_action(action: BoardAction) -> Result<String, CliError> {
    match action {
        BoardAction::List { json } => format_board_list(json),
        BoardAction::Show { board, json } => format_board_show(&board, json),
        BoardAction::Scoped(argv) => board_scope::run_board_scoped(argv),
    }
}

fn run_log_command(
    tool: Option<String>,
    surface: Option<String>,
    status: Option<String>,
    trigger: Option<String>,
    since: Option<String>,
    limit: usize,
    json: bool,
) -> Result<String, CliError> {
    let since_ms = since.as_deref().map(parse_log_since).transpose()?;
    let records = execution_log::read_records(&execution_log::LogFilter {
        tool_id: trimmed_non_empty(tool),
        surface: trimmed_non_empty(surface),
        status: trimmed_non_empty(status),
        trigger: trimmed_non_empty(trigger),
        since_ms,
        limit: Some(limit),
    })
    .map_err(|e| CliError::tool_failed(format!("read execution log: {e}")))?;
    Ok(execution_log::format_records(&records, json))
}

fn trimmed_non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn run_credential_action(action: CredentialAction) -> Result<String, CliError> {
    match action {
        CredentialAction::Add {
            name,
            value_type,
            store,
            env,
            service,
            account,
            target,
        } => {
            let record =
                credentials::add_reference_with_schema(credentials::CredentialReferenceSpec {
                    name: &name,
                    value_type: value_type.as_deref(),
                    store: store.as_deref(),
                    env: env.as_deref(),
                    keychain_service: service.as_deref(),
                    keychain_account: account.as_deref(),
                    target: target.as_deref(),
                })
                .map_err(|e| CliError::tool_failed(format!("credential add: {e}")))?;
            Ok(credentials::format_reference(&record))
        }
        CredentialAction::List { json } => {
            let records = credentials::list_references()
                .map_err(|e| CliError::tool_failed(format!("credential list: {e}")))?;
            Ok(credentials::format_references(&records, json))
        }
    }
}

fn run_trigger_action(
    action: TriggerAction,
    active_board: Option<&str>,
) -> Result<String, CliError> {
    match action {
        TriggerAction::List { json } => Ok(triggers::format_list(json)),
        TriggerAction::Run { watch } => {
            if watch {
                triggers::run_watch(active_board);
            }
            Ok(triggers::run_once(active_board))
        }
        TriggerAction::Fire { id, args, arg } => {
            let input_spec = toolbox_tool(&id).map(|meta| &meta.input_spec);
            // Same order as every other call path: the board context is
            // resolved first so its pin preset participates in the merge
            // that validation then judges (see `call_args_for_context`).
            let context = cli_execution_context_for(Surface::Cli, active_board, &id)?;
            let parsed_args =
                call_args_for_context(args, arg, input_spec, reserved_inputs_for(&id), &context)?;
            ensure_cli_surface(&id)?;
            // `_upeg.trigger` carries the *fired trigger*, never the Tool id
            // (see docs/architecture/call-envelope.md). `fire` names a Tool, so
            // it stamps that Tool's first declared binding; a Tool with no
            // binding is stamped with nothing.
            let fired = upeg_runtime::manual_fire_trigger_label(&id);
            // `trigger fire` renders like a bare `upeg call`, so it
            // gets the same live mirror while the tool runs.
            let outcome = dispatch::with_live_output(CallOutputMode::Primary.live_output(), || {
                dispatch_tool_call(&id, parsed_args, &context, fired.as_deref())
            });
            run_dispatch_outcome(id.clone(), outcome, CallOutputMode::Primary)
        }
    }
}

fn run_mcp_server(board: Option<&str>) -> Result<String, CliError> {
    let board = board
        .map(|raw| {
            upeg_core::BoardKey::parse(raw)
                .map_err(|e| CliError::tool_failed(format!("mcp --board: {e}")))
        })
        .transpose()?;
    if let Some(board) = &board {
        // Fail fast on a board the user never defined instead of
        // serving an empty tools/list the client can't diagnose.
        let state = upeg_sources::pegboard::load_state();
        if !upeg_sources::pegboard::board_exists_in(&state, board.as_str()) {
            let boards = upeg_sources::pegboard::board_keys_in(&state).join(", ");
            return Err(CliError::tool_failed(format!(
                "mcp --board: unknown board `{board}` (boards: {boards})"
            )));
        }
    }
    mcp::serve(board);
    Ok(String::new())
}

fn run_http_command(
    action: Option<HttpAction>,
    addr: Option<String>,
    daemon: bool,
    token: Option<String>,
    token_file: Option<std::path::PathBuf>,
    log_file: Option<std::path::PathBuf>,
    cors_origin: Vec<String>,
) -> Result<String, CliError> {
    match action {
        None | Some(HttpAction::Start) => {
            run_http_start(addr, daemon, token, token_file, log_file, cors_origin)
        }
        Some(HttpAction::Status { json, pairing }) => {
            if json {
                let v = lifecycle::status_json();
                Ok(format!(
                    "{}\n",
                    serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())
                ))
            } else {
                let mut out = lifecycle::status_text();
                if pairing {
                    out.push('\n');
                    out.push_str(&http::pairing_status_block());
                }
                Ok(out)
            }
        }
        Some(HttpAction::Stop { force }) => {
            lifecycle::stop(force).map_err(|e| CliError::tool_failed(format!("stop: {e}")))
        }
        Some(HttpAction::Restart { force }) => {
            lifecycle::prepare_restart(force)
                .map_err(|e| CliError::tool_failed(format!("restart (stop phase): {e}")))?;
            run_http_start(addr, daemon, token, token_file, log_file, cors_origin)
        }
        Some(HttpAction::Logs { lines }) => {
            let path = match lifecycle::log_path() {
                Some(p) => p.display().to_string(),
                None => "(unavailable)".to_string(),
            };
            let lines = lifecycle::logs_tail(lines)
                .map_err(|e| CliError::tool_failed(format!("logs ({path}): {e}")))?;
            let mut out = lines.join("\n");
            if !out.is_empty() {
                out.push('\n');
            }
            Ok(out)
        }
    }
}

fn run_host_command(action: HostAction) -> Result<String, CliError> {
    match action {
        HostAction::Start {
            daemon,
            addr,
            log_file,
            cors_origin,
        } => run_http_start(addr, daemon, None, None, log_file, cors_origin),
        HostAction::Status { json } => {
            if json {
                let v = lifecycle::status_json();
                Ok(format!(
                    "{}\n",
                    serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())
                ))
            } else {
                Ok(lifecycle::status_text())
            }
        }
        HostAction::Stop { force } => {
            lifecycle::stop(force).map_err(|e| CliError::tool_failed(format!("stop: {e}")))
        }
        HostAction::Logs { lines } => {
            let path = match lifecycle::log_path() {
                Some(p) => p.display().to_string(),
                None => "(unavailable)".to_string(),
            };
            let lines = lifecycle::logs_tail(lines)
                .map_err(|e| CliError::tool_failed(format!("logs ({path}): {e}")))?;
            let mut out = lines.join("\n");
            if !out.is_empty() {
                out.push('\n');
            }
            Ok(out)
        }
    }
}

fn run_http_start(
    addr: Option<String>,
    detach: bool,
    token: Option<String>,
    token_file: Option<std::path::PathBuf>,
    log_file: Option<std::path::PathBuf>,
    cors_origin: Vec<String>,
) -> Result<String, CliError> {
    // Pre-flight: refuse to bind on top of a live host. The detached
    // child skips this check because its parent already proved the
    // host slot is free.
    if !daemonize::is_detached_child()
        && let Some(info) = discovery::read_reachable()
    {
        return Err(CliError::tool_failed(format!(
            "upeg http is already running on {} (pid {}); use `upeg http stop` or `upeg http restart`",
            info.endpoint, info.pid
        )));
    }

    if detach && !daemonize::is_detached_child() {
        let log_path = log_file
            .clone()
            .or_else(paths::http_log_path)
            .ok_or_else(|| {
                CliError::tool_failed(
                    "cannot resolve log path; set UPEG_HTTP_LOG_PATH or pass --log-file",
                )
            })?;
        // `--token` is deliberately absent here: the child receives it
        // through its environment (`daemonize::detach_and_exit`), never
        // on a `ps`-readable command line.
        let mut extra: Vec<String> = Vec::new();
        if let Some(a) = &addr {
            extra.push("--addr".into());
            extra.push(a.clone());
        }
        if let Some(tf) = &token_file {
            extra.push("--token-file".into());
            extra.push(tf.display().to_string());
        }
        if let Some(lf) = &log_file {
            extra.push("--log-file".into());
            extra.push(lf.display().to_string());
        }
        for origin in &cors_origin {
            extra.push("--cors-origin".into());
            extra.push(origin.clone());
        }
        daemonize::detach_and_exit(log_path, &extra, token.as_deref())
            .map_err(|e| CliError::tool_failed(format!("detach: {e}")))?;
        // Marker env saw a re-entry: fall through to foreground.
    }

    let resolved = auth::resolve_token(token.as_deref(), token_file.as_deref())
        .map_err(|e| CliError::tool_failed(format!("token: {e}")))?;

    let notifications_enabled = crate::infrastructure::notify::enabled_from(
        std::env::var(crate::infrastructure::paths::env::NOTIFY)
            .ok()
            .as_deref(),
    );

    let opts = http::ServerOptions {
        addr: addr.unwrap_or_else(|| http::DEFAULT_BIND.into()),
        token: resolved,
        publish_discovery: true,
        allow_non_loopback: false,
        notifications_enabled,
        cors_origins: cors_origin,
        // Every lane reaching here is a user-invoked host start
        // (foreground or the detached child of `--daemon`). The
        // desktop's in-process embed goes through
        // `ServerOptions::loopback_ephemeral`, which stamps
        // `HostOrigin::Embedded` instead.
        origin: discovery::HostOrigin::Explicit,
    };
    // Long-lived server process: register MCP imports eagerly, before
    // the listener starts answering `tools/list`. Failures are reported
    // and non-fatal — a broken upstream must not stop the host from
    // serving its local Toolbox (docs/architecture/mcp.md).
    crate::infrastructure::mcp_imports::load_and_report_for_host();
    http::serve_with_options(opts).map_err(|e| CliError::tool_failed(format!("http: {e}")))?;
    Ok(String::new())
}

#[cfg(feature = "wasm-plugin")]
fn run_wasm_action(action: WasmAction) -> Result<String, CliError> {
    match action {
        WasmAction::Load { path } => {
            let ids = upeg_wasm::load_and_register(std::path::Path::new(&path))
                .map_err(|e| CliError::tool_failed(format!("wasm `{path}`: {e}")))?;
            let mut out = String::with_capacity(ids.len() * 16);
            for id in ids {
                out.push_str(id);
                out.push('\n');
            }
            Ok(out)
        }
        WasmAction::Template => Ok(crate::surfaces::cli::wasm_template::wasm_plugin_template()),
    }
}

fn run_call_command(
    tool_id: String,
    args: String,
    arg: Vec<(String, serde_json::Value)>,
    dry_run: bool,
    output: CallOutputPlan,
    attach_policy: HostAttachPolicy,
    active_board: Option<&str>,
) -> Result<String, CliError> {
    let normalized_tool_id = normalize_tool_id_segments(&tool_id);
    let meta = toolbox_tool(&normalized_tool_id);
    let input_spec = meta.map(|meta| &meta.input_spec);
    let reserved = reserved_inputs_for(&normalized_tool_id);
    // Resolve the board scope BEFORE validating: with `--board B` the
    // pin's preset may supply required inputs, exactly as it does for
    // `board <b> call`, so requiredness is judged after the merge
    // `call_args_for_context` performs. Which board contributes a
    // preset does not depend on the dispatch surface, so resolving as
    // `Surface::Cli` here is correct even when the call later attaches
    // to a host over HTTP.
    let context = cli_execution_context_for(Surface::Cli, active_board, &normalized_tool_id)?;
    let parsed_args = call_args_for_context(args, arg, input_spec, reserved, &context)?;

    if dry_run {
        // Show exactly what dispatch would receive: preset merge (when
        // the tool is pinned on the board) + context annotations.
        let parsed_args = prepare_tool_args(parsed_args, &context, None);
        let mut out =
            serde_json::to_string_pretty(&parsed_args).unwrap_or_else(|_| "{}".to_string());
        out.push('\n');
        return Ok(out);
    }

    // Attach-vs-local routing (PRD §5.2 L4) is factored into
    // `dispatch_local_or_attached`, which honours the `--local` policy and
    // applies the correct surface context on each path. The resulting
    // outcome is then rendered — writing any `File` primary output to disk
    // first — by `finish_call_outcome`.
    let live = output.mode.live_output();
    let outcome = dispatch_local_or_attached(
        &normalized_tool_id,
        parsed_args,
        active_board,
        attach_policy,
        live,
    )?;
    finish_call_outcome(normalized_tool_id, outcome, output)
}

/// Render a dispatch outcome, first writing any `File` output to disk.
///
/// When a successful result's primary output is a `File`, its bytes are
/// written via [`write_file_output`] (to `out`, or the current directory
/// under the file's own name). In the human/`--field`/`--pretty` modes the
/// call then reports the written path instead of dumping the file's JSON;
/// `--json` still emits the canonical envelope after the side-effecting write.
fn finish_call_outcome(
    tool_id: String,
    outcome: dispatch::Outcome,
    output: CallOutputPlan,
) -> Result<String, CliError> {
    let CallOutputPlan { mode, file } = output;
    if let dispatch::Outcome::Success(success) = &outcome
        && let Some(written) = write_file_output(success, &file)?
        && !matches!(mode, CallOutputMode::Json)
    {
        return Ok(format!("wrote {written}\n"));
    }
    run_dispatch_outcome(tool_id, outcome, mode)
}

fn ensure_cli_surface(tool_id: &str) -> Result<(), CliError> {
    if let Some(message) = cli_surface_error(tool_id) {
        return Err(CliError::tool_failed(message));
    }
    Ok(())
}

pub(crate) fn cli_surface_error(tool_id: &str) -> Option<String> {
    let meta = toolbox_tool(tool_id)?;
    if meta.is_on_surface(Surface::Cli) {
        return None;
    }
    Some(format!(
        "tool `{tool_id}` is not on surface `cli` (surfaces: {})",
        meta.surfaces_label(),
    ))
}

#[cfg(test)]
pub(crate) fn v21_single_tool_toml(flat_tool: &str) -> String {
    use std::fmt::Write as _;

    let mut tool: toml::Table = toml::from_str(flat_tool).expect("valid flat test fixture");
    if tool.contains_key("tools") {
        return flat_tool.to_string();
    }

    let full_id = tool
        .get("id")
        .and_then(toml::Value::as_str)
        .map(str::to_string);
    let toolkit = tool
        .remove("toolkit")
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();

    if let Some(full_id) = full_id {
        let local_id = upeg_core::ToolId::parse_canonical_in_toolkit(&full_id, &toolkit)
            .map(|identity| identity.local().to_string())
            .unwrap_or(full_id);
        tool.insert("id".into(), toml::Value::String(local_id));
    }
    tool.entry("pegboard_units")
        .or_insert_with(|| toml::Value::String("U1".into()));

    let mut out = String::new();
    writeln!(&mut out, "id = {}", toml::Value::String(toolkit)).expect("write to string");
    out.push_str("\n[[tools]]\n");
    for (key, value) in tool {
        writeln!(&mut out, "{key} = {value}").expect("write to string");
    }
    out
}
