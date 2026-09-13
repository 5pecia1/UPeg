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
    McpImportPhase, McpImportTally, phase_listener_count, publish_phase_for_tests, 페이즈_직렬화,
};
use crate::surfaces::http::router;

/// JSON-RPC / MCP names this file asserts on. Spelled out here on
/// purpose: a test that reused the production constants would pass even
/// if both sides renamed a method together.
const METHOD_NOTIFICATIONS_MESSAGE: &str = "notifications/message";
const METHOD_TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";
const LOGGING_CAPABILITY: &str = "logging";
const TRANSPORT_LOGGER: &str = "upeg.transport";

const 스트리밍_툴킷_TOML: &str = r#"
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

const 스트리밍_툴_ID: &str = "httpmcpsse.two_lines";
const 오래_사는_툴_ID: &str = "httpmcpsse.announce_and_sleep";

/// The toolbox is process-global and the tests run in parallel, so the
/// fixture loads exactly once for the whole binary.
static 픽스처_적재: std::sync::Once = std::sync::Once::new();

fn 픽스처를_적재한다() {
    픽스처_적재.call_once(|| {
        let dir = std::env::temp_dir().join(format!("upeg-http-mcp-sse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("SSE 픽스처 디렉터리");
        std::fs::write(dir.join("httpmcpsse.toml"), 스트리밍_툴킷_TOML).expect("SSE 픽스처 TOML");
        let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
        assert!(
            outcome.failed.is_empty(),
            "SSE 픽스처는 실패 없이 적재된다: {:?}",
            outcome.failed
        );
        let _ = std::fs::remove_dir_all(&dir);
    });
}

/// Every JSON-RPC frame in an SSE body, in order. Comments (keep-alive)
/// carry no `data:` line and drop out here.
fn sse_프레임들(body: &str) -> Vec<Value> {
    let prefix = format!("{SSE_DATA_FIELD}{SSE_FIELD_SEPARATOR}");
    body.split(SSE_EVENT_TERMINATOR)
        .filter_map(|event| {
            event
                .lines()
                .find_map(|line| line.strip_prefix(prefix.as_str()))
        })
        .map(|payload| serde_json::from_str(payload).expect("한 이벤트는 JSON 프레임 하나다"))
        .collect()
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

fn 콘텐츠_타입(response: &http::Response<Body>) -> String {
    response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap().to_string())
        .unwrap_or_default()
}

async fn 본문_문자열(body: Body) -> String {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).expect("SSE 본문은 UTF-8이다")
}

// === POST /mcp ===

#[tokio::test]
async fn sse_호출은_진행_알림_뒤에_응답_프레임을_마지막으로_보낸다() {
    픽스처를_적재한다();

    let response = mcp_post(Some(SSE_CONTENT_TYPE), 호출_요청(스트리밍_툴_ID)).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(콘텐츠_타입(&response), SSE_CONTENT_TYPE);

    let frames = sse_프레임들(&본문_문자열(response.into_body()).await);
    assert!(
        frames.len() >= 2,
        "알림 하나와 응답 하나는 최소한 나온다: {frames:?}"
    );

    let last = frames.last().expect("마지막 프레임");
    assert_eq!(last["id"], 1, "응답 프레임은 언제나 마지막이다");
    assert_eq!(last["result"]["isError"], Value::Null);

    for frame in &frames[..frames.len() - 1] {
        assert_eq!(
            frame["method"], METHOD_NOTIFICATIONS_MESSAGE,
            "응답 앞은 전부 로그 알림이다"
        );
    }

    let 출력: String = frames
        .iter()
        .filter(|frame| frame["params"]["data"]["stream"] == "stdout")
        .filter_map(|frame| frame["params"]["data"]["text"].as_str())
        .collect();
    assert_eq!(출력, "out-one\n");
}

#[tokio::test]
async fn sse_응답_프레임은_json_경로의_응답과_같다() {
    픽스처를_적재한다();

    let streamed = {
        let response = mcp_post(Some(SSE_CONTENT_TYPE), 호출_요청(스트리밍_툴_ID)).await;
        let frames = sse_프레임들(&본문_문자열(response.into_body()).await);
        frames.last().expect("응답 프레임").clone()
    };

    let response = mcp_post(None, 호출_요청(스트리밍_툴_ID)).await;
    let buffered: Value =
        serde_json::from_str(&본문_문자열(response.into_body()).await).expect("JSON 하나");

    assert_eq!(streamed, buffered, "전송 방식이 응답을 바꾸지 않는다");
}

#[tokio::test]
async fn sse를_요청하지_않은_post는_예전처럼_json_하나다() {
    픽스처를_적재한다();

    // `*/*` is what curl sends by default: "anything", not "a stream".
    for accept in [None, Some("*/*"), Some("application/json")] {
        let response = mcp_post(accept, 호출_요청(스트리밍_툴_ID)).await;
        assert_eq!(response.status(), StatusCode::OK, "{accept:?}");
        assert!(
            콘텐츠_타입(&response).starts_with("application/json"),
            "{accept:?} → {}",
            콘텐츠_타입(&response)
        );
        let body: Value =
            serde_json::from_str(&본문_문자열(response.into_body()).await).expect("JSON 하나");
        assert_eq!(body["id"], 1, "{accept:?}");
    }
}

#[tokio::test]
async fn 응답이_없는_알림은_스트림을_열지_않는다() {
    let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let response = mcp_post(Some(SSE_CONTENT_TYPE), notification.to_string()).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn initialize는_logging_능력과_세션_id를_돌려준다() {
    let request = json!({ "jsonrpc": "2.0", "id": 7, "method": "initialize" }).to_string();
    let response = mcp_post(None, request).await;

    let session = response
        .headers()
        .get(MCP_SESSION_HEADER)
        .map(|value| value.to_str().unwrap().to_string())
        .expect("initialize는 세션 id를 발급한다");
    assert!(!session.is_empty());

    let body: Value =
        serde_json::from_str(&본문_문자열(response.into_body()).await).expect("JSON 하나");
    assert!(
        body["result"]["capabilities"][LOGGING_CAPABILITY].is_object(),
        "push할 수 있는 lane은 logging을 선언한다: {body}"
    );
}

#[tokio::test]
async fn 세션_바닥을_올리면_그_세션의_진행_알림이_멈춘다() {
    픽스처를_적재한다();

    let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }).to_string();
    let response = mcp_post(None, initialize).await;
    let session = response
        .headers()
        .get(MCP_SESSION_HEADER)
        .expect("세션 id")
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
                .body(Body::from(호출_요청(스트리밍_툴_ID)))
                .unwrap(),
        )
        .await
        .unwrap();
    let frames = sse_프레임들(&본문_문자열(response.into_body()).await);

    assert_eq!(
        frames.len(),
        1,
        "바닥을 warning으로 올렸으면 info 진행 알림은 나오지 않는다: {frames:?}"
    );
    assert_eq!(frames[0]["id"], 1, "남는 것은 tools/call 응답 프레임뿐이다");
}

// === GET /mcp ===

#[tokio::test]
async fn get은_이벤트_스트림을_받지_않겠다면_406이다() {
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
async fn list_changed를_기다린다(mut body: Body, 제한: Duration) -> bool {
    let 마감 = tokio::time::Instant::now() + 제한;
    let mut buffer = String::new();
    loop {
        let 남은 = 마감.saturating_duration_since(tokio::time::Instant::now());
        if 남은.is_zero() {
            return false;
        }
        let Ok(Some(Ok(frame))) = tokio::time::timeout(남은, body.frame()).await else {
            return false;
        };
        if let Some(data) = frame.data_ref() {
            buffer.push_str(&String::from_utf8_lossy(data));
        }
        if sse_프레임들(&buffer)
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
fn get_스트림은_임포트_로드가_끝나면_list_changed를_받는다() {
    let _serial = 페이즈_직렬화();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("테스트 런타임")
        .block_on(get_스트림_시나리오());
}

async fn get_스트림_시나리오() {
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
    assert_eq!(콘텐츠_타입(&response), SSE_CONTENT_TYPE);

    // The handler subscribed before it returned, so a transition
    // published now cannot be missed.
    publish_phase_for_tests(McpImportPhase::Loading);
    publish_phase_for_tests(McpImportPhase::Done(McpImportTally {
        servers_loaded: 1,
        servers_failed: 0,
        tools: 3,
    }));

    let 받았다 = list_changed를_기다린다(response.into_body(), Duration::from_secs(5)).await;
    publish_phase_for_tests(McpImportPhase::NotStarted);

    assert!(받았다, "로드가 끝나면 붙어 있는 클라이언트가 통보를 받는다");
}

// === Accept 해석 ===

fn accept_모양(value: Option<&str>) -> ResponseShape {
    let mut headers = http::HeaderMap::new();
    if let Some(value) = value {
        headers.insert(http::header::ACCEPT, value.parse().unwrap());
    }
    response_shape(&headers)
}

#[test]
fn accept가_이벤트_스트림을_명시할_때만_스트림을_연다() {
    assert_eq!(
        accept_모양(Some(SSE_CONTENT_TYPE)),
        ResponseShape::EventStream
    );
    assert_eq!(
        accept_모양(Some("application/json, text/event-stream")),
        ResponseShape::EventStream
    );
    assert_eq!(
        accept_모양(Some("text/event-stream;q=0.9")),
        ResponseShape::EventStream
    );
    // 와일드카드는 "아무거나"이지 "스트림"이 아니다.
    assert_eq!(accept_모양(Some("*/*")), ResponseShape::Json);
    assert_eq!(accept_모양(Some("application/json")), ResponseShape::Json);
    assert_eq!(accept_모양(None), ResponseShape::Json);
}

// === 예산 ===

fn 패딩_프레임(bytes: usize) -> Value {
    json!({ "jsonrpc": "2.0", "method": METHOD_NOTIFICATIONS_MESSAGE, "pad": "x".repeat(bytes) })
}

#[test]
fn 예산을_넘긴_알림은_버려지고_하나의_표지로_합산_보고된다() {
    let (frames, mut receiver, budget) = frame_queue();
    let 절반 = SSE_QUEUE_BUDGET_BYTES / 2;

    frames.offer(&패딩_프레임(절반));
    // 자리가 없다 — 두 프레임 모두 버려진다.
    frames.offer(&패딩_프레임(절반));
    frames.offer(&패딩_프레임(절반));
    assert_eq!(budget.dropped.load(std::sync::atomic::Ordering::Relaxed), 2);

    // 소비자가 첫 프레임을 읽어 예산을 되돌려 준다.
    let first = receiver.try_recv().expect("첫 프레임");
    budget.release(first.len());

    frames.offer(&패딩_프레임(1));
    let marker: Value = sse_프레임들(&receiver.try_recv().expect("표지"))
        .pop()
        .expect("표지 프레임");
    assert_eq!(marker["params"]["logger"], TRANSPORT_LOGGER);
    assert_eq!(marker["params"]["data"]["dropped"], 2);
    assert_eq!(
        marker["params"]["level"], "warning",
        "잃은 것은 진행 알림보다 높은 심각도로 알린다"
    );

    // 표지 다음이 실제로 자리를 얻은 프레임이다.
    assert!(receiver.try_recv().is_ok());
}

#[test]
fn 표지를_전달하지_못하면_버린_프레임_수가_그대로_남는다() {
    // 손실 수는 표지를 만들기 전에 `swap`으로 **청구**된다 — 두
    // 스레드가 같은 총계를 읽어 각자 보고하면 소비자는 잃은 수의 두
    // 배를 듣는다. 청구한 뒤 전달에 실패하면 되돌려 놓아야 하고, 이
    // 테스트가 그 되돌림을 붙든다.
    let (frames, receiver, budget) = frame_queue();
    frames.offer(&패딩_프레임(SSE_QUEUE_BUDGET_BYTES / 2));
    frames.offer(&패딩_프레임(SSE_QUEUE_BUDGET_BYTES));
    assert_eq!(budget.dropped.load(std::sync::atomic::Ordering::Relaxed), 1);

    drop(receiver);
    frames.flush_dropped();

    assert_eq!(
        budget.dropped.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "보고하지 못한 손실은 사라지지 않는다"
    );
}

#[test]
fn 응답_프레임은_예산이_꽉_차도_반드시_나간다() {
    let (frames, mut receiver, budget) = frame_queue();
    frames.offer(&패딩_프레임(SSE_QUEUE_BUDGET_BYTES / 2));
    frames.offer(&패딩_프레임(SSE_QUEUE_BUDGET_BYTES));

    frames.finish(&json!({ "jsonrpc": "2.0", "id": 1, "result": {} }));

    let mut 프레임들 = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        프레임들.extend(sse_프레임들(&event));
    }
    let last = 프레임들.last().expect("마지막 프레임");
    assert_eq!(last["id"], 1, "응답은 예산과 무관하게 마지막으로 나간다");
    assert!(budget.queued_bytes() > 0, "아직 아무것도 방출되지 않았다");
}

// === OpenAPI ===

#[tokio::test]
async fn openapi는_두_방향의_mcp_경로를_싣는다() {
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
        serde_json::from_str(&본문_문자열(response.into_body()).await).expect("OpenAPI JSON");

    let mcp = &spec["paths"]["/mcp"];
    assert!(
        mcp["post"]["responses"]["200"]["content"][SSE_CONTENT_TYPE].is_object(),
        "POST /mcp의 SSE 표현이 문서에 있어야 한다: {mcp}"
    );
    assert!(
        mcp["post"]["responses"]["200"]["content"]["application/json"].is_object(),
        "JSON 표현도 그대로 남아 있어야 한다: {mcp}"
    );
    assert!(
        mcp["get"]["responses"]["200"]["content"][SSE_CONTENT_TYPE].is_object(),
        "GET /mcp 스트림이 문서에 있어야 한다: {mcp}"
    );
    assert!(
        mcp["get"]["responses"]["406"].is_object(),
        "스트림을 받지 않겠다는 GET의 답도 문서에 있어야 한다: {mcp}"
    );
}

// === 연결 종료와 keep-alive ===

/// Deadlines for the cancellation test. Generous: a loaded CI box can be
/// slow to spawn `sh`, and the test's own failure mode is a 30-second
/// orphan either way.
#[cfg(unix)]
const 자식_등장_대기: Duration = Duration::from_secs(10);
#[cfg(unix)]
const 자식_종료_대기: Duration = Duration::from_secs(10);
#[cfg(unix)]
const 생존_확인_간격: Duration = Duration::from_millis(20);

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
async fn sse_연결을_끊으면_실행_중인_자식이_종료된다() {
    // `POST …/stream`이 오래 지켜온 계약이다. `/mcp`의 SSE도 같은
    // 전송이므로 같은 답을 내야 한다 — 아무도 읽지 않는 10분짜리
    // 빌드는 멈춰야 하는 실행이다.
    픽스처를_적재한다();
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
            "name": 오래_사는_툴_ID,
            "arguments": { "pid_file": pid_path.to_string_lossy() },
        },
    })
    .to_string();

    let response = mcp_post(Some(SSE_CONTENT_TYPE), request).await;
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
        "SSE 연결이 끊겼는데 자식(pid={자식_pid})이 {자식_종료_대기:?} 동안 살아 있었다"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn 조용한_sse_호출도_keep_alive를_흘린다() {
    // 프레임 하나 없이 조용히 도는 호출도 소켓에 바이트를 얹어야
    // 한다. 중간의 idle-timeout 프록시는 조용한 연결과 죽은 연결을
    // 구별하지 못한다.
    픽스처를_적재한다();
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
            "name": 오래_사는_툴_ID,
            "arguments": { "pid_file": pid_path.to_string_lossy() },
        },
    })
    .to_string();

    let response = mcp_post(Some(SSE_CONTENT_TYPE), request).await;
    let mut body = response.into_body();

    // `Interval`의 첫 tick은 즉시 온다. 그러므로 아무 출력도 내지
    // 않는 호출의 첫 바이트는 keep-alive 주석이어야 한다 — 예전에는
    // 도구가 끝날 때까지 소켓에 단 한 바이트도 나가지 않았다.
    let 첫_바이트 = tokio::time::timeout(자식_등장_대기, body.frame())
        .await
        .expect("조용한 호출도 곧바로 바이트를 내야 한다")
        .expect("본문이 끝나면 안 된다")
        .expect("프레임 오류 없음");
    let text = String::from_utf8_lossy(첫_바이트.data_ref().expect("데이터 프레임")).to_string();

    drop(body);
    let 자식_pid = 자식_pid를_기다린다(&pid_path).await;
    let _ = std::process::Command::new("kill")
        .args(["-KILL", &자식_pid.to_string()])
        .status();
    let _ = std::fs::remove_file(&pid_path);

    assert!(
        text.starts_with(SSE_COMMENT_PREFIX),
        "조용한 호출의 첫 바이트는 keep-alive 주석이다: {text:?}"
    );
}

#[tokio::test]
async fn keep_alive_타이머는_밀린_tick을_몰아치지_않는다() {
    // 두 스트림 본문이 같은 타이머를 쓴다. 1분 굶은 스트림이 갚아야
    // 할 것은 keep-alive 하나이지 1분어치가 아니다.
    let timer = super::keepalive_timer();
    assert_eq!(timer.period(), SSE_KEEPALIVE_INTERVAL);
    assert_eq!(
        timer.missed_tick_behavior(),
        tokio::time::MissedTickBehavior::Delay
    );
}

// === 페이즈 리스너 등록부 ===

/// Not `#[tokio::test]`: the registry is process-wide and the phase
/// serialising lock is a blocking guard.
#[test]
fn get_스트림을_닫으면_페이즈_리스너가_등록부에서_사라진다() {
    let _serial = 페이즈_직렬화();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("테스트 런타임")
        .block_on(리스너_누수_시나리오());
}

async fn 리스너_누수_시나리오() {
    let 처음 = phase_listener_count();

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
        // 연결이 끊긴다. 예전에는 여기서 sender가 남아, 다음
        // publish가 있을 때까지 등록부가 계속 자랐다 — publish가
        // 다시는 없는 프로세스에서는 영원히.
        drop(response.into_body());
    }

    assert_eq!(
        phase_listener_count(),
        처음,
        "닫힌 스트림은 등록부에 자기 자리를 남기지 않는다"
    );
}
