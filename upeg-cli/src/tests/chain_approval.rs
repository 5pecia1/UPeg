//! E-3/B-2 — `upeg call <chain-tool> -a approve=true` /
//! `_upeg.approvedSteps` end-to-end, through the real CLI arg pipeline
//! (`app::args::named_args_from_cli` / `validate_call_args`), not just
//! the loader dispatcher's own approval unit coverage
//! (`upeg-loader/src/tests/dispatcher_chain.rs` — that file proves the
//! loader-side policy works; these tests prove the CLI plumbing that
//! used to strip/reject the approval actually reaches it).
//!
//! The later half of the file covers the two things the arg pipeline
//! cannot prove on its own, because both depend on which *surface* is
//! asking: approval authorization (`_upeg.surface` decides whether a
//! caller-supplied approval is honored) and the per-step summary every
//! chain envelope carries. Those run through the real MCP and HTTP
//! entry points rather than the CLI's, since the CLI can only ever
//! stamp `cli`.

use super::common::parse;
use crate::*;
use std::sync::atomic::{AtomicUsize, Ordering};

const CHAIN_APPROVAL_STEP_KEY: &str = "approved";

static CHAIN_APPROVAL_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Loads a fresh Chain fixture with one `requires_approval` step
/// (`approved` → `text.repeat`) under its OWN toolkit id/directory —
/// these tests run concurrently within one process (shared runtime
/// toolbox), so each call needs an isolated id to avoid racing another
/// call's `remove_dir_all`/load. Returns the fixture's full Tool id.
fn load_chain_approval_fixture() -> String {
    load_chain_approval_fixture_with(None)
}

/// Same fixture with an explicit `approval_surfaces` declaration, for the
/// tests that prove a chain can move the approval boundary off the
/// default (`cli`) onto a surface it names itself.
fn load_chain_approval_fixture_with(approval_surfaces: Option<&str>) -> String {
    let n = CHAIN_APPROVAL_COUNTER.fetch_add(1, Ordering::SeqCst);
    let toolkit = format!("chainapproval{}_{n}", std::process::id());
    let tool_id = format!("{toolkit}.gate");

    let workspace_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("upeg-cli must live under workspace root");
    let dir = workspace_dir
        .join("target/test-tmp/cli-chain-approval")
        .join(&toolkit);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("chain approval fixture dir");
    std::fs::write(
        dir.join("chainapproval.toml"),
        format!(
            r#"id = "{toolkit}"

[[tools]]
id = "gate"
description = "Chain approval CLI fixture (E-3/B-2)"
pin = "Chain"
pegboard_units = "U1"
invoker = "Chain"
surfaces = ["cli", "tui", "desktop", "pwa", "ext", "mcp", "http"]
{approval_surfaces}
[[tools.steps]]
id = "{CHAIN_APPROVAL_STEP_KEY}"
tool = "text.repeat"
requires_approval = true
args = '{{"input":"ok","count":1}}'
"#,
            approval_surfaces = approval_surfaces
                .map_or_else(String::new, |list| format!("approval_surfaces = {list}")),
        ),
    )
    .expect("write chain approval fixture");

    let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
    assert_eq!(
        outcome.failed.len(),
        0,
        "chain approval fixture load failed: {:?}",
        outcome.failed
    );
    assert_eq!(outcome.loaded, vec![tool_id.as_str()]);
    tool_id
}

#[test]
fn a_chain_call_without_approval_returns_a_requires_approval_error() {
    let tool_id = load_chain_approval_fixture();
    let err = run(parse(&["upeg", "call", &tool_id, "--local"]))
        .expect_err("unapproved chain step must fail");
    assert!(
        err.message().contains("requires approval"),
        "got: {}",
        err.message()
    );
}

#[test]
fn a_approve_true_runs_the_chain_approval_step() {
    let tool_id = load_chain_approval_fixture();
    let out = run(parse(&[
        "upeg",
        "call",
        &tool_id,
        "-a",
        "approve=true",
        "--local",
    ]))
    .expect("-a approve=true must approve the gated step");
    assert_eq!(out, "ok\n");
}

#[test]
fn raw_json_approve_true_also_runs_the_chain_approval_step() {
    let tool_id = load_chain_approval_fixture();
    let out = run(parse(&[
        "upeg",
        "call",
        &tool_id,
        r#"{"approve":true}"#,
        "--local",
    ]))
    .expect("raw JSON approve:true must approve the gated step");
    assert_eq!(out, "ok\n");
}

#[test]
fn caller_supplied_upeg_approved_steps_runs_the_chain_approval_step() {
    // FIX A (upeg-runtime/src/execution.rs): a caller-supplied
    // `_upeg.approvedSteps` used to be wiped by
    // `apply_surface_context`'s reserved-block reset before this ever
    // reached the Chain dispatcher.
    let tool_id = load_chain_approval_fixture();
    let raw_args = format!(r#"{{"_upeg":{{"approvedSteps":["{CHAIN_APPROVAL_STEP_KEY}"]}}}}"#);
    let out = run(parse(&["upeg", "call", &tool_id, &raw_args, "--local"]))
        .expect("caller-supplied _upeg.approvedSteps must survive to the chain dispatcher");
    assert_eq!(out, "ok\n");
}

#[test]
fn a_approve_nope_is_a_boolean_error_naming_approve() {
    let tool_id = load_chain_approval_fixture();
    let err = run(parse(&[
        "upeg",
        "call",
        &tool_id,
        "-a",
        "approve=nope",
        "--local",
    ]))
    .expect_err("-a approve=nope must fail boolean coercion");
    assert!(
        err.message().contains("approve") && err.message().contains("boolean"),
        "got: {}",
        err.message()
    );
}

#[test]
fn on_a_non_chain_tool_a_approve_true_is_still_an_unknown_input_error() {
    // text.repeat is a plain Function-invoker builtin — `approve` is
    // not one of its declared inputs and its invoker is not Chain, so
    // ReservedInputs::NONE applies: `approve` stays an ordinary unknown
    // key, same as any other typo'd `-a` name.
    let err = run(parse(&[
        "upeg",
        "call",
        "text.repeat",
        "-a",
        "input=hi",
        "-a",
        "approve=true",
        "--local",
    ]))
    .expect_err("approve must not be recognized on a non-chain tool");
    assert!(
        err.message().contains("unknown input `approve`"),
        "got: {}",
        err.message()
    );
}

#[test]
fn trigger_fire_raw_json_approve_true_also_runs_the_chain_approval_step() {
    // `trigger fire` runs through a different arg pipeline than
    // `upeg call`. Its raw-JSON branch used to hard-code a validator
    // that did not know the reserved inputs, so `{"approve":true}`
    // was rejected as "unknown input".
    let tool_id = load_chain_approval_fixture();
    let out = run(parse(&[
        "upeg",
        "trigger",
        "fire",
        &tool_id,
        r#"{"approve":true}"#,
    ]))
    .expect("trigger fire raw JSON approve:true must also pass the approval step");
    assert_eq!(out, "ok\n");
}

#[test]
fn trigger_fire_a_approve_true_also_runs_the_chain_approval_step() {
    let tool_id = load_chain_approval_fixture();
    let out = run(parse(&[
        "upeg",
        "trigger",
        "fire",
        &tool_id,
        "-a",
        "approve=true",
    ]))
    .expect("trigger fire -a approve=true must also pass the approval step");
    assert_eq!(out, "ok\n");
}

#[test]
fn a_board_scoped_chain_call_also_accepts_a_approve_true() {
    // `board <b> call` runs its own arg pipeline (deferred requiredness
    // until after the pin-preset merge), so it needs its own proof that
    // the reserved `approve` input reaches the dispatcher — the E-3 fix
    // is worthless if the board surface still rejects it.
    let tool_id = load_chain_approval_fixture();
    let board = "chain-approval-board";
    let placement_id: &'static str = Box::leak(tool_id.clone().into_boxed_str());

    let out = crate::test_support::with_seeded_pegboard_home(
        "chain-approval-board",
        |state| {
            state.boards.push(upeg_sources::pegboard::BoardData {
                key: board.into(),
                title: "Chain Approval".into(),
                guidance: upeg_core::BoardGuidance::default(),
            });
            state.layouts.insert(
                board.into(),
                vec![upeg_core::Placement::new(placement_id, 0, 0)],
            );
        },
        || {
            run(parse(&[
                "upeg",
                "board",
                board,
                "call",
                &tool_id,
                "-a",
                "approve=true",
                "--local",
            ]))
        },
    )
    .expect("-a approve=true must pass the approval step under board scope too");

    assert_eq!(out, "ok\n");
}

// ─── Surface authorization ─────────────────────────────────────────

use serde_json::{Value, json};

/// Error code on the failure envelope. An approval denial must go out
/// as an honest typed code so an AI caller can tell "cannot approve
/// here" apart from a retryable failure.
const APPROVAL_DENIED_CODE: &str = "approval_denied_for_surface";

/// Code for when the **caller**, not the surface, is the reason.
/// Widening `approval_surfaces` or entering through a different gate
/// does not change the answer.
const APPROVAL_DENIED_FOR_PRINCIPAL_CODE: &str = "approval_denied_for_principal";

/// Agent token this host issued to a program. Same shape as the
/// operator token; only the privileges differ.
const AGENT_TOKEN: &str = "chain-approval-agent-token";

/// `steps` summary row/key. Output row id on the success envelope and
/// `error.details` key on the failure envelope.
const STEPS_SUMMARY_KEY: &str = "steps";

fn mcp_approval_call(tool_id: &str) -> Value {
    crate::surfaces::mcp::handle(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": tool_id, "arguments": { "approve": true } },
    }))
    .expect("tools/call returns a response")
}

/// `POST /v1/tools/{id}` bearing the operator token. No origin-surface
/// header is attached, so the call is stamped as an `http`-surface
/// operator — exactly what a person hitting the host with `curl` looks
/// like in production.
async fn http_approval_call(tool_id: &str) -> Value {
    http_approval_call_with(
        crate::surfaces::http::router_with_token(HOST_TOKEN),
        tool_id,
        Some(HOST_TOKEN),
        None,
    )
    .await
}

#[test]
fn mcp_surface_approve_true_is_denied_by_surface_authorization() {
    let tool_id = load_chain_approval_fixture();
    let resp = mcp_approval_call(&tool_id);
    let envelope = &resp["result"]["structuredContent"];
    assert_eq!(envelope["error"]["code"], APPROVAL_DENIED_CODE);
    let message = envelope["error"]["message"]
        .as_str()
        .expect("denial message string");
    assert!(
        message.contains("only from cli"),
        "the denial message must say who can approve: {message}"
    );
    assert!(
        message.contains(&format!("upeg call {tool_id} -a approve=true")),
        "the denial message must spell out the command to run instead: {message}"
    );
}

#[tokio::test]
async fn http_surface_approve_true_is_also_denied_by_surface_authorization() {
    // Authenticated with the operator token, so the principal gate
    // passes — the surface is the only remaining reason, and the
    // message must say so.
    let tool_id = load_chain_approval_fixture();
    let envelope = http_approval_call(&tool_id).await;
    assert_eq!(envelope["error"]["code"], APPROVAL_DENIED_CODE);
}

#[tokio::test]
async fn agent_token_request_cannot_approve_even_with_an_origin_surface_header() {
    // This is the whole point of an agent token: even claiming `cli`
    // via header falls back to the `http` surface, and the principal
    // gate answers first anyway.
    let tool_id = load_chain_approval_fixture();
    let envelope = http_approval_call_with(
        crate::surfaces::http::router_with_tokens(
            crate::infrastructure::auth::HostTokens::with_agents(
                HOST_TOKEN,
                vec![AGENT_TOKEN.to_string()],
            ),
        ),
        &tool_id,
        Some(AGENT_TOKEN),
        Some("cli"),
    )
    .await;

    assert_eq!(
        envelope["error"]["code"], APPROVAL_DENIED_FOR_PRINCIPAL_CODE,
        "{envelope}"
    );
    let message = envelope["error"]["message"]
        .as_str()
        .expect("denial message string");
    assert!(
        message.contains("`agent`") && message.contains("operator"),
        "the denial message must say the caller, not the surface, is the reason: {message}"
    );
}

#[tokio::test]
async fn agent_token_request_cannot_approve_even_when_the_chain_authorizes_http() {
    // `approval_surfaces = ["http"]` only widens the gate; it cannot
    // grant a privilege the operator holds back.
    let tool_id = load_chain_approval_fixture_with(Some(r#"["http"]"#));
    let envelope = http_approval_call_with(
        crate::surfaces::http::router_with_tokens(
            crate::infrastructure::auth::HostTokens::with_agents(
                HOST_TOKEN,
                vec![AGENT_TOKEN.to_string()],
            ),
        ),
        &tool_id,
        Some(AGENT_TOKEN),
        None,
    )
    .await;

    assert_eq!(
        envelope["error"]["code"], APPROVAL_DENIED_FOR_PRINCIPAL_CODE,
        "{envelope}"
    );
}

#[tokio::test]
async fn operator_token_request_approves_when_the_chain_authorizes_http() {
    // Same gate, same request body, different token — the only
    // difference between the two tokens.
    let tool_id = load_chain_approval_fixture_with(Some(r#"["http"]"#));
    let envelope = http_approval_call_with(
        crate::surfaces::http::router_with_tokens(
            crate::infrastructure::auth::HostTokens::with_agents(
                HOST_TOKEN,
                vec![AGENT_TOKEN.to_string()],
            ),
        ),
        &tool_id,
        Some(HOST_TOKEN),
        None,
    )
    .await;

    assert_eq!(envelope["ok"], true, "{envelope}");
}

#[test]
fn approval_surfaces_naming_mcp_honors_an_mcp_approval() {
    let tool_id = load_chain_approval_fixture_with(Some(r#"["mcp"]"#));
    let resp = mcp_approval_call(&tool_id);
    assert!(
        resp["result"]["isError"].is_null(),
        "an approval from an explicitly named surface must pass: {resp}"
    );
    assert_eq!(resp["result"]["content"][0]["text"], "ok");
}

// ─── Step metadata (surface parity) ─────────────────────────────────

fn steps_summary_row(envelope: &Value) -> &Value {
    envelope["outputs"]
        .as_array()
        .expect("success envelope outputs array")
        .iter()
        .find(|row| row["id"] == STEPS_SUMMARY_KEY)
        .expect("every successful chain envelope must carry a steps row")
}

#[test]
fn cli_json_envelope_carries_a_steps_summary_row() {
    let tool_id = load_chain_approval_fixture();
    let out = run(parse(&[
        "upeg",
        "call",
        &tool_id,
        "-a",
        "approve=true",
        "--json",
        "--local",
    ]))
    .expect("approved chain must succeed");
    let envelope = serde_json::from_str::<Value>(&out).expect("canonical JSON envelope");
    let row = steps_summary_row(&envelope);
    assert_eq!(row["kind"], "json");
    assert_eq!(row["value"][0]["id"], CHAIN_APPROVAL_STEP_KEY);
    assert_eq!(row["value"][0]["status"], "ran");
    assert_eq!(row["value"][0]["tool"], "text.repeat");
    assert!(row["value"][0]["duration_ms"].is_u64());
}

#[tokio::test]
async fn http_envelope_carries_the_same_steps_summary_row() {
    let tool_id = load_chain_approval_fixture_with(Some(r#"["http"]"#));
    let envelope = http_approval_call(&tool_id).await;
    let row = steps_summary_row(&envelope);
    assert_eq!(row["value"][0]["id"], CHAIN_APPROVAL_STEP_KEY);
    assert_eq!(row["value"][0]["status"], "ran");
}

#[tokio::test]
async fn approval_denial_envelope_carries_denied_status_in_details_steps() {
    let tool_id = load_chain_approval_fixture();
    let envelope = http_approval_call(&tool_id).await;
    let rows = envelope["error"]["details"][STEPS_SUMMARY_KEY]
        .as_array()
        .expect("failure envelope must also carry a steps summary");
    assert_eq!(rows[0]["id"], CHAIN_APPROVAL_STEP_KEY);
    assert_eq!(rows[0]["status"], "denied");
}

// ─── Attach origin-surface header ───────────────────────────────────

/// Host-issued bearer token. The header is honored only on **authenticated**
/// requests, so this branch requires a router configured with a token.
const HOST_TOKEN: &str = "chain-approval-origin-token";

/// Header through which an attach client identifies its local calling surface.
/// `upeg-cli/src/infrastructure/attach.rs` sets the value using `Surface::label()`.
const ORIGIN_SURFACE_HEADER: &str = "x-upeg-origin-surface";

/// Sends `POST /v1/tools/{id}` with the requested headers. A `token` authenticates
/// the request; omitting it exercises rejection of a spoofed surface header.
async fn http_approval_call_with(
    router: axum::Router,
    tool_id: &str,
    token: Option<&str>,
    origin_surface: Option<&str>,
) -> Value {
    use axum::body::{Body, to_bytes};
    use tower::ServiceExt;

    let mut request = http::Request::builder()
        .method("POST")
        .uri(format!("/v1/tools/{tool_id}"))
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    if let Some(surface) = origin_surface {
        request = request.header(ORIGIN_SURFACE_HEADER, surface);
    }
    let response = router
        .oneshot(
            request
                .body(Body::from(r#"{"approve":true}"#))
                .expect("valid request"),
        )
        .await
        .expect("router response");
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body collection");
    serde_json::from_slice(&bytes).expect("canonical JSON envelope")
}

/// Sends one `tools/call` request to `POST /mcp`. Unlike `/v1`, this lane has
/// a fixed surface; no header may claim a different one.
async fn mcp_lane_approval_call(
    router: axum::Router,
    tool_id: &str,
    token: &str,
    surface_header: Option<&str>,
) -> Value {
    use axum::body::{Body, to_bytes};
    use tower::ServiceExt;

    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": tool_id, "arguments": { "approve": true } },
    })
    .to_string();

    let mut request = http::Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"));
    if let Some(surface) = surface_header {
        request = request.header(RETIRED_MCP_SURFACE_HEADER, surface);
    }
    let response = router
        .oneshot(request.body(Body::from(body)).expect("valid request"))
        .await
        .expect("router response");
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body collection");
    let frame: Value = serde_json::from_slice(&bytes).expect("JSON-RPC response");
    frame["result"]["structuredContent"].clone()
}

/// Retired header that `/mcp` must not trust. Keep its spelling literal here:
/// sharing a production constant could hide a regression that restores the
/// same behavior under a different header name.
const RETIRED_MCP_SURFACE_HEADER: &str = "x-upeg-surface";

#[tokio::test]
async fn mcp_lane_agent_token_cannot_approve_even_with_a_surface_header() {
    // `/mcp` must stamp the authenticated principal and ignore surface claims.
    // Otherwise the approval-capable `local` default plus a trusted
    // `x-upeg-surface: cli` header could elevate an agent token to
    // `{operator, cli}` and bypass the default approval boundary.
    let tool_id = load_chain_approval_fixture();
    let envelope = mcp_lane_approval_call(
        crate::surfaces::http::router_with_tokens(
            crate::infrastructure::auth::HostTokens::with_agents(
                HOST_TOKEN,
                vec![AGENT_TOKEN.to_string()],
            ),
        ),
        &tool_id,
        AGENT_TOKEN,
        Some("cli"),
    )
    .await;

    assert_eq!(
        envelope["error"]["code"], APPROVAL_DENIED_FOR_PRINCIPAL_CODE,
        "{envelope}"
    );
}

#[tokio::test]
async fn mcp_lane_operator_token_can_approve_only_when_the_chain_authorizes_mcp() {
    // The principal gate passes; only the surface gate remains, and `mcp`
    // is not an approval surface by default.
    let tool_id = load_chain_approval_fixture();
    let envelope = mcp_lane_approval_call(
        crate::surfaces::http::router_with_token(HOST_TOKEN),
        &tool_id,
        HOST_TOKEN,
        None,
    )
    .await;
    assert_eq!(
        envelope["error"]["code"], APPROVAL_DENIED_CODE,
        "{envelope}"
    );

    // Naming `mcp` in the chain allows the same request with the same token.
    let tool_id = load_chain_approval_fixture_with(Some(r#"["mcp"]"#));
    let envelope = mcp_lane_approval_call(
        crate::surfaces::http::router_with_token(HOST_TOKEN),
        &tool_id,
        HOST_TOKEN,
        None,
    )
    .await;
    assert_eq!(envelope["ok"], true, "{envelope}");
}

#[tokio::test]
async fn mcp_lane_agent_token_cannot_approve_even_when_the_chain_authorizes_mcp() {
    // `approval_surfaces = ["mcp"]` only widens the surface gate; it cannot
    // grant privileges withheld by the operator, exactly as on `/v1`.
    let tool_id = load_chain_approval_fixture_with(Some(r#"["mcp"]"#));
    let envelope = mcp_lane_approval_call(
        crate::surfaces::http::router_with_tokens(
            crate::infrastructure::auth::HostTokens::with_agents(
                HOST_TOKEN,
                vec![AGENT_TOKEN.to_string()],
            ),
        ),
        &tool_id,
        AGENT_TOKEN,
        None,
    )
    .await;

    assert_eq!(
        envelope["error"]["code"], APPROVAL_DENIED_FOR_PRINCIPAL_CODE,
        "{envelope}"
    );
}

#[tokio::test]
async fn authenticated_origin_surface_header_is_honored_for_cli_approval() {
    // This is how `upeg call <chain> -a approve=true` works with a running host.
    // Without the header, attached calls are stamped `http`, preventing
    // operators from approving their own chains from their terminal.
    let tool_id = load_chain_approval_fixture();
    let envelope = http_approval_call_with(
        crate::surfaces::http::router_with_token(HOST_TOKEN),
        &tool_id,
        Some(HOST_TOKEN),
        Some("cli"),
    )
    .await;

    assert_eq!(
        envelope["ok"], true,
        "approval originating from CLI must be honored: {envelope}"
    );
}

#[tokio::test]
async fn no_caller_can_approve_on_a_host_without_authentication() {
    // This bootstrap router requires no token and cannot identify callers.
    // Claiming `cli` in a header must not elevate anyone to operator;
    // without authentication, no caller has that privilege.
    let tool_id = load_chain_approval_fixture();
    let envelope =
        http_approval_call_with(crate::surfaces::http::router(), &tool_id, None, Some("cli")).await;

    assert_eq!(
        envelope["error"]["code"], APPROVAL_DENIED_FOR_PRINCIPAL_CODE,
        "{envelope}"
    );
}

#[tokio::test]
async fn authenticated_requests_ignore_origin_surfaces_outside_the_allowlist() {
    // `desktop` runs in-process through FRB and is not an attach client.
    // The allowlist contains only surfaces that actually attach.
    let tool_id = load_chain_approval_fixture_with(Some(r#"["desktop"]"#));
    let envelope = http_approval_call_with(
        crate::surfaces::http::router_with_token(HOST_TOKEN),
        &tool_id,
        Some(HOST_TOKEN),
        Some("desktop"),
    )
    .await;

    assert_eq!(envelope["error"]["code"], APPROVAL_DENIED_CODE);
    assert!(
        envelope["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("surface `http`")),
        "a value outside the allowlist must fall back to http: {envelope}"
    );
}
