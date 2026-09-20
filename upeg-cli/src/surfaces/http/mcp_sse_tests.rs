//! `/mcp`'s server→client direction, exercised through the in-process
//! router.

use std::time::Duration;

use axum::body::{Body, to_bytes};
use http::{Request, StatusCode};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;

use super::{
    MCP_SESSION_HEADER, ResponseShape, SSE_COMMENT_PREFIX, SSE_CONTENT_TYPE, SSE_DATA_FIELD,
    SSE_EVENT_TERMINATOR, SSE_FIELD_SEPARATOR, SSE_KEEPALIVE_INTERVAL, SSE_QUEUE_BUDGET_BYTES,
    frame_queue, response_shape,
};
use crate::infrastructure::mcp_imports::{
    McpImportPhase, McpImportTally, phase_listener_count, phase_serial, publish_phase_for_tests,
};
use crate::surfaces::http::router;

/// JSON-RPC / MCP names this file asserts on. Spelled out here on
/// purpose: a test that reused the production constants would pass even
/// if both sides renamed a method together.
const METHOD_NOTIFICATIONS_MESSAGE: &str = "notifications/message";
const METHOD_TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";
const LOGGING_CAPABILITY: &str = "logging";
const TRANSPORT_LOGGER: &str = "upeg.transport";

const STREAMING_TOOLKIT_TOML: &str = r#"
id = "httpmcpsse"

[[tools]]
id = "two_lines"
description = "HTTP MCP SSE fixture"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "echo out-one; echo err-one 1>&2"]
surfaces = ["mcp"]

# A tool that announces the pid of the child upeg spawned and then
# outlives any reasonable test. It exists to prove that hanging up on the
# SSE body kills that child, exactly as hanging up on `POST …/stream`
# does — the two lanes share `CancelOnDrop`.
[[tools]]
id = "announce_and_sleep"
description = "HTTP MCP SSE fixture that runs until it is stopped"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "echo $$ > \"$0\"; sleep 30", "{pid_file}"]
surfaces = ["mcp"]

[[tools.inputs]]
name = "pid_file"
type = "string"
required = true
"#;

const STREAMING_TOOL_ID: &str = "httpmcpsse.two_lines";
const LONG_LIVED_TOOL_ID: &str = "httpmcpsse.announce_and_sleep";

/// The toolbox is process-global and the tests run in parallel, so the
/// fixture loads exactly once for the whole binary.
static LOAD_FIXTURE: std::sync::Once = std::sync::Once::new();

fn load_fixture() {
    LOAD_FIXTURE.call_once(|| {
        let dir = std::env::temp_dir().join(format!("upeg-http-mcp-sse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("SSE fixture directory");
        std::fs::write(dir.join("httpmcpsse.toml"), STREAMING_TOOLKIT_TOML)
            .expect("SSE fixture TOML");
        let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
        assert!(
            outcome.failed.is_empty(),
            "SSE fixture loads without failures: {:?}",
            outcome.failed
        );
        let _ = std::fs::remove_dir_all(&dir);
    });
}

/// Every JSON-RPC frame in an SSE body, in order. Comments (keep-alive)
/// carry no `data:` line and drop out here.
fn sse_frames(body: &str) -> Vec<Value> {
    let prefix = format!("{SSE_DATA_FIELD}{SSE_FIELD_SEPARATOR}");
    body.split(SSE_EVENT_TERMINATOR)
        .filter_map(|event| {
            event
                .lines()
                .find_map(|line| line.strip_prefix(prefix.as_str()))
        })
        .map(|payload| serde_json::from_str(payload).expect("one event is one JSON frame"))
        .collect()
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

/// One `POST /mcp`, with whatever `Accept` the caller wants.
async fn mcp_post(accept: Option<&str>, body: String) -> http::Response<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json");
    if let Some(accept) = accept {
        request = request.header(http::header::ACCEPT, accept);
    }
    router()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap()
}

fn content_type(response: &http::Response<Body>) -> String {
    response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap().to_string())
        .unwrap_or_default()
}

async fn body_string(body: Body) -> String {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).expect("SSE body is UTF-8")
}

// === POST /mcp ===

#[tokio::test]
async fn sse_call_sends_progress_notifications_then_the_response_frame_last() {
    load_fixture();

    let response = mcp_post(Some(SSE_CONTENT_TYPE), call_request(STREAMING_TOOL_ID)).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), SSE_CONTENT_TYPE);

    let frames = sse_frames(&body_string(response.into_body()).await);
    assert!(
        frames.len() >= 2,
        "at least one notification and one response come out: {frames:?}"
    );

    let last = frames.last().expect("last frame");
    assert_eq!(last["id"], 1, "the response frame is always last");
    assert_eq!(last["result"]["isError"], Value::Null);

    for frame in &frames[..frames.len() - 1] {
        assert_eq!(
            frame["method"], METHOD_NOTIFICATIONS_MESSAGE,
            "everything before the response is a log notification"
        );
    }

    let output: String = frames
        .iter()
        .filter(|frame| frame["params"]["data"]["stream"] == "stdout")
        .filter_map(|frame| frame["params"]["data"]["text"].as_str())
        .collect();
    assert_eq!(output, "out-one\n");
}

#[tokio::test]
async fn sse_response_frame_matches_the_json_lane_response() {
    load_fixture();

    let streamed = {
        let response = mcp_post(Some(SSE_CONTENT_TYPE), call_request(STREAMING_TOOL_ID)).await;
        let frames = sse_frames(&body_string(response.into_body()).await);
        frames.last().expect("response frame").clone()
    };

    let response = mcp_post(None, call_request(STREAMING_TOOL_ID)).await;
    let buffered: Value =
        serde_json::from_str(&body_string(response.into_body()).await).expect("single JSON");

    assert_eq!(streamed, buffered, "transport does not change the response");
}

#[tokio::test]
async fn post_without_sse_request_is_a_single_json_as_before() {
    load_fixture();

    // `*/*` is what curl sends by default: "anything", not "a stream".
    for accept in [None, Some("*/*"), Some("application/json")] {
        let response = mcp_post(accept, call_request(STREAMING_TOOL_ID)).await;
        assert_eq!(response.status(), StatusCode::OK, "{accept:?}");
        assert!(
            content_type(&response).starts_with("application/json"),
            "{accept:?} → {}",
            content_type(&response)
        );
        let body: Value =
            serde_json::from_str(&body_string(response.into_body()).await).expect("single JSON");
        assert_eq!(body["id"], 1, "{accept:?}");
    }
}

#[tokio::test]
async fn notification_without_response_does_not_open_a_stream() {
    let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let response = mcp_post(Some(SSE_CONTENT_TYPE), notification.to_string()).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn initialize_returns_logging_capability_and_session_id() {
    let request = json!({ "jsonrpc": "2.0", "id": 7, "method": "initialize" }).to_string();
    let response = mcp_post(None, request).await;

    let session = response
        .headers()
        .get(MCP_SESSION_HEADER)
        .map(|value| value.to_str().unwrap().to_string())
        .expect("initialize issues a session id");
    assert!(!session.is_empty());

    let body: Value =
        serde_json::from_str(&body_string(response.into_body()).await).expect("single JSON");
    assert!(
        body["result"]["capabilities"][LOGGING_CAPABILITY].is_object(),
        "a lane that can push declares logging: {body}"
    );
}

#[tokio::test]
async fn raising_the_session_floor_stops_that_sessions_progress_notifications() {
    load_fixture();

    let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }).to_string();
    let response = mcp_post(None, initialize).await;
    let session = response
        .headers()
        .get(MCP_SESSION_HEADER)
        .expect("session id")
        .to_str()
        .unwrap()
        .to_string();

    let set_level = json!({
        "jsonrpc": "2.0", "id": 2,
        "method": "logging/setLevel", "params": { "level": "warning" },
    })
    .to_string();
    let response = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .header(MCP_SESSION_HEADER, session.as_str())
                .body(Body::from(set_level))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .header(http::header::ACCEPT, SSE_CONTENT_TYPE)
                .header(MCP_SESSION_HEADER, session.as_str())
                .body(Body::from(call_request(STREAMING_TOOL_ID)))
                .unwrap(),
        )
        .await
        .unwrap();
    let frames = sse_frames(&body_string(response.into_body()).await);

    assert_eq!(
        frames.len(),
        1,
        "with the floor raised to warning, info progress notifications do not come out: {frames:?}"
    );
    assert_eq!(
        frames[0]["id"], 1,
        "all that remains is the tools/call response frame"
    );
}

// === GET /mcp ===

#[tokio::test]
async fn get_that_refuses_event_streams_is_406() {
    let response = router()
        .oneshot(
            Request::builder()
                .uri("/mcp")
                .header(http::header::ACCEPT, "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
}

/// Read the stream until a `tools/list_changed` frame shows up, or the
/// deadline passes. Incremental (not `to_bytes`) because this body never
/// ends on its own — that is the point of it.
async fn wait_for_list_changed(mut body: Body, limit: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + limit;
    let mut buffer = String::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return false;
        }
        let Ok(Some(Ok(frame))) = tokio::time::timeout(remaining, body.frame()).await else {
            return false;
        };
        if let Some(data) = frame.data_ref() {
            buffer.push_str(&String::from_utf8_lossy(data));
        }
        if sse_frames(&buffer)
            .iter()
            .any(|frame| frame["method"] == METHOD_TOOLS_LIST_CHANGED)
        {
            return true;
        }
    }
}

/// Not `#[tokio::test]`: the phase is process-wide, so this test holds
/// the serialising lock for its whole body — including across the
/// awaits. Driving the runtime explicitly keeps a blocking guard out of
/// an `async fn`.
#[test]
fn get_stream_receives_list_changed_once_import_load_finishes() {
    let _serial = phase_serial();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
        .block_on(get_stream_scenario());
}

async fn get_stream_scenario() {
    let response = router()
        .oneshot(
            Request::builder()
                .uri("/mcp")
                .header(http::header::ACCEPT, SSE_CONTENT_TYPE)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), SSE_CONTENT_TYPE);

    // The handler subscribed before it returned, so a transition
    // published now cannot be missed.
    publish_phase_for_tests(McpImportPhase::Loading);
    publish_phase_for_tests(McpImportPhase::Done(McpImportTally {
        servers_loaded: 1,
        servers_failed: 0,
        tools: 3,
    }));

    let received = wait_for_list_changed(response.into_body(), Duration::from_secs(5)).await;
    publish_phase_for_tests(McpImportPhase::NotStarted);

    assert!(
        received,
        "an attached client is notified when the load finishes"
    );
}

// === Accept resolution ===

fn accept_shape(value: Option<&str>) -> ResponseShape {
    let mut headers = http::HeaderMap::new();
    if let Some(value) = value {
        headers.insert(http::header::ACCEPT, value.parse().unwrap());
    }
    response_shape(&headers)
}

#[test]
fn stream_opens_only_when_accept_names_event_stream() {
    assert_eq!(
        accept_shape(Some(SSE_CONTENT_TYPE)),
        ResponseShape::EventStream
    );
    assert_eq!(
        accept_shape(Some("application/json, text/event-stream")),
        ResponseShape::EventStream
    );
    assert_eq!(
        accept_shape(Some("text/event-stream;q=0.9")),
        ResponseShape::EventStream
    );
    // A wildcard means "anything", not "a stream".
    assert_eq!(accept_shape(Some("*/*")), ResponseShape::Json);
    assert_eq!(accept_shape(Some("application/json")), ResponseShape::Json);
    assert_eq!(accept_shape(None), ResponseShape::Json);
}

// === Budget ===

fn padding_frame(bytes: usize) -> Value {
    json!({ "jsonrpc": "2.0", "method": METHOD_NOTIFICATIONS_MESSAGE, "pad": "x".repeat(bytes) })
}

#[test]
fn notifications_over_budget_are_dropped_and_reported_as_one_marker() {
    let (frames, mut receiver, budget) = frame_queue();
    let half = SSE_QUEUE_BUDGET_BYTES / 2;

    frames.offer(&padding_frame(half));
    // No room — both frames are dropped.
    frames.offer(&padding_frame(half));
    frames.offer(&padding_frame(half));
    assert_eq!(budget.dropped.load(std::sync::atomic::Ordering::Relaxed), 2);

    // The consumer reads the first frame, returning budget.
    let first = receiver.try_recv().expect("first frame");
    budget.release(first.len());

    frames.offer(&padding_frame(1));
    let marker: Value = sse_frames(&receiver.try_recv().expect("marker"))
        .pop()
        .expect("marker frame");
    assert_eq!(marker["params"]["logger"], TRANSPORT_LOGGER);
    assert_eq!(marker["params"]["data"]["dropped"], 2);
    assert_eq!(
        marker["params"]["level"], "warning",
        "a loss is reported at higher severity than a progress notification"
    );

    // The frame after the marker is the one that actually got a slot.
    assert!(receiver.try_recv().is_ok());
}

#[test]
fn failing_to_deliver_the_marker_leaves_the_dropped_count_in_place() {
    // The loss count is **charged** via `swap` before the marker is
    // built — if two threads read the same total and each reports it,
    // the consumer hears double the lost count. A charge that fails to
    // deliver must be put back, and this test pins that refund.
    let (frames, receiver, budget) = frame_queue();
    frames.offer(&padding_frame(SSE_QUEUE_BUDGET_BYTES / 2));
    frames.offer(&padding_frame(SSE_QUEUE_BUDGET_BYTES));
    assert_eq!(budget.dropped.load(std::sync::atomic::Ordering::Relaxed), 1);

    drop(receiver);
    frames.flush_dropped();

    assert_eq!(
        budget.dropped.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "an unreported loss does not disappear"
    );
}

#[test]
fn response_frame_always_goes_out_even_with_a_full_budget() {
    let (frames, mut receiver, budget) = frame_queue();
    frames.offer(&padding_frame(SSE_QUEUE_BUDGET_BYTES / 2));
    frames.offer(&padding_frame(SSE_QUEUE_BUDGET_BYTES));

    frames.finish(&json!({ "jsonrpc": "2.0", "id": 1, "result": {} }));

    let mut all_frames = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        all_frames.extend(sse_frames(&event));
    }
    let last = all_frames.last().expect("last frame");
    assert_eq!(
        last["id"], 1,
        "the response goes out last regardless of budget"
    );
    assert!(budget.queued_bytes() > 0, "nothing has been released yet");
}

// === OpenAPI ===

#[tokio::test]
async fn openapi_documents_both_mcp_directions() {
    let response = router()
        .oneshot(
            Request::builder()
                .uri("/v1/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let spec: Value =
        serde_json::from_str(&body_string(response.into_body()).await).expect("OpenAPI JSON");

    let mcp = &spec["paths"]["/mcp"];
    assert!(
        mcp["post"]["responses"]["200"]["content"][SSE_CONTENT_TYPE].is_object(),
        "the POST /mcp SSE representation must be documented: {mcp}"
    );
    assert!(
        mcp["post"]["responses"]["200"]["content"]["application/json"].is_object(),
        "the JSON representation must remain too: {mcp}"
    );
    assert!(
        mcp["get"]["responses"]["200"]["content"][SSE_CONTENT_TYPE].is_object(),
        "the GET /mcp stream must be documented: {mcp}"
    );
    assert!(
        mcp["get"]["responses"]["406"].is_object(),
        "the answer to a GET that refuses streams must be documented too: {mcp}"
    );
}

// === Disconnect and keep-alive ===

/// Deadlines for the cancellation test. Generous: a loaded CI box can be
/// slow to spawn `sh`, and the test's own failure mode is a 30-second
/// orphan either way.
#[cfg(unix)]
const CHILD_APPEAR_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(unix)]
const CHILD_EXIT_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(unix)]
const LIVENESS_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[cfg(unix)]
async fn wait_for_child_pid(path: &std::path::Path) -> u32 {
    let deadline = std::time::Instant::now() + CHILD_APPEAR_TIMEOUT;
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && let Ok(pid) = text.trim().parse::<u32>()
        {
            return pid;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "child did not report its pid within {CHILD_APPEAR_TIMEOUT:?}"
        );
        tokio::time::sleep(LIVENESS_POLL_INTERVAL).await;
    }
}

#[cfg(unix)]
async fn wait_for_process_exit(pid: u32) -> bool {
    let deadline = std::time::Instant::now() + CHILD_EXIT_TIMEOUT;
    loop {
        let alive = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !alive {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(LIVENESS_POLL_INTERVAL).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn dropping_the_sse_connection_kills_the_running_child() {
    // A contract `POST …/stream` has long kept. `/mcp` SSE is the same
    // transport, so it owes the same answer — a 10-minute build nobody
    // reads is a run that must stop.
    load_fixture();
    let pid_path = std::env::temp_dir().join(format!(
        "upeg-http-mcp-sse-cancel-{}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_path);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": LONG_LIVED_TOOL_ID,
            "arguments": { "pid_file": pid_path.to_string_lossy() },
        },
    })
    .to_string();

    let response = mcp_post(Some(SSE_CONTENT_TYPE), request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let stream_body = response.into_body();
    let child_pid = wait_for_child_pid(&pid_path).await;

    // When: the consumer hangs up — dropping the body is exactly what a
    // closed connection does.
    drop(stream_body);

    // Then
    let exited = wait_for_process_exit(child_pid).await;
    if !exited {
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &child_pid.to_string()])
            .status();
    }
    let _ = std::fs::remove_file(&pid_path);
    assert!(
        exited,
        "SSE connection dropped but child (pid={child_pid}) stayed alive for {CHILD_EXIT_TIMEOUT:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn even_a_silent_sse_call_emits_keep_alive() {
    // Even a call that runs without a single frame must put bytes on
    // the socket. An idle-timeout proxy in between cannot tell a silent
    // connection from a dead one.
    load_fixture();
    let pid_path = std::env::temp_dir().join(format!(
        "upeg-http-mcp-sse-keepalive-{}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_path);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": LONG_LIVED_TOOL_ID,
            "arguments": { "pid_file": pid_path.to_string_lossy() },
        },
    })
    .to_string();

    let response = mcp_post(Some(SSE_CONTENT_TYPE), request).await;
    let mut body = response.into_body();

    // `Interval`'s first tick fires immediately, so the first bytes of
    // a call that produces no output must be a keep-alive comment —
    // previously not a single byte left the socket until the tool
    // finished.
    let first_bytes = tokio::time::timeout(CHILD_APPEAR_TIMEOUT, body.frame())
        .await
        .expect("a silent call must still emit bytes promptly")
        .expect("the body must not end")
        .expect("no frame error");
    let text = String::from_utf8_lossy(first_bytes.data_ref().expect("data frame")).to_string();

    drop(body);
    let child_pid = wait_for_child_pid(&pid_path).await;
    let _ = std::process::Command::new("kill")
        .args(["-KILL", &child_pid.to_string()])
        .status();
    let _ = std::fs::remove_file(&pid_path);

    assert!(
        text.starts_with(SSE_COMMENT_PREFIX),
        "a silent call's first bytes are a keep-alive comment: {text:?}"
    );
}

#[tokio::test]
async fn keep_alive_timer_does_not_burst_missed_ticks() {
    // Two stream bodies share the same timer. A stream starved for a
    // minute owes one keep-alive, not a minute's worth.
    let timer = super::keepalive_timer();
    assert_eq!(timer.period(), SSE_KEEPALIVE_INTERVAL);
    assert_eq!(
        timer.missed_tick_behavior(),
        tokio::time::MissedTickBehavior::Delay
    );
}

// === Phase listener registry ===

/// Not `#[tokio::test]`: the registry is process-wide and the phase
/// serialising lock is a blocking guard.
#[test]
fn closing_a_get_stream_removes_the_phase_listener_from_the_registry() {
    let _serial = phase_serial();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
        .block_on(listener_leak_scenario());
}

async fn listener_leak_scenario() {
    let initial = phase_listener_count();

    for _ in 0..8 {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/mcp")
                    .header(http::header::ACCEPT, SSE_CONTENT_TYPE)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        // The connection drops. Previously a sender lingered here and
        // the registry kept growing until the next publish — forever,
        // in a process that never publishes again.
        drop(response.into_body());
    }

    assert_eq!(
        phase_listener_count(),
        initial,
        "a closed stream leaves no slot in the registry"
    );
}
