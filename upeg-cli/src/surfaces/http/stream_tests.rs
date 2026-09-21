//! NDJSON streaming route, exercised through the in-process router.

use axum::body::{Body, to_bytes};
use http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt as _;
use upeg_runtime::{ProgressEvent, ProgressSink as _, ProgressStream};

use super::{
    ChunkForwarder, EVENT_CHUNK, EVENT_DROPPED, EVENT_FIELD, EVENT_RESULT,
    STREAM_QUEUE_BUDGET_BYTES, line, line_queue,
};
use crate::surfaces::http::router;

/// A tool that writes to both streams with a pause between them, so a
/// buffered-until-exit implementation could not produce the ordering
/// these tests assert.
const STREAMING_TOOLKIT_TOML: &str = r#"
id = "httpstream"

[[tools]]
id = "two_lines"
description = "HTTP streaming fixture"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "echo out-one; echo err-one 1>&2; echo out-two"]
surfaces = ["http"]

# A Chain whose only step is the External tool above. Chain steps run on
# the dispatching thread, so the ambient progress sink reaches them with
# no extra wiring — this fixture is what proves it.
[[tools]]
id = "chained"
description = "HTTP streaming fixture, wrapped in a Chain"
pegboard_units = "U1"
invoker = "Chain"
surfaces = ["http"]

[[tools.steps]]
id = "run"
tool = "httpstream.two_lines"

# Two External steps in one Chain. Each step's invoker captures its own
# progress reporter, so this is the fixture that proves `seq` belongs to
# the *call* and not to the step — a per-step counter would produce
# 0,1,0,1 here.
[[tools]]
id = "chained_twice"
description = "HTTP streaming fixture, two External steps"
pegboard_units = "U1"
invoker = "Chain"
surfaces = ["http"]

[[tools.steps]]
id = "first"
tool = "httpstream.two_lines"

[[tools.steps]]
id = "second"
tool = "httpstream.two_lines"

# A tool that announces the pid of the child upeg spawned and then
# outlives any reasonable test. It exists to prove that hanging up on
# the stream kills that child rather than leaving it to its (absent)
# `timeout_ms`.
[[tools]]
id = "announce_and_sleep"
description = "HTTP streaming fixture that runs until it is stopped"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "echo $$ > \"$0\"; sleep 30", "{pid_file}"]
surfaces = ["http"]

[[tools.inputs]]
name = "pid_file"
type = "string"
required = true
"#;

const STREAMING_TOOL_ID: &str = "httpstream.two_lines";
const CHAIN_TOOL_ID: &str = "httpstream.chained";
const TWO_STEP_CHAIN_TOOL_ID: &str = "httpstream.chained_twice";
const LONG_LIVED_TOOL_ID: &str = "httpstream.announce_and_sleep";

/// The toolbox is process-global and the tests run in parallel, so the
/// fixture loads exactly once for the whole binary.
static LOAD_FIXTURE: std::sync::Once = std::sync::Once::new();

fn load_fixture() {
    LOAD_FIXTURE.call_once(|| {
        let dir = std::env::temp_dir().join(format!("upeg-http-stream-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("streaming fixture directory");
        std::fs::write(dir.join("httpstream.toml"), STREAMING_TOOLKIT_TOML)
            .expect("streaming fixture TOML");

        let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
        assert!(
            outcome.failed.is_empty(),
            "streaming fixture loads without failures: {:?}",
            outcome.failed
        );
        let _ = std::fs::remove_dir_all(&dir);
    });
}

async fn stream_lines(uri: &str, body: &'static str) -> (StatusCode, String, Vec<Value>) {
    let response = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap().to_string())
        .unwrap_or_default();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).expect("NDJSON body is UTF-8");
    let lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("one line is one JSON value"))
        .collect();
    (status, content_type, lines)
}

fn chunk_text(lines: &[Value], stream: &str) -> String {
    lines
        .iter()
        .filter(|line| line["event"] == "chunk" && line["stream"] == stream)
        .filter_map(|line| line["data"].as_str())
        .collect()
}

#[tokio::test]
async fn streaming_call_sends_ndjson_chunks_then_a_result_line() {
    load_fixture();

    let (status, content_type, lines) =
        stream_lines(&format!("/v1/tools/{STREAMING_TOOL_ID}/stream"), "{}").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "application/x-ndjson");
    assert!(
        lines.len() >= 2,
        "at least one chunk and one result come out"
    );

    let last = lines.last().expect("last line");
    assert_eq!(last["event"], "result", "the result line is always last");
    assert_eq!(last["result"]["ok"], true);

    // Every non-final line is a chunk, and no chunk may follow the result.
    for line in &lines[..lines.len() - 1] {
        assert_eq!(
            line["event"], "chunk",
            "everything before the result line is a chunk"
        );
    }

    assert_eq!(chunk_text(&lines, "stdout"), "out-one\nout-two\n");
    assert_eq!(chunk_text(&lines, "stderr"), "err-one\n");
}

/// The `seq` of every chunk line, sorted.
///
/// Sorted, not in arrival order: the two streams are drained by two
/// threads, so which of them hands its chunk over first is a race. `seq`
/// exists *because* of that race — the contract is that the numbers form
/// one contiguous run the consumer can sort by, not that they arrive
/// sorted.
fn sorted_seqs(lines: &[Value]) -> Vec<u64> {
    let mut seqs: Vec<u64> = lines
        .iter()
        .filter(|line| line["event"] == "chunk")
        .map(|line| line["seq"].as_u64().expect("seq is an integer"))
        .collect();
    seqs.sort_unstable();
    seqs
}

#[tokio::test]
async fn chunk_seqs_are_contiguous_from_zero() {
    load_fixture();

    let (_, _, lines) = stream_lines(&format!("/v1/tools/{STREAMING_TOOL_ID}/stream"), "{}").await;

    let seqs = sorted_seqs(&lines);
    assert_eq!(seqs, (0..seqs.len() as u64).collect::<Vec<_>>());
}

#[tokio::test]
async fn streaming_result_line_matches_the_buffered_envelope() {
    load_fixture();

    let (_, _, lines) = stream_lines(&format!("/v1/tools/{STREAMING_TOOL_ID}/stream"), "{}").await;
    let streamed = lines.last().expect("result line")["result"].clone();

    let response = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/tools/{STREAMING_TOOL_ID}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let buffered: Value =
        serde_json::from_slice(&bytes).expect("the buffered lane is a single JSON");

    assert_eq!(streamed, buffered, "streaming does not change the envelope");
}

#[tokio::test]
async fn unknown_tool_is_404_without_opening_a_stream() {
    let (status, content_type, lines) = stream_lines("/v1/tools/no.such.tool/stream", "{}").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        !content_type.contains("ndjson"),
        "a routing error is a plain JSON response"
    );
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["error"]["code"], "unknown_tool");
}

#[tokio::test]
async fn malformed_body_is_400_without_opening_a_stream() {
    load_fixture();

    let (status, _, lines) =
        stream_lines(&format!("/v1/tools/{STREAMING_TOOL_ID}/stream"), "\"nope\"").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(lines[0]["error"]["code"], "invalid_request");
}

#[tokio::test]
async fn unpinned_board_tool_is_404() {
    load_fixture();

    let (status, _, _) = stream_lines(
        &format!("/v1/boards/no-such-board/tools/{STREAMING_TOOL_ID}/stream"),
        "{}",
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn openapi_documents_streaming_routes_as_ndjson() {
    let response = router()
        .oneshot(
            Request::builder()
                .uri("/v1/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let spec: Value = serde_json::from_slice(&bytes).expect("the OpenAPI document is JSON");

    let streaming = &spec["paths"]["/v1/tools/{id}/stream"]["post"];
    assert!(
        streaming.is_object(),
        "streaming route missing from the document: {:?}",
        spec["paths"].as_object().map(serde_json::Map::len)
    );
    assert!(
        streaming["responses"]["200"]["content"]["application/x-ndjson"]["schema"]["oneOf"]
            .is_array(),
        "the 200 response declares the NDJSON line schema"
    );
    assert!(
        streaming["responses"]["422"].is_null(),
        "in streaming, tool failure does not become a status code"
    );
    assert!(
        spec["paths"]["/v1/boards/{board}/tools/{id}/stream"]["post"].is_object(),
        "the board-scoped streaming route is documented too"
    );
}

#[tokio::test]
async fn chain_step_output_streams_too() {
    load_fixture();

    let (status, _, lines) = stream_lines(&format!("/v1/tools/{CHAIN_TOOL_ID}/stream"), "{}").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        chunk_text(&lines, "stdout"),
        "out-one\nout-two\n",
        "a chain step runs on the same thread, so it inherits the ambient sink"
    );
    assert_eq!(lines.last().expect("result line")["event"], "result");
}

#[tokio::test]
async fn two_step_chain_chunk_seqs_do_not_restart_per_step() {
    load_fixture();

    let (status, _, lines) =
        stream_lines(&format!("/v1/tools/{TWO_STEP_CHAIN_TOOL_ID}/stream"), "{}").await;

    assert_eq!(status, StatusCode::OK);
    let seqs = sorted_seqs(&lines);

    assert!(
        seqs.len() >= 4,
        "two External steps each write stdout/stderr: {seqs:?}"
    );
    assert_eq!(
        seqs,
        (0..seqs.len() as u64).collect::<Vec<_>>(),
        "one call is one sequence — restarting at 0 per step leaves the consumer unable to restore order"
    );
    assert_eq!(
        chunk_text(&lines, "stdout"),
        "out-one\nout-two\nout-one\nout-two\n",
        "both steps' output comes out"
    );
}

// ---------------------------------------------------------------------
// The queue between the dispatch and the socket, exercised without a
// socket: a stalled or vanished consumer is precisely the case a router
// test cannot stage.
// ---------------------------------------------------------------------

/// One chunk big enough that a handful of them fill the budget, and
/// still small enough on its own that the first one always fits.
const CHUNK_BYTES: usize = 64 * 1024;

fn progress_event(seq: u64) -> ProgressEvent {
    ProgressEvent {
        stream: ProgressStream::Stdout,
        seq,
        chunk: "x".repeat(CHUNK_BYTES),
    }
}

fn result_line() -> String {
    line(json!({ EVENT_FIELD: EVENT_RESULT }))
}

#[test]
fn an_unread_consumer_accumulates_up_to_budget_and_reports_the_rest_as_dropped() {
    let (lines, mut receiver, _budget) = line_queue();
    let forwarder = ChunkForwarder::new(lines.clone());

    // Twice the budget's worth, with nobody draining the receiver.
    let sent_chunks = (STREAM_QUEUE_BUDGET_BYTES / CHUNK_BYTES) * 2;
    for seq in 0..sent_chunks as u64 {
        forwarder.emit(progress_event(seq));
    }
    lines.finish(result_line());

    let mut raw = Vec::new();
    while let Ok(line) = receiver.try_recv() {
        raw.push(line);
    }
    let parsed_lines: Vec<Value> = raw
        .iter()
        .map(|line| serde_json::from_str(line).expect("one line is one JSON value"))
        .collect();

    let chunk_bytes_total: usize = raw
        .iter()
        .zip(&parsed_lines)
        .filter(|(_, parsed)| parsed["event"] == EVENT_CHUNK)
        .map(|(raw, _)| raw.len())
        .sum();
    assert!(
        chunk_bytes_total <= STREAM_QUEUE_BUDGET_BYTES,
        "bytes queued as chunks exceeded the budget: {chunk_bytes_total}"
    );
    assert!(
        parsed_lines.len() < sent_chunks,
        "over budget yet not a single line was dropped: {} lines",
        parsed_lines.len()
    );

    let dropped: Vec<&Value> = parsed_lines
        .iter()
        .filter(|line| line["event"] == EVENT_DROPPED)
        .collect();
    assert_eq!(dropped.len(), 1, "multiple losses are summed into one line");
    let dropped_bytes = dropped[0]["bytes"].as_u64().expect("bytes is an integer");
    assert_eq!(
        dropped_bytes,
        (sent_chunks - (parsed_lines.len() - 2)) as u64 * CHUNK_BYTES as u64,
        "the reported value is the sum of output bytes the tool wrote"
    );

    assert_eq!(
        parsed_lines.last().expect("last line")["event"],
        EVENT_RESULT,
        "the result line goes out last regardless of budget"
    );
    assert_eq!(
        parsed_lines[parsed_lines.len() - 2]["event"],
        EVENT_DROPPED,
        "the loss report sits right before the result line"
    );
}

#[test]
fn a_vanished_consumer_stops_forwarding_and_holds_nothing() {
    let (lines, receiver, budget) = line_queue();
    let forwarder = ChunkForwarder::new(lines);
    drop(receiver);

    forwarder.emit(progress_event(0));
    assert!(
        forwarder.is_stopped(),
        "once the disconnect is noticed, forwarding folds"
    );

    forwarder.emit(progress_event(1));
    assert_eq!(
        budget.queued_bytes(),
        0,
        "the host must not hold a line nobody will read"
    );
}

#[test]
fn failing_to_deliver_the_marker_leaves_dropped_bytes_in_place() {
    // The loss count is **charged** via `swap` before the marker is
    // built — if two threads read the same total and each reports it,
    // the consumer hears double the lost amount. A charge that fails to
    // deliver must be put back, and this test pins that refund.
    let (lines, receiver, budget) = line_queue();
    let forwarder = ChunkForwarder::new(lines.clone());

    // Fill the budget and force one chunk to be dropped.
    let sent_chunks = (STREAM_QUEUE_BUDGET_BYTES / CHUNK_BYTES) + 2;
    for seq in 0..sent_chunks as u64 {
        forwarder.emit(progress_event(seq));
    }
    let dropped_bytes = budget.dropped.load(std::sync::atomic::Ordering::Relaxed);
    assert!(
        dropped_bytes > 0,
        "over budget means there must be dropped bytes"
    );

    // Flush after the consumer is gone: the marker cannot get out.
    drop(receiver);
    lines.flush_dropped();

    assert_eq!(
        budget.dropped.load(std::sync::atomic::Ordering::Relaxed),
        dropped_bytes,
        "an unreported loss does not disappear"
    );
}

#[test]
fn budget_returns_as_the_consumer_reads() {
    let (lines, mut receiver, budget) = line_queue();
    let forwarder = ChunkForwarder::new(lines);

    forwarder.emit(progress_event(0));
    let queued = budget.queued_bytes();
    assert!(queued > CHUNK_BYTES, "one chunk line occupies budget");

    let queued_line = receiver.try_recv().expect("one chunk line");
    budget.release(queued_line.len());
    assert_eq!(
        budget.queued_bytes(),
        0,
        "budget returns by the amount read"
    );
}

/// Hanging up on a stream must stop the tool, so these two bounds are
/// what "stop" means: how long the child is given to announce itself,
/// and how long it may take to die afterwards. Both are hang detectors
/// — the child itself would run for thirty seconds.
#[cfg(unix)]
const CHILD_APPEAR_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
#[cfg(unix)]
const CHILD_EXIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
#[cfg(unix)]
const LIVENESS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(20);

/// The pid the fixture wrote, once it has written one.
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

/// Whether `pid` is gone within the deadline. `kill -0` rather than
/// `/proc`, so the check reads the same on Linux and macOS; upeg reaps
/// the child it killed, so a zombie cannot make this answer wrong.
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
async fn dropping_the_stream_connection_kills_the_running_child() {
    load_fixture();
    let pid_path = std::env::temp_dir().join(format!(
        "upeg-http-stream-cancel-{}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_path);

    let request_body = json!({ "pid_file": pid_path.to_string_lossy() }).to_string();
    let response = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/tools/{LONG_LIVED_TOOL_ID}/stream"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
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
        "connection dropped but child (pid={child_pid}) stayed alive for {CHILD_EXIT_TIMEOUT:?}"
    );
}
