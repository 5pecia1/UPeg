use std::sync::mpsc::Receiver;
use std::thread;

use super::*;
use crate::api::embed::{BindingRoleDto, BindingWaitConditionDto};
use crate::api::tools::{CanonicalOutputValue, dispatch_tool_impl};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);
const TEST_URL: &str = "https://example.com/shared-webview";

#[test]
fn 긴_바인딩_파이프라인은_모든_대기와_안정화_시간을_보장한다() {
    const BINDING_COUNT: usize = 10;
    let binding = upeg_core::SelectorBinding {
        role: upeg_core::BindingRole::Output,
        field: "answer".into(),
        selector: "#answer".into(),
        trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
        wait: Some(upeg_core::BindingWait {
            timeout_ms: upeg_core::MAX_CONTROLLED_EMBED_WAIT_MS,
            settle_ms: upeg_core::MAX_CONTROLLED_EMBED_WAIT_MS,
            ..Default::default()
        }),
    };
    let bindings = vec![binding; BINDING_COUNT];
    let mut run = request("test.long_pipeline");
    run.bindings = &bindings;
    let declared_wait = Duration::from_millis(upeg_core::MAX_CONTROLLED_EMBED_WAIT_MS);
    assert_eq!(
        response_timeout(&run),
        BASE_RESPONSE_TIMEOUT + declared_wait * (BINDING_COUNT as u32 * 2)
    );
    assert_eq!(
        response_timeout(&request("test.no_wait")),
        BASE_RESPONSE_TIMEOUT
    );
}

#[test]
fn 같은_도구의_대기_요청은_선행_실행의_시간을_추가로_확보한다() {
    let (bridge, id, events) = connected_bridge();
    let mut runs = Vec::new();
    for tool in ["test.queued", "test.queued", "test.independent"] {
        let worker = Arc::clone(&bridge);
        let handle = thread::spawn(move || worker.run_with_timeout(request(tool), TEST_TIMEOUT));
        let run = next_request(&events);
        runs.push((run, handle));
    }
    {
        let state = bridge.state().unwrap();
        let pending = &state.as_ref().unwrap().pending;
        let deadline = |index: usize| pending[&RequestId(runs[index].0.request_id)].deadline;
        assert_eq!(deadline(1).duration_since(deadline(0)), TEST_TIMEOUT);
        assert!(deadline(2) < deadline(1));
    }
    for (run, handle) in runs {
        assert!(bridge.complete(id, run.request_id, success("ok")));
        assert!(handle.join().unwrap().is_ok());
    }
    assert!(bridge.state().unwrap().as_ref().unwrap().pending.is_empty());
}

#[test]
fn 표현할_수_없는_응답_기한은_패닉이나_실행_없이_거절한다() {
    let (bridge, _, events) = connected_bridge();
    assert!(matches!(
        bridge.run_with_timeout(request("test.overflow"), Duration::MAX),
        Err(ControlledEmbedError::BackendFailed(_))
    ));
    assert!(events.try_recv().is_err());
    assert!(bridge.state().unwrap().as_ref().unwrap().pending.is_empty());
}

fn request(tool_id: &str) -> ControlledEmbedRequest<'_> {
    ControlledEmbedRequest {
        tool_id,
        url: TEST_URL,
        bindings: &[],
        inputs: &[("query", "upeg")],
        settings: upeg_core::ControlledEmbedSettings::default(),
    }
}

fn connected_bridge() -> (Arc<WebViewBridge>, u64, Receiver<WebViewExecutionEventDto>) {
    let bridge = Arc::new(WebViewBridge::new());
    let id = bridge.reserve().expect("프로바이더 예약");
    let (sender, receiver) = mpsc::channel();
    bridge
        .attach(id, Arc::new(move |event| sender.send(event).is_ok()))
        .expect("스트림 연결");
    assert!(matches!(
        receiver.recv_timeout(TEST_TIMEOUT).unwrap(),
        WebViewExecutionEventDto::Ready
    ));
    (bridge, id, receiver)
}

fn next_request(receiver: &Receiver<WebViewExecutionEventDto>) -> WebViewExecutionRequestDto {
    match receiver.recv_timeout(TEST_TIMEOUT).expect("실행 요청") {
        WebViewExecutionEventDto::Execute { request } => request,
        event => panic!("실행 요청이어야 한다: {event:?}"),
    }
}

fn success(value: &str) -> WebViewExecutionCompletionDto {
    WebViewExecutionCompletionDto::Success {
        outputs: vec![("answer".to_string(), value.to_string())],
    }
}

#[test]
fn 동시에_실행한_요청은_역순으로_완료해도_자기_결과를_받는다() {
    let (bridge, id, events) = connected_bridge();
    let handles: Vec<_> = ["test.first", "test.second"]
        .into_iter()
        .map(|tool| {
            let bridge = Arc::clone(&bridge);
            (
                tool,
                thread::spawn(move || bridge.run_with_timeout(request(tool), TEST_TIMEOUT)),
            )
        })
        .collect();
    let first = next_request(&events);
    let second = next_request(&events);
    assert_ne!(first.request_id, second.request_id);
    assert_eq!(first.url, TEST_URL);
    assert_eq!(
        first.inputs,
        vec![("query".to_string(), "upeg".to_string())]
    );
    assert!(bridge.complete(id, second.request_id, success(&second.tool_id)));
    assert!(bridge.complete(id, first.request_id, success(&first.tool_id)));
    assert!(!bridge.complete(id, first.request_id, success("late")));
    for (tool, handle) in handles {
        assert_eq!(
            handle.join().unwrap().unwrap().outputs,
            vec![("answer".to_string(), tool.to_string())]
        );
    }
}

#[test]
fn 셀렉터_대기_시간초과는_역할과_조건과_오류코드를_보존한다() {
    let (bridge, id, events) = connected_bridge();
    let worker = Arc::clone(&bridge);
    let handle = thread::spawn(move || worker.run_with_timeout(request("test.wait"), TEST_TIMEOUT));
    let run = next_request(&events);
    const WAIT_MS: u64 = 1234;
    assert!(bridge.complete(
        id,
        run.request_id,
        WebViewExecutionCompletionDto::WaitTimeout {
            role: BindingRoleDto::Output,
            selector: "#answer".to_string(),
            for_selector: "#ready".to_string(),
            condition: BindingWaitConditionDto::Visible,
            timeout_ms: WAIT_MS,
        }
    ));
    let error = handle.join().unwrap().unwrap_err();
    assert_eq!(error.code(), "wait-timeout");
    assert_eq!(
        error,
        ControlledEmbedError::WaitTimeout {
            role: upeg_core::BindingRole::Output,
            selector: "#answer".to_string(),
            for_selector: "#ready".to_string(),
            condition: upeg_core::BindingWaitCondition::Visible,
            timeout_ms: WAIT_MS,
        }
    );
}

#[test]
fn 웹뷰의_실행실패와_사용자_취소는_서로_다른_오류로_돌아온다() {
    let (bridge, id, events) = connected_bridge();
    for (completion, expected) in [
        (
            WebViewExecutionCompletionDto::Failed {
                message: "javascript evaluation failed".to_string(),
            },
            ControlledEmbedError::BackendFailed("javascript evaluation failed".to_string()),
        ),
        (
            WebViewExecutionCompletionDto::Cancelled,
            ControlledEmbedError::Cancelled,
        ),
    ] {
        let worker = Arc::clone(&bridge);
        let handle =
            thread::spawn(move || worker.run_with_timeout(request("test.error"), TEST_TIMEOUT));
        let run = next_request(&events);
        assert!(bridge.complete(id, run.request_id, completion));
        assert_eq!(handle.join().unwrap(), Err(expected));
    }
}

#[test]
fn 프로바이더_교체는_대기중인_실행을_깨우고_이전_세대의_응답과_해제를_거부한다() {
    let (bridge, previous_id, events) = connected_bridge();
    let worker = Arc::clone(&bridge);
    let handle =
        thread::spawn(move || worker.run_with_timeout(request("test.disconnected"), TEST_TIMEOUT));
    let run = next_request(&events);
    let current_id = bridge.reserve().unwrap();
    assert_ne!(current_id, previous_id);
    assert!(matches!(
        events.recv_timeout(TEST_TIMEOUT).unwrap(),
        WebViewExecutionEventDto::Cancel { request_id } if request_id == run.request_id
    ));
    assert_eq!(
        handle.join().unwrap().unwrap_err().code(),
        "controlled_embed_unavailable"
    );
    assert!(!bridge.complete(previous_id, run.request_id, success("late")));
    assert!(!bridge.unregister(previous_id));
    assert!(bridge.attach(current_id, Arc::new(|_| true)).is_ok());
    assert!(bridge.unregister(current_id));
    assert_eq!(
        bridge.run(request("test.unavailable")).unwrap_err().code(),
        "controlled_embed_unavailable"
    );
}

#[test]
fn 프로바이더_해제는_아직_연결된_웹뷰의_진행중인_요청을_취소한다() {
    let (bridge, provider_id, events) = connected_bridge();
    let worker = Arc::clone(&bridge);
    let handle =
        thread::spawn(move || worker.run_with_timeout(request("test.unregister"), TEST_TIMEOUT));
    let run = next_request(&events);

    assert!(bridge.unregister(provider_id));

    assert!(matches!(
        events.recv_timeout(TEST_TIMEOUT).unwrap(),
        WebViewExecutionEventDto::Cancel { request_id } if request_id == run.request_id
    ));
    assert_eq!(
        handle.join().unwrap().unwrap_err().code(),
        "controlled_embed_unavailable"
    );
    assert!(!bridge.complete(provider_id, run.request_id, success("late")));
}

#[test]
fn 구독전에_폐기한_프로바이더는_늦게_연결돼도_복구되지_않는다() {
    let bridge = WebViewBridge::new();
    let id = bridge.reserve().unwrap();
    assert_eq!(
        bridge.run(request("test.not_ready")).unwrap_err().code(),
        "controlled_embed_unavailable"
    );
    assert!(bridge.unregister(id));
    assert!(matches!(
        bridge.attach(id, Arc::new(|_| true)),
        Err(FrbError::HostUnavailable)
    ));
}

#[test]
fn 취소는_웹뷰에도_전달하고_늦게_돌아온_출력을_버린다() {
    let (bridge, id, events) = connected_bridge();
    let token = upeg_runtime::CancellationToken::new();
    let worker_token = token.clone();
    let worker = Arc::clone(&bridge);
    let handle = thread::spawn(move || {
        upeg_runtime::with_cancellation(worker_token, || {
            worker.run_with_timeout(request("test.cancel"), TEST_TIMEOUT)
        })
    });
    let run = next_request(&events);
    token.cancel();
    assert_eq!(handle.join().unwrap(), Err(ControlledEmbedError::Cancelled));
    assert!(
        matches!(events.recv_timeout(TEST_TIMEOUT).unwrap(), WebViewExecutionEventDto::Cancel { request_id } if request_id == run.request_id)
    );
    assert!(!bridge.complete(id, run.request_id, success("late")));
}

#[test]
fn 응답이_없는_웹뷰는_제한시간에_취소하고_바인딩_시간초과와_구분한다() {
    const SHORT_TIMEOUT: Duration = Duration::from_millis(10);
    let (bridge, id, events) = connected_bridge();
    let result = bridge.run_with_timeout(request("test.timeout"), SHORT_TIMEOUT);
    let run = next_request(&events);
    assert_eq!(
        result.unwrap_err().code(),
        "controlled_embed_execution_timeout"
    );
    assert!(
        matches!(events.recv_timeout(TEST_TIMEOUT).unwrap(), WebViewExecutionEventDto::Cancel { request_id } if request_id == run.request_id)
    );
    assert!(!bridge.complete(id, run.request_id, success("late")));
}

#[test]
fn 연결이_끊긴_스트림은_새_호출을_실행하지_않고_사용불가를_반환한다() {
    let (bridge, _, events) = connected_bridge();
    drop(events);
    assert_eq!(
        bridge.run(request("test.closed")).unwrap_err().code(),
        "controlled_embed_unavailable"
    );
    assert!(bridge.state().unwrap().is_none());
}

/// Route only this test's unique tool to the bridge; leave unrelated tests'
/// dispatchers on the prior backend while the real router is exercised.
struct ScopedBackend {
    tool_id: String,
    bridge: Arc<WebViewBridge>,
    previous: Arc<dyn ControlledEmbedBackend>,
}

impl ControlledEmbedBackend for ScopedBackend {
    fn run(&self, request: ControlledEmbedRequest<'_>) -> ExecutionResult {
        if request.tool_id == self.tool_id {
            self.bridge.run_with_timeout(request, TEST_TIMEOUT)
        } else {
            self.previous.run(request)
        }
    }
}

struct RestoreBackend(Arc<dyn ControlledEmbedBackend>);

impl Drop for RestoreBackend {
    fn drop(&mut self) {
        set_controlled_embed_backend(Arc::clone(&self.0));
    }
}

#[test]
fn 데스크톱과_http는_같은_웹뷰_요청과_출력타입과_대표결과를_사용한다() {
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let _guard = crate::api::test_support::host_lock().lock().unwrap();
    const TOOLKIT: &str = "frb_webview_integration";
    const TOOL_ID: &str = "frb_webview_integration.shared";
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("shared.toml"), format!(r##"
id = "{TOOLKIT}"
[[tools]]
id = "shared"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "{TEST_URL}"
surfaces = ["desktop", "http", "cli"]
primary_output_id = "answer"
inputs = [{{ name = "query", type = "string", required = true }}]
outputs = [{{ name = "summary", type = "string" }}, {{ name = "answer", type = "integer", label = "Answer" }}]
controlled_embed = {{ bindings = [
  {{ role = "input", field = "query", selector = "#query" }},
  {{ role = "trigger", field = "", selector = "#go" }},
  {{ role = "output", field = "summary", selector = "#summary" }},
  {{ role = "output", field = "answer", selector = "#answer" }},
] }}
"##)).unwrap();
    let loaded = upeg_loader::load_and_register_dir_verbose(dir.path());
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    assert_eq!(loaded.loaded, vec![TOOL_ID]);
    let (bridge, provider_id, events) = connected_bridge();
    let previous = upeg_runtime::controlled_embed::controlled_embed_backend();
    let _restore = RestoreBackend(Arc::clone(&previous));
    set_controlled_embed_backend(Arc::new(ScopedBackend {
        tool_id: TOOL_ID.to_string(),
        bridge: Arc::clone(&bridge),
        previous,
    }));

    let responder = thread::spawn(move || {
        for answer in ["42", "42", "not-an-integer"] {
            let run = next_request(&events);
            assert_eq!(run.tool_id, TOOL_ID);
            assert_eq!(run.url, TEST_URL);
            assert!(
                run.inputs
                    .contains(&("query".to_string(), "upeg".to_string()))
            );
            assert_eq!(run.bindings.len(), 4);
            assert!(bridge.complete(
                provider_id,
                run.request_id,
                WebViewExecutionCompletionDto::Success {
                    outputs: vec![
                        ("summary".to_string(), "result".to_string()),
                        ("answer".to_string(), answer.to_string())
                    ],
                }
            ));
        }
    });
    let desktop = dispatch_tool_impl(TOOL_ID, r#"{"query":"upeg"}"#, None, false);
    assert!(desktop.ok, "{:?}", desktop.error);
    assert_eq!(desktop.primary_output_id.as_deref(), Some("answer"));
    assert_eq!(desktop.outputs[1].kind, "integer");
    assert_eq!(
        desktop.outputs[1].value,
        CanonicalOutputValue::Integer { value: 42 }
    );

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let http: serde_json::Value = runtime.block_on(async {
        let response = upeg_cli::http_router()
            .oneshot(
                Request::post(format!("/v1/tools/{TOOL_ID}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"query":"upeg"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    });
    assert_eq!(http["ok"], true);
    assert_eq!(http["primary_output_id"], "answer");
    assert_eq!(http["outputs"][1]["id"], "answer");
    assert_eq!(http["outputs"][1]["kind"], "integer");
    assert_eq!(http["outputs"][1]["value"], 42);

    let invalid = crate::api::webview::normalize_webview_result(
        TOOL_ID.to_string(),
        WebViewExecutionCompletionDto::Success {
            outputs: vec![
                ("summary".to_string(), "result".to_string()),
                ("answer".to_string(), "not-an-integer".to_string()),
            ],
        },
    );
    let dispatched = dispatch_tool_impl(TOOL_ID, r#"{"query":"upeg"}"#, None, false);
    assert_eq!(invalid, dispatched);
    assert_eq!(invalid.error.unwrap().code, "output_conversion_error");
    responder.join().unwrap();
}

#[test]
fn 정규화는_도구를_실행하지_않고_출력_타입과_라벨과_대표값과_오류를_보존한다() {
    use crate::api::webview::normalize_webview_result;
    const TOOL_ID: &str = "frb_webview_normalize.result";
    let (toolkit, mut tools) = upeg_loader::parse_toolkit_full(r##"
id = "frb_webview_normalize"
[[tools]]
id = "result"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "https://example.com/normalization"
primary_output_id = "answer"
outputs = [{ name = "summary", type = "string" }, { name = "answer", type = "integer", label = "Answer" }]
controlled_embed = { bindings = [
  { role = "input", field = "query", selector = "#query" },
  { role = "trigger", field = "", selector = "#go" },
  { role = "output", field = "summary", selector = "#summary" },
  { role = "output", field = "answer", selector = "#answer" },
] }
"##).unwrap();
    let (meta, _) = tools.pop().unwrap();
    let calls = Arc::new(AtomicU64::new(0));
    let executed = Arc::clone(&calls);
    upeg_runtime::toolbox_add_toolkit(toolkit);
    upeg_runtime::toolbox_add_tool_with_dispatcher(meta, move |_| {
        executed.fetch_add(1, Ordering::Relaxed);
        upeg_runtime::tool_failure(
            "unexpected_dispatch",
            "normalization must not execute a tool",
        )
    });
    let result = normalize_webview_result(
        TOOL_ID.to_string(),
        WebViewExecutionCompletionDto::Success {
            outputs: vec![
                ("summary".to_string(), "result".to_string()),
                ("answer".to_string(), "42".to_string()),
            ],
        },
    );
    assert!(result.ok);
    assert_eq!(result.primary_output_id.as_deref(), Some("answer"));
    assert_eq!(result.outputs[1].label.as_deref(), Some("Answer"));
    assert_eq!(result.outputs[1].kind, "integer");
    assert_eq!(
        result.outputs[1].value,
        CanonicalOutputValue::Integer { value: 42 }
    );

    let invalid = normalize_webview_result(TOOL_ID.to_string(), success("not-an-integer"));
    assert!(!invalid.ok);
    assert!(invalid.outputs.is_empty());
    assert_eq!(invalid.error.unwrap().code, "output_conversion_error");

    for error in [
        ControlledEmbedError::Unavailable {
            reason: "provider disconnected".to_string(),
        },
        ControlledEmbedError::Cancelled,
        ControlledEmbedError::BackendFailed("script failed".to_string()),
        ControlledEmbedError::ResponseTimeout { timeout_ms: 123 },
        ControlledEmbedError::WaitTimeout {
            role: upeg_core::BindingRole::Output,
            selector: "#answer".to_string(),
            for_selector: "#ready".to_string(),
            condition: upeg_core::BindingWaitCondition::Visible,
            timeout_ms: 456,
        },
    ] {
        let expected_code = error.code();
        let expected_message = error.to_string();
        let result = upeg_loader::normalize_controlled_embed_result(TOOL_ID, Err(error));
        let upeg_core::ToolResult::Failure(failure) = result else {
            panic!("오류여야 한다")
        };
        assert_eq!(failure.error.code, expected_code);
        assert_eq!(failure.error.message, expected_message);
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn 등록되지_않은_도구의_정규화는_조회_오류를_반환한다() {
    let result = crate::api::webview::normalize_webview_result(
        "frb_webview_normalize.missing".to_string(),
        success("42"),
    );
    assert!(!result.ok);
    assert_eq!(result.error.unwrap().code, "tool_not_found");
}
