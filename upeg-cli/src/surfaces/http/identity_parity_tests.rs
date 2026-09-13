//! `_upeg.surface` and `_upeg.principal.surface` name the same caller,
//! on every route that dispatches.
//!
//! Two answers to one question, resolved from the same headers by two
//! different helpers — which is exactly how they drifted: the buffered
//! routes resolved the request's origin surface while the streaming and
//! trigger routes hard-coded `http`, so an attached `upeg call --stream`
//! produced a call whose envelope said `surface = "http"` and
//! `principal = { operator, cli }`. A tool reading either one got a
//! different caller depending on which it read.
//!
//! This file walks every dispatching route with one fixture and asserts
//! the pair agrees — with and without the attach header, so a route that
//! ignores the header fails as loudly as one that mis-stamps it.

use axum::body::{Body, to_bytes};
use http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt as _;
use upeg_core::{
    EXECUTION_CONTEXT_ARG, EXECUTION_CONTEXT_PRINCIPAL, EXECUTION_CONTEXT_SURFACE, Invoker,
    PRINCIPAL_SURFACE_KEY, PegboardUnits, PinKind, Placement, Source, Surface, ToolId, ToolMeta,
};

use crate::infrastructure::auth::HostTokens;
use crate::surfaces::http::{ORIGIN_SURFACE_HEADER, router_with_tokens};

const HOST_TOKEN: &str = "identity-parity-operator-token";
const TOOLKIT: &str = "parity";
const TOOL_ID: &str = "parity.echo";
const BOARD: &str = "identity-parity-board";

/// The `/mcp` lane's own surface. It is fixed, so the pair must agree
/// there too — and on `mcp`, not on whatever a header says.
const MCP_LANE_SURFACE: Surface = Surface::Mcp;

static 픽스처_등록: std::sync::Once = std::sync::Once::new();

fn 픽스처를_등록한다() {
    픽스처_등록.call_once(|| {
        upeg_runtime::toolbox_add_tool(ToolMeta {
            id: TOOL_ID,
            toolkit: TOOLKIT,
            local_id: ToolId::parse_canonical_in_toolkit(TOOL_ID, TOOLKIT)
                .expect("픽스처 id는 정규형이다")
                .local(),
            tags: &[],
            display_label: "identity parity fixture",
            description: "",
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            source: Source::UserInput,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            // Every surface the routes below can be answered as, so the
            // visibility gate never hides the drift being measured.
            surfaces: upeg_core::ALL_SURFACES,
            boards: &[],
        });
        upeg_runtime::register_single_text_runtime_dispatcher(TOOL_ID, |args| {
            let context = args
                .get(EXECUTION_CONTEXT_ARG)
                .ok_or_else(|| "call carried no execution context".to_string())?;
            Ok(format!(
                "{}|{}",
                context[EXECUTION_CONTEXT_SURFACE]
                    .as_str()
                    .unwrap_or("<none>"),
                context[EXECUTION_CONTEXT_PRINCIPAL][PRINCIPAL_SURFACE_KEY]
                    .as_str()
                    .unwrap_or("<none>"),
            ))
        });
        upeg_runtime::set_trigger_bindings(
            TOOL_ID,
            vec![upeg_runtime::TriggerBinding {
                tool_id: TOOL_ID,
                source: "webhook".into(),
                condition: None,
            }],
        );
    });
}

/// One authenticated `POST`, with the attach header when `origin` names
/// one. Returns the raw response body as text.
async fn post(uri: &str, body: &str, origin: Option<Surface>) -> String {
    let mut request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {HOST_TOKEN}"));
    if let Some(origin) = origin {
        request = request.header(ORIGIN_SURFACE_HEADER, origin.label());
    }
    let response = router_with_tokens(HostTokens::with_agents(HOST_TOKEN, Vec::new()))
        .oneshot(
            request
                .body(Body::from(body.to_string()))
                .expect("유효한 요청"),
        )
        .await
        .expect("라우터 응답");
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("본문 수집");
    String::from_utf8(bytes.to_vec()).expect("본문은 UTF-8이다")
}

/// The `surface|principal.surface` pair the dispatch actually saw, out
/// of a canonical JSON envelope.
fn 봉투에서_짝(body: &str) -> String {
    let envelope: Value = serde_json::from_str(body).expect("정본 JSON 봉투");
    envelope["outputs"][0]["value"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Same, out of an NDJSON stream: the terminal `result` line carries the
/// identical envelope.
fn ndjson에서_짝(body: &str) -> String {
    let result = body
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|line| line["event"] == "result")
        .expect("스트림의 마지막 줄은 언제나 result다");
    result["result"]["outputs"][0]["value"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Same, out of a `/mcp` JSON-RPC response.
fn jsonrpc에서_짝(body: &str) -> String {
    let frame: Value = serde_json::from_str(body).expect("JSON-RPC 응답");
    frame["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn 짝이_같아야_한다(pair: &str, expected: Surface, route: &str) {
    let label = expected.label();
    assert_eq!(
        pair,
        format!("{label}|{label}"),
        "{route}: `_upeg.surface`와 `_upeg.principal.surface`가 같은 호출자를 가리켜야 한다"
    );
}

/// Every `/v1/*` route that dispatches, board-scoped ones included.
///
/// Not `#[tokio::test]`: the board routes need a seeded `UPEG_HOME`, and
/// that helper is a blocking guard.
#[test]
fn 모든_dispatch_라우트에서_surface와_principal_surface가_일치한다() {
    픽스처를_등록한다();
    crate::test_support::with_seeded_pegboard_home(
        "http-identity-parity",
        |state| {
            state.boards.push(upeg_sources::pegboard::BoardData {
                guidance: upeg_core::BoardGuidance::default(),
                key: BOARD.into(),
                title: "Identity Parity".into(),
            });
            state
                .layouts
                .insert(BOARD.into(), vec![Placement::new(TOOL_ID, 0, 0)]);
        },
        || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("테스트 런타임")
                .block_on(라우트_시나리오());
        },
    );
}

async fn 라우트_시나리오() {
    // 헤더가 없으면 전송 surface인 `http`, 있으면 그 사람이 앉아 있는
    // `cli`. 어느 쪽이든 두 값은 같아야 한다.
    for (origin, expected) in [(None, Surface::Http), (Some(Surface::Cli), Surface::Cli)] {
        let route = format!("/v1/tools/{TOOL_ID}");
        짝이_같아야_한다(
            &봉투에서_짝(&post(&route, "{}", origin).await),
            expected,
            &route,
        );

        let route = format!("/v1/tools/{TOOL_ID}/stream");
        짝이_같아야_한다(
            &ndjson에서_짝(&post(&route, "{}", origin).await),
            expected,
            &route,
        );

        let route = format!("/v1/boards/{BOARD}/tools/{TOOL_ID}");
        짝이_같아야_한다(
            &봉투에서_짝(&post(&route, "{}", origin).await),
            expected,
            &route,
        );

        let route = format!("/v1/boards/{BOARD}/tools/{TOOL_ID}/stream");
        짝이_같아야_한다(
            &ndjson에서_짝(&post(&route, "{}", origin).await),
            expected,
            &route,
        );

        let route = format!("/v1/trigger/{TOOL_ID}");
        짝이_같아야_한다(
            &봉투에서_짝(&post(&route, "{}", origin).await),
            expected,
            &route,
        );
    }

    // `/mcp`는 surface가 고정이므로 헤더와 무관하게 `mcp|mcp`다.
    let call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": TOOL_ID, "arguments": {} },
    })
    .to_string();
    for origin in [None, Some(Surface::Cli)] {
        짝이_같아야_한다(
            &jsonrpc에서_짝(&post("/mcp", &call, origin).await),
            MCP_LANE_SURFACE,
            "/mcp",
        );
    }
}
