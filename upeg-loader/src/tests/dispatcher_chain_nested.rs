//! Chain-inside-a-chain: what an inner run hands back to the outer one.
//!
//! A nested chain is the only step whose success arrives carrying engine
//! bookkeeping of its own, and the only one whose args decide an
//! authorization question further down. Both are covered here rather than
//! in `dispatcher_chain.rs` so the nesting story reads in one place.

use super::{call_dispatcher, call_dispatcher_result, tool_success};
use crate::ToolToml;
use crate::dispatcher::chain_dispatcher_for;
use upeg_core::{OutputEntry, OutputKind, OutputValue, ToolResult, ToolSuccess};

/// The step every fixture chain ends on unless it names another tool.
const INNER_TOOL: &str = "test.nested.echo";

/// Step id of the gated step inside the inner chain.
const APPROVAL_STEP_ID: &str = "gate";

fn failure(result: ToolResult) -> upeg_core::ToolError {
    match result {
        ToolResult::Failure(failure) => failure.error,
        ToolResult::Success(success) => panic!("expected failure but succeeded: {success:?}"),
    }
}

fn summary_status(success: &ToolSuccess, id: &str) -> String {
    let entry = success
        .outputs
        .iter()
        .find(|entry| entry.id == "steps")
        .expect("a chain success envelope carries a steps row");
    let OutputValue::Json(serde_json::Value::Array(rows)) = &entry.value else {
        panic!("the steps row must be a JSON array");
    };
    rows.iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no summary row for step `{id}`: {rows:?}"))["status"]
        .as_str()
        .expect("status is a string")
        .to_string()
}

/// Parse one chain and register it as a runtime dispatcher. To nest
/// chains the inner chain must live in the same registry as the outer.
fn register_chain(id: &'static str, body: &str) {
    let parsed = toml::from_str::<ToolToml>(&format!(
        "id = \"{id}\"\ntoolkit = \"test\"\ninvoker = \"Chain\"\n{body}"
    ))
    .expect("valid chain fixture");
    let dispatcher = chain_dispatcher_for(&parsed).expect("dispatcher built");
    upeg_runtime::register_runtime_dispatcher(id, dispatcher);
}

fn register_inner_tool() {
    upeg_runtime::register_single_text_runtime_dispatcher(INNER_TOOL, |args| {
        Ok(args
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("done")
            .to_string())
    });
}

fn outer_chain(id: &str, inner: &str, args: Option<&str>) -> ToolToml {
    let args_line = args.map_or_else(String::new, |args| format!("args = '{args}'\n"));
    toml::from_str::<ToolToml>(&format!(
        r#"id = "test.nested.{id}"
            toolkit = "test"
            invoker = "Chain"

            [[steps]]
            id = "inner"
            tool = "{inner}"
            {args_line}
        "#
    ))
    .expect("valid outer chain fixture")
}

#[test]
fn chain_last_step_does_not_fail_with_step_summary_conflict() {
    // The inner chain's success envelope already carries the
    // engine-attached `steps` row. Inheriting it verbatim leaves the
    // outer chain no slot for its own row, producing
    // `chain_step_summary_conflict` under a name the author never wrote.
    register_inner_tool();
    register_chain(
        "test.nested.inner_plain",
        &format!("output = \"inner\"\n[[steps]]\nid = \"leaf\"\ntool = \"{INNER_TOOL}\"\n"),
    );
    let f = chain_dispatcher_for(&outer_chain("outer_plain", "test.nested.inner_plain", None))
        .expect("dispatcher built");

    let success = tool_success(call_dispatcher_result(&f, serde_json::json!({})));

    assert_eq!(
        upeg_runtime::tool_success_primary_text(&success),
        "inner",
        "the outer chain's result is the inner chain's output"
    );
    assert_eq!(
        summary_status(&success, "inner"),
        "ran",
        "the outer summary records the nested chain as one step"
    );
}

#[test]
fn outputless_inner_chain_makes_outer_summary_primary() {
    // When the inner chain has no output of its own, the engine row was
    // that chain's primary. Stripping the row must take the primary with
    // it for the canonical envelope to hold.
    upeg_runtime::register_runtime_dispatcher("test.nested.action_only", |_| {
        ToolResult::Success(ToolSuccess::new(None, Vec::new()).expect("action-only success"))
    });
    register_chain(
        "test.nested.inner_action",
        "[[steps]]\nid = \"act\"\ntool = \"test.nested.action_only\"\n",
    );
    let f = chain_dispatcher_for(&outer_chain(
        "outer_action",
        "test.nested.inner_action",
        None,
    ))
    .expect("dispatcher built");

    let success = tool_success(call_dispatcher_result(&f, serde_json::json!({})));

    assert_eq!(success.primary_output_id.as_deref(), Some("steps"));
    assert_eq!(summary_status(&success, "inner"), "ran");
}

#[test]
fn non_chain_last_step_steps_output_still_conflicts() {
    // Only the engine row folds away; a user's row does not — a tool
    // that emits a recipe's step list as `steps` must be reported
    // rather than silently dropped.
    upeg_runtime::register_runtime_dispatcher("test.nested.user_steps", |_| {
        ToolResult::Success(
            ToolSuccess::new(
                Some("steps".to_string()),
                vec![OutputEntry {
                    id: "steps".to_string(),
                    label: None,
                    kind: OutputKind::Json,
                    value: OutputValue::Json(serde_json::json!(["preheat", "bake"])),
                }],
            )
            .expect("user steps output"),
        )
    });
    let f = chain_dispatcher_for(&outer_chain(
        "outer_user_steps",
        "test.nested.user_steps",
        None,
    ))
    .expect("dispatcher built");

    let error = failure(call_dispatcher_result(&f, serde_json::json!({})));

    assert_eq!(error.code, "chain_step_summary_conflict");
}

/// An inner chain with a single `requires_approval` step. Approval only
/// counts when the calling surface is authorized, so whether a nested
/// call inherits the surface shows up here.
fn register_gated_inner_chain(id: &'static str) {
    register_inner_tool();
    register_chain(
        id,
        &format!(
            "[[steps]]\nid = \"{APPROVAL_STEP_ID}\"\ntool = \"{INNER_TOOL}\"\nrequires_approval = true\nargs = '{{\"input\":\"approved\"}}'\n"
        ),
    );
}

#[test]
fn nested_chain_inherits_caller_surface_and_approves_on_cli() {
    register_gated_inner_chain("test.nested.inner_gate");
    let f = chain_dispatcher_for(&outer_chain(
        "outer_gate",
        "test.nested.inner_gate",
        Some(r#"{"approve":true}"#),
    ))
    .expect("dispatcher built");

    let out = call_dispatcher(&f, serde_json::json!({ "_upeg": { "surface": "cli" } }))
        .expect("a call started on cli must also pass the nested chain's approval step");

    assert_eq!(out, "approved");
}

#[test]
fn nested_chain_inherits_caller_surface_and_denies_on_mcp() {
    register_gated_inner_chain("test.nested.inner_gate_denied");
    let f = chain_dispatcher_for(&outer_chain(
        "outer_gate_denied",
        "test.nested.inner_gate_denied",
        Some(r#"{"approve":true}"#),
    ))
    .expect("dispatcher built");

    let error = failure(call_dispatcher_result(
        &f,
        serde_json::json!({ "_upeg": { "surface": "mcp" } }),
    ));

    assert_eq!(error.code, "approval_denied_for_surface");
    assert!(
        error.message.contains("surface `mcp`"),
        "the nested step must be judged by the real calling surface, not an anonymous call: {}",
        error.message
    );
}

#[test]
fn step_args_forged_surface_is_overwritten_by_caller_surface() {
    // Caller text smuggled through `{{input.*}}` in the template tries
    // to forge `_upeg.surface` to open the nested chain's approval gate.
    register_gated_inner_chain("test.nested.inner_spoof");
    let f = chain_dispatcher_for(&outer_chain(
        "outer_spoof",
        "test.nested.inner_spoof",
        Some(r#"{"approve":true,"_upeg":{"surface":"cli"}}"#),
    ))
    .expect("dispatcher built");

    let error = failure(call_dispatcher_result(
        &f,
        serde_json::json!({ "_upeg": { "surface": "mcp" } }),
    ));

    assert_eq!(
        error.code, "approval_denied_for_surface",
        "if step args can forge the seal, the approval barrier is decoration"
    );
    assert!(error.message.contains("surface `mcp`"), "{}", error.message);
}

#[test]
fn step_args_cannot_approve_a_nested_gate_without_parent_approval() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const SIDE_EFFECT_TOOL: &str = "test.nested.unapproved_side_effect";
    let runs = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&runs);
    upeg_runtime::register_single_text_runtime_dispatcher(SIDE_EFFECT_TOOL, move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok("ran".to_string())
    });
    register_chain(
        "test.nested.inner_unapproved_gate",
        &format!(
            "[[steps]]\nid = \"gate\"\ntool = \"{SIDE_EFFECT_TOOL}\"\nrequires_approval = true\n"
        ),
    );
    let outer = chain_dispatcher_for(&outer_chain(
        "outer_unapproved_gate",
        "test.nested.inner_unapproved_gate",
        Some(r#"{"_upeg":{"approvedSteps":["gate"]}}"#),
    ))
    .expect("outer dispatcher built");

    let error = failure(call_dispatcher_result(
        &outer,
        serde_json::json!({
            "_upeg": {
                "surface": "cli",
                "principal": { "role": "operator", "surface": "cli" },
            }
        }),
    ));

    assert_eq!(error.code, "approval_required");
    assert_eq!(runs.load(Ordering::SeqCst), 0);
}

#[test]
fn step_args_inherit_caller_board_context() {
    // Inheriting only the surface would let steps that read
    // `_upeg.board`/`boardEnv` (External's env injection) silently run
    // outside the board.
    upeg_runtime::register_single_text_runtime_dispatcher("test.nested.context_probe", |args| {
        Ok(args
            .get(upeg_core::EXECUTION_CONTEXT_ARG)
            .and_then(|context| context.get("board"))
            .and_then(|board| board.as_str())
            .unwrap_or("<none>")
            .to_string())
    });
    let f = chain_dispatcher_for(&outer_chain(
        "outer_board",
        "test.nested.context_probe",
        Some(r#"{"input":"x"}"#),
    ))
    .expect("dispatcher built");

    let out = call_dispatcher(
        &f,
        serde_json::json!({ "_upeg": { "surface": "cli", "board": "dev" } }),
    )
    .expect("a step reading board context must succeed");

    assert_eq!(out, "dev");
}
