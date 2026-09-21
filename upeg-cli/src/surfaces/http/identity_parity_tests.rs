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

static REGISTER_FIXTURE: std::sync::Once = std::sync::Once::new();

fn register_fixture() {
    REGISTER_FIXTURE.call_once(|| {
        upeg_runtime::toolbox_add_tool(ToolMeta {
            id: TOOL_ID,
            toolkit: TOOLKIT,
            local_id: ToolId::parse_canonical_in_toolkit(TOOL_ID, TOOLKIT)
                .expect("fixture id is canonical")
                .local(),
            tags: &[],
            display_label: "identity parity fixture",
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
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    String::from_utf8(bytes.to_vec()).expect("body is UTF-8")
}

/// The `surface|principal.surface` pair the dispatch actually saw, out
/// of a canonical JSON envelope.
fn pair_from_envelope(body: &str) -> String {
    let envelope: Value = serde_json::from_str(body).expect("canonical JSON envelope");
    envelope["outputs"][0]["value"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Same, out of an NDJSON stream: the terminal `result` line carries the
/// identical envelope.
fn pair_from_ndjson(body: &str) -> String {
    let result = body
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|line| line["event"] == "result")
        .expect("the stream's last line is always a result");
    result["result"]["outputs"][0]["value"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Same, out of a `/mcp` JSON-RPC response.
fn pair_from_jsonrpc(body: &str) -> String {
    let frame: Value = serde_json::from_str(body).expect("JSON-RPC response");
    frame["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn assert_pair_matches(pair: &str, expected: Surface, route: &str) {
    let label = expected.label();
    assert_eq!(
        pair,
        format!("{label}|{label}"),
        "{route}: `_upeg.surface` and `_upeg.principal.surface` must name the same caller"
    );
}

/// Every `/v1/*` route that dispatches, board-scoped ones included.
///
/// Not `#[tokio::test]`: the board routes need a seeded `UPEG_HOME`, and
/// that helper is a blocking guard.
#[test]
fn surface_and_principal_surface_agree_on_every_dispatch_route() {
    register_fixture();
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
                .expect("test runtime")
                .block_on(route_scenario());
        },
    );
}

async fn route_scenario() {
    // Without the header the surface is the transport `http`; with it,
    // the `cli` the person sits at. Either way the two values agree.
    for (origin, expected) in [(None, Surface::Http), (Some(Surface::Cli), Surface::Cli)] {
        let route = format!("/v1/tools/{TOOL_ID}");
        assert_pair_matches(
            &pair_from_envelope(&post(&route, "{}", origin).await),
            expected,
            &route,
        );

        let route = format!("/v1/tools/{TOOL_ID}/stream");
        assert_pair_matches(
            &pair_from_ndjson(&post(&route, "{}", origin).await),
            expected,
            &route,
        );

        let route = format!("/v1/boards/{BOARD}/tools/{TOOL_ID}");
        assert_pair_matches(
            &pair_from_envelope(&post(&route, "{}", origin).await),
            expected,
            &route,
        );

        let route = format!("/v1/boards/{BOARD}/tools/{TOOL_ID}/stream");
        assert_pair_matches(
            &pair_from_ndjson(&post(&route, "{}", origin).await),
            expected,
            &route,
        );

        let route = format!("/v1/trigger/{TOOL_ID}");
        assert_pair_matches(
            &pair_from_envelope(&post(&route, "{}", origin).await),
            expected,
            &route,
        );
    }

    // `/mcp`'s surface is fixed, so it is `mcp|mcp` regardless of the header.
    let call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": TOOL_ID, "arguments": {} },
    })
    .to_string();
    for origin in [None, Some(Surface::Cli)] {
        assert_pair_matches(
            &pair_from_jsonrpc(&post("/mcp", &call, origin).await),
            MCP_LANE_SURFACE,
            "/mcp",
        );
    }
}
