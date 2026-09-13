//! 스트리밍 attach 클라이언트: 가짜 host와 실제 in-process 라우터 양쪽.
//!
//! 가짜 host는 host가 절대 보장하지 않는 것(줄 경계, 취소 시점, 오래된
//! host의 404)을 결정적으로 재현하고, 라우터 테스트는 우리가 파싱하는
//! 것이 host가 실제로 쓰는 것과 같은 wire임을 고정한다.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};
use upeg_runtime::{CancellationToken, ProgressEvent, ProgressReporter, ProgressSink};

use super::*;
use crate::infrastructure::discovery::ServerInfo;

/// 테스트 host가 응답을 다 쓸 때까지 클라이언트가 기다리는 상한.
const 테스트_대기_상한: Duration = Duration::from_secs(10);
/// 취소가 관측될 때까지 도는 폴링 주기.
const 취소_폴링_주기: Duration = Duration::from_millis(10);

const 테스트_툴_ID: &str = "attachstream.fixture";
const 테스트_보드: &str = "dev";
const 테스트_토큰: &str = "attach-stream-token";

/// 진행 이벤트를 모아 두는 sink. `SharedProgressSink`가 `Send + Sync`를
/// 요구하므로 내부는 Mutex 하나다.
#[derive(Default)]
struct 수집Sink {
    events: Mutex<Vec<ProgressEvent>>,
}

impl ProgressSink for 수집Sink {
    fn emit(&self, event: ProgressEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

impl 수집Sink {
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

fn 라이브_호출(sink: &Arc<수집Sink>, cancel: Option<CancellationToken>) -> LiveCall {
    let shared: upeg_runtime::SharedProgressSink = Arc::<수집Sink>::clone(sink);
    LiveCall::new(Some(ProgressReporter::new(shared)), cancel)
}

fn 서버_정보(address: std::net::SocketAddr) -> ServerInfo {
    ServerInfo::new(format!("http://{address}"), 테스트_토큰)
}

/// 요청 헤더와 본문을 끝까지 읽는다. 클라이언트는 `Content-Length`를
/// 항상 붙이므로 본문 길이는 헤더에서 알 수 있다.
fn 요청을_읽는다(stream: &mut TcpStream) -> String {
    let mut raw = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let read = stream.read(&mut byte).expect("요청 읽기");
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
        stream.read_exact(&mut body).expect("요청 본문 읽기");
    }
    format!("{head}{}", String::from_utf8_lossy(&body))
}

/// NDJSON 줄 하나 — host가 쓰는 것과 같은 모양.
fn ndjson(value: &Value) -> String {
    format!("{value}\n")
}

fn chunk_줄(seq: u64, stream: &str, data: &str) -> String {
    ndjson(&json!({ "event": "chunk", "stream": stream, "seq": seq, "data": data }))
}

fn 성공_결과_줄() -> String {
    ndjson(&json!({
        "event": "result",
        "result": { "ok": true, "primary_output_id": null, "outputs": [] },
    }))
}

/// 대본대로 답하는 가짜 host. `script`는 헤더 뒤에 순서대로 쓸 본문
/// 조각들이고, 조각 사이에서 `pause`만큼 쉰다.
fn 대본_host(
    status_line: &'static str,
    script: Vec<String>,
    pause: Duration,
) -> (std::net::SocketAddr, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("테스트 host 바인딩");
    let address = listener.local_addr().expect("테스트 host 주소");
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("연결 수락");
        let request = 요청을_읽는다(&mut stream);
        let head = format!("{status_line}\r\nContent-Type: application/x-ndjson\r\n\r\n");
        if stream.write_all(head.as_bytes()).is_err() {
            return request;
        }
        for piece in script {
            if stream.write_all(piece.as_bytes()).is_err() || stream.flush().is_err() {
                // 클라이언트가 끊었다 — 취소 경로가 바로 이 모양이다.
                return request;
            }
            std::thread::sleep(pause);
        }
        request
    });
    (address, handle)
}

#[test]
fn 스트리밍_dispatch는_chunk를_진행_sink로_흘리고_결과를_돌려준다() {
    let (address, host) = 대본_host(
        "HTTP/1.0 200 OK",
        vec![
            chunk_줄(0, "stdout", "first\n"),
            chunk_줄(1, "stderr", "second\n"),
            성공_결과_줄(),
        ],
        Duration::ZERO,
    );
    let sink = Arc::new(수집Sink::default());

    let dispatched = dispatch_tool_streamed(
        &서버_정보(address),
        테스트_툴_ID,
        &json!({}),
        Surface::Tui,
        &라이브_호출(&sink, None),
    )
    .expect("스트리밍 dispatch");
    let request = host.join().expect("테스트 host");

    match dispatched {
        StreamedDispatch::Streamed(Outcome::Success(_)) => {}
        other => panic!("성공 봉투를 기대했지만 {other:?}를 받았다"),
    }
    assert_eq!(sink.chunks(), "first\nsecond\n");
    assert_eq!(
        sink.streams(),
        [
            upeg_runtime::ProgressStream::Stdout,
            upeg_runtime::ProgressStream::Stderr
        ],
        "stream 이름은 wire 그대로 복원된다"
    );
    assert!(
        request.contains(&format!("/v1/tools/{테스트_툴_ID}/stream")),
        "스트리밍 라우트를 부른다: {request}"
    );
    assert!(
        request.contains(crate::surfaces::http::ORIGIN_SURFACE_HEADER),
        "원점 surface 헤더를 싣는다: {request}"
    );
}

#[test]
fn 보드_스코프_스트리밍은_보드_라우트를_부른다() {
    let (address, host) = 대본_host("HTTP/1.0 200 OK", vec![성공_결과_줄()], Duration::ZERO);
    let sink = Arc::new(수집Sink::default());

    let _ = dispatch_tool_on_board_streamed(
        &서버_정보(address),
        테스트_보드,
        테스트_툴_ID,
        &json!({}),
        Surface::Tui,
        &라이브_호출(&sink, None),
    )
    .expect("보드 스트리밍 dispatch");
    let request = host.join().expect("테스트 host");

    assert!(
        request.contains(&format!(
            "/v1/boards/{테스트_보드}/tools/{테스트_툴_ID}/stream"
        )),
        "보드 스코프 스트리밍 라우트를 부른다: {request}"
    );
}

#[test]
fn 줄이_여러_조각에_걸쳐_와도_하나로_이어_붙인다() {
    let 한_줄 = chunk_줄(0, "stdout", "split across reads\n");
    let (앞, 뒤) = 한_줄.split_at(한_줄.len() / 2);
    let (address, host) = 대본_host(
        "HTTP/1.0 200 OK",
        vec![앞.to_string(), 뒤.to_string(), 성공_결과_줄()],
        취소_폴링_주기,
    );
    let sink = Arc::new(수집Sink::default());

    let _ = dispatch_tool_streamed(
        &서버_정보(address),
        테스트_툴_ID,
        &json!({}),
        Surface::Tui,
        &라이브_호출(&sink, None),
    )
    .expect("스트리밍 dispatch");
    let _ = host.join();

    assert_eq!(sink.chunks(), "split across reads\n");
}

#[test]
fn 취소되면_본문을_끊고_취소_봉투로_끝낸다() {
    // host는 결과 줄을 절대 보내지 않는다 — 오래 걸리는 tool 그 자체다.
    let (address, host) = 대본_host(
        "HTTP/1.0 200 OK",
        (0..1000)
            .map(|seq| chunk_줄(seq, "stdout", "still working\n"))
            .collect(),
        취소_폴링_주기,
    );
    let sink = Arc::new(수집Sink::default());
    let token = CancellationToken::new();
    let 취소자 = token.clone();
    let (관측_송신, 관측) = mpsc::channel();
    let 감시 = std::thread::spawn(move || {
        // 첫 chunk가 도착한 뒤에 취소한다.
        let _ = 관측.recv_timeout(테스트_대기_상한);
        취소자.cancel();
    });

    let sink_for_watch = Arc::clone(&sink);
    let 신호 = std::thread::spawn(move || {
        let 마감 = std::time::Instant::now() + 테스트_대기_상한;
        while sink_for_watch.chunks().is_empty() && std::time::Instant::now() < 마감 {
            std::thread::sleep(취소_폴링_주기);
        }
        let _ = 관측_송신.send(());
    });

    let dispatched = dispatch_tool_streamed(
        &서버_정보(address),
        테스트_툴_ID,
        &json!({}),
        Surface::Tui,
        &라이브_호출(&sink, Some(token)),
    )
    .expect("취소된 dispatch도 봉투 하나로 끝난다");
    신호.join().expect("감시 스레드");
    감시.join().expect("취소 스레드");
    let _ = host.join();

    match dispatched {
        StreamedDispatch::Streamed(Outcome::Failure(failure)) => {
            assert_eq!(failure.error.code, CANCELLED_ERROR_CODE);
        }
        other => panic!("취소 봉투를 기대했지만 {other:?}를 받았다"),
    }
    assert!(!sink.chunks().is_empty(), "끊기 전까지의 출력은 남는다");
}

#[test]
fn 스트리밍_라우트를_모르는_host는_버퍼_경로로_되돌린다() {
    let (address, host) = 대본_host("HTTP/1.0 404 Not Found", vec![], Duration::ZERO);
    let sink = Arc::new(수집Sink::default());

    let dispatched = dispatch_tool_streamed(
        &서버_정보(address),
        테스트_툴_ID,
        &json!({}),
        Surface::Tui,
        &라이브_호출(&sink, None),
    )
    .expect("404는 오류가 아니다");
    let _ = host.join();

    assert!(matches!(dispatched, StreamedDispatch::RouteUnavailable));
}

#[test]
fn 결과_줄_없이_끊긴_stream은_실패로_끝난다() {
    let (address, host) = 대본_host(
        "HTTP/1.0 200 OK",
        vec![chunk_줄(0, "stdout", "half a run\n")],
        Duration::ZERO,
    );
    let sink = Arc::new(수집Sink::default());

    let dispatched = dispatch_tool_streamed(
        &서버_정보(address),
        테스트_툴_ID,
        &json!({}),
        Surface::Tui,
        &라이브_호출(&sink, None),
    )
    .expect("끊긴 stream도 봉투 하나로 끝난다");
    let _ = host.join();

    match dispatched {
        StreamedDispatch::Streamed(Outcome::Failure(failure)) => {
            assert_eq!(failure.error.code, TRUNCATED_STREAM_ERROR_CODE);
        }
        other => panic!("실패 봉투를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 실패_봉투도_그대로_복원한다() {
    let 실패_줄 = ndjson(&json!({
        "event": "result",
        "result": { "ok": false, "error": { "code": "tool_error", "message": "boom" } },
    }));
    let (address, host) = 대본_host("HTTP/1.0 200 OK", vec![실패_줄], Duration::ZERO);
    let sink = Arc::new(수집Sink::default());

    let dispatched = dispatch_tool_streamed(
        &서버_정보(address),
        테스트_툴_ID,
        &json!({}),
        Surface::Tui,
        &라이브_호출(&sink, None),
    )
    .expect("스트리밍 dispatch");
    let _ = host.join();

    match dispatched {
        StreamedDispatch::Streamed(Outcome::Failure(failure)) => {
            assert_eq!(failure.error.code, "tool_error");
            assert_eq!(failure.error.message, "boom");
        }
        other => panic!("실패 봉투를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 버려진_출력_표시는_tail에_정직하게_남는다() {
    let 버림_줄 = ndjson(&json!({ "event": "dropped", "bytes": 4096 }));
    let (address, host) = 대본_host(
        "HTTP/1.0 200 OK",
        vec![버림_줄, 성공_결과_줄()],
        Duration::ZERO,
    );
    let sink = Arc::new(수집Sink::default());

    let _ = dispatch_tool_streamed(
        &서버_정보(address),
        테스트_툴_ID,
        &json!({}),
        Surface::Tui,
        &라이브_호출(&sink, None),
    )
    .expect("스트리밍 dispatch");
    let _ = host.join();

    assert!(
        sink.chunks().contains("4096"),
        "host가 버린 바이트 수를 사람에게 보여 준다: {}",
        sink.chunks()
    );
}

// ─── 실제 in-process 라우터와의 wire 호환 ────────────────────────

/// 진행을 실제로 흘리는 External tool 하나. 툴박스는 프로세스 전역이라
/// 바이너리당 한 번만 적재한다.
const 라우터_툴킷_TOML: &str = r#"
id = "attachstream"

[[tools]]
id = "two_lines"
description = "attach streaming fixture"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "echo out-one; echo err-one 1>&2; echo out-two"]
# host는 attach한 TUI를 원점 surface 헤더대로 `tui`로 답한다. 그래서
# 픽스처는 두 표면 모두에 보여야 이 경로를 지나갈 수 있다.
surfaces = ["http", "tui"]
"#;

const 라우터_툴_ID: &str = "attachstream.two_lines";

static 라우터_픽스처: std::sync::Once = std::sync::Once::new();

fn 라우터_픽스처를_적재한다() {
    라우터_픽스처.call_once(|| {
        let dir = std::env::temp_dir().join(format!("upeg-attach-stream-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("픽스처 디렉터리");
        std::fs::write(dir.join("attachstream.toml"), 라우터_툴킷_TOML).expect("픽스처 TOML");
        let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
        assert!(
            outcome.failed.is_empty(),
            "attach 스트리밍 픽스처는 실패 없이 적재된다: {:?}",
            outcome.failed
        );
        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn 실제_host_라우터를_상대로도_같은_wire를_읽는다() {
    라우터_픽스처를_적재한다();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("라우터 바인딩");
    let address = listener.local_addr().expect("라우터 주소");
    let router = crate::surfaces::http::router_with_token(테스트_토큰);
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let sink = Arc::new(수집Sink::default());
    let sink_for_call = Arc::clone(&sink);
    let dispatched = tokio::task::spawn_blocking(move || {
        dispatch_tool_streamed(
            &서버_정보(address),
            라우터_툴_ID,
            &json!({}),
            Surface::Tui,
            &라이브_호출(&sink_for_call, None),
        )
    })
    .await
    .expect("blocking 작업")
    .expect("스트리밍 dispatch");
    server.abort();

    match dispatched {
        StreamedDispatch::Streamed(Outcome::Success(_)) => {}
        other => panic!("성공 봉투를 기대했지만 {other:?}를 받았다"),
    }
    let chunks = sink.chunks();
    assert!(chunks.contains("out-one"), "표준 출력이 흘러온다: {chunks}");
    assert!(chunks.contains("err-one"), "표준 오류도 흘러온다: {chunks}");
}
