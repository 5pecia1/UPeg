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
static REGISTER_FIXTURES: std::sync::Once = std::sync::Once::new();

fn register_fixtures() {
    REGISTER_FIXTURES.call_once(|| {
        register(MCP_ONLY_TOOL, &[Surface::Mcp]);
        register(CLI_ONLY_TOOL, &[Surface::Cli]);
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

fn register(id: &'static str, surfaces: &'static [Surface]) {
    upeg_runtime::toolbox_add_tool(ToolMeta {
        id,
        toolkit: TOOLKIT,
        local_id: ToolId::parse_canonical_in_toolkit(id, TOOLKIT)
            .expect("fixture id is canonical")
            .local(),
        tags: &[],
        display_label: "RPC lane fixture",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces,
        boards: &[],
    });
}

fn host_tokens() -> HostTokens {
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
    let response = router_with_tokens(host_tokens())
        .oneshot(
            request
                .body(Body::from(body.to_string()))
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    serde_json::from_slice(&bytes).expect("a single JSON-RPC response")
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

fn exposed_tools(response: &Value) -> Vec<String> {
    response["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(str::to_string))
        .collect()
}

/// The `surface/role/principal-surface` triple the dispatch saw.
fn stamp(response: &Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn mcp_lane_lists_as_mcp_whatever_the_surface_header_says() {
    // This lane's surface is not negotiable. An MCP client is a
    // program, and no header turns it into the surface a person sits
    // at.
    register_fixtures();

    for headers in [
        Vec::new(),
        vec![(RETIRED_SURFACE_HEADER, "cli")],
        vec![(ORIGIN_SURFACE_HEADER, "cli")],
    ] {
        let listed = exposed_tools(&mcp_post(HOST_TOKEN, &headers, tools_list()).await);
        assert!(
            listed.iter().any(|id| id == MCP_ONLY_TOOL),
            "{headers:?} → the mcp tool is always visible: {listed:?}"
        );
        assert!(
            !listed.iter().any(|id| id == CLI_ONLY_TOOL),
            "{headers:?} → the cli-only tool must not leak out: {listed:?}"
        );
    }
}

#[tokio::test]
async fn mcp_lane_surface_header_does_not_unlock_cli_only_tools() {
    // Calls pass the same gate as listing.
    register_fixtures();

    let response = mcp_post(
        HOST_TOKEN,
        &[(RETIRED_SURFACE_HEADER, "cli")],
        tools_call(CLI_ONLY_TOOL),
    )
    .await;

    assert_eq!(
        response["error"]["code"], -32601,
        "an invisible tool is indistinguishable from an unknown method: {response}"
    );
}

#[tokio::test]
async fn mcp_lane_stamps_the_role_the_bearer_proved_into_the_call_envelope() {
    // This lane authenticates, so the envelope must carry the role the
    // token proved — not the in-process stdio lane's `local` default.
    // The surface is fixed to `mcp` for both tokens.
    register_fixtures();

    for (token, role) in [(HOST_TOKEN, "operator"), (AGENT_TOKEN, "agent")] {
        let response = mcp_post(token, &[], tools_call(MCP_ONLY_TOOL)).await;
        assert_eq!(stamp(&response), format!("mcp/{role}/mcp"), "{response}");
    }
}

#[tokio::test]
async fn a_forged_surface_header_cannot_move_the_stamp() {
    // Agent token + `x-upeg-surface: cli`. This combination used to
    // stamp `{operator, cli}` and slip past the chain approval barrier.
    register_fixtures();

    let response = mcp_post(
        AGENT_TOKEN,
        &[
            (RETIRED_SURFACE_HEADER, "cli"),
            (ORIGIN_SURFACE_HEADER, "cli"),
        ],
        tools_call(MCP_ONLY_TOOL),
    )
    .await;

    assert_eq!(stamp(&response), "mcp/agent/mcp", "{response}");
}
