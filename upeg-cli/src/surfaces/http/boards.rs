//! `/v1/boards*` handlers — user-pegboard-backed board enumeration and
//! board-scoped tool calls.
//!
//! The gate and the enumeration read the *user's* pegboard state
//! (`PegboardState.layouts` via the shared store), not the manifest's
//! static `boards` arrays: a tool the user pinned is callable, a tool
//! they didn't is 404 (the pre-overhaul manifest gate 404'd user pins).

use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde_json::{Value, json};
use upeg_core::{BoardKey, Surface};
use upeg_runtime::{ExecutionContext, ToolMetaRuntimeExt};
use upeg_sources::pegboard::{self, PegboardState};

use super::{
    HttpState, dispatch_http_tool_with_context, not_found_response, origin_surface_from_headers,
    principal_from_headers,
};

pub(super) async fn boards_list() -> Json<Value> {
    let state = pegboard::load_state();
    Json(json!({
        "boards": pegboard::board_keys_in(&state)
            .iter()
            .map(|board| board_json(&state, board))
            .collect::<Vec<_>>()
    }))
}

pub(super) async fn board_show(Path(board): Path<String>) -> impl IntoResponse {
    let state = pegboard::load_state();
    if !pegboard::board_exists_in(&state, &board) {
        return not_found_response("board", &board);
    }
    (StatusCode::OK, Json(board_json(&state, &board)))
}

/// Board-scoped call gate: the tool must be pinned on the board AND
/// visible on the calling surface; the pin's saved args preset merges as
/// defaults through the Board execution context.
///
/// The surface is the request's own
/// ([`origin_surface_from_headers`]) — an attached `upeg board <b> call`
/// is answered as `cli`, so the pin gate and the chain approval gate see
/// the same caller the person would have been without a host running.
pub(super) async fn board_tools_call(
    State(state): State<HttpState>,
    Path((board, id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let surface = origin_surface_from_headers(&state, &headers);
    let pegboard_state = pegboard::load_state();
    let Ok(board_key) = BoardKey::parse(&board) else {
        return not_found_response("board tool", &format!("{board}/{id}"));
    };
    let placement =
        pegboard::board_placement_on_surface_in(&pegboard_state, board_key.as_str(), &id, surface);
    let Some(placement) = placement else {
        return not_found_response("board tool", &format!("{board}/{id}"));
    };
    let context = ExecutionContext::board(
        surface,
        board_key,
        placement
            .args_preset
            .clone()
            .unwrap_or_else(upeg_runtime::empty_args_preset),
    )
    .with_principal(principal_from_headers(&state, &headers));
    dispatch_http_tool_with_context(state.notifications_enabled, id, body, context, None)
}

/// Board JSON from the user's pegboard state: the board's pinned tools
/// visible on the HTTP surface, in stored order. Built from
/// `upeg_sources::pegboard::board_entries_on_surface_in` — the same
/// enumeration CLI's `upeg board <b> list --json` and MCP's
/// board-scoped `tools/list` route through, so the three surfaces
/// cannot disagree about *which ids* a board lists (only the JSON
/// shape each surface wraps them in differs).
fn board_json(state: &PegboardState, board: &str) -> Value {
    board_json_on_surface(state, board, Surface::Http)
}

pub(super) fn board_json_on_surface(state: &PegboardState, board: &str, surface: Surface) -> Value {
    json!({
        "board": board,
        "tools": pegboard::board_entries_on_surface_in(state, board, None, surface)
            .into_iter()
            .map(|(_, tool)| tool.to_json_object("name"))
            .collect::<Vec<_>>(),
    })
}
