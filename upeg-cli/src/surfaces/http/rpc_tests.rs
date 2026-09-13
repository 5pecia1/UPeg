//! What the `/mcp` lane decides before it dispatches: the surface it
//! answers as, and the identity it stamps.

use axum::body::{Body, to_bytes};
use http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt as _;
use upeg_core::{
    EXECUTION_CONTEXT_ARG, EXECUTION_CONTEXT_PRINCIPAL, EXECUTION_CONTEXT_SURFACE, Invoker,
    PRINCIPAL_ROLE_KEY, PRINCIPAL_SURFACE_KEY, PegboardUnits, PinKind, Source, Surface, ToolId,
    ToolMeta,
};

use crate::infrastructure::auth::HostTokens;
use crate::surfaces::http::{ORIGIN_SURFACE_HEADER, router_with_tokens};

/// The header this lane used to honor from any authenticated caller.
///
/// Spelled out rather than imported: the production constant is gone,
/// and a test sharing a name with it would stop proving anything the
/// moment someone reintroduced it under another one.
const RETIRED_SURFACE_HEADER: &str = "x-upeg-surface";

const HOST_TOKEN: &str = "rpc-lane-operator-token";
const AGENT_TOKEN: &str = "rpc-lane-agent-token";

const TOOLKIT: &str = "rpclane";
/// Visible on `mcp`, and echoes the identity the dispatch actually saw.
const MCP_ONLY_TOOL: &str = "rpclane.stamp";
/// Visible on `cli` only — the tool a forged surface header would open.
const CLI_ONLY_TOOL: &str = "rpclane.cli_only";

/// The toolbox is process-global and these tests run in parallel, so the
/// fixtures register exactly once for the whole binary.
static 픽스처_등록: std::sync::Once = std::sync::Once::new();

fn 픽스처를_등록한다() {
    픽스처_등록.call_once(|| {
        등록한다(MCP_ONLY_TOOL, &[Surface::Mcp]);
        등록한다(CLI_ONLY_TOOL, &[Surface::Cli]);
        upeg_runtime::register_single_text_runtime_dispatcher(MCP_ONLY_TOOL, |args| {
            let context = args
                .get(EXECUTION_CONTEXT_ARG)
                .ok_or_else(|| "call carried no execution context".to_string())?;
            let principal = &context[EXECUTION_CONTEXT_PRINCIPAL];
            Ok(format!(
                "{}/{}/{}",
                context[EXECUTION_CONTEXT_SURFACE]
                    .as_str()
                    .unwrap_or("<none>"),
                principal[PRINCIPAL_ROLE_KEY].as_str().unwrap_or("<none>"),
                principal[PRINCIPAL_SURFACE_KEY]
                    .as_str()
                    .unwrap_or("<none>"),
            ))
        });
        upeg_runtime::register_single_text_runtime_dispatcher(CLI_ONLY_TOOL, |_| {
            Ok("would-have-run".to_string())
        });
    });
}

fn 등록한다(id: &'static str, surfaces: &'static [Surface]) {
    upeg_runtime::toolbox_add_tool(ToolMeta {
        id,
        toolkit: TOOLKIT,
        local_id: ToolId::parse_canonical_in_toolkit(id, TOOLKIT)
            .expect("픽스처 id는 정규형이다")
            .local(),
        tags: &[],
        display_label: "RPC lane fixture",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces,
        boards: &[],
    });
}

fn 호스트_토큰들() -> HostTokens {
    HostTokens::with_agents(HOST_TOKEN, vec![AGENT_TOKEN.to_string()])
}

/// One `POST /mcp`, authenticated with `token` and carrying whatever
/// surface-ish headers the caller wants to forge.
async fn mcp_post(token: &str, headers: &[(&str, &str)], body: Value) -> Value {
    let mut request = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"));
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = router_with_tokens(호스트_토큰들())
        .oneshot(
            request
                .body(Body::from(body.to_string()))
                .expect("유효한 요청"),
        )
        .await
        .expect("라우터 응답");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("본문 수집");
    serde_json::from_slice(&bytes).expect("JSON-RPC 응답 하나")
}

fn tools_list() -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })
}

fn tools_call(tool: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": { "name": tool, "arguments": {} },
    })
}

fn 노출된_도구들(response: &Value) -> Vec<String> {
    response["result"]["tools"]
        .as_array()
        .expect("tools 배열")
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(str::to_string))
        .collect()
}

/// The `surface/role/principal-surface` triple the dispatch saw.
fn 각인(response: &Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn mcp_lane은_surface_헤더가_무엇이든_mcp로_열거한다() {
    // 이 lane의 surface는 협상 대상이 아니다. MCP 클라이언트는
    // 프로그램이고, 어떤 헤더도 그것을 사람이 앉아 있는 surface로
    // 바꾸지 못한다.
    픽스처를_등록한다();

    for headers in [
        Vec::new(),
        vec![(RETIRED_SURFACE_HEADER, "cli")],
        vec![(ORIGIN_SURFACE_HEADER, "cli")],
    ] {
        let listed = 노출된_도구들(&mcp_post(HOST_TOKEN, &headers, tools_list()).await);
        assert!(
            listed.iter().any(|id| id == MCP_ONLY_TOOL),
            "{headers:?} → mcp 도구는 언제나 보인다: {listed:?}"
        );
        assert!(
            !listed.iter().any(|id| id == CLI_ONLY_TOOL),
            "{headers:?} → cli 전용 도구가 새어나오면 안 된다: {listed:?}"
        );
    }
}

#[tokio::test]
async fn mcp_lane의_surface_헤더는_cli_전용_도구를_열어주지_않는다() {
    // 열거뿐 아니라 호출도 같은 게이트를 지난다.
    픽스처를_등록한다();

    let response = mcp_post(
        HOST_TOKEN,
        &[(RETIRED_SURFACE_HEADER, "cli")],
        tools_call(CLI_ONLY_TOOL),
    )
    .await;

    assert_eq!(
        response["error"]["code"], -32601,
        "보이지 않는 도구는 알 수 없는 메서드와 구별되지 않는다: {response}"
    );
}

#[tokio::test]
async fn mcp_lane은_bearer가_증명한_역할을_호출_봉투에_각인한다() {
    // 이 lane은 인증한다. 그러므로 in-process stdio lane의 기본값인
    // `local`이 아니라, 토큰이 증명한 역할이 봉투에 실려야 한다.
    // surface는 두 토큰 모두 `mcp`로 고정된다.
    픽스처를_등록한다();

    for (token, role) in [(HOST_TOKEN, "operator"), (AGENT_TOKEN, "agent")] {
        let response = mcp_post(token, &[], tools_call(MCP_ONLY_TOOL)).await;
        assert_eq!(각인(&response), format!("mcp/{role}/mcp"), "{response}");
    }
}

#[tokio::test]
async fn 위조된_surface_헤더도_각인을_움직이지_못한다() {
    // agent 토큰 + `x-upeg-surface: cli`. 예전에는 이 조합이
    // `{operator, cli}`를 각인해 체인 승인 장벽까지 넘겼다.
    픽스처를_등록한다();

    let response = mcp_post(
        AGENT_TOKEN,
        &[
            (RETIRED_SURFACE_HEADER, "cli"),
            (ORIGIN_SURFACE_HEADER, "cli"),
        ],
        tools_call(MCP_ONLY_TOOL),
    )
    .await;

    assert_eq!(각인(&response), "mcp/agent/mcp", "{response}");
}
