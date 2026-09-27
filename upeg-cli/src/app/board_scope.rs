//! `upeg board <board> …` — board-scoped pin listing, editing, and calls.
//!
//! The gate and the enumeration both read the *user's* pegboard state
//! (`PegboardState.layouts` via the shared store), not the manifest's
//! static `boards` arrays: a board is what the user pinned, and a call
//! through a board merges that pin's saved [`upeg_core::ArgsPreset`] as
//! argument defaults (explicit args override).
//!
//! `pin` / `unpin` / `move` (see [`pin`]) edit that same state, so a
//! headless host can build a board without a GUI — the backlog item this
//! route closes.

use upeg_core::{BoardKey, Surface};
use upeg_runtime::ToolMetaRuntimeExt;
use upeg_sources::pegboard::{self, PegboardState};

use crate::error::CliError;
use crate::surfaces::cli::{BoardScopedAction, BoardScopedCli};

use super::call_output::{CallOutputMode, run_dispatch_outcome};
use super::{
    ExecutionContext, HostAttachPolicy, attach_notice_line, call_args_for_context,
    normalize_tool_id_segments, prepare_tool_args,
};
use clap::Parser as _;
use upeg_runtime::toolbox_tool;

mod guidance;
mod pin;

/// Where a `board <b> …` operation lands — in this process or through a
/// reachable host.
///
/// Both routes are the `cli` [`Surface`]: `board <b> call` is a person
/// at a terminal either way, and the host is told so by the
/// origin-surface header the attach client sends
/// (`infrastructure::attach`), which it validates and stamps as
/// `_upeg.surface`. Attaching used to hand the call to the host's plain
/// `http` surface, which quietly changed who the caller *was* — a chain
/// step's approval gate then refused the terminal that typed the
/// command. Listing (`board <b> list`) never attaches, so it is
/// [`Self::Local`] by construction. One type, one `surface()`, so the
/// listing and the call gate cannot disagree about which pins exist (the
/// bug this replaced: the gate used the unfiltered `placement_in`, so a
/// pin the listing hid was still callable).
enum BoardRoute {
    /// Dispatch in this process.
    Local,
    /// Dispatch through a reachable host, which answers as the surface
    /// this client declares.
    Attached(Box<crate::infrastructure::discovery::DiscoveredHost>),
}

impl BoardRoute {
    /// The route a `board <b> call` takes: attach only when the policy
    /// allows it, the board is not project-declared, the tool is not
    /// pinned to this process (D-1), and a host is actually reachable.
    ///
    /// The board check mirrors D-1 one level up. A project board exists
    /// only while THIS process detects the Project Manifest that
    /// declares it (`upeg_sources::project` module docs); the host is
    /// a separate process that resolved its own `upeg.toml`, or none, so
    /// it does not have this board at all. Attaching would send the call
    /// to a host whose `/v1/boards/<b>` is a 404 — a confusing remote
    /// error for a board that is perfectly real right here. Staying
    /// local is the only route that can succeed.
    fn for_call(board: &BoardKey, tool_id: &str, attach_policy: HostAttachPolicy) -> Self {
        Self::for_call_with(
            board,
            tool_id,
            attach_policy,
            pegboard::BoardVisibility::from_process(),
            crate::infrastructure::attach::current_host,
        )
    }

    /// [`Self::for_call`] with board visibility and the host probe injected,
    /// so tests do not mutate process-global project scope and the routing
    /// rules can be asserted against a host that is definitely
    /// reachable — "would not attach" is only a real claim when
    /// something was there to attach to.
    fn for_call_with(
        board: &BoardKey,
        tool_id: &str,
        attach_policy: HostAttachPolicy,
        visibility: pegboard::BoardVisibility,
        reachable_host: impl FnOnce() -> Option<crate::infrastructure::discovery::DiscoveredHost>,
    ) -> Self {
        if attach_policy != HostAttachPolicy::Auto
            || visibility.is_project_board(board.as_str())
            || crate::domain::execution::context::requires_local_dispatch(tool_id)
        {
            return Self::Local;
        }
        reachable_host().map_or(Self::Local, |host| Self::Attached(Box::new(host)))
    }

    /// The surface a `board <b> …` operation is evaluated on. Constant
    /// today, and a method anyway: the pin gate, the "not pinned"
    /// message, and the execution context must keep reading it from one
    /// place, the way the listing and the call gate already do.
    const fn surface(&self) -> Surface {
        match self {
            Self::Local | Self::Attached(_) => Surface::Cli,
        }
    }
}

/// Entry point for the `BoardAction::Scoped` route. `argv[0]` is the
/// board key; the rest parses as [`BoardScopedCli`].
pub(super) fn run_board_scoped(argv: Vec<String>) -> Result<String, CliError> {
    let mut tokens = argv.into_iter();
    let board_token = tokens
        .next()
        .ok_or_else(|| CliError::tool_failed("board: missing board key"))?;
    let board =
        BoardKey::parse(&board_token).map_err(|e| CliError::tool_failed(format!("board: {e}")))?;
    let rest: Vec<String> = tokens.collect();
    let parsed = BoardScopedCli::try_parse_from(rest)
        .map_err(|e| CliError::tool_failed(e.render().to_string()))?;

    let mut state = pegboard::load_state();
    ensure_board_exists(&state, &board)?;

    match parsed.action {
        BoardScopedAction::List { json } => Ok(format_board_pins(&state, &board, json)),
        BoardScopedAction::Call {
            tool_id,
            args,
            arg,
            json,
            field,
            pretty,
            local,
        } => run_board_call(
            &state,
            &board,
            tool_id,
            args,
            arg,
            CallOutputMode::from_flags(json, field, pretty),
            HostAttachPolicy::from_local_flag(local),
        ),
        BoardScopedAction::Pin {
            tool_id,
            units,
            at,
            json,
        } => pin::run_pin(
            &mut state,
            &board,
            &tool_id,
            units.as_deref(),
            at.as_deref(),
            json,
        ),
        BoardScopedAction::Unpin { tool_id, json } => {
            pin::run_unpin(&mut state, &board, &tool_id, json)
        }
        BoardScopedAction::Move { tool_id, at, json } => {
            pin::run_move(&mut state, &board, &tool_id, &at, json)
        }
        BoardScopedAction::Context { json } => guidance::run_context(&board, json),
        BoardScopedAction::Connect => guidance::run_connect(&board),
        BoardScopedAction::Describe {
            description,
            clear_description,
            instructions,
            clear_instructions,
            json,
        } => guidance::run_describe(
            &state,
            &board,
            description,
            clear_description,
            instructions,
            clear_instructions,
            json,
        ),
    }
}

fn ensure_board_exists(state: &PegboardState, board: &BoardKey) -> Result<(), CliError> {
    if pegboard::board_exists_in(state, board.as_str()) {
        return Ok(());
    }
    let boards = pegboard::board_keys_in(state).join(", ");
    Err(CliError::tool_failed(format!(
        "unknown board `{board}` (boards: {boards})"
    )))
}

/// Pins on this board, in stored `(y, x)` order. Rows are
/// `<id>\t<description>`; `--json` mirrors the tools_list entry shape
/// plus each pin's `argsPreset`.
fn format_board_pins(state: &PegboardState, board: &BoardKey, json: bool) -> String {
    // Listing never attaches, so it enumerates the local route's
    // surface — the same [`BoardRoute`] decision `run_board_call`'s gate
    // reads, rather than a second hard-coded surface constant.
    let pairs = pegboard::board_entries_on_surface_in(
        state,
        board.as_str(),
        None,
        BoardRoute::Local.surface(),
    );
    if json {
        let entries: Vec<serde_json::Value> = pairs
            .iter()
            .map(|(placement, tool)| {
                let mut entry = tool.to_json_object("name");
                if let Some(object) = entry.as_object_mut() {
                    object.insert(
                        "argsPreset".into(),
                        placement
                            .args_preset
                            .as_ref()
                            .map_or(serde_json::Value::Null, |preset| {
                                serde_json::Value::Object(preset.to_object())
                            }),
                    );
                }
                entry
            })
            .collect();
        let mut out = serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string());
        out.push('\n');
        return out;
    }
    let mut out = String::new();
    for (_, tool) in pairs {
        out.push_str(&format!("{}\t{}\n", tool.id, tool.description));
    }
    out
}

fn run_board_call(
    state: &PegboardState,
    board: &BoardKey,
    tool_id: String,
    args: String,
    arg: Vec<(String, serde_json::Value)>,
    output_mode: CallOutputMode,
    attach_policy: HostAttachPolicy,
) -> Result<String, CliError> {
    let normalized_tool_id = normalize_tool_id_segments(&tool_id);
    // D-1: a Project Manifest tool — or a Project Manifest BOARD — only
    // exists in THIS process (the host resolved its own `upeg.toml`, or
    // none), so neither auto-attaches; the tool half is the same rule
    // `dispatch_local_or_attached` applies. Routing is decided FIRST
    // because the gate below must be evaluated on the surface this call
    // actually dispatches on.
    let route = BoardRoute::for_call(board, &normalized_tool_id, attach_policy);
    let surface = route.surface();
    let Some(placement) = pegboard::board_placement_on_surface_in(
        state,
        board.as_str(),
        &normalized_tool_id,
        surface,
    ) else {
        return Err(CliError::tool_failed(unpinned_tool_message(
            state,
            board,
            &normalized_tool_id,
            surface,
        )));
    };
    let preset = placement.args_preset.clone();

    // Parse → pin-preset merge → validate, through the one pipeline
    // every CLI call path shares. Requiredness deliberately lands after
    // the merge so "preset is the default, caller args override" holds for
    // required inputs too. E-3/B-2: a Chain tool's `approve` is a
    // reserved input here too, so `board <b> call <chain> -a
    // approve=true` behaves like `upeg call`.
    let input_spec = toolbox_tool(&normalized_tool_id).map(|meta| &meta.input_spec);
    let reserved = super::reserved_inputs_for(&normalized_tool_id);
    let context = ExecutionContext::for_optional_board(surface, Some(board.clone()), preset);
    let parsed_args = call_args_for_context(args, arg, input_spec, reserved, &context)?;

    let prepared = prepare_tool_args(parsed_args, &context, None);
    let outcome = match route {
        BoardRoute::Attached(host) => {
            // D-2: the host runs External tools from the daemon's cwd
            // unless the caller says otherwise.
            let prepared = crate::domain::execution::context::with_caller_cwd(prepared);
            eprintln!("{}", attach_notice_line(&host.endpoint));
            crate::infrastructure::attach::dispatch_tool_on_board(
                &host,
                board.as_str(),
                &normalized_tool_id,
                &prepared,
                surface,
            )
            .map_err(|e| CliError::tool_failed(format!("attach: {e}")))?
        }
        // No `ensure_cli_surface` here: the pin gate above already asked
        // `board_placement_on_surface_in` about this exact surface, so a
        // tool reaching this line is registered on `cli` by
        // construction. `dispatch_tool_on_surface` keeps the gate anyway.
        BoardRoute::Local => {
            // Same live-output rule as `upeg call`: the human renderings
            // mirror the child's output to stderr while it runs, the
            // machine ones stay silent.
            crate::domain::execution::dispatch::with_live_output(output_mode.live_output(), || {
                super::dispatch_tool_on_surface(&normalized_tool_id, &prepared, Surface::Cli)
            })
        }
    };
    run_dispatch_outcome(normalized_tool_id, outcome, output_mode)
}

/// Error body for calling a tool that has no *reachable* pin on this
/// board: name the board's actual pins so the user can pick one (or pin
/// the tool first). The hint is filtered on the SAME `surface` the gate
/// rejected on (via the same [`pegboard::board_entries_on_surface_in`]
/// the listing uses) — a hint suggesting a tool this surface can't call
/// would be its own bug.
fn unpinned_tool_message(
    state: &PegboardState,
    board: &BoardKey,
    tool_id: &str,
    surface: Surface,
) -> String {
    let pinned: Vec<String> =
        pegboard::board_entries_on_surface_in(state, board.as_str(), None, surface)
            .into_iter()
            .map(|(_, tool)| tool.id.to_string())
            .collect();
    if pinned.is_empty() {
        format!(
            "tool `{tool_id}` is not pinned on board `{board}`; the board has no pinned tools \
             (pin one first, e.g. from the desktop/TUI board view)"
        )
    } else {
        format!(
            "tool `{tool_id}` is not pinned on board `{board}`; pinned tools: {}",
            pinned.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use upeg_core::BoardKey;
    use upeg_runtime::pegboard_project::{ProjectBoardDecl, ProjectBoardScope};
    use upeg_sources::pegboard::BoardVisibility;

    use super::{BoardRoute, HostAttachPolicy};
    use crate::infrastructure::discovery::{DiscoveredHost, HostOrigin, ServerInfo};

    const PROJECT_BOARD_ID: &str = "board-route-proj";
    const GLOBAL_BOARD_ID: &str = "board-route-global";
    const TOOL_ID: &str = "num.hex_to_decimal";
    const TEST_ENDPOINT: &str = "http://127.0.0.1:1";

    fn reachable_host() -> Option<DiscoveredHost> {
        Some(DiscoveredHost::for_test(ServerInfo {
            endpoint: TEST_ENDPOINT.to_string(),
            mcp_endpoint: format!("{TEST_ENDPOINT}/mcp"),
            token: "test-token".to_string(),
            pid: std::process::id(),
            started_at_ms: 0,
            origin: HostOrigin::Explicit,
        }))
    }

    #[test]
    fn a_project_board_call_stays_local_even_with_a_host_to_attach() {
        let board = BoardKey::parse(PROJECT_BOARD_ID).expect("board key");
        let visibility = BoardVisibility::for_project(ProjectBoardScope::for_manifest(
            std::path::Path::new("/board-route-scratch/upeg.toml"),
            vec![ProjectBoardDecl::new(
                board.clone(),
                PROJECT_BOARD_ID.to_string(),
            )],
        ));

        let route = BoardRoute::for_call_with(
            &board,
            TOOL_ID,
            HostAttachPolicy::Auto,
            visibility,
            reachable_host,
        );

        assert!(
            matches!(route, BoardRoute::Local),
            "the host does not have this board — attaching would 404"
        );
    }

    #[test]
    fn a_global_board_call_attaches_to_a_reachable_host() {
        // The counter-evidence that the project-board rule does not tie
        // all of board scope to local.
        let board = BoardKey::parse(GLOBAL_BOARD_ID).expect("board key");

        let route = BoardRoute::for_call_with(
            &board,
            TOOL_ID,
            HostAttachPolicy::Auto,
            BoardVisibility::global_only(),
            reachable_host,
        );

        assert!(
            matches!(route, BoardRoute::Attached(_)),
            "a global board must auto-attach as before"
        );
    }
}
