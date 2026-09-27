//! Streaming attach client: against both a fake host and the real
//! in-process router.
//!
//! The fake host deterministically reproduces what the host never
//! guarantees (line boundaries, cancellation timing, an old host's
//! 404), and the router tests pin that what we parse is the same wire
//! the host actually writes.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};
use upeg_runtime::{CancellationToken, ProgressEvent, ProgressReporter, ProgressSink};

use super::*;
use crate::infrastructure::discovery::{DiscoveredHost, ServerInfo};

/// Upper bound on how long the client waits for the test host to
/// finish writing its response.
const TEST_WAIT_LIMIT: Duration = Duration::from_secs(10);
/// Polling interval run until a cancellation is observed.
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(10);

const TEST_TOOL_ID: &str = "attachstream.fixture";
const TEST_BOARD: &str = "dev";
const TEST_TOKEN: &str = "attach-stream-token";

/// A sink that collects progress events. `SharedProgressSink` requires
/// `Send + Sync`, so the inside is a single Mutex.
#[derive(Default)]
struct CollectSink {
    events: Mutex<Vec<ProgressEvent>>,
}

impl ProgressSink for CollectSink {
    fn emit(&self, event: ProgressEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

impl CollectSink {
    fn chunks(&self) -> String {
        self.events
            .lock()
            .map(|events| {
                events
                    .iter()
                    .map(|event| event.chunk.as_str())
                    .collect::<String>()
            })
            .unwrap_or_default()
    }

    fn streams(&self) -> Vec<upeg_runtime::ProgressStream> {
        self.events
            .lock()
            .map(|events| events.iter().map(|event| event.stream).collect())
            .unwrap_or_default()
    }
}

fn live_call(sink: &Arc<CollectSink>, cancel: Option<CancellationToken>) -> LiveCall {
    let shared: upeg_runtime::SharedProgressSink = Arc::<CollectSink>::clone(sink);
    LiveCall::new(Some(ProgressReporter::new(shared)), cancel)
}

fn server_info(address: std::net::SocketAddr) -> DiscoveredHost {
    DiscoveredHost::for_test(ServerInfo::new(format!("http://{address}"), TEST_TOKEN))
}

/// Read request headers and body to the end. The client always sends
/// `Content-Length`, so the body length is known from the headers.
fn read_request(stream: &mut TcpStream) -> String {
    let mut raw = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let read = stream.read(&mut byte).expect("read request");
        if read == 0 {
            break;
        }
        raw.push(byte[0]);
        if raw.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8_lossy(&raw).to_string();
    let length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or_default();
    let mut body = vec![0_u8; length];
    if length > 0 {
        stream.read_exact(&mut body).expect("read request body");
    }
    format!("{head}{}", String::from_utf8_lossy(&body))
}

/// One NDJSON line — the same shape the host writes.
fn ndjson(value: &Value) -> String {
    format!("{value}\n")
}

fn chunk_line(seq: u64, stream: &str, data: &str) -> String {
    ndjson(&json!({ "event": "chunk", "stream": stream, "seq": seq, "data": data }))
}

fn success_result_line() -> String {
    ndjson(&json!({
        "event": "result",
        "result": { "ok": true, "primary_output_id": null, "outputs": [] },
    }))
}

/// A fake host that answers by script. `script` is the body pieces to
/// write in order after the headers, pausing `pause` between pieces.
fn scripted_host(
    status_line: &'static str,
    script: Vec<String>,
    pause: Duration,
) -> (std::net::SocketAddr, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test host bind");
    scripted_host_for(listener, status_line, script, pause)
}

fn scripted_host_for(
    listener: TcpListener,
    status_line: &'static str,
    script: Vec<String>,
    pause: Duration,
) -> (std::net::SocketAddr, std::thread::JoinHandle<String>) {
    let address = listener.local_addr().expect("test host address");
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept connection");
        let request = read_request(&mut stream);
        let head = format!("{status_line}\r\nContent-Type: application/x-ndjson\r\n\r\n");
        if stream.write_all(head.as_bytes()).is_err() {
            return request;
        }
        for piece in script {
            if stream.write_all(piece.as_bytes()).is_err() || stream.flush().is_err() {
                // The client hung up — this is exactly the shape of the
                // cancellation path.
                return request;
            }
            std::thread::sleep(pause);
        }
        request
    });
    (address, handle)
}

#[test]
fn ipv6_plain_board_and_stream_attach_use_bracketed_authority() {
    let Ok(first) = TcpListener::bind("[::1]:0") else {
        return; // IPv6 is unavailable on this runner.
    };
    let success = json!({ "ok": true, "primary_output_id": null, "outputs": [] }).to_string();
    let (address, server) = scripted_host_for(
        first,
        "HTTP/1.0 200 OK",
        vec![success.clone()],
        Duration::ZERO,
    );
    let host = server_info(address);
    assert!(matches!(
        super::super::dispatch_tool(&host, TEST_TOOL_ID, &json!({}), Surface::Tui).unwrap(),
        Outcome::Success(_)
    ));
    let request = server.join().unwrap();
    assert!(request.contains(&format!("Host: [{address_ip}]:", address_ip = address.ip())));

    let second = TcpListener::bind("[::1]:0").unwrap();
    let (address, server) =
        scripted_host_for(second, "HTTP/1.0 200 OK", vec![success], Duration::ZERO);
    let host = server_info(address);
    assert!(matches!(
        super::super::dispatch_tool_on_board(
            &host,
            TEST_BOARD,
            TEST_TOOL_ID,
            &json!({}),
            Surface::Tui
        )
        .unwrap(),
        Outcome::Success(_)
    ));
    let request = server.join().unwrap();
    assert!(request.contains(&format!("/v1/boards/{TEST_BOARD}/tools/{TEST_TOOL_ID}")));
    assert!(request.contains(&format!("Host: [{address_ip}]:", address_ip = address.ip())));

    let third = TcpListener::bind("[::1]:0").unwrap();
    let (address, server) = scripted_host_for(
        third,
        "HTTP/1.0 200 OK",
        vec![success_result_line()],
        Duration::ZERO,
    );
    let host = server_info(address);
    let sink = Arc::new(CollectSink::default());
    assert!(matches!(
        dispatch_tool_on_board_streamed(
            &host,
            TEST_BOARD,
            TEST_TOOL_ID,
            &json!({}),
            Surface::Tui,
            &live_call(&sink, None)
        )
        .unwrap(),
        StreamedDispatch::Streamed(Outcome::Success(_))
    ));
    let request = server.join().unwrap();
    assert!(request.contains(&format!(
        "/v1/boards/{TEST_BOARD}/tools/{TEST_TOOL_ID}/stream"
    )));
    assert!(request.contains(&format!("Host: [{address_ip}]:", address_ip = address.ip())));
}

#[test]
fn streaming_dispatch_flows_chunks_to_progress_sink_and_returns_result() {
    let (address, host) = scripted_host(
        "HTTP/1.0 200 OK",
        vec![
            chunk_line(0, "stdout", "first\n"),
            chunk_line(1, "stderr", "second\n"),
            success_result_line(),
        ],
        Duration::ZERO,
    );
    let sink = Arc::new(CollectSink::default());

    let dispatched = dispatch_tool_streamed(
        &server_info(address),
        TEST_TOOL_ID,
        &json!({}),
        Surface::Tui,
        &live_call(&sink, None),
    )
    .expect("streaming dispatch");
    let request = host.join().expect("test host");

    match dispatched {
        StreamedDispatch::Streamed(Outcome::Success(_)) => {}
        other => panic!("expected a success envelope, got {other:?}"),
    }
    assert_eq!(sink.chunks(), "first\nsecond\n");
    assert_eq!(
        sink.streams(),
        [
            upeg_runtime::ProgressStream::Stdout,
            upeg_runtime::ProgressStream::Stderr
        ],
        "stream names round-trip off the wire"
    );
    assert!(
        request.contains(&format!("/v1/tools/{TEST_TOOL_ID}/stream")),
        "calls the streaming route: {request}"
    );
    assert!(
        request.contains(crate::surfaces::http::ORIGIN_SURFACE_HEADER),
        "carries the origin surface header: {request}"
    );
}

#[test]
fn board_scoped_streaming_calls_the_board_route() {
    let (address, host) = scripted_host(
        "HTTP/1.0 200 OK",
        vec![success_result_line()],
        Duration::ZERO,
    );
    let sink = Arc::new(CollectSink::default());

    let _ = dispatch_tool_on_board_streamed(
        &server_info(address),
        TEST_BOARD,
        TEST_TOOL_ID,
        &json!({}),
        Surface::Tui,
        &live_call(&sink, None),
    )
    .expect("board streaming dispatch");
    let request = host.join().expect("test host");

    assert!(
        request.contains(&format!(
            "/v1/boards/{TEST_BOARD}/tools/{TEST_TOOL_ID}/stream"
        )),
        "calls the board-scoped streaming route: {request}"
    );
}

#[test]
fn line_split_across_fragments_is_joined_into_one() {
    let one_line = chunk_line(0, "stdout", "split across reads\n");
    let (front, back) = one_line.split_at(one_line.len() / 2);
    let (address, host) = scripted_host(
        "HTTP/1.0 200 OK",
        vec![front.to_string(), back.to_string(), success_result_line()],
        CANCEL_POLL_INTERVAL,
    );
    let sink = Arc::new(CollectSink::default());

    let _ = dispatch_tool_streamed(
        &server_info(address),
        TEST_TOOL_ID,
        &json!({}),
        Surface::Tui,
        &live_call(&sink, None),
    )
    .expect("streaming dispatch");
    let _ = host.join();

    assert_eq!(sink.chunks(), "split across reads\n");
}

#[test]
fn cancel_drops_the_body_and_ends_with_a_cancelled_envelope() {
    // The host never sends a result line — it is the long-running tool
    // itself.
    let (address, host) = scripted_host(
        "HTTP/1.0 200 OK",
        (0..1000)
            .map(|seq| chunk_line(seq, "stdout", "still working\n"))
            .collect(),
        CANCEL_POLL_INTERVAL,
    );
    let sink = Arc::new(CollectSink::default());
    let token = CancellationToken::new();
    let canceller = token.clone();
    let (observe_tx, observe) = mpsc::channel();
    let watcher = std::thread::spawn(move || {
        // Cancel after the first chunk has arrived.
        let _ = observe.recv_timeout(TEST_WAIT_LIMIT);
        canceller.cancel();
    });

    let sink_for_watch = Arc::clone(&sink);
    let signal = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + TEST_WAIT_LIMIT;
        while sink_for_watch.chunks().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(CANCEL_POLL_INTERVAL);
        }
        let _ = observe_tx.send(());
    });

    let dispatched = dispatch_tool_streamed(
        &server_info(address),
        TEST_TOOL_ID,
        &json!({}),
        Surface::Tui,
        &live_call(&sink, Some(token)),
    )
    .expect("a cancelled dispatch still ends in a single envelope");
    signal.join().expect("watch thread");
    watcher.join().expect("cancel thread");
    let _ = host.join();

    match dispatched {
        StreamedDispatch::Streamed(Outcome::Failure(failure)) => {
            assert_eq!(failure.error.code, CANCELLED_ERROR_CODE);
        }
        other => panic!("expected a cancelled envelope, got {other:?}"),
    }
    assert!(
        !sink.chunks().is_empty(),
        "output up to the drop point survives"
    );
}

#[test]
fn host_unaware_of_the_streaming_route_falls_back_to_buffered_path() {
    let (address, host) = scripted_host("HTTP/1.0 404 Not Found", vec![], Duration::ZERO);
    let sink = Arc::new(CollectSink::default());

    let dispatched = dispatch_tool_streamed(
        &server_info(address),
        TEST_TOOL_ID,
        &json!({}),
        Surface::Tui,
        &live_call(&sink, None),
    )
    .expect("a 404 is not an error");
    let _ = host.join();

    assert!(matches!(dispatched, StreamedDispatch::RouteUnavailable));
}

#[test]
fn stream_cut_without_a_result_line_ends_in_failure() {
    let (address, host) = scripted_host(
        "HTTP/1.0 200 OK",
        vec![chunk_line(0, "stdout", "half a run\n")],
        Duration::ZERO,
    );
    let sink = Arc::new(CollectSink::default());

    let dispatched = dispatch_tool_streamed(
        &server_info(address),
        TEST_TOOL_ID,
        &json!({}),
        Surface::Tui,
        &live_call(&sink, None),
    )
    .expect("a cut stream still ends in a single envelope");
    let _ = host.join();

    match dispatched {
        StreamedDispatch::Streamed(Outcome::Failure(failure)) => {
            assert_eq!(failure.error.code, TRUNCATED_STREAM_ERROR_CODE);
        }
        other => panic!("expected a failure envelope, got {other:?}"),
    }
}

#[test]
fn failure_envelope_is_restored_verbatim() {
    let failure_line = ndjson(&json!({
        "event": "result",
        "result": { "ok": false, "error": { "code": "tool_error", "message": "boom" } },
    }));
    let (address, host) = scripted_host("HTTP/1.0 200 OK", vec![failure_line], Duration::ZERO);
    let sink = Arc::new(CollectSink::default());

    let dispatched = dispatch_tool_streamed(
        &server_info(address),
        TEST_TOOL_ID,
        &json!({}),
        Surface::Tui,
        &live_call(&sink, None),
    )
    .expect("streaming dispatch");
    let _ = host.join();

    match dispatched {
        StreamedDispatch::Streamed(Outcome::Failure(failure)) => {
            assert_eq!(failure.error.code, "tool_error");
            assert_eq!(failure.error.message, "boom");
        }
        other => panic!("expected a failure envelope, got {other:?}"),
    }
}

#[test]
fn dropped_output_marker_stays_honestly_in_tail() {
    let dropped_line = ndjson(&json!({ "event": "dropped", "bytes": 4096 }));
    let (address, host) = scripted_host(
        "HTTP/1.0 200 OK",
        vec![dropped_line, success_result_line()],
        Duration::ZERO,
    );
    let sink = Arc::new(CollectSink::default());

    let _ = dispatch_tool_streamed(
        &server_info(address),
        TEST_TOOL_ID,
        &json!({}),
        Surface::Tui,
        &live_call(&sink, None),
    )
    .expect("streaming dispatch");
    let _ = host.join();

    assert!(
        sink.chunks().contains("4096"),
        "the byte count the host dropped is shown to the user: {}",
        sink.chunks()
    );
}

// ─── Wire compatibility with the real in-process router ─────────────

/// One External tool that actually emits progress. The toolbox is
/// process-global, so it is loaded once per binary.
const ROUTER_TOOLKIT_TOML: &str = r#"
id = "attachstream"

[[tools]]
id = "two_lines"
description = "attach streaming fixture"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "echo out-one; echo err-one 1>&2; echo out-two"]
# The host answers the attached TUI as `tui` per the origin surface
# header, so the fixture must be visible on both surfaces to pass
# through this path.
surfaces = ["http", "tui"]
"#;

const ROUTER_TOOL_ID: &str = "attachstream.two_lines";

static ROUTER_FIXTURE: std::sync::Once = std::sync::Once::new();

fn load_router_fixture() {
    ROUTER_FIXTURE.call_once(|| {
        let dir = std::env::temp_dir().join(format!("upeg-attach-stream-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("fixture directory");
        std::fs::write(dir.join("attachstream.toml"), ROUTER_TOOLKIT_TOML).expect("fixture TOML");
        let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
        assert!(
            outcome.failed.is_empty(),
            "the attach streaming fixture loads without failures: {:?}",
            outcome.failed
        );
        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn same_wire_is_read_against_the_real_host_router() {
    load_router_fixture();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("router bind");
    let address = listener.local_addr().expect("router address");
    let router = crate::surfaces::http::router_with_token(TEST_TOKEN);
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let sink = Arc::new(CollectSink::default());
    let sink_for_call = Arc::clone(&sink);
    let dispatched = tokio::task::spawn_blocking(move || {
        dispatch_tool_streamed(
            &server_info(address),
            ROUTER_TOOL_ID,
            &json!({}),
            Surface::Tui,
            &live_call(&sink_for_call, None),
        )
    })
    .await
    .expect("blocking work")
    .expect("streaming dispatch");
    server.abort();

    match dispatched {
        StreamedDispatch::Streamed(Outcome::Success(_)) => {}
        other => panic!("expected a success envelope, got {other:?}"),
    }
    let chunks = sink.chunks();
    assert!(chunks.contains("out-one"), "stdout flows through: {chunks}");
    assert!(
        chunks.contains("err-one"),
        "stderr flows through too: {chunks}"
    );
}
