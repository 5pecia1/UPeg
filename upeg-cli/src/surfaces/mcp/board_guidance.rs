//! The Board guide is introspection, not an executable pin. Stdio sessions bind
//! their callable configuration and require reconnecting when it changes.

use serde_json::{Value, json};
use upeg_core::{BoardKey, Surface};

use super::{
    FIELD_ID, FIELD_METHOD, METHOD_INITIALIZE, METHOD_TOOLS_CALL, METHOD_TOOLS_LIST, McpLogWriter,
    handle_with_progress_in, invalid_params, method_not_found,
};
use crate::board_agent::{
    BOARD_CONTEXT_TOOL, BoardAgentContext, BoardAgentError, BoardAgentSnapshot, board_context,
    board_snapshot, context_from_snapshot,
};
use upeg_runtime::ExecutionContext;

const BOARD_CONFIGURATION_ERROR: i32 = -32001;

fn configuration_error(error: impl std::fmt::Display) -> Value {
    json!({"code": BOARD_CONFIGURATION_ERROR, "message": error.to_string(), "data": {"reconnectRequired": true}})
}

pub(super) fn validate_call_args(
    name: &str,
    args: &Value,
    context: &upeg_runtime::ExecutionContext,
) -> Result<(), Value> {
    if let upeg_runtime::ExecutionContext::Board { preset, .. } = context
        && let Some(meta) = upeg_runtime::toolbox_tool(name)
    {
        let merged = upeg_runtime::merge_args_with_preset(args.clone(), preset);
        crate::app::args::validate_call_args(
            &meta.input_spec,
            crate::app::reserved_inputs_for(name),
            &merged,
        )
        .map_err(|error| invalid_params(&error.message()))?;
    }
    Ok(())
}

pub(super) fn validate_board(
    surface: Surface,
    board: Option<&BoardKey>,
    state: Option<&upeg_sources::pegboard::PegboardState>,
) -> Result<(), Value> {
    if surface == Surface::Mcp
        && let Some(board) = board
    {
        resolved_context(board, state).map_err(configuration_error)?;
    }
    Ok(())
}

pub(super) fn initialize(
    mut result: Value,
    surface: Surface,
    board: Option<&BoardKey>,
    state: Option<&upeg_sources::pegboard::PegboardState>,
) -> Result<Value, Value> {
    if surface == Surface::Mcp
        && let Some(board) = board
    {
        let context = resolved_context(board, state).map_err(configuration_error)?;
        result["instructions"] = json!(format!(
            "Board: {} ({}). {} Read {} before choosing tools. Guidance describes use; it does not grant permission to execute. {}",
            context.title,
            context.board,
            context.description,
            BOARD_CONTEXT_TOOL,
            context.update_policy,
        ));
    }
    Ok(result)
}

pub(super) fn context_result(
    args: &Value,
    surface: Surface,
    board: Option<&BoardKey>,
    state: Option<&upeg_sources::pegboard::PegboardState>,
) -> Result<Value, Value> {
    let board = board
        .filter(|_| surface == Surface::Mcp)
        .ok_or_else(|| method_not_found(BOARD_CONTEXT_TOOL))?;
    if args.as_object().is_some_and(|args| !args.is_empty()) {
        return Err(invalid_params("board context takes no arguments"));
    }
    let context = resolved_context(board, state).map_err(configuration_error)?;
    let value = serde_json::to_value(context).map_err(configuration_error)?;
    Ok(json!({
        "content": [{"type":"text", "text":value.to_string()}],
        "structuredContent": value,
    }))
}

/// Bound to one transport, never global: two agents can use different Boards.
pub(super) struct BoardSession {
    snapshot: Option<Result<BoardAgentSnapshot, BoardAgentError>>,
}

impl BoardSession {
    pub(super) fn new(board: Option<&BoardKey>) -> Self {
        Self {
            snapshot: board.map(board_snapshot),
        }
    }

    pub(super) fn handle(
        &self,
        request: Value,
        board: Option<&BoardKey>,
        log: Option<&McpLogWriter>,
    ) -> Option<Value> {
        let method = request.get(FIELD_METHOD).and_then(Value::as_str);
        let state = if matches!(
            method,
            Some(METHOD_INITIALIZE | METHOD_TOOLS_LIST | METHOD_TOOLS_CALL)
        ) && let (Some(board), Some(expected)) = (board, &self.snapshot)
        {
            let error = match (expected, board_context(board)) {
                (Err(error), _) => Some(configuration_error(error)),
                (_, Err(error)) => Some(configuration_error(error)),
                (Ok(expected), Ok(current)) if expected.context.revision != current.revision => {
                    Some(configuration_error(
                        "Board configuration changed. Reconnect the MCP server to reload its guide and tools.",
                    ))
                }
                _ => None,
            };
            if let Some(error) = error {
                let id = request.get(FIELD_ID)?.clone();
                return Some(json!({"jsonrpc":"2.0", "id":id, "error":error}));
            }
            expected.as_ref().ok().map(|snapshot| &snapshot.state)
        } else {
            None
        };
        handle_with_progress_in(request, board, log, state)
    }
}

fn resolved_context(
    board: &BoardKey,
    state: Option<&upeg_sources::pegboard::PegboardState>,
) -> Result<BoardAgentContext, BoardAgentError> {
    state.map_or_else(
        || board_context(board),
        |state| context_from_snapshot(board, state),
    )
}

/// Board-scoped `tools/list`: the user's pinned tools for this board
/// (surface-filtered), sorted by id like the global list.
pub(super) fn tools_list(surface: Surface, board: &BoardKey) -> Value {
    let state = upeg_sources::pegboard::load_state();
    tools_list_in(surface, board, &state)
}

pub(super) fn tools_list_in(
    surface: Surface,
    board: &BoardKey,
    state: &upeg_sources::pegboard::PegboardState,
) -> Value {
    let mut tools: Vec<Value> =
        upeg_sources::pegboard::board_entries_on_surface_in(state, board.as_str(), None, surface)
            .into_iter()
            .filter(|(_, tool)| tool.id != crate::board_agent::BOARD_CONTEXT_TOOL)
            .map(|(placement, tool)| {
                crate::board_agent::board_tool_json(tool, placement.args_preset.as_ref())
            })
            .collect();
    if surface == Surface::Mcp {
        let description = state
            .boards
            .iter()
            .find(|entry| entry.key == board.as_str())
            .map_or("", |entry| entry.guidance.description.as_str());
        tools.push(crate::board_agent::context_tool_json(description));
    }
    tools.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    json!({
        "tools": tools,
    })
}

pub(super) fn call_context(
    name: &str,
    surface: Surface,
    board: &BoardKey,
    state: Option<&upeg_sources::pegboard::PegboardState>,
) -> Result<ExecutionContext, Value> {
    let loaded;
    let state = if let Some(state) = state {
        state
    } else {
        loaded = upeg_sources::pegboard::load_state();
        &loaded
    };
    match upeg_sources::pegboard::board_placement_on_surface_in(
        state,
        board.as_str(),
        name,
        surface,
    ) {
        Some(placement) => Ok(ExecutionContext::board(
            surface,
            board.clone(),
            placement
                .args_preset
                .clone()
                .unwrap_or_else(upeg_runtime::empty_args_preset),
        )),
        None => Err(board_method_not_found(name, surface, board, state)),
    }
}

fn board_method_not_found(
    method: &str,
    surface: Surface,
    board: &BoardKey,
    state: &upeg_sources::pegboard::PegboardState,
) -> Value {
    let pinned =
        upeg_sources::pegboard::board_entries_on_surface_in(state, board.as_str(), None, surface)
            .into_iter()
            .map(|(_, tool)| tool.id)
            .collect::<Vec<_>>();
    let hint = if pinned.is_empty() {
        String::new()
    } else {
        format!(" (board `{board}` pins: {})", pinned.join(", "))
    };
    json!({
        "code": -32601,
        "message": format!("Method not found: {}{hint}", crate::display_id(method)),
    })
}
