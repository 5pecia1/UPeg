//! Live `OpenAPI` 3.0 spec for the HTTP surface (PRD §6.8).
//!
//! Extracted from `surfaces/http.rs` to keep that file under the
//! workspace 1000-line file-size budget. The spec is built from
//! `toolbox_tools()` filtered to `Surface::Http`; the route is
//! registered in `surfaces/http.rs`'s router assembly.

use axum::Json;
use serde_json::{Value, json};
use upeg_core::Surface;
use upeg_runtime::{ToolMetaRuntimeExt, toolbox_tools};

mod schemas;

pub(crate) use schemas::error_response;
use schemas::{ndjson_event_schema, sse_stream_schema, tool_call_response_schema};

const UPEG_OUTPUT_FIELDS_EXTENSION_KEY: &str = "x-upeg-output-fields";
/// Media type of a streaming call's response body.
const NDJSON_MEDIA_TYPE: &str = "application/x-ndjson";
/// Media type of the two `/mcp` server→client streams.
const SSE_MEDIA_TYPE: &str = "text/event-stream";
/// JSON-RPC path and the header that names a session on it.
const MCP_PATH: &str = "/mcp";
const MCP_SESSION_HEADER: &str = "Mcp-Session-Id";

pub(crate) async fn openapi_spec() -> Json<Value> {
    let mut tools: Vec<_> = toolbox_tools()
        .filter(|t| t.is_on_surface(Surface::Http))
        .collect();
    tools.sort_by_key(|t| t.id);

    let mut paths = serde_json::Map::new();
    for t in &tools {
        let request_schema = t.input_schema_value();
        let response_schema = tool_call_response_schema();
        let summary = if t.description.is_empty() {
            t.id.to_string()
        } else {
            t.description.to_string()
        };
        let embed_url = upeg_runtime::embed_url_for(t.id);
        let bindings = upeg_runtime::selector_bindings_json_for(t.id);
        let triggers = upeg_runtime::trigger_bindings_json_for(t.id);

        let path_item = json!({
            "post": {
                "operationId": t.id,
                "tags": t.tag_labels(),
                "summary": summary,
                "x-pin":             t.pin.label(),
                "x-pegboard-units":     t.pegboard_units.label(),
                "x-invoker":            t.invoker.label(),
                "x-surfaces":           t.surface_labels(),
                "x-boards":             t.boards,
                "x-embed-url":          embed_url,
                "x-selector-bindings":  bindings,
                "x-triggers":           triggers,
                (UPEG_OUTPUT_FIELDS_EXTENSION_KEY): t.output_spec.to_json_schema_value(),
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": { "schema": request_schema }
                    }
                },
                "responses": {
                    "200": {
                        "description": "Tool ran and returned the canonical result envelope.",
                        "content": {
                            "application/json": {
                                "schema": response_schema
                            }
                        }
                    },
                    "400": error_response("Request body is not valid JSON."),
                    "422": error_response("Tool returned an error."),
                    "404": error_response(
                        "Tool id is unknown (or not exposed on the http surface)."
                    ),
                }
            }
        });
        paths.insert(format!("/v1/tools/{}", t.id), path_item);
    }
    insert_v21_resource_paths(&mut paths);
    insert_mcp_paths(&mut paths);

    Json(json!({
        "openapi": "3.0.3",
        "info": {
            "title": "upeg HTTP",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Universal Pegboard — tool dispatch over HTTP. \
                            Live spec built from the running registry; \
                            tools surface here only when their `surfaces` \
                            field includes `http`.",
        },
        "paths": Value::Object(paths),
    }))
}

fn insert_v21_resource_paths(paths: &mut serde_json::Map<String, Value>) {
    let json_response = json!({
        "description": "JSON response.",
        "content": { "application/json": { "schema": { "type": "object" } } }
    });
    let call_ok = json!({
        "description": "Tool ran and returned the canonical result envelope.",
        "content": {
            "application/json": {
                "schema": tool_call_response_schema()
            }
        }
    });
    let call_responses = json!({
        "200": call_ok,
        "400": error_response("Request body is not valid JSON."),
        "404": error_response("Resource, board, or tool id is unknown."),
        "422": error_response("Tool returned an error."),
    });
    for (path, summary, params, has_storage_error) in [
        ("/v1/toolkits", "List Toolkits", &[][..], false),
        (
            "/v1/toolkits/{toolkit}",
            "Show one Toolkit",
            &["toolkit"][..],
            false,
        ),
        (
            "/v1/toolkits/{toolkit}/{tool}",
            "Show one Tool inside a Toolkit",
            &["toolkit", "tool"][..],
            false,
        ),
        ("/v1/tags", "List Tags", &[][..], false),
        (
            "/v1/tags/{tag}",
            "Show Tools with a Tag",
            &["tag"][..],
            false,
        ),
        ("/v1/boards", "List Boards", &[][..], false),
        (
            "/v1/boards/{board}",
            "Show one Board",
            &["board"][..],
            false,
        ),
        (
            "/v1/credentials",
            "List Credential references",
            &[][..],
            true,
        ),
        (
            "/v1/logs",
            "List metadata-only Execution Log events",
            &[][..],
            true,
        ),
        ("/v1/triggers", "List Trigger bindings", &[][..], false),
    ] {
        let mut responses = serde_json::Map::new();
        responses.insert("200".into(), json_response.clone());
        responses.insert("404".into(), error_response("Resource id is unknown."));
        if has_storage_error {
            responses.insert(
                "500".into(),
                error_response("Backing store could not be read."),
            );
        }
        paths.insert(
            path.to_string(),
            json!({
                "get": with_path_parameters(json!({
                    "summary": summary,
                    "responses": Value::Object(responses)
                }), params)
            }),
        );
    }

    paths.insert(
        "/v1/tools/{id}/readiness".to_string(),
        json!({
            "get": {
                "operationId": "toolReadiness",
                "summary": "Inspect External tool prerequisites without executing it.",
                "parameters": [
                    { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
                    { "name": "board", "in": "query", "required": false, "schema": { "type": "string" } }
                ],
                "responses": {
                    "200": {
                        "description": "External readiness, or null when this Tool has no host-process prerequisites.",
                        "content": {
                            "application/json": {
                                "schema": { "type": "object", "nullable": true }
                            }
                        }
                    },
                    "401": error_response("Bearer authentication is required."),
                    "404": error_response("Tool is unknown, unavailable on this surface, or not pinned on the Board.")
                }
            }
        }),
    );

    insert_streaming_call_paths(paths);

    for (path, summary, params) in [
        (
            "/v1/boards/{board}/tools/{id}",
            "Run a Tool with Board execution context",
            &["board", "id"][..],
        ),
        (
            "/v1/trigger/{id}",
            "Fire a webhook trigger Tool",
            &["id"][..],
        ),
    ] {
        paths.insert(
            path.to_string(),
            json!({
                "post": with_path_parameters(json!({
                    "summary": summary,
                    "requestBody": {
                        "required": false,
                        "content": {
                            "application/json": {
                                "schema": { "type": "object" }
                            }
                        }
                    },
                    "responses": call_responses.clone(),
                }), params)
            }),
        );
    }
}

/// The NDJSON streaming siblings of the two call routes.
///
/// Templated (`{id}`) rather than expanded per tool: unlike
/// `/v1/tools/{tool-id}`, the streaming body's shape does not vary with
/// the tool — every stream is the same two line kinds, and the tool's
/// own input schema is already documented on its buffered path.
///
/// `200` is the only success code by construction: response headers are
/// written before the tool has run, so a tool failure arrives as the
/// final `result` line rather than a `422`.
fn insert_streaming_call_paths(paths: &mut serde_json::Map<String, Value>) {
    let streamed_ok = json!({
        "description": "Stream opened. One JSON object per line: zero or more \
                        `chunk` events, optional `dropped` markers when the \
                        client falls behind, then exactly one terminal \
                        `result` event carrying the canonical envelope. \
                        Disconnecting stops the stream but does not cancel the \
                        running tool — declare `timeout_ms` on anything \
                        reachable here.",
        "content": {
            NDJSON_MEDIA_TYPE: { "schema": ndjson_event_schema() }
        }
    });
    let responses = json!({
        "200": streamed_ok,
        "400": error_response("Request body is not valid JSON."),
        "404": error_response("Resource, board, or tool id is unknown."),
    });

    for (path, summary, params) in [
        (
            "/v1/tools/{id}/stream",
            "Run a Tool, streaming its output as NDJSON",
            &["id"][..],
        ),
        (
            "/v1/boards/{board}/tools/{id}/stream",
            "Run a Tool with Board execution context, streaming its output as NDJSON",
            &["board", "id"][..],
        ),
    ] {
        paths.insert(
            path.to_string(),
            json!({
                "post": with_path_parameters(json!({
                    "summary": summary,
                    "requestBody": {
                        "required": false,
                        "content": {
                            "application/json": { "schema": { "type": "object" } }
                        }
                    },
                    "responses": responses.clone(),
                }), params)
            }),
        );
    }
}

/// The JSON-RPC lane, both directions.
///
/// Not templated per method the way tool calls are templated per tool:
/// `/mcp` is one path whose body names the method, so what OpenAPI can
/// usefully say is which *representations* it answers with — and that is
/// exactly where the SSE opt-in lives.
fn insert_mcp_paths(paths: &mut serde_json::Map<String, Value>) {
    let call_stream = sse_stream_schema(
        "One `event: message` per JSON-RPC frame, `data:` holding the compact          JSON. Progress `notifications/message` frames arrive while the tool          runs; the frame carrying this request's `id` is the last event, and          the stream closes after it. A stream that ends without it means the          connection dropped.",
    );
    let server_stream = sse_stream_schema(
        "Server-initiated frames belonging to no request — today          `notifications/tools/list_changed`, emitted when the MCP-import load          finishes — interleaved with `:` keep-alive comments. The stream stays          open until the client closes it.",
    );

    paths.insert(
        MCP_PATH.to_string(),
        json!({
            "post": {
                "summary": "JSON-RPC 2.0 request on the MCP surface",
                "parameters": [mcp_session_parameter()],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": { "schema": { "type": "object" } }
                    }
                },
                "responses": {
                    "200": {
                        "description": "The JSON-RPC response. `Accept: text/event-stream`                                         (exactly — `*/*` does not count) asks for it on an                                         SSE body that also carries the running tool's output.",
                        "content": {
                            "application/json": { "schema": { "type": "object" } },
                            SSE_MEDIA_TYPE: { "schema": call_stream }
                        }
                    },
                    "204": {
                        "description": "The request was a JSON-RPC notification (no `id`),                                         which owes no response — so no stream is opened for                                         it either."
                    },
                    "400": error_response("Body is not valid JSON, or a scope header is malformed."),
                }
            },
            "get": {
                "summary": "Server-initiated MCP notifications",
                "parameters": [mcp_session_parameter()],
                "responses": {
                    "200": {
                        "description": "Long-lived notification stream.",
                        "content": { SSE_MEDIA_TYPE: { "schema": server_stream } }
                    },
                    "406": error_response(
                        "`Accept` did not name `text/event-stream`; this resource has no \
                         other representation."
                    ),
                }
            }
        }),
    );
}

/// The optional session header, declared on both directions of `/mcp`.
fn mcp_session_parameter() -> Value {
    json!({
        "name": MCP_SESSION_HEADER,
        "in": "header",
        "required": false,
        "schema": { "type": "string" },
        "description": "Session id issued on the `initialize` response. Carrying it \
                        back keeps this session's `logging/setLevel` severity floor. \
                        Never required, and never authorization — the bearer token \
                        is.",
    })
}

fn with_path_parameters(mut operation: Value, names: &[&str]) -> Value {
    if !names.is_empty() {
        operation["parameters"] =
            Value::Array(names.iter().map(|name| path_parameter(name)).collect());
    }
    operation
}

fn path_parameter(name: &str) -> Value {
    json!({
        "name": name,
        "in": "path",
        "required": true,
        "schema": { "type": "string" },
    })
}
