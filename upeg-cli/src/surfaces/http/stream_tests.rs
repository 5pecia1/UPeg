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
const 스트리밍_툴킷_TOML: &str = r#"
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

const 스트리밍_툴_ID: &str = "httpstream.two_lines";
const 체인_툴_ID: &str = "httpstream.chained";
const 두_단계_체인_툴_ID: &str = "httpstream.chained_twice";
const 오래_사는_툴_ID: &str = "httpstream.announce_and_sleep";

/// The toolbox is process-global and the tests run in parallel, so the
/// fixture loads exactly once for the whole binary.
static 픽스처_적재: std::sync::Once = std::sync::Once::new();

fn 픽스처를_적재한다() {
    픽스처_적재.call_once(|| {
        let dir = std::env::temp_dir().join(format!("upeg-http-stream-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("스트리밍 픽스처 디렉터리");
        std::fs::write(dir.join("httpstream.toml"), 스트리밍_툴킷_TOML)
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

async fn 스트림_줄들(uri: &str, body: &'static str) -> (StatusCode, String, Vec<Value>) {
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
    let text = String::from_utf8(bytes.to_vec()).expect("NDJSON 본문은 UTF-8이다");
    let lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("한 줄은 JSON 하나다"))
        .collect();
    (status, content_type, lines)
}

fn 청크_텍스트(lines: &[Value], stream: &str) -> String {
    lines
        .iter()
        .filter(|line| line["event"] == "chunk" && line["stream"] == stream)
        .filter_map(|line| line["data"].as_str())
        .collect()
}

#[tokio::test]
async fn 스트리밍_호출은_ndjson_청크_뒤에_결과_줄을_보낸다() {
    픽스처를_적재한다();

    let (status, content_type, lines) =
        스트림_줄들(&format!("/v1/tools/{스트리밍_툴_ID}/stream"), "{}").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "application/x-ndjson");
    assert!(lines.len() >= 2, "청크 하나와 결과 하나는 최소한 나온다");

    let last = lines.last().expect("마지막 줄");
    assert_eq!(last["event"], "result", "결과 줄은 언제나 마지막이다");
    assert_eq!(last["result"]["ok"], true);

    // Every non-final line is a chunk, and no chunk may follow the result.
    for line in &lines[..lines.len() - 1] {
        assert_eq!(line["event"], "chunk", "결과 줄 앞은 전부 청크다");
    }

    assert_eq!(청크_텍스트(&lines, "stdout"), "out-one\nout-two\n");
    assert_eq!(청크_텍스트(&lines, "stderr"), "err-one\n");
}

/// The `seq` of every chunk line, sorted.
///
/// Sorted, not in arrival order: the two streams are drained by two
/// threads, so which of them hands its chunk over first is a race. `seq`
/// exists *because* of that race — the contract is that the numbers form
/// one contiguous run the consumer can sort by, not that they arrive
/// sorted.
fn 정렬된_순번(lines: &[Value]) -> Vec<u64> {
    let mut seqs: Vec<u64> = lines
        .iter()
        .filter(|line| line["event"] == "chunk")
        .map(|line| line["seq"].as_u64().expect("seq는 정수다"))
        .collect();
    seqs.sort_unstable();
    seqs
}

#[tokio::test]
async fn 청크_순번은_0부터_이어진다() {
    픽스처를_적재한다();

    let (_, _, lines) = 스트림_줄들(&format!("/v1/tools/{스트리밍_툴_ID}/stream"), "{}").await;

    let seqs = 정렬된_순번(&lines);
    assert_eq!(seqs, (0..seqs.len() as u64).collect::<Vec<_>>());
}

#[tokio::test]
async fn 스트리밍_결과_줄은_비스트리밍_봉투와_같다() {
    픽스처를_적재한다();

    let (_, _, lines) = 스트림_줄들(&format!("/v1/tools/{스트리밍_툴_ID}/stream"), "{}").await;
    let streamed = lines.last().expect("결과 줄")["result"].clone();

    let response = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/tools/{스트리밍_툴_ID}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let buffered: Value = serde_json::from_slice(&bytes).expect("표준 경로는 JSON 하나다");

    assert_eq!(streamed, buffered, "스트리밍은 봉투를 바꾸지 않는다");
}

#[tokio::test]
async fn 알_수_없는_도구는_스트림을_열지_않고_404다() {
    let (status, content_type, lines) = 스트림_줄들("/v1/tools/no.such.tool/stream", "{}").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        !content_type.contains("ndjson"),
        "라우팅 오류는 평범한 JSON 응답이다"
    );
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["error"]["code"], "unknown_tool");
}

#[tokio::test]
async fn 잘못된_본문은_스트림을_열지_않고_400이다() {
    픽스처를_적재한다();

    let (status, _, lines) =
        스트림_줄들(&format!("/v1/tools/{스트리밍_툴_ID}/stream"), "\"nope\"").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(lines[0]["error"]["code"], "invalid_request");
}

#[tokio::test]
async fn 핀되지_않은_board_도구는_404다() {
    픽스처를_적재한다();

    let (status, _, _) = 스트림_줄들(
        &format!("/v1/boards/no-such-board/tools/{스트리밍_툴_ID}/stream"),
        "{}",
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn openapi는_스트리밍_경로를_ndjson으로_문서화한다() {
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
    let spec: Value = serde_json::from_slice(&bytes).expect("OpenAPI 문서는 JSON이다");

    let 스트리밍 = &spec["paths"]["/v1/tools/{id}/stream"]["post"];
    assert!(
        스트리밍.is_object(),
        "스트리밍 경로가 문서에 없다: {:?}",
        spec["paths"].as_object().map(serde_json::Map::len)
    );
    assert!(
        스트리밍["responses"]["200"]["content"]["application/x-ndjson"]["schema"]["oneOf"]
            .is_array(),
        "200 응답은 NDJSON 줄 스키마를 선언한다"
    );
    assert!(
        스트리밍["responses"]["422"].is_null(),
        "스트리밍에서는 도구 실패가 상태 코드가 되지 않는다"
    );
    assert!(
        spec["paths"]["/v1/boards/{board}/tools/{id}/stream"]["post"].is_object(),
        "board 스코프 스트리밍 경로도 문서화된다"
    );
}

#[tokio::test]
async fn chain_step의_실행_중_출력도_흘러나온다() {
    픽스처를_적재한다();

    let (status, _, lines) = 스트림_줄들(&format!("/v1/tools/{체인_툴_ID}/stream"), "{}").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        청크_텍스트(&lines, "stdout"),
        "out-one\nout-two\n",
        "chain step은 같은 스레드에서 돌므로 주변 sink를 그대로 물려받는다"
    );
    assert_eq!(lines.last().expect("결과 줄")["event"], "result");
}

#[tokio::test]
async fn 두_단계_chain의_청크_순번은_단계마다_되돌아가지_않는다() {
    픽스처를_적재한다();

    let (status, _, lines) =
        스트림_줄들(&format!("/v1/tools/{두_단계_체인_툴_ID}/stream"), "{}").await;

    assert_eq!(status, StatusCode::OK);
    let seqs = 정렬된_순번(&lines);

    assert!(
        seqs.len() >= 4,
        "External 단계 두 개가 각각 stdout/stderr를 쓴다: {seqs:?}"
    );
    assert_eq!(
        seqs,
        (0..seqs.len() as u64).collect::<Vec<_>>(),
        "한 호출은 한 순번이다 — 단계마다 0으로 되돌아가면 소비자가 순서를 복원할 수 없다"
    );
    assert_eq!(
        청크_텍스트(&lines, "stdout"),
        "out-one\nout-two\nout-one\nout-two\n",
        "두 단계의 출력이 모두 흘러나온다"
    );
}

// ---------------------------------------------------------------------
// The queue between the dispatch and the socket, exercised without a
// socket: a stalled or vanished consumer is precisely the case a router
// test cannot stage.
// ---------------------------------------------------------------------

/// One chunk big enough that a handful of them fill the budget, and
/// still small enough on its own that the first one always fits.
const 청크_바이트: usize = 64 * 1024;

fn 진행_이벤트(seq: u64) -> ProgressEvent {
    ProgressEvent {
        stream: ProgressStream::Stdout,
        seq,
        chunk: "x".repeat(청크_바이트),
    }
}

fn 결과_줄() -> String {
    line(json!({ EVENT_FIELD: EVENT_RESULT }))
}

#[test]
fn 읽지_않는_소비자는_예산까지만_쌓이고_나머지는_dropped로_보고된다() {
    let (lines, mut receiver, _budget) = line_queue();
    let forwarder = ChunkForwarder::new(lines.clone());

    // Twice the budget's worth, with nobody draining the receiver.
    let 보낸_청크_수 = (STREAM_QUEUE_BUDGET_BYTES / 청크_바이트) * 2;
    for seq in 0..보낸_청크_수 as u64 {
        forwarder.emit(진행_이벤트(seq));
    }
    lines.finish(결과_줄());

    let mut 원문 = Vec::new();
    while let Ok(line) = receiver.try_recv() {
        원문.push(line);
    }
    let 줄들: Vec<Value> = 원문
        .iter()
        .map(|line| serde_json::from_str(line).expect("한 줄은 JSON 하나다"))
        .collect();

    let 청크_바이트_합: usize = 원문
        .iter()
        .zip(&줄들)
        .filter(|(_, parsed)| parsed["event"] == EVENT_CHUNK)
        .map(|(raw, _)| raw.len())
        .sum();
    assert!(
        청크_바이트_합 <= STREAM_QUEUE_BUDGET_BYTES,
        "청크로 쌓인 양이 예산을 넘었다: {청크_바이트_합}"
    );
    assert!(
        줄들.len() < 보낸_청크_수,
        "예산을 넘겼는데 한 줄도 버리지 않았다: {}줄",
        줄들.len()
    );

    let 버림: Vec<&Value> = 줄들
        .iter()
        .filter(|line| line["event"] == EVENT_DROPPED)
        .collect();
    assert_eq!(버림.len(), 1, "여러 번의 손실은 한 줄로 합산된다");
    let 버린_바이트 = 버림[0]["bytes"].as_u64().expect("bytes는 정수다");
    assert_eq!(
        버린_바이트,
        (보낸_청크_수 - (줄들.len() - 2)) as u64 * 청크_바이트 as u64,
        "보고되는 값은 도구가 쓴 출력 바이트의 합이다"
    );

    assert_eq!(
        줄들.last().expect("마지막 줄")["event"],
        EVENT_RESULT,
        "결과 줄은 예산과 무관하게 마지막에 나간다"
    );
    assert_eq!(
        줄들[줄들.len() - 2]["event"],
        EVENT_DROPPED,
        "손실 보고는 결과 줄 바로 앞에 붙는다"
    );
}

#[test]
fn 소비자가_사라지면_전달을_멈추고_아무것도_붙들지_않는다() {
    let (lines, receiver, budget) = line_queue();
    let forwarder = ChunkForwarder::new(lines);
    drop(receiver);

    forwarder.emit(진행_이벤트(0));
    assert!(
        forwarder.is_stopped(),
        "연결이 끊긴 것을 알아채면 전달을 접는다"
    );

    forwarder.emit(진행_이벤트(1));
    assert_eq!(
        budget.queued_bytes(),
        0,
        "읽을 사람이 없는 줄을 호스트가 들고 있으면 안 된다"
    );
}

#[test]
fn 표지를_전달하지_못하면_버린_바이트가_그대로_남는다() {
    // 손실 수는 표지를 만들기 전에 `swap`으로 **청구**된다 — 두
    // 스레드가 같은 총계를 읽어 각자 보고하면 소비자는 잃은 양의 두
    // 배를 듣는다. 청구한 뒤 전달에 실패하면 되돌려 놓아야 하고, 이
    // 테스트가 그 되돌림을 붙든다.
    let (lines, receiver, budget) = line_queue();
    let forwarder = ChunkForwarder::new(lines.clone());

    // 예산을 채우고 한 청크를 버리게 만든다.
    let 보낸_청크_수 = (STREAM_QUEUE_BUDGET_BYTES / 청크_바이트) + 2;
    for seq in 0..보낸_청크_수 as u64 {
        forwarder.emit(진행_이벤트(seq));
    }
    let 버린_바이트 = budget.dropped.load(std::sync::atomic::Ordering::Relaxed);
    assert!(버린_바이트 > 0, "예산을 넘겼으면 버린 바이트가 있어야 한다");

    // 소비자가 사라진 뒤의 flush: 표지는 나가지 못한다.
    drop(receiver);
    lines.flush_dropped();

    assert_eq!(
        budget.dropped.load(std::sync::atomic::Ordering::Relaxed),
        버린_바이트,
        "보고하지 못한 손실은 사라지지 않는다"
    );
}

#[test]
fn 소비자가_읽어가면_예산이_돌아온다() {
    let (lines, mut receiver, budget) = line_queue();
    let forwarder = ChunkForwarder::new(lines);

    forwarder.emit(진행_이벤트(0));
    let 쌓인 = budget.queued_bytes();
    assert!(쌓인 > 청크_바이트, "청크 한 줄이 예산을 차지한다");

    let 줄 = receiver.try_recv().expect("청크 한 줄");
    budget.release(줄.len());
    assert_eq!(budget.queued_bytes(), 0, "읽어간 만큼 예산이 돌아온다");
}

/// Hanging up on a stream must stop the tool, so these two bounds are
/// what "stop" means: how long the child is given to announce itself,
/// and how long it may take to die afterwards. Both are hang detectors
/// — the child itself would run for thirty seconds.
#[cfg(unix)]
const 자식_등장_대기: std::time::Duration = std::time::Duration::from_secs(10);
#[cfg(unix)]
const 자식_종료_대기: std::time::Duration = std::time::Duration::from_secs(10);
#[cfg(unix)]
const 생존_확인_간격: std::time::Duration = std::time::Duration::from_millis(20);

/// The pid the fixture wrote, once it has written one.
#[cfg(unix)]
async fn 자식_pid를_기다린다(path: &std::path::Path) -> u32 {
    let 마감 = std::time::Instant::now() + 자식_등장_대기;
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && let Ok(pid) = text.trim().parse::<u32>()
        {
            return pid;
        }
        assert!(
            std::time::Instant::now() < 마감,
            "자식이 {자식_등장_대기:?} 안에 자기 pid를 알리지 않았다"
        );
        tokio::time::sleep(생존_확인_간격).await;
    }
}

/// Whether `pid` is gone within the deadline. `kill -0` rather than
/// `/proc`, so the check reads the same on Linux and macOS; upeg reaps
/// the child it killed, so a zombie cannot make this answer wrong.
#[cfg(unix)]
async fn 프로세스_종료를_기다린다(pid: u32) -> bool {
    let 마감 = std::time::Instant::now() + 자식_종료_대기;
    loop {
        let 살아있다 = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !살아있다 {
            return true;
        }
        if std::time::Instant::now() >= 마감 {
            return false;
        }
        tokio::time::sleep(생존_확인_간격).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn 스트림_연결을_끊으면_실행_중인_자식이_종료된다() {
    픽스처를_적재한다();
    let pid_path = std::env::temp_dir().join(format!(
        "upeg-http-stream-cancel-{}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_path);

    let 본문 = json!({ "pid_file": pid_path.to_string_lossy() }).to_string();
    let response = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/tools/{오래_사는_툴_ID}/stream"))
                .header("content-type", "application/json")
                .body(Body::from(본문))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let 스트림_본문 = response.into_body();
    let 자식_pid = 자식_pid를_기다린다(&pid_path).await;

    // When: the consumer hangs up — dropping the body is exactly what a
    // closed connection does.
    drop(스트림_본문);

    // Then
    let 종료됨 = 프로세스_종료를_기다린다(자식_pid).await;
    if !종료됨 {
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &자식_pid.to_string()])
            .status();
    }
    let _ = std::fs::remove_file(&pid_path);
    assert!(
        종료됨,
        "연결이 끊겼는데 자식(pid={자식_pid})이 {자식_종료_대기:?} 동안 살아 있었다"
    );
}
