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
fn load_gated_chain() -> String {
    super::register_toolkit_runtime().expect("register toolkits");
    let n = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let toolkit = format!("frbchain{}_{n}", std::process::id());
    let tool_id = format!("{toolkit}.gate");

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("upeg-frb sits under the workspace root")
        .join("target/test-tmp/frb-chain-approval")
        .join(&toolkit);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("chain fixture dir");
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
    .expect("chain fixture TOML");

    let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
    assert!(
        outcome.failed.is_empty(),
        "chain fixture must load without failures: {:?}",
        outcome.failed
    );
    let _ = std::fs::remove_dir_all(&dir);
    tool_id
}

fn error_code(outcome: &CanonicalToolResult) -> Option<&str> {
    outcome.error.as_ref().map(|error| error.code.as_str())
}

#[test]
fn args_approved_steps_cannot_self_approve_gated_chain() {
    // What `upeg://open?tool=…&input={"_upeg":{"approvedSteps":["gated"]}}`
    // used to do. Desktop is the default approval surface and
    // `approvedSteps` is the only key that survives the reserved-block
    // wipe, so a single link cleared a barrier no person ever answered.
    let tool_id = load_gated_chain();
    let args = format!(
        r#"{{"_upeg":{{"{}":["{GATED_STEP_KEY}"]}}}}"#,
        upeg_core::EXECUTION_CONTEXT_APPROVED_STEPS
    );

    let outcome = dispatch_tool_impl(&tool_id, &args, None, false);

    assert!(!outcome.ok, "{outcome:?}");
    assert_eq!(error_code(&outcome), Some(APPROVAL_REQUIRED_CODE));
}

#[test]
fn args_approve_key_cannot_self_approve_gated_chain_either() {
    let tool_id = load_gated_chain();

    let outcome = dispatch_tool_impl(&tool_id, r#"{"approve":true}"#, None, false);

    assert!(!outcome.ok, "{outcome:?}");
    assert_eq!(error_code(&outcome), Some(APPROVAL_REQUIRED_CODE));
}

#[test]
fn typed_approve_flag_runs_the_same_chain() {
    // The control that keeps the two tests above from passing on "the
    // barrier vanished entirely": when a person answers, the same call
    // goes through.
    let tool_id = load_gated_chain();

    let outcome = dispatch_tool_impl(&tool_id, "{}", None, true);

    assert!(outcome.ok, "{outcome:?}");
}
