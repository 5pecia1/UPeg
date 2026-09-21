//! Source-located tests for the MCP surface. Extracted into this
//! sibling file so `mcp.rs` itself stays focused on the JSON-RPC
//! dispatch + `serve_loop`.
//!
//! Tests cover: `read_line_capped` helper, `handle/handle_for_surface`
//! shape, surface gating, method-not-found hints, tools/list+call
//! shapes, invalid-params/request frames, and `serve_loop` EOF/cap
//! behaviour.

use crate::surfaces::mcp::*;
use serde_json::{Value, json};

// helper was only exercised through serve_loop / daemon
// integration tests — a bug in any individual outcome arm
// could be masked by the surrounding handler logic. These
// pin each `LineReadOutcome` variant in isolation.

#[test]
fn capped_line_read_returns_eof_on_empty_input() {
    use std::io::Cursor;
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut line = String::new();
    let outcome = read_line_capped(&mut reader, &mut line).expect("ok");
    assert_eq!(
        outcome,
        LineReadOutcome::Eof,
        "empty stream must return Eof, not Line {{ bytes: 0 }}"
    );
    assert!(line.is_empty(), "buffer must be untouched on Eof");
}

#[test]
fn capped_line_read_returns_terminated_line() {
    use std::io::Cursor;
    let mut reader = Cursor::new(b"hello\n".to_vec());
    let mut line = String::new();
    let outcome = read_line_capped(&mut reader, &mut line).expect("ok");
    assert_eq!(
        outcome,
        LineReadOutcome::Line { bytes: 6 },
        "well-formed line must return Line with full byte count"
    );
    assert_eq!(line, "hello\n", "buffer keeps the trailing newline");
}

#[test]
fn capped_line_read_surfaces_unterminated_tail_then_eof() {
    // Partial line at EOF is treated as a (malformed) Line so the
    // caller can attempt to parse and reply with a graceful error.
    // A subsequent call returns Eof. Distinct from CapHit which
    // signals "stream is poisoned, close the connection".
    use std::io::Cursor;
    let mut reader = Cursor::new(b"hi".to_vec());
    let mut line = String::new();
    let outcome = read_line_capped(&mut reader, &mut line).expect("ok");
    assert_eq!(
        outcome,
        LineReadOutcome::Line { bytes: 2 },
        "EOF mid-line must surface as Line, not CapHit (caller can still try-parse)"
    );
    assert_eq!(line, "hi");

    // Second call: stream is now empty → Eof.
    let mut line2 = String::new();
    let next = read_line_capped(&mut reader, &mut line2).expect("ok");
    assert_eq!(
        next,
        LineReadOutcome::Eof,
        "second call after exhausting the stream must return Eof"
    );
}

#[test]
fn capped_line_read_returns_cap_hit_when_limit_exceeded() {
    // The cornerstone DoS-resistance contract: payload > MAX_LINE_BYTES
    // without a `\n` must surface as CapHit. Without the cap this
    // would be unbounded buffer growth → OOM.
    use std::io::Cursor;
    let payload = vec![b'x'; MAX_LINE_BYTES + 1];
    let mut reader = Cursor::new(payload);
    let mut line = String::new();
    let outcome = read_line_capped(&mut reader, &mut line).expect("ok");
    assert_eq!(
        outcome,
        LineReadOutcome::CapHit,
        "oversized newline-less input must return CapHit"
    );
    // Line buffer holds the partial bytes (caller usually discards).
    assert_eq!(line.len(), MAX_LINE_BYTES);
}

#[test]
fn consecutive_capped_line_reads_have_independent_budgets() {
    // Each call recreates the Take wrapper, so the budget resets.
    // A cap miss on call 1 must not poison call 2.
    use std::io::Cursor;
    let payload = "first\nsecond\n".to_string();
    let mut reader = Cursor::new(payload.into_bytes());
    let mut line1 = String::new();
    let r1 = read_line_capped(&mut reader, &mut line1).expect("ok");
    assert_eq!(r1, LineReadOutcome::Line { bytes: 6 });
    assert_eq!(line1, "first\n");
    let mut line2 = String::new();
    let r2 = read_line_capped(&mut reader, &mut line2).expect("ok");
    assert_eq!(r2, LineReadOutcome::Line { bytes: 7 });
    assert_eq!(line2, "second\n");
}

// ─── serve_loop request-line cap  ─────────────────

#[test]
fn serve_loop_caps_oversized_request_lines() {
    // Parallel to the daemon's request-line cap. Reads from a
    // generic BufRead so we can inject a payload exceeding the
    // 1 MB cap without going through real stdin.
    use std::io::Cursor;
    // 1 MB + 1 byte without newline.
    let payload = vec![b'x'; MAX_LINE_BYTES + 1];
    let mut reader = Cursor::new(payload);
    let mut writer: Vec<u8> = Vec::new();
    serve_loop_with_board(&mut reader, &mut writer, None);
    let response = String::from_utf8(writer).expect("utf-8 response");
    let resp_json: Value = serde_json::from_str(response.trim()).expect("response is valid JSON");
    assert_eq!(
        resp_json["error"]["code"], -32700,
        "oversized request must surface as -32700 Parse error"
    );
    let msg = resp_json["error"]["message"].as_str().expect("message");
    assert!(
        msg.contains("exceeds"),
        "message must mention the cap was hit; got `{msg}`"
    );
}

#[test]
fn serve_loop_handles_normal_request() {
    // Sanity: a normal request (well under the cap) still flows
    // through. Sends a single tools/list request and reads the
    // response.
    use std::io::Cursor;
    let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_string() + "\n";
    let mut reader = Cursor::new(request.into_bytes());
    let mut writer: Vec<u8> = Vec::new();
    serve_loop_with_board(&mut reader, &mut writer, None);
    let response = String::from_utf8(writer).expect("utf-8 response");
    let resp_json: Value = serde_json::from_str(response.trim()).expect("response is valid JSON");
    assert_eq!(resp_json["id"], 1);
    assert!(
        resp_json["result"]["tools"].is_array(),
        "tools/list must return an array; got {resp_json}"
    );
}

// ─── parse_error_response helper  ─────────────────

#[test]
fn parse_error_response_has_protocol_required_shape() {
    // Pin the JSON-RPC 2.0 -32700 Parse error shape so the two
    // transports (mcp::serve stdio, daemon::handle_connection
    // socket) cannot drift on the response template; the shape is
    // centralised so future tweaks ripple to both.
    let v = parse_error_response("expected `,` at line 3");
    assert_eq!(
        v["jsonrpc"], "2.0",
        "Parse errors are JSON-RPC 2.0 responses"
    );
    assert_eq!(
        v["id"],
        serde_json::Value::Null,
        "Parse errors use null id (caller's id is unparseable by definition)"
    );
    assert_eq!(
        v["error"]["code"], -32700,
        "JSON-RPC 2.0 reserves -32700 for Parse error"
    );
    let msg = v["error"]["message"].as_str().expect("string message");
    assert!(
        msg.starts_with("Parse error:"),
        "message must lead with `Parse error:` per spec; got `{msg}`"
    );
    assert!(
        msg.contains("expected `,`"),
        "message must include the underlying detail; got `{msg}`"
    );
}

// ─── extract_text_content helper  ─────────────────

#[test]
fn extract_text_content_joins_multiple_text_parts() {
    // Single helper, single contract: text parts joined with `\n`,
    // non-text parts skipped. Pin so a future edit (e.g., switching
    // to space separation, dropping parts past N, etc.) is a loud
    // diff.
    let result = json!({
        "content": [
            { "type": "text", "text": "alpha" },
            { "type": "text", "text": "beta" },
            { "type": "text", "text": "gamma" },
        ]
    });
    assert_eq!(extract_text_content(&result), "alpha\nbeta\ngamma");
}

#[test]
fn extract_text_content_skips_non_text_parts() {
    let result = json!({
        "content": [
            { "type": "image", "data": "<base64>" },
            { "type": "text",  "text": "hello" },
            { "type": "resource", "uri": "/foo" },
            { "type": "text",  "text": "world" },
        ]
    });
    assert_eq!(extract_text_content(&result), "hello\nworld");
}

#[test]
fn extract_text_content_returns_empty_without_content() {
    // `result.content` missing entirely → empty.
    assert_eq!(extract_text_content(&json!({})), "");
    // Empty array → empty.
    assert_eq!(extract_text_content(&json!({"content": []})), "");
    // Array of all non-text parts → empty.
    assert_eq!(
        extract_text_content(&json!({
            "content": [{ "type": "image", "data": "x" }]
        })),
        "",
    );
}

#[test]
fn extract_text_content_skips_empty_text_parts() {
    // Empty `text: ""` parts must not produce a dangling trailing
    // newline. If the join branch fires for empty parts,
    // `[hello, ""]` becomes `"hello\n"`. Pin both an interior-empty
    // case (between two non-empty parts) and a trailing-empty case.
    let result_trailing = json!({
        "content": [
            { "type": "text", "text": "hello" },
            { "type": "text", "text": "" },
        ]
    });
    assert_eq!(
        extract_text_content(&result_trailing),
        "hello",
        "trailing empty part must not add a dangling newline"
    );

    let result_interior = json!({
        "content": [
            { "type": "text", "text": "alpha" },
            { "type": "text", "text": "" },
            { "type": "text", "text": "beta" },
        ]
    });
    assert_eq!(
        extract_text_content(&result_interior),
        "alpha\nbeta",
        "interior empty part must collapse to single newline between siblings"
    );

    let result_all_empty = json!({
        "content": [
            { "type": "text", "text": "" },
            { "type": "text", "text": "" },
        ]
    });
    assert_eq!(
        extract_text_content(&result_all_empty),
        "",
        "all-empty parts must produce empty string, not a string of newlines"
    );
}

#[test]
fn extract_text_content_handles_single_part() {
    // Single-part happy path: pin it to ensure the loop continues
    // to handle the original single-`content[0].text` case.
    let result = json!({
        "content": [{ "type": "text", "text": "just one" }]
    });
    assert_eq!(extract_text_content(&result), "just one");
}

#[test]
fn initialize_returns_protocol_handshake() {
    let resp = handle(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
    }))
    .expect("initialize must respond");
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    let result = &resp["result"];
    assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(result["serverInfo"]["name"], "upeg");
    assert!(result["capabilities"]["tools"].is_object());
}

#[test]
fn tools_list_returns_registered_tools_sorted() {
    let resp = handle(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
    }))
    .expect("tools/list");
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    assert!(!tools.is_empty(), "expected at least one tool");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "tools/list must be sorted by id");
    assert!(names.contains(&"num.hex_to_decimal"));
    assert!(names.contains(&"id.uuid_v7"));
}

#[test]
fn tools_list_baseline_includes_builtin_mcp_tool_shape() {
    let resp = handle(
        json!({        "jsonrpc": "2.0", "id": 20260515, "method": "tools/list",
        }),
    )
    .expect("tools/list");
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    let hex = tools
        .iter()
        .find(|tool| tool["name"] == "num.hex_to_decimal")
        .expect("num.hex_to_decimal must be exposed on MCP tools/list");
    assert!(!hex["description"].as_str().unwrap().is_empty());
    assert_eq!(hex["inputSchema"]["type"], "object");
    assert_eq!(hex["outputSchema"]["type"], "object");
    assert!(
        hex["surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "mcp"),
        "MCP entry must advertise the mcp surface: {hex}"
    );
}

#[test]
fn tools_list_entries_have_input_schema() {
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/list",
    }))
    .unwrap();
    for tool in resp["result"]["tools"].as_array().unwrap() {
        assert!(
            tool["inputSchema"]["type"] == "object",
            "entry missing input schema: {tool}"
        );
        assert!(
            tool["outputSchema"]["type"] == "object",
            "entry missing output schema: {tool}"
        );
        assert!(tool["description"].as_str().is_some());
    }
}

#[test]
fn tools_list_entries_carry_full_metadata() {
    // Cross-protocol parity: MCP tools/list must surface the same
    // 10-field shape HTTP /v1/tools does. Without the parity,
    // clients can't see pin kind, surfaces, pinned boards, embed
    // URLs, or selector bindings even though every other JSON
    // surface (CLI `--json`, HTTP) surfaces them. Pin the contract
    // so a future MCP-shape edit that drops a field is a loud diff,
    // mirroring the HTTP test
    // (`/v1/tools` entry-shape test).
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 161, "method": "tools/list",
    }))
    .unwrap();
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty(), "registry must not be empty");
    for entry in tools {
        for field in &[
            "name",
            "toolkit",
            "tool",
            "tags",
            "description",
            "inputSchema",
            "outputSchema",
            "pin",
            "pegboardUnits",
            "pegboardSpan",
            "invoker",
            "surfaces",
            "boards",
            "embedUrl",
            "selectorBindings",
        ] {
            assert!(
                entry.get(*field).is_some(),
                "MCP tools/list entry missing `{field}`; got: {entry}"
            );
        }
    }
}

#[test]
fn tools_call_hex_to_dec_returns_success() {
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": { "name": "num.hex_to_decimal", "arguments": { "input": "0xff" } },
    }))
    .unwrap();
    assert_eq!(resp["result"]["content"][0]["type"], "text");
    assert_eq!(resp["result"]["content"][0]["text"], "255");
    assert!(resp["result"]["isError"].is_null());
}

#[test]
fn tools_call_returns_structured_content_when_output_specified() {
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 265, "method": "tools/call",
        "params": { "name": "num.hex_to_decimal", "arguments": { "input": "0xff" } },
    }))
    .unwrap();

    assert_eq!(resp["result"]["content"][0]["text"], "255");
    assert_eq!(
        resp["result"]["structuredContent"]["primary_output_id"],
        "result"
    );
    assert_eq!(
        resp["result"]["structuredContent"]["outputs"][0]["value"],
        255
    );
}

mod controlled_embed_mcp {
    use super::*;
    use std::sync::Arc;
    use upeg_core::Surface;

    const TOOL_ID: &str = "cemcp.run";
    const EMBED_URL: &str = "https://example.test/tool";
    const INPUT_FIELD: &str = "query";
    const INPUT_VALUE: &str = "upeg";
    const OUTPUT_ID: &str = "summary";
    const OUTPUT_VALUE: &str = "mcp ok";
    const UNAVAILABLE_ERROR_CODE: &str = "controlled_embed_unavailable";
    const WAIT_TIMEOUT_ERROR_CODE: &str =
        upeg_runtime::controlled_embed::CONTROLLED_EMBED_WAIT_TIMEOUT_CODE;
    const WAIT_TIMEOUT_SELECTOR: &str = "#summary";
    const WAIT_TIMEOUT_FOR_SELECTOR: &str = "#ready";
    const WAIT_TIMEOUT_MS: u64 = 125;

    const TOOLKIT_TOML: &str = r##"
id = "cemcp"

[[tools]]
id = "run"
description = "Controlled Embed fixture"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "https://example.test/tool"
surfaces = ["cli", "tui", "http", "mcp"]
primary_output_id = "summary"

[[tools.inputs]]
name = "query"
type = "string"
required = true

[[tools.outputs]]
name = "summary"
type = "string"
label = "Summary"

[[tools.controlled_embed.bindings]]
role = "input"
field = "query"
selector = "#q"

[[tools.controlled_embed.bindings]]
role = "trigger"
field = ""
selector = "button"

[[tools.controlled_embed.bindings]]
role = "output"
field = "summary"
selector = "#summary"
"##;

    struct SuccessControlledEmbedBackend;

    impl upeg_runtime::controlled_embed::ControlledEmbedBackend for SuccessControlledEmbedBackend {
        fn run(
            &self,
            request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
        ) -> Result<
            upeg_runtime::controlled_embed::ControlledEmbedResponse,
            upeg_runtime::controlled_embed::ControlledEmbedError,
        > {
            assert_eq!(request.url, EMBED_URL);
            assert!(
                request
                    .inputs
                    .iter()
                    .any(|(field, value)| *field == INPUT_FIELD && *value == INPUT_VALUE),
                "Controlled Embed backend must receive the JSON-RPC arguments as page inputs"
            );
            Ok(upeg_runtime::controlled_embed::ControlledEmbedResponse {
                outputs: vec![(OUTPUT_ID.into(), OUTPUT_VALUE.into())],
            })
        }
    }

    struct WaitTimeoutControlledEmbedBackend;

    impl upeg_runtime::controlled_embed::ControlledEmbedBackend for WaitTimeoutControlledEmbedBackend {
        fn run(
            &self,
            _request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
        ) -> Result<
            upeg_runtime::controlled_embed::ControlledEmbedResponse,
            upeg_runtime::controlled_embed::ControlledEmbedError,
        > {
            Err(
                upeg_runtime::controlled_embed::ControlledEmbedError::WaitTimeout {
                    role: upeg_core::BindingRole::Output,
                    selector: WAIT_TIMEOUT_SELECTOR.into(),
                    for_selector: WAIT_TIMEOUT_FOR_SELECTOR.into(),
                    condition: upeg_core::BindingWaitCondition::Visible,
                    timeout_ms: WAIT_TIMEOUT_MS,
                },
            )
        }
    }

    fn ensure_fixture_loaded() {
        let dir =
            std::env::temp_dir().join(format!("upeg-controlled-embed-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir is created");
        std::fs::write(dir.join("cemcp.toml"), TOOLKIT_TOML).expect("fixture TOML is written");

        let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
        assert!(
            outcome.failed.is_empty(),
            "Controlled Embed fixture must load without failures: {:?}",
            outcome.failed
        );
        assert!(
            outcome.loaded.contains(&TOOL_ID),
            "Controlled Embed fixture tool must be loaded: {:?}",
            outcome.loaded
        );
    }

    fn request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": TOOL_ID,
                "arguments": { INPUT_FIELD: INPUT_VALUE },
            },
        })
    }

    fn result_for(surface: Surface) -> Value {
        let response = handle_for_surface(request(), surface).expect("JSON-RPC response");
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(
            response["error"].is_null(),
            "tool execution failures must stay inside result, not JSON-RPC error: {response}"
        );
        response["result"].clone()
    }

    fn assert_successful_structured_content(result: &Value) {
        assert_eq!(result["content"][0]["text"], OUTPUT_VALUE);
        assert!(result["isError"].is_null());

        let structured = &result["structuredContent"];
        assert_eq!(structured["ok"], true);
        assert_eq!(structured["primary_output_id"], OUTPUT_ID);
        assert_eq!(structured["outputs"][0]["id"], OUTPUT_ID);
        assert_eq!(structured["outputs"][0]["label"], "Summary");
        assert_eq!(structured["outputs"][0]["kind"], "string");
        assert_eq!(structured["outputs"][0]["value"], OUTPUT_VALUE);
    }

    fn assert_failure_structured_content(result: &Value) {
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().expect("text content");
        assert!(
            text.contains("Controlled Embed feature is disabled"),
            "failure text must carry the backend error message: {text}"
        );

        let structured = &result["structuredContent"];
        assert_eq!(structured["ok"], false);
        assert_eq!(structured["error"]["code"], UNAVAILABLE_ERROR_CODE);
        assert_eq!(structured["error"]["message"], text);
    }

    fn assert_wait_timeout_structured_content(result: &Value) {
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().expect("text content");
        assert!(
            text.contains(WAIT_TIMEOUT_SELECTOR),
            "failure text must carry the timed-out binding selector: {text}"
        );
        assert!(
            text.contains(WAIT_TIMEOUT_FOR_SELECTOR),
            "failure text must carry the waited selector: {text}"
        );

        let structured = &result["structuredContent"];
        assert_eq!(structured["ok"], false);
        assert_eq!(structured["error"]["code"], WAIT_TIMEOUT_ERROR_CODE);
        assert_eq!(structured["error"]["message"], text);
    }

    #[test]
    fn controlled_embed_mcp_success_puts_runtime_dispatch_output_in_structured_content() {
        let _guard = crate::test_support::controlled_embed_backend_test_lock()
            .lock()
            .unwrap();
        let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
        ensure_fixture_loaded();
        upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
            SuccessControlledEmbedBackend,
        ));

        let result = result_for(Surface::Mcp);

        assert_successful_structured_content(&result);
    }

    #[test]
    fn controlled_embed_mcp_failure_puts_canonical_error_in_structured_content() {
        let _guard = crate::test_support::controlled_embed_backend_test_lock()
            .lock()
            .unwrap();
        let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
        ensure_fixture_loaded();
        upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
            upeg_runtime::controlled_embed::NoopControlledEmbedBackend,
        ));

        let result = result_for(Surface::Mcp);

        assert_failure_structured_content(&result);
    }

    #[test]
    fn controlled_embed_mcp_wait_timeout_preserves_structured_content_error_code() {
        let _guard = crate::test_support::controlled_embed_backend_test_lock()
            .lock()
            .unwrap();
        let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
        ensure_fixture_loaded();
        upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
            WaitTimeoutControlledEmbedBackend,
        ));

        let result = result_for(Surface::Mcp);

        assert_wait_timeout_structured_content(&result);
    }

    #[test]
    fn controlled_embed_http_surface_success_shares_runtime_output() {
        let _guard = crate::test_support::controlled_embed_backend_test_lock()
            .lock()
            .unwrap();
        let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
        ensure_fixture_loaded();
        upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
            SuccessControlledEmbedBackend,
        ));

        let result = result_for(Surface::Http);

        assert_successful_structured_content(&result);
    }

    #[test]
    fn controlled_embed_http_surface_failure_shares_structured_content() {
        let _guard = crate::test_support::controlled_embed_backend_test_lock()
            .lock()
            .unwrap();
        let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
        ensure_fixture_loaded();
        upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
            upeg_runtime::controlled_embed::NoopControlledEmbedBackend,
        ));

        let result = result_for(Surface::Http);

        assert_failure_structured_content(&result);
    }
}

#[test]
fn tools_call_hex_to_dec_error_marks_is_error() {
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 5, "method": "tools/call",
        "params": { "name": "num.hex_to_decimal", "arguments": { "input": "0xZZ" } },
    }))
    .unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert!(
        resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("invalid hex")
    );
}

#[test]
fn tools_call_uuid_v7_returns_canonical_string() {
    let resp = handle(
        json!({        "jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": { "name": "id.uuid_v7", "arguments": {} },
        }),
    )
    .unwrap();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(text.len(), 36);
    assert_eq!(text.chars().nth(14), Some('7'));
}

#[test]
fn unknown_tool_call_is_method_not_found() {
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": { "name": "no.such.tool", "arguments": {} },
    }))
    .unwrap();
    assert_eq!(resp["error"]["code"], -32601);
}

#[test]
fn unknown_method_is_method_not_found() {
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 8, "method": "definitely/not/a/method",
    }))
    .unwrap();
    assert_eq!(resp["error"]["code"], -32601);
}

#[test]
fn notifications_get_no_response() {
    // Per JSON-RPC 2.0: no `id` → no response.
    let resp = handle(json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
    }));
    assert!(resp.is_none());
}

#[test]
fn initialized_notification_is_silently_accepted() {
    let resp = handle(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
    }));
    assert!(resp.is_none());
}

#[test]
fn request_with_id_but_no_method_is_invalid_request() {
    // A request with `id` but missing `method` must not return
    // None silently — the client would hang waiting for a response.
    // JSON-RPC 2.0 §5: a request without method is malformed and
    // should return -32600 Invalid Request.
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 42,
        // No `method` field
    }))
    .expect("request with id must always get a response");
    assert_eq!(
        resp["error"]["code"], -32600,
        "missing `method` must be -32600 Invalid Request; got {resp}"
    );
    assert_eq!(resp["id"], 42, "response must echo the request id");
    // Error message should explain the cause for the client.
    let msg = resp["error"]["message"].as_str().expect("message string");
    assert!(
        msg.contains("method"),
        "error message must mention the missing field; got {msg:?}"
    );
}

#[test]
fn request_with_non_string_method_is_invalid_request() {
    // Corollary: method must be a string. A request with method as
    // a number/bool/etc is also Invalid Request.
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 43, "method": 42,
    }))
    .expect("malformed request must get a response when id is present");
    assert_eq!(resp["error"]["code"], -32600);
}

#[test]
fn methodless_notification_is_still_silently_accepted() {
    // the notification path (no id) MUST stay silent
    // even when method is missing — JSON-RPC 2.0 says no response
    // for any notification, malformed or not. Don't accidentally
    // send error replies for notifications.
    let resp = handle(json!({
        "jsonrpc": "2.0",
        // No id, no method.
    }));
    assert!(
        resp.is_none(),
        "malformed notification (no id) must produce no response; got {resp:?}"
    );
}

#[test]
fn unknown_tool_call_includes_did_you_mean() {
    // MCP tools/call NotFound now appends "did you mean    // ..." hints scoped to Mcp-surface tools, mirroring CLI/HTTP.
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 100, "method": "tools/call",
        "params": { "name": "num.hex_to_decimai", "arguments": {} },
    }))
    .expect("response present (id=100)");
    assert_eq!(resp["error"]["code"], -32601);
    let msg = resp["error"]["message"].as_str().expect("message");
    assert!(
        msg.starts_with("Method not found:"),
        "must keep JSON-RPC convention; got {msg:?}"
    );
    assert!(
        msg.contains("did you mean"),
        "MCP error must include suggestion for typo; got {msg:?}"
    );
    assert!(
        msg.contains("num.hex_to_decimal"),
        "the close match should appear; got {msg:?}"
    );
}

#[test]
fn far_off_unknown_tool_call_has_no_hint() {
    // Sanity: nonsense id stays terse (no noise hints).
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 101, "method": "tools/call",
        "params": { "name": "qwerty.totally_made_up_xx", "arguments": {} },
    }))
    .expect("response");
    let msg = resp["error"]["message"].as_str().expect("message");
    assert!(
        !msg.contains("did you mean"),
        "far-off id must not append a misleading hint; got {msg:?}"
    );
}

#[test]
fn tools_call_with_missing_params_returns_invalid_params() {
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 9, "method": "tools/call",
    }))
    .unwrap();
    assert_eq!(resp["error"]["code"], -32602);
}

#[test]
fn tools_call_rejects_whitespace_padded_name() {
    for padded_name in [" num.hex_to_decimal ", "\tnum.hex_to_decimal\n"] {
        let resp = handle(json!({
            "jsonrpc": "2.0", "id": 225, "method": "tools/call",
            "params": { "name": padded_name, "arguments": { "input": "0xff" } },
        }))
        .unwrap();
        assert_eq!(
            resp["error"]["code"], -32602,
            "padded name `{padded_name:?}` should be invalid params"
        );
        let msg = resp["error"]["message"].as_str().unwrap_or("");
        assert!(
            msg.contains("canonical and unpadded"),
            "error should explain canonical name contract, got {msg:?}"
        );
    }
}

#[test]
fn tools_call_reports_wrong_type_name_as_missing_or_non_string() {
    // Align with the `"missing or non-string"` disambiguation used
    // for the top-level `method` field. A request with `name: 42`
    // must not return "missing `params.name`" — that would mislead
    // the client into looking for the field instead of fixing its
    // type. Pin both failure modes (None + wrong-type) returning
    // the same disambiguating message.
    for params_value in [
        json!({"arguments": {}}),                // missing entirely
        json!({"name": 42, "arguments": {}}),    // wrong type: number
        json!({"name": null, "arguments": {}}),  // wrong type: null
        json!({"name": ["a"], "arguments": {}}), // wrong type: array
    ] {
        let resp = handle(json!({
            "jsonrpc": "2.0", "id": 182, "method": "tools/call",
            "params": params_value,
        }))
        .unwrap();
        assert_eq!(
            resp["error"]["code"], -32602,
            "all params.name failure modes must be -32602 Invalid params"
        );
        let msg = resp["error"]["message"].as_str().expect("message");
        assert!(
            msg.contains("missing or non-string"),
            "the disambiguating message must apply uniformly; got `{msg}`"
        );
        assert!(
            msg.contains("params.name"),
            "error must name the offending field; got `{msg}`"
        );
    }
}

#[test]
fn tools_call_rejects_non_object_arguments() {
    // The MCP spec says `params.arguments` is an object (or absent/
    // null for zero-arg tools). Accepting any JSON value would
    // route `arguments: "hello"` (a string) into the dispatcher
    // where `args.get("input")` returns None, the tool runs with
    // empty args, and the client gets a confusing "empty input"
    // tool error. Surface the protocol violation as -32602 Invalid
    // params with a message naming the offending field.
    for bad_args in [
        json!("string-not-object"),
        json!(42),
        json!(true),
        json!([1, 2, 3]),
    ] {
        let resp = handle(json!({
            "jsonrpc": "2.0", "id": 248, "method": "tools/call",
            "params": { "name": "num.hex_to_decimal", "arguments": bad_args },
        }))
        .unwrap();
        assert_eq!(
            resp["error"]["code"], -32602,
            "non-object arguments must surface as -32602 Invalid params"
        );
        let msg = resp["error"]["message"].as_str().expect("message");
        assert!(
            msg.contains("params.arguments"),
            "error must name the offending field; got `{msg}`"
        );
        assert!(
            msg.contains("object"),
            "error should mention the expected shape; got `{msg}`"
        );
    }
}

#[test]
fn tools_call_accepts_null_or_absent_arguments() {
    // zero-arg tools (uuid_v7, etc.) work whether
    // arguments is null, absent, or an empty object. All three
    // must produce successful results.
    for params_value in [
        json!({"name": "id.uuid_v7", "arguments": null}),
        json!({"name": "id.uuid_v7"}), // absent
        json!({"name": "id.uuid_v7", "arguments": {}}),
    ] {
        let resp = handle(json!({
            "jsonrpc": "2.0", "id": 248, "method": "tools/call",
            "params": params_value,
        }))
        .unwrap();
        assert!(
            resp["error"].is_null(),
            "null/absent/empty-object arguments must succeed; got error: {}",
            resp["error"]
        );
        let text = resp["result"]["content"][0]["text"].as_str().expect("text");
        // uuid_v7 is 36 chars (8-4-4-4-12 + 4 hyphens).
        assert_eq!(
            text.len(),
            36,
            "uuid_v7 must produce a 36-char canonical UUID; got {text:?}"
        );
    }
}
