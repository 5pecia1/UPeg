//! The GUI dispatch path against a real Chain approval barrier.
//!
//! `shape_approval_arg`'s unit tests prove the two reserved levers leave
//! the args map. This file proves the thing that matters: with them
//! gone, a caller-built args map cannot lift a barrier the person never
//! answered — and with the typed flag set, the same call runs. Both
//! halves are needed; one alone would pass on a dispatch path that had
//! stopped gating at all.
//!
//! Native only. The barrier is built by the loader at registration time,
//! and `upeg-loader` is a native-only dev dependency (see `Cargo.toml`).

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

/// The chain's gated step key, named in `_upeg.approvedSteps` by the
/// test that tries to self-approve.
const GATED_STEP_KEY: &str = "gated";
/// The refusal a barrier nobody lifted produces
/// (`upeg-loader/src/dispatcher/chain/approval.rs`).
const APPROVAL_REQUIRED_CODE: &str = "approval_required";

static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Load a fresh gated Chain under its own toolkit id.
///
/// Per-test ids because the toolbox is process-global and these tests
/// run in parallel — a shared directory would race another test's
/// `remove_dir_all`.
fn 게이트된_체인을_적재한다() -> String {
    upeg_tools::register_all();
    let n = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let toolkit = format!("frbchain{}_{n}", std::process::id());
    let tool_id = format!("{toolkit}.gate");

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("upeg-frb는 워크스페이스 루트 아래에 있다")
        .join("target/test-tmp/frb-chain-approval")
        .join(&toolkit);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("체인 픽스처 디렉터리");
    std::fs::write(
        dir.join("frbchain.toml"),
        format!(
            r#"id = "{toolkit}"

[[tools]]
id = "gate"
description = "FRB chain approval fixture"
pin = "Chain"
pegboard_units = "U1"
invoker = "Chain"
surfaces = ["cli", "tui", "desktop", "pwa", "ext", "mcp", "http"]

[[tools.steps]]
id = "{GATED_STEP_KEY}"
tool = "text.repeat"
requires_approval = true
args = '{{"input":"ok","count":1}}'
"#
        ),
    )
    .expect("체인 픽스처 TOML");

    let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
    assert!(
        outcome.failed.is_empty(),
        "체인 픽스처는 실패 없이 적재된다: {:?}",
        outcome.failed
    );
    let _ = std::fs::remove_dir_all(&dir);
    tool_id
}

fn 오류_코드(outcome: &CanonicalToolResult) -> Option<&str> {
    outcome.error.as_ref().map(|error| error.code.as_str())
}

#[test]
fn args의_approved_steps는_게이트된_체인을_스스로_승인하지_못한다() {
    // `upeg://open?tool=…&input={"_upeg":{"approvedSteps":["gated"]}}`
    // 가 하던 일이다. desktop은 기본 승인 표면이고 `approvedSteps`는
    // 예약 블록 wipe에서 살아남는 유일한 키라, 링크 하나가 사람이
    // 답한 적 없는 장벽을 넘었다.
    let tool_id = 게이트된_체인을_적재한다();
    let args = format!(
        r#"{{"_upeg":{{"{}":["{GATED_STEP_KEY}"]}}}}"#,
        upeg_core::EXECUTION_CONTEXT_APPROVED_STEPS
    );

    let outcome = dispatch_tool_impl(&tool_id, &args, None, false);

    assert!(!outcome.ok, "{outcome:?}");
    assert_eq!(오류_코드(&outcome), Some(APPROVAL_REQUIRED_CODE));
}

#[test]
fn args의_approve_키도_게이트된_체인을_스스로_승인하지_못한다() {
    let tool_id = 게이트된_체인을_적재한다();

    let outcome = dispatch_tool_impl(&tool_id, r#"{"approve":true}"#, None, false);

    assert!(!outcome.ok, "{outcome:?}");
    assert_eq!(오류_코드(&outcome), Some(APPROVAL_REQUIRED_CODE));
}

#[test]
fn 타입_있는_approve_플래그는_같은_체인을_실행시킨다() {
    // 위 두 테스트가 "장벽이 아예 없어졌다"로도 통과하지 않게 하는
    // 대조군이다: 사람이 답하면 같은 호출이 지나간다.
    let tool_id = 게이트된_체인을_적재한다();

    let outcome = dispatch_tool_impl(&tool_id, "{}", None, true);

    assert!(outcome.ok, "{outcome:?}");
}
