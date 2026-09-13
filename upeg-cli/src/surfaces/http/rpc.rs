//! The `/mcp` JSON-RPC lane: request routing, and the choice of which
//! body shape answers it.
//!
//! `POST /mcp` has always been "one request in, one JSON response out".
//! It still is — unless the client asks for `text/event-stream`, in which
//! case the same dispatch answers on an SSE body that can also carry the
//! frames a running tool produces. `GET /mcp` is the other half of that
//! transport: a stream for frames belonging to no request. Both live in
//! [`mcp_sse`]; this module decides which one a request gets and hands
//! the dispatch the caller identity, board scope and session it belongs
//! to.
//!
//! **The surface is not negotiable here.** Every request on this lane is
//! dispatched as [`Surface::Mcp`] — an MCP client is a program, and no
//! header moves it (`docs/architecture/http-api.md`). What the request
//! *can* change is the role: an operator token and an agent token both
//! pass the bearer gate, and only the first may lift a Chain's approval
//! barrier. That is [`principal_on_surface`]'s answer, stamped into the
//! call envelope through `ExecutionContext::with_principal` so the
//! Chain dispatcher decides against a proved identity rather than the
//! `mcp` surface's in-process default.

use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::{Value, json};
use upeg_core::{BoardKey, Principal, Surface};

use crate::surfaces::mcp::{self, McpLogWriter};

use super::{BOARD_SCOPE_HEADER, HttpState, principal_on_surface};

// Declared here rather than in the surface's module list because it is
// this lane's own transport, reachable through no other route. The
// `#[path]` keeps the file a sibling of this one.
#[path = "mcp_sse.rs"]
pub(super) mod mcp_sse;

use mcp_sse::ResponseShape;

/// The surface every `/mcp` request is dispatched as.
///
/// Fixed, not derived: this lane speaks the MCP protocol, so the caller
/// is an MCP client no matter which process relays it. The attach header
/// that narrows `/v1/…` calls to a person's own terminal deliberately
/// never reaches here (`surfaces/http/origin_surface.rs`).
const MCP_LANE_SURFACE: Surface = Surface::Mcp;

pub(super) fn routes() -> Router<HttpState> {
    // `/mcp` is the canonical JSON-RPC-over-HTTP path (PRD §5.6, §5.7).
    // One path, two directions: `POST` carries a client's requests,
    // `GET` opens the server's own stream (MCP Streamable HTTP).
    Router::new().route("/mcp", post(mcp_rpc).get(mcp_sse::mcp_events))
}

async fn mcp_rpc(State(state): State<HttpState>, headers: HeaderMap, body: Bytes) -> Response {
    let board = match board_from_headers(&headers) {
        Ok(board) => board,
        Err(msg) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))).into_response();
        }
    };
    let request: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(mcp::parse_error_response(e))).into_response();
        }
    };
    let principal = principal_on_surface(&state, &headers, MCP_LANE_SURFACE);

    // A handshake that named no session gets one. Nothing else mints an
    // id, and nothing anywhere refuses a request for lacking one — see
    // `mcp_sse`'s module doc.
    let existing = mcp_sse::session_from_headers(&headers);
    let issued = (existing.is_none() && mcp::is_initialize_request(&request))
        .then(mcp_sse::open_session)
        .flatten();
    let session = issued.clone().or(existing);

    // A notification has no response to carry, so there is nothing to
    // open a stream for — it answers `204` whatever the client accepts.
    let response = match (
        mcp_sse::response_shape(&headers),
        mcp::expects_response(&request),
    ) {
        (ResponseShape::EventStream, true) => {
            mcp_sse::streamed_response(request, principal, board, session)
        }
        _ => buffered_response(request, principal, board.as_ref(), session.as_ref()),
    };
    mcp_sse::with_session_header(response, issued.as_ref())
}

/// The lane as it has always been: dispatch, then one JSON response (or
/// `204` for a notification).
///
/// The log writer discards: this request opened no stream, so the frames
/// a running tool produces have nowhere to go. It is still a writer, not
/// `None`, because `None` on this lane would mean "the `/mcp` transport
/// cannot push at all" — and it can. That is what lets `initialize`
/// advertise `capabilities.logging` and `logging/setLevel` move this
/// session's floor from a plain JSON request, for the SSE requests that
/// follow it.
fn buffered_response(
    request: Value,
    principal: Principal,
    board: Option<&BoardKey>,
    session: Option<&mcp_sse::SessionId>,
) -> Response {
    let log = McpLogWriter::discarding(mcp_sse::session_gate(session));
    match mcp::handle_with_log(request, principal, board, Some(&log)) {
        Some(response) => (StatusCode::OK, Json(response)).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

/// Optional board scope on JSON-RPC requests (`x-upeg-board`), set by
/// the `upeg mcp --board <b>` stdio proxy. Absent → unscoped.
fn board_from_headers(headers: &HeaderMap) -> Result<Option<BoardKey>, &'static str> {
    let Some(raw) = headers.get(BOARD_SCOPE_HEADER) else {
        return Ok(None);
    };
    let Ok(value) = raw.to_str() else {
        return Err("invalid x-upeg-board header");
    };
    BoardKey::parse(value)
        .map(Some)
        .map_err(|_| "invalid x-upeg-board header")
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
