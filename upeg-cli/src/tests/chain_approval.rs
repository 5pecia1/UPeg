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
fn 승인_없는_체인_호출은_승인_요구_오류를_반환한다() {
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
fn a_approve_true는_체인_승인_단계를_실행한다() {
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
fn 원시_json_approve_true도_체인_승인_단계를_실행한다() {
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
fn 호출자가_제공한_upeg_approved_steps는_체인_승인_단계를_실행한다() {
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
fn a_approve_nope는_approve를_지목한_불리언_오류다() {
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
fn 체인이_아닌_도구에서는_a_approve_true가_여전히_알수없는_입력_오류다() {
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
fn trigger_fire의_원시_json_approve_true도_체인_승인_단계를_실행한다() {
    // `trigger fire`는 `upeg call`과 다른 arg 파이프라인을 지나간다.
    // 그 raw-JSON 갈래가 예약 입력을 모르는 검증기를 하드코딩해
    // 부르던 탓에 `{"approve":true}`가 "unknown input"으로 거절됐다.
    let tool_id = load_chain_approval_fixture();
    let out = run(parse(&[
        "upeg",
        "trigger",
        "fire",
        &tool_id,
        r#"{"approve":true}"#,
    ]))
    .expect("trigger fire의 raw JSON approve:true도 승인 단계를 통과해야 한다");
    assert_eq!(out, "ok\n");
}

#[test]
fn trigger_fire의_a_approve_true도_체인_승인_단계를_실행한다() {
    let tool_id = load_chain_approval_fixture();
    let out = run(parse(&[
        "upeg",
        "trigger",
        "fire",
        &tool_id,
        "-a",
        "approve=true",
    ]))
    .expect("trigger fire의 -a approve=true도 승인 단계를 통과해야 한다");
    assert_eq!(out, "ok\n");
}

#[test]
fn 보드_범위_체인_호출도_a_approve_true를_받는다() {
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
    .expect("board 범위에서도 -a approve=true 가 승인 단계를 통과해야 한다");

    assert_eq!(out, "ok\n");
}

// ─── 표면 인가 (surface authorization) ───────────────────────────────

use serde_json::{Value, json};

/// 실패 봉투의 에러 코드. 승인 거부는 정직한 타입 코드로 나가야 AI
/// 호출자가 "여기서는 승인할 수 없다"를 재시도와 구분할 수 있다.
const APPROVAL_DENIED_CODE: &str = "approval_denied_for_surface";

/// 표면이 아니라 **호출자**가 이유일 때의 코드. 이쪽은 다른 문으로
/// 들어와도, `approval_surfaces`를 넓혀도 답이 바뀌지 않는다.
const APPROVAL_DENIED_FOR_PRINCIPAL_CODE: &str = "approval_denied_for_principal";

/// 이 호스트가 프로그램에게 발급한 agent 토큰. operator 토큰과 형태는
/// 같고 권한만 다르다.
const AGENT_TOKEN: &str = "chain-approval-agent-token";

/// `steps` 요약 행/키. 성공 봉투의 output row id이자 실패 봉투의
/// `error.details` 키다.
const STEPS_SUMMARY_KEY: &str = "steps";

fn mcp_승인_호출(tool_id: &str) -> Value {
    crate::surfaces::mcp::handle(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": tool_id, "arguments": { "approve": true } },
    }))
    .expect("tools/call은 응답을 돌려준다")
}

/// operator 토큰을 실은 `POST /v1/tools/{id}`. 원점 surface 헤더는
/// 붙이지 않으므로 호출은 `http` 표면의 operator로 각인된다 — 실제
/// 운영에서 `curl`로 호스트를 부르는 사람의 모습 그대로다.
async fn http_승인_호출(tool_id: &str) -> Value {
    http_승인_호출_with(
        crate::surfaces::http::router_with_token(HOST_TOKEN),
        tool_id,
        Some(HOST_TOKEN),
        None,
    )
    .await
}

#[test]
fn mcp_표면의_approve_true는_표면_인가에서_거부된다() {
    let tool_id = load_chain_approval_fixture();
    let resp = mcp_승인_호출(&tool_id);
    let envelope = &resp["result"]["structuredContent"];
    assert_eq!(envelope["error"]["code"], APPROVAL_DENIED_CODE);
    let message = envelope["error"]["message"]
        .as_str()
        .expect("거부 메시지 문자열");
    assert!(
        message.contains("only from cli"),
        "거부 메시지는 누가 승인할 수 있는지 알려야 한다: {message}"
    );
    assert!(
        message.contains(&format!("upeg call {tool_id} -a approve=true")),
        "거부 메시지는 대신 실행할 명령을 그대로 알려야 한다: {message}"
    );
}

#[tokio::test]
async fn http_표면의_approve_true도_표면_인가에서_거부된다() {
    // operator 토큰으로 인증했으니 주체 게이트는 통과한다 — 남는 이유는
    // 표면 하나뿐이고, 메시지도 그렇게 말해야 한다.
    let tool_id = load_chain_approval_fixture();
    let envelope = http_승인_호출(&tool_id).await;
    assert_eq!(envelope["error"]["code"], APPROVAL_DENIED_CODE);
}

#[tokio::test]
async fn agent_토큰_요청은_원점_surface_헤더를_붙여도_승인하지_못한다() {
    // agent 토큰의 전부다: 헤더로 `cli`를 자칭해도 표면은 `http`로
    // 떨어지고, 그 전에 주체 게이트가 먼저 답한다.
    let tool_id = load_chain_approval_fixture();
    let envelope = http_승인_호출_with(
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
        .expect("거부 메시지 문자열");
    assert!(
        message.contains("`agent`") && message.contains("operator"),
        "거부 메시지는 표면이 아니라 호출자가 이유임을 말해야 한다: {message}"
    );
}

#[tokio::test]
async fn agent_토큰_요청은_체인이_http를_인가해도_승인하지_못한다() {
    // `approval_surfaces = ["http"]`는 문을 넓힐 뿐, operator가 보류한
    // 권한을 만들어 주지는 못한다.
    let tool_id = load_chain_approval_fixture_with(Some(r#"["http"]"#));
    let envelope = http_승인_호출_with(
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
async fn operator_토큰_요청은_체인이_http를_인가하면_승인된다() {
    // 같은 문, 같은 요청 본문, 다른 토큰 — 이것이 두 토큰의 유일한 차이다.
    let tool_id = load_chain_approval_fixture_with(Some(r#"["http"]"#));
    let envelope = http_승인_호출_with(
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
fn approval_surfaces가_mcp를_지목하면_mcp_승인이_인정된다() {
    let tool_id = load_chain_approval_fixture_with(Some(r#"["mcp"]"#));
    let resp = mcp_승인_호출(&tool_id);
    assert!(
        resp["result"]["isError"].is_null(),
        "명시적으로 지목된 surface의 승인은 통과해야 한다: {resp}"
    );
    assert_eq!(resp["result"]["content"][0]["text"], "ok");
}

// ─── step 메타데이터 (surface parity) ────────────────────────────────

fn steps_요약_행(envelope: &Value) -> &Value {
    envelope["outputs"]
        .as_array()
        .expect("성공 봉투의 outputs 배열")
        .iter()
        .find(|row| row["id"] == STEPS_SUMMARY_KEY)
        .expect("모든 chain 성공 봉투는 steps 행을 싣는다")
}

#[test]
fn cli_json_봉투는_step_요약_행을_싣는다() {
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
    .expect("승인된 체인은 성공해야 한다");
    let envelope = serde_json::from_str::<Value>(&out).expect("정본 JSON 봉투");
    let row = steps_요약_행(&envelope);
    assert_eq!(row["kind"], "json");
    assert_eq!(row["value"][0]["id"], CHAIN_APPROVAL_STEP_KEY);
    assert_eq!(row["value"][0]["status"], "ran");
    assert_eq!(row["value"][0]["tool"], "text.repeat");
    assert!(row["value"][0]["duration_ms"].is_u64());
}

#[tokio::test]
async fn http_봉투도_같은_step_요약_행을_싣는다() {
    let tool_id = load_chain_approval_fixture_with(Some(r#"["http"]"#));
    let envelope = http_승인_호출(&tool_id).await;
    let row = steps_요약_행(&envelope);
    assert_eq!(row["value"][0]["id"], CHAIN_APPROVAL_STEP_KEY);
    assert_eq!(row["value"][0]["status"], "ran");
}

#[tokio::test]
async fn 승인_거부_봉투는_details_steps에_denied를_싣는다() {
    let tool_id = load_chain_approval_fixture();
    let envelope = http_승인_호출(&tool_id).await;
    let rows = envelope["error"]["details"][STEPS_SUMMARY_KEY]
        .as_array()
        .expect("실패 봉투도 step 요약을 실어야 한다");
    assert_eq!(rows[0]["id"], CHAIN_APPROVAL_STEP_KEY);
    assert_eq!(rows[0]["status"], "denied");
}

// ─── attach 원점 surface 헤더 ────────────────────────────────────────

/// 호스트가 발급한 bearer 토큰. 헤더는 **인증된** 요청에서만 인정되므로
/// 이 갈래는 토큰이 있는 라우터로만 증명할 수 있다.
const HOST_TOKEN: &str = "chain-approval-origin-token";

/// attach 클라이언트가 "나는 이 로컬 surface에서 부른다"고 밝히는 헤더.
/// 값은 `upeg-cli/src/infrastructure/attach.rs`가 `Surface::label()`로
/// 채운다.
const ORIGIN_SURFACE_HEADER: &str = "x-upeg-origin-surface";

/// `POST /v1/tools/{id}`를 원하는 헤더와 함께 부른다. `token`이 있으면
/// 인증된 요청이 되고, 없으면 위조 헤더가 무시되는지 보이는 갈래가 된다.
async fn http_승인_호출_with(
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
                .expect("유효한 요청"),
        )
        .await
        .expect("라우터 응답");
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("본문 수집");
    serde_json::from_slice(&bytes).expect("정본 JSON 봉투")
}

/// `POST /mcp`의 `tools/call` 한 번. `/v1`과 달리 이 lane의 surface는
/// 고정이므로, 헤더로 자칭할 수 있는 것은 아무것도 없어야 한다.
async fn mcp_lane_승인_호출(
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
        .oneshot(request.body(Body::from(body)).expect("유효한 요청"))
        .await
        .expect("라우터 응답");
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("본문 수집");
    let frame: Value = serde_json::from_slice(&bytes).expect("JSON-RPC 응답");
    frame["result"]["structuredContent"].clone()
}

/// `/mcp`가 예전에 누구에게서든 믿어 주던 헤더. 프로덕션 상수는
/// 사라졌으므로 여기에 그대로 적는다 — 상수를 공유하면 누군가 다른
/// 이름으로 되살렸을 때 이 테스트가 아무것도 증명하지 못한다.
const RETIRED_MCP_SURFACE_HEADER: &str = "x-upeg-surface";

#[tokio::test]
async fn mcp_lane의_agent_토큰은_surface_헤더를_붙여도_승인하지_못한다() {
    // 이것이 이 lane의 구멍이었다: `/mcp`는 주체를 각인하지 않았고
    // (`mcp` surface의 기본값은 승인 가능한 `local`이다), 게다가
    // `x-upeg-surface: cli`를 인증된 아무에게서나 믿었다. 둘이 겹치면
    // agent 토큰 하나로 `{operator, cli}`가 되어 기본 승인 표면을
    // 통과했다.
    let tool_id = load_chain_approval_fixture();
    let envelope = mcp_lane_승인_호출(
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
async fn mcp_lane의_operator_토큰도_체인이_mcp를_인가해야_승인된다() {
    // 주체 게이트는 통과한다 — 남는 것은 표면 게이트 하나뿐이고,
    // `mcp`는 기본 승인 표면이 아니다.
    let tool_id = load_chain_approval_fixture();
    let envelope = mcp_lane_승인_호출(
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

    // 체인이 스스로 `mcp`를 지목하면 같은 토큰의 같은 요청이 통과한다.
    let tool_id = load_chain_approval_fixture_with(Some(r#"["mcp"]"#));
    let envelope = mcp_lane_승인_호출(
        crate::surfaces::http::router_with_token(HOST_TOKEN),
        &tool_id,
        HOST_TOKEN,
        None,
    )
    .await;
    assert_eq!(envelope["ok"], true, "{envelope}");
}

#[tokio::test]
async fn mcp_lane의_agent_토큰은_체인이_mcp를_인가해도_승인하지_못한다() {
    // `approval_surfaces = ["mcp"]`는 문을 넓힐 뿐, operator가 보류한
    // 권한을 만들어 주지 않는다 — `/v1`에서와 정확히 같은 규칙이다.
    let tool_id = load_chain_approval_fixture_with(Some(r#"["mcp"]"#));
    let envelope = mcp_lane_승인_호출(
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
async fn 인증된_요청의_원점_surface_헤더는_cli_승인으로_인정된다() {
    // 호스트가 떠 있을 때 `upeg call <chain> -a approve=true`가 하는 일이다.
    // 헤더가 없으면 attach는 전부 `http`로 각인되어 사람이 자기 터미널에서
    // 자기 체인을 승인하지 못했다.
    let tool_id = load_chain_approval_fixture();
    let envelope = http_승인_호출_with(
        crate::surfaces::http::router_with_token(HOST_TOKEN),
        &tool_id,
        Some(HOST_TOKEN),
        Some("cli"),
    )
    .await;

    assert_eq!(
        envelope["ok"], true,
        "cli에서 온 승인은 인정되어야 한다: {envelope}"
    );
}

#[tokio::test]
async fn 인증하지_않는_호스트에서는_아무도_승인할_수_없다() {
    // 토큰을 요구하지 않는 브링업 라우터다. 헤더만 붙여 `cli`를 자칭하는
    // 시도는 물론이고, 애초에 이 호스트는 누구도 식별하지 못하므로 어떤
    // 호출자도 operator로 승격되지 않는다.
    let tool_id = load_chain_approval_fixture();
    let envelope =
        http_승인_호출_with(crate::surfaces::http::router(), &tool_id, None, Some("cli")).await;

    assert_eq!(
        envelope["error"]["code"], APPROVAL_DENIED_FOR_PRINCIPAL_CODE,
        "{envelope}"
    );
}

#[tokio::test]
async fn 인증되었어도_허용_목록_밖의_원점_surface는_무시된다() {
    // `desktop`은 in-process(FRB)로 돌기 때문에 attach 클라이언트가 아니다.
    // 허용 목록은 실제로 attach하는 surface만 담는다.
    let tool_id = load_chain_approval_fixture_with(Some(r#"["desktop"]"#));
    let envelope = http_승인_호출_with(
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
        "허용 목록 밖의 값은 http로 떨어져야 한다: {envelope}"
    );
}
