use serde_json::{Value, json};

pub(crate) fn error_response(description: &str) -> Value {
    json!({
        "description": description,
        "content": {
            "application/json": {
                "schema": {
                    "type": "object",
                    "required": ["ok", "error"],
                    "properties": {
                        "ok": { "type": "boolean", "enum": [false] },
                        "error": {
                            "type": "object",
                            "required": ["code", "message"],
                            "properties": {
                                "code": { "type": "string" },
                                "message": { "type": "string" },
                                "details": {}
                            }
                        }
                    }
                }
            }
        }
    })
}

pub(super) fn tool_call_response_schema() -> Value {
    json!({
        "type": "object",
        "required": ["ok", "primary_output_id", "outputs"],
        "properties": {
            "ok": { "type": "boolean", "enum": [true] },
            "primary_output_id": {
                "oneOf": [
                    { "type": "string" },
                    { "type": "null" }
                ]
            },
            "outputs": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "label", "kind", "value"],
                    "properties": {
                        "id": { "type": "string" },
                        "label": {
                            "oneOf": [
                                { "type": "string" },
                                { "type": "null" }
                            ]
                        },
                        "kind": { "type": "string" },
                        "value": {}
                    }
                }
            }
        }
    })
}

/// One line of a streaming call's `application/x-ndjson` body.
///
/// Three shapes discriminated by `event`: incremental `chunk` lines, the
/// `dropped` marker that accounts for output a slow consumer cost
/// itself, and the single terminal `result` line. Modeled as `oneOf` so
/// a generated client narrows on `event` rather than guessing from which
/// fields happen to be present.
pub(super) fn ndjson_event_schema() -> Value {
    json!({
        "oneOf": [
            {
                "title": "chunk",
                "type": "object",
                "required": ["event", "stream", "seq", "data"],
                "properties": {
                    "event": { "type": "string", "enum": ["chunk"] },
                    "stream": { "type": "string", "enum": ["stdout", "stderr"] },
                    "seq": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Contiguous from 0 across both streams and every \
                                        step of one call."
                    },
                    "data": {
                        "type": "string",
                        "description": "Verbatim bytes the tool wrote, lossily decoded as UTF-8."
                    }
                }
            },
            {
                "title": "dropped",
                "type": "object",
                "required": ["event", "bytes"],
                "properties": {
                    "event": { "type": "string", "enum": ["dropped"] },
                    "bytes": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Bytes of tool output the host discarded because \
                                        this client was not reading fast enough. The host \
                                        holds a bounded amount of unread output per call; \
                                        beyond that, chunks are dropped and coalesced into \
                                        one such marker. `seq` numbers stay contiguous, so a \
                                        gap in the output is only visible here."
                    }
                }
            },
            {
                "title": "result",
                "type": "object",
                "required": ["event", "result"],
                "properties": {
                    "event": { "type": "string", "enum": ["result"] },
                    "result": {
                        "description": "The canonical ToolResult envelope — success or failure."
                    }
                }
            }
        ]
    })
}

/// The body of a `text/event-stream` response, as far as OpenAPI can
/// say it.
///
/// A `string`, not a `oneOf` of frames: OpenAPI 3.0 has no vocabulary
/// for "a sequence of SSE events", so pretending to schema the frames
/// would describe a body no client ever receives whole. The description
/// carries what a generated client actually needs — the framing, and the
/// rule for when to stop reading.
pub(super) fn sse_stream_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "format": "text/event-stream",
        "description": description,
    })
}
