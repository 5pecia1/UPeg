//! Authenticated, non-executing External readiness route.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use upeg_core::Surface;

use super::{HttpState, READINESS_CWD_HEADER, not_found_response, origin_surface_from_headers};

#[derive(serde::Deserialize)]
pub(super) struct ReadinessQuery {
    board: Option<String>,
}

pub(super) async fn tool_readiness(
    State(state): State<HttpState>,
    Path(id): Path<String>,
    Query(query): Query<ReadinessQuery>,
    headers: HeaderMap,
) -> Response {
    let surface = origin_surface_from_headers(&state, &headers);
    let board = match query.board {
        Some(board) => match upeg_core::BoardKey::parse(&board) {
            Ok(board) => Some(board),
            Err(_) => {
                return not_found_response("board tool", &format!("{board}/{id}")).into_response();
            }
        },
        None => None,
    };
    if let Some(board) = board.as_ref() {
        let state = upeg_sources::pegboard::load_state();
        if upeg_sources::pegboard::board_placement_on_surface_in(
            &state,
            board.as_str(),
            &id,
            surface,
        )
        .is_none()
        {
            return not_found_response("board tool", &format!("{board}/{id}")).into_response();
        }
    } else if upeg_runtime::toolbox_tool(&id).is_none_or(|meta| !meta.is_on_surface(surface)) {
        return not_found_response("tool", &id).into_response();
    }
    let working_directory = match working_directory(&headers, surface) {
        Ok(directory) => directory,
        Err(message) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response();
        }
    };
    match crate::app::tool_readiness::inspect_tool_readiness_at(
        &id,
        board.as_ref(),
        working_directory,
    ) {
        Ok(Some(readiness)) => (StatusCode::OK, Json(readiness)).into_response(),
        Ok(None) => (StatusCode::OK, Json(Value::Null)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.message() })),
        )
            .into_response(),
    }
}

pub(super) fn working_directory(
    headers: &HeaderMap,
    surface: Surface,
) -> Result<std::path::PathBuf, &'static str> {
    let Some(value) = headers.get(READINESS_CWD_HEADER) else {
        return std::env::current_dir().map_err(|_| "host working directory is unavailable");
    };
    if surface != Surface::Cli {
        return Err("readiness working directory override is only accepted from cli");
    }
    let value = value
        .to_str()
        .map_err(|_| "readiness working directory must be valid UTF-8")?;
    if value.is_empty() {
        return Err("readiness working directory must not be empty");
    }
    let path = std::path::PathBuf::from(value);
    if !path.is_absolute() {
        return Err("readiness working directory must be absolute");
    }
    Ok(path)
}
