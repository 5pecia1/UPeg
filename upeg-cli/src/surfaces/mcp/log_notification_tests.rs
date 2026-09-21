//! `tools/call` streams the running tool's output as
//! `notifications/message` frames, and leaves the final result frame
//! exactly as it was.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use upeg_core::{BoardKey, Surface};

use super::{
    McpLogWriter, PROTOCOL_VERSION, handle_with_board, serve_loop_once, serve_loop_once_logged,
};

/// JSON-RPC `-32602 Invalid params`.
const INVALID_PARAMS_CODE: i64 = -32602;
/// JSON-RPC `-32601 Method not found`.
const METHOD_NOT_FOUND_CODE: i64 = -32601;
const METHOD_LOGGING_SET_LEVEL: &str = "logging/setLevel";

const STREAMING_TOOLKIT_TOML: &str = r#"
id = "mcpstream"

[[tools]]
id = "two_lines"
description = "MCP streaming fixture"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "echo out-one; echo err-one 1>&2"]
surfaces = ["mcp"]
"#;

const STREAMING_TOOL_ID: &str = "mcpstream.two_lines";

/// The toolbox is process-global and the tests run in parallel, so the
/// fixture loads exactly once for the whole binary.
static FIXTURE_LOADED: std::sync::Once = std::sync::Once::new();

fn load_fixture() {
    FIXTURE_LOADED.call_once(|| {
        let dir = std::env::temp_dir().join(format!("upeg-mcp-stream-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("streaming fixture dir");
        std::fs::write(dir.join("mcpstream.toml"), STREAMING_TOOLKIT_TOML)
            .expect("streaming fixture TOML");
        let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
        assert!(
            outcome.failed.is_empty(),
            "streaming fixture must load without failures: {:?}",
            outcome.failed
        );
        let _ = std::fs::remove_dir_all(&dir);
    });
}

fn call_request(tool: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": tool, "arguments": {} },
    })
    .to_string()
}

/// Runs one request with a log stream attached and returns
/// `(response lines, notification lines)`.
fn run_with_log(request: String, board: Option<&BoardKey>) -> (Vec<Value>, Vec<Value>) {
    let notifications = Arc::new(Mutex::new(Vec::<u8>::new()));
    let log = McpLogWriter::new(Arc::clone(&notifications));
    let mut responses = Vec::<u8>::new();

    serve_loop_once_logged(request, &mut responses, board, Some(&log));

    let to_frames = |bytes: &[u8]| -> Vec<Value> {
        String::from_utf8_lossy(bytes)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("each line is one JSON-RPC frame"))
            .collect()
    };
    let notices = to_frames(&notifications.lock().expect("notification buffer lock"));
    (to_frames(&responses), notices)
}

#[test]
fn tools_call_streams_runtime_output_as_log_notifications() {
    load_fixture();

    let (responses, notifications) = run_with_log(call_request(STREAMING_TOOL_ID), None);

    assert!(!notifications.is_empty(), "no notification frames arrived");
    for frame in &notifications {
        assert_eq!(frame["jsonrpc"], "2.0");
        assert_eq!(frame["method"], "notifications/message");
        assert_eq!(frame["params"]["level"], "info");
        assert_eq!(frame["params"]["logger"], "upeg.tool");
        assert_eq!(frame["params"]["data"]["tool"], STREAMING_TOOL_ID);
        assert!(frame["id"].is_null(), "notifications carry no id");
    }

    let text_for = |stream: &str| -> String {
        notifications
            .iter()
            .filter(|frame| frame["params"]["data"]["stream"] == stream)
            .filter_map(|frame| frame["params"]["data"]["text"].as_str())
            .collect()
    };
    assert_eq!(text_for("stdout"), "out-one\n");
    assert_eq!(text_for("stderr"), "err-one\n");

    // The response frame is untouched by streaming.
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["structuredContent"]["ok"], true);
}

#[test]
fn without_log_stream_only_the_response_goes_out_as_before() {
    load_fixture();

    let mut responses = Vec::<u8>::new();
    serve_loop_once(call_request(STREAMING_TOOL_ID), &mut responses, None);

    let text = String::from_utf8(responses).expect("response is UTF-8");
    assert_eq!(
        text.lines().count(),
        1,
        "a single response line, no notifications"
    );
    assert!(!text.contains("notifications/message"));
}

#[test]
fn non_tools_call_requests_emit_no_log_notifications() {
    let request = json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/list" }).to_string();

    let (responses, notifications) = run_with_log(request, None);

    assert!(
        notifications.is_empty(),
        "tools/list has no output to stream"
    );
    assert_eq!(responses.len(), 1);
}

#[test]
fn initialize_advertises_logging_capability() {
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }).to_string();

    let (responses, _) = run_with_log(request, None);

    assert_eq!(responses[0]["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert!(
        responses[0]["result"]["capabilities"]["logging"].is_object(),
        "a server that sends notifications must advertise the logging capability"
    );
}

fn level_request(level: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": METHOD_LOGGING_SET_LEVEL,
        "params": { "level": level },
    })
    .to_string()
}

/// Runs a whole session's worth of lines against one log writer, so a
/// `logging/setLevel` can outlive the request that set it.
fn one_session(requests: &[String]) -> (Vec<Value>, Vec<Value>) {
    let notifications = Arc::new(Mutex::new(Vec::<u8>::new()));
    let log = McpLogWriter::new(Arc::clone(&notifications));
    let mut responses = Vec::<u8>::new();
    for request in requests {
        serve_loop_once_logged(request.clone(), &mut responses, None, Some(&log));
    }

    let to_frames = |bytes: &[u8]| -> Vec<Value> {
        String::from_utf8_lossy(bytes)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("each line is one JSON-RPC frame"))
            .collect()
    };
    let notices = to_frames(&notifications.lock().expect("notification buffer lock"));
    (to_frames(&responses), notices)
}

#[test]
fn set_level_accepts_known_severity() {
    let (responses, _) = one_session(&[level_request(json!("warning"))]);

    assert_eq!(responses.len(), 1);
    assert!(
        responses[0]["error"].is_null(),
        "a known severity is not an error: {}",
        responses[0]
    );
    assert!(responses[0]["result"].is_object());
}

#[test]
fn unknown_severity_is_invalid_params() {
    let (responses, _) = one_session(&[level_request(json!("verbose"))]);

    assert_eq!(responses[0]["error"]["code"], INVALID_PARAMS_CODE);
    let message = responses[0]["error"]["message"]
        .as_str()
        .expect("error message");
    assert!(message.contains("verbose"), "{message}");
    assert!(
        message.contains("emergency"),
        "the message must list the allowed values: {message}"
    );
}

#[test]
fn non_string_severity_is_invalid_params() {
    let (responses, _) = one_session(&[level_request(json!(3))]);

    assert_eq!(responses[0]["error"]["code"], INVALID_PARAMS_CODE);
}

#[test]
fn severity_above_info_blocks_progress_notifications() {
    load_fixture();

    let (responses, notifications) = one_session(&[
        level_request(json!("error")),
        call_request(STREAMING_TOOL_ID),
    ]);

    assert!(
        notifications.is_empty(),
        "a client that asked for error-and-above must not receive info frames: {notifications:?}"
    );
    let call_response = responses.last().expect("tools/call response");
    assert_eq!(
        call_response["result"]["structuredContent"]["ok"], true,
        "the severity filter drops only notifications — the result frame is untouched"
    );
}

#[test]
fn lowering_to_info_or_below_keeps_progress_notifications() {
    load_fixture();

    let (_, notifications) = one_session(&[
        level_request(json!("debug")),
        call_request(STREAMING_TOOL_ID),
    ]);

    assert!(!notifications.is_empty(), "debug includes info");
}

#[test]
fn lane_without_log_stream_does_not_advertise_logging() {
    // `handle_with_board` is exactly what the HTTP `/mcp` route
    // (`surfaces/http/rpc.rs`) and the proxy lane's local fallback call.
    // Neither can emit an unsolicited frame, so neither may claim the
    // capability — a client that saw it would wait for log frames that
    // can never arrive.
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" });

    let response = handle_with_board(request, Surface::Mcp, None).expect("initialize response");

    assert!(response["result"]["capabilities"]["tools"].is_object());
    assert!(
        response["result"]["capabilities"]["logging"].is_null(),
        "a lane with nowhere to write frames must not advertise logging: {response}"
    );
}

#[test]
fn set_level_is_method_not_found_on_lane_without_log_stream() {
    let request = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": METHOD_LOGGING_SET_LEVEL,
        "params": { "level": "debug" },
    });

    let response = handle_with_board(request, Surface::Mcp, None).expect("response");

    assert_eq!(response["error"]["code"], METHOD_NOT_FOUND_CODE);
}
