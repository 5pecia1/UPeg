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

const 스트리밍_툴킷_TOML: &str = r#"
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

const 스트리밍_툴_ID: &str = "mcpstream.two_lines";

/// The toolbox is process-global and the tests run in parallel, so the
/// fixture loads exactly once for the whole binary.
static 픽스처_적재: std::sync::Once = std::sync::Once::new();

fn 픽스처를_적재한다() {
    픽스처_적재.call_once(|| {
        let dir = std::env::temp_dir().join(format!("upeg-mcp-stream-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("스트리밍 픽스처 디렉터리");
        std::fs::write(dir.join("mcpstream.toml"), 스트리밍_툴킷_TOML)
            .expect("스트리밍 픽스처 TOML");
        let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
        assert!(
            outcome.failed.is_empty(),
            "스트리밍 픽스처는 실패 없이 적재된다: {:?}",
            outcome.failed
        );
        let _ = std::fs::remove_dir_all(&dir);
    });
}

fn 호출_요청(tool: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": tool, "arguments": {} },
    })
    .to_string()
}

/// Runs one request with a log stream attached and returns
/// `(응답 줄들, 알림 줄들)`.
fn 로그와_함께_실행(request: String, board: Option<&BoardKey>) -> (Vec<Value>, Vec<Value>) {
    let notifications = Arc::new(Mutex::new(Vec::<u8>::new()));
    let log = McpLogWriter::new(Arc::clone(&notifications));
    let mut responses = Vec::<u8>::new();

    serve_loop_once_logged(request, &mut responses, board, Some(&log));

    let 줄로 = |bytes: &[u8]| -> Vec<Value> {
        String::from_utf8_lossy(bytes)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("한 줄은 JSON-RPC 프레임 하나다"))
            .collect()
    };
    let 알림 = 줄로(&notifications.lock().expect("알림 버퍼 잠금"));
    (줄로(&responses), 알림)
}

#[test]
fn tools_call은_실행_중_출력을_로그_알림으로_흘려보낸다() {
    픽스처를_적재한다();

    let (responses, notifications) = 로그와_함께_실행(호출_요청(스트리밍_툴_ID), None);

    assert!(!notifications.is_empty(), "알림 프레임이 하나도 없다");
    for frame in &notifications {
        assert_eq!(frame["jsonrpc"], "2.0");
        assert_eq!(frame["method"], "notifications/message");
        assert_eq!(frame["params"]["level"], "info");
        assert_eq!(frame["params"]["logger"], "upeg.tool");
        assert_eq!(frame["params"]["data"]["tool"], 스트리밍_툴_ID);
        assert!(frame["id"].is_null(), "알림에는 id가 없다");
    }

    let 텍스트 = |stream: &str| -> String {
        notifications
            .iter()
            .filter(|frame| frame["params"]["data"]["stream"] == stream)
            .filter_map(|frame| frame["params"]["data"]["text"].as_str())
            .collect()
    };
    assert_eq!(텍스트("stdout"), "out-one\n");
    assert_eq!(텍스트("stderr"), "err-one\n");

    // The response frame is untouched by streaming.
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["structuredContent"]["ok"], true);
}

#[test]
fn 로그_스트림이_없으면_예전과_똑같이_응답만_나간다() {
    픽스처를_적재한다();

    let mut responses = Vec::<u8>::new();
    serve_loop_once(호출_요청(스트리밍_툴_ID), &mut responses, None);

    let text = String::from_utf8(responses).expect("응답은 UTF-8이다");
    assert_eq!(text.lines().count(), 1, "알림 없이 응답 한 줄뿐이다");
    assert!(!text.contains("notifications/message"));
}

#[test]
fn tools_call이_아닌_요청은_로그_알림을_만들지_않는다() {
    let request = json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/list" }).to_string();

    let (responses, notifications) = 로그와_함께_실행(request, None);

    assert!(
        notifications.is_empty(),
        "tools/list는 흘려보낼 출력이 없다"
    );
    assert_eq!(responses.len(), 1);
}

#[test]
fn initialize는_logging_능력을_선언한다() {
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }).to_string();

    let (responses, _) = 로그와_함께_실행(request, None);

    assert_eq!(responses[0]["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert!(
        responses[0]["result"]["capabilities"]["logging"].is_object(),
        "알림을 보내는 서버는 logging 능력을 선언해야 한다"
    );
}

fn 수준_요청(level: Value) -> String {
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
fn 한_세션(requests: &[String]) -> (Vec<Value>, Vec<Value>) {
    let notifications = Arc::new(Mutex::new(Vec::<u8>::new()));
    let log = McpLogWriter::new(Arc::clone(&notifications));
    let mut responses = Vec::<u8>::new();
    for request in requests {
        serve_loop_once_logged(request.clone(), &mut responses, None, Some(&log));
    }

    let 줄로 = |bytes: &[u8]| -> Vec<Value> {
        String::from_utf8_lossy(bytes)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("한 줄은 JSON-RPC 프레임 하나다"))
            .collect()
    };
    let 알림 = 줄로(&notifications.lock().expect("알림 버퍼 잠금"));
    (줄로(&responses), 알림)
}

#[test]
fn set_level은_알려진_심각도를_받아들인다() {
    let (responses, _) = 한_세션(&[수준_요청(json!("warning"))]);

    assert_eq!(responses.len(), 1);
    assert!(
        responses[0]["error"].is_null(),
        "알려진 심각도는 오류가 아니다: {}",
        responses[0]
    );
    assert!(responses[0]["result"].is_object());
}

#[test]
fn 모르는_심각도는_invalid_params다() {
    let (responses, _) = 한_세션(&[수준_요청(json!("verbose"))]);

    assert_eq!(responses[0]["error"]["code"], INVALID_PARAMS_CODE);
    let message = responses[0]["error"]["message"]
        .as_str()
        .expect("오류 메시지");
    assert!(message.contains("verbose"), "{message}");
    assert!(
        message.contains("emergency"),
        "허용 목록을 알려줘야 한다: {message}"
    );
}

#[test]
fn 심각도가_문자열이_아니면_invalid_params다() {
    let (responses, _) = 한_세션(&[수준_요청(json!(3))]);

    assert_eq!(responses[0]["error"]["code"], INVALID_PARAMS_CODE);
}

#[test]
fn info보다_높은_심각도는_진행_알림을_막는다() {
    픽스처를_적재한다();

    let (responses, notifications) =
        한_세션(&[수준_요청(json!("error")), 호출_요청(스트리밍_툴_ID)]);

    assert!(
        notifications.is_empty(),
        "error 이상만 보겠다고 한 클라이언트에게 info 프레임을 보내면 안 된다: {notifications:?}"
    );
    let 호출_응답 = responses.last().expect("tools/call 응답");
    assert_eq!(
        호출_응답["result"]["structuredContent"]["ok"], true,
        "심각도 필터는 알림만 거른다 — 결과 프레임은 그대로다"
    );
}

#[test]
fn info_이하로_내리면_진행_알림은_계속_나간다() {
    픽스처를_적재한다();

    let (_, notifications) = 한_세션(&[수준_요청(json!("debug")), 호출_요청(스트리밍_툴_ID)]);

    assert!(!notifications.is_empty(), "debug는 info를 포함한다");
}

#[test]
fn 로그_스트림이_없는_lane은_logging을_선언하지_않는다() {
    // `handle_with_board` is exactly what the HTTP `/mcp` route
    // (`surfaces/http/rpc.rs`) and the proxy lane's local fallback call.
    // Neither can emit an unsolicited frame, so neither may claim the
    // capability — a client that saw it would wait for log frames that
    // can never arrive.
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" });

    let response = handle_with_board(request, Surface::Mcp, None).expect("initialize 응답");

    assert!(response["result"]["capabilities"]["tools"].is_object());
    assert!(
        response["result"]["capabilities"]["logging"].is_null(),
        "프레임을 쓸 곳이 없는 lane은 logging을 선언하지 않는다: {response}"
    );
}

#[test]
fn 로그_스트림이_없는_lane에서_set_level은_없는_메서드다() {
    let request = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": METHOD_LOGGING_SET_LEVEL,
        "params": { "level": "debug" },
    });

    let response = handle_with_board(request, Surface::Mcp, None).expect("응답");

    assert_eq!(response["error"]["code"], METHOD_NOT_FOUND_CODE);
}
