//! REST route assembly.

use axum::{
    Router,
    routing::{get, post},
};

use super::{
    HttpState, board_show, board_tools_call, boards_list, clients_heartbeat, clients_list,
    credentials_list, ext_boards, logs_list, readiness, stream, tag_show, tags_list, toolkit_show,
    toolkit_tool_show, toolkits_list, tools_call, tools_list, trigger_call, triggers_list,
};

pub(super) fn rest_routes() -> Router<HttpState> {
    Router::new()
        .route("/v1/toolkits", get(toolkits_list))
        .route("/v1/toolkits/{toolkit}", get(toolkit_show))
        .route("/v1/toolkits/{toolkit}/{tool}", get(toolkit_tool_show))
        .route("/v1/tags", get(tags_list))
        .route("/v1/tags/{tag}", get(tag_show))
        .route("/v1/boards", get(boards_list))
        .route("/v1/boards/{board}", get(board_show))
        .route("/v1/boards/{board}/tools/{id}", post(board_tools_call))
        .route("/v1/ext/boards", get(ext_boards::ext_boards_list))
        .route("/v1/ext/boards/{board}", get(ext_boards::ext_board_show))
        .route(
            "/v1/ext/boards/{board}/tools/{id}",
            post(ext_boards::ext_board_tools_call),
        )
        .route(
            "/v1/boards/{board}/tools/{id}/stream",
            post(stream::board_tools_call_stream),
        )
        .route("/v1/credentials", get(credentials_list))
        .route("/v1/logs", get(logs_list))
        .route("/v1/tools", get(tools_list))
        .route("/v1/tools/{id}/readiness", get(readiness::tool_readiness))
        .route("/v1/tools/{id}", post(tools_call))
        .route("/v1/tools/{id}/stream", post(stream::tools_call_stream))
        .route("/v1/triggers", get(triggers_list))
        .route("/v1/trigger/{id}", post(trigger_call))
        .route("/v1/clients", get(clients_list))
        .route("/v1/clients/heartbeat", post(clients_heartbeat))
        .route(
            "/v1/openapi.json",
            get(crate::surfaces::http::openapi::openapi_spec),
        )
}
