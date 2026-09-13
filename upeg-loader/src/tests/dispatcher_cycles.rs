//! Chain cycle/depth dispatcher regressions split from dispatcher.rs.

use super::{call_dispatcher, runtime_failure_message};
use crate::ToolToml;
use crate::dispatcher::{MAX_CHAIN_DEPTH, chain_dispatcher_for};
use std::sync::atomic::{AtomicUsize, Ordering};

static FANOUT_RECURSIVE_STEP_CALLS: AtomicUsize = AtomicUsize::new(0);
const FANOUT_RECURSION_TEST_BUDGET: usize = MAX_CHAIN_DEPTH + 16;

fn fanout_recursive_step(args: upeg_runtime::DispatchArgs<'_>) -> Result<String, String> {
    let calls = FANOUT_RECURSIVE_STEP_CALLS.fetch_add(1, Ordering::SeqCst);
    if calls > FANOUT_RECURSION_TEST_BUDGET {
        return Err("fanout recursion reached test budget before chain depth guard".into());
    }
    upeg_runtime::tool_result_text(
        upeg_runtime::try_runtime_dispatch("iter259.fanout_cycle", args.as_value())
            .expect("fanout cycle dispatcher must be registered"),
    )
}

#[test]
fn 체인_dispatcher는_직접_자기참조_순환을_끊는다() {
    // Iter 171: A → A. Pre-iter-171 this overflowed the thread
    // stack at runtime. The `--resolve-chain` validate flag would
    // catch it but is opt-in; runtime needs its own guard.
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: "iter171.cycle_self",
        toolkit: "iter171",
        local_id: "cycle_self",
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "iter171.cycle_self"
            toolkit = "iter171"
            invoker = "Chain"
            steps = [{ tool = "iter171.cycle_self" }]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");
    // Register the chain dispatcher under the same id so the
    // recursive call hits a real chain dispatcher, not a no-op.
    upeg_runtime::register_runtime_dispatcher("iter171.cycle_self", f);

    // Now invoke through the registry. The depth guard must trip
    // and return an error rather than overflowing the stack.
    let result = upeg_runtime::try_runtime_dispatch(
        "iter171.cycle_self",
        &serde_json::json!({"input": "x"}),
    );
    let msg = runtime_failure_message(result);
    assert!(
        msg.contains("chain depth exceeded"),
        "expected depth-limit error, got: {msg}"
    );
    assert!(
        msg.contains(&format!("{MAX_CHAIN_DEPTH}")),
        "error must mention the actual limit so the user knows what threshold to bump"
    );
}

#[test]
fn 체인_dispatcher는_간접_순환을_끊는다() {
    // Iter 171: A → B → A. The opt-in `--resolve-chain` validate
    // command only catches the *direct* self-reference case (step
    // == own id), so indirect cycles slipped past validate and
    // crashed at runtime. The depth guard catches both shapes
    // uniformly.
    for id in ["iter171.cycle_a", "iter171.cycle_b"] {
        upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
            id,
            toolkit: "iter171",
            local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "iter171")
                .expect("cycle test tool id must be canonical")
                .local(),
            tags: &[],
            display_label: "Test tool",
            description: "",
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            source: upeg_core::Source::UserInput,
            pin: upeg_core::PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: upeg_core::Invoker::Function,
            surfaces: upeg_core::ALL_SURFACES,
            boards: &[],
        });
    }
    let parsed_a = toml::from_str::<ToolToml>(
        r#"id = "iter171.cycle_a"
            toolkit = "iter171"
            invoker = "Chain"
            steps = [{ tool = "iter171.cycle_b" }]"#,
    )
    .unwrap();
    let parsed_b = toml::from_str::<ToolToml>(
        r#"id = "iter171.cycle_b"
            toolkit = "iter171"
            invoker = "Chain"
            steps = [{ tool = "iter171.cycle_a" }]"#,
    )
    .unwrap();
    let f_a = chain_dispatcher_for(&parsed_a).expect("a built");
    let f_b = chain_dispatcher_for(&parsed_b).expect("b built");
    upeg_runtime::register_runtime_dispatcher("iter171.cycle_a", f_a);
    upeg_runtime::register_runtime_dispatcher("iter171.cycle_b", f_b);

    let result =
        upeg_runtime::try_runtime_dispatch("iter171.cycle_a", &serde_json::json!({"input": "x"}));
    let msg = runtime_failure_message(result);
    assert!(
        msg.contains("chain depth exceeded"),
        "indirect cycle must also trip the depth guard, got: {msg}"
    );
}

#[test]
fn 체인_dispatcher는_팬아웃_재귀_순환을_끊는다() {
    FANOUT_RECURSIVE_STEP_CALLS.store(0, Ordering::SeqCst);
    upeg_runtime::register_single_text_runtime_dispatcher(
        "iter259.fanout_left",
        fanout_recursive_step,
    );
    upeg_runtime::register_single_text_runtime_dispatcher(
        "iter259.fanout_right",
        fanout_recursive_step,
    );
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "iter259.fanout_cycle"
            toolkit = "iter259"
            invoker = "Chain"
            steps = [
                { id = "left", tool = "iter259.fanout_left" },
                { id = "right", tool = "iter259.fanout_right" },
            ]"#,
    )
    .unwrap();
    let dispatcher = chain_dispatcher_for(&parsed).expect("fanout cycle dispatcher built");
    upeg_runtime::register_runtime_dispatcher("iter259.fanout_cycle", dispatcher);

    let err = runtime_failure_message(upeg_runtime::try_runtime_dispatch(
        "iter259.fanout_cycle",
        &serde_json::json!({"input": "x"}),
    ));
    assert!(
        err.contains("chain depth exceeded"),
        "fanout recursion must return the bounded depth error, got: {err}"
    );
    assert!(
        FANOUT_RECURSIVE_STEP_CALLS.load(Ordering::SeqCst) <= FANOUT_RECURSION_TEST_BUDGET,
        "fanout recursion should stop before the defensive test budget"
    );
}

#[test]
fn 체인_깊이_카운터는_반복된_깊이_제한_오류_후_초기화된다() {
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: "iter258.cycle_self",
        toolkit: "iter258",
        local_id: "cycle_self",
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    let cycle = toml::from_str::<ToolToml>(
        r#"id = "iter258.cycle_self"
            toolkit = "iter258"
            invoker = "Chain"
            steps = [{ tool = "iter258.cycle_self" }]"#,
    )
    .unwrap();
    let cycle_dispatcher = chain_dispatcher_for(&cycle).expect("cycle dispatcher built");
    upeg_runtime::register_runtime_dispatcher("iter258.cycle_self", cycle_dispatcher);

    upeg_runtime::register_single_text_runtime_dispatcher("iter258.simple_step", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("ok({s})"))
    });
    let simple = toml::from_str::<ToolToml>(
        r#"id = "iter258.simple"
            toolkit = "iter258"
            invoker = "Chain"
            steps = [{ tool = "iter258.simple_step" }]"#,
    )
    .unwrap();
    let simple_dispatcher = chain_dispatcher_for(&simple).expect("simple dispatcher built");

    for i in 0..(MAX_CHAIN_DEPTH + 2) {
        let err = runtime_failure_message(upeg_runtime::try_runtime_dispatch(
            "iter258.cycle_self",
            &serde_json::json!({"input": "x"}),
        ));
        assert!(err.contains("chain depth exceeded"), "{err}");

        let out = call_dispatcher(
            &simple_dispatcher,
            serde_json::json!({"input": format!("{i}")}),
        )
        .expect("unrelated chain must still start from a clean depth");
        assert_eq!(out, format!("ok({i})"));
    }
}

#[test]
fn 체인_깊이_카운터는_최상위_호출_사이에_초기화된다() {
    // Iter 171: the depth counter is thread-local + RAII-decremented
    // on closure exit. After a successful chain run, the next
    // top-level call must see depth=0 again — a leaked counter
    // would slowly poison the thread until even legitimate chains
    // hit the limit.
    upeg_runtime::register_single_text_runtime_dispatcher("iter171.simple_step", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("ok({s})"))
    });
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "iter171.simple"
            toolkit = "iter171"
            invoker = "Chain"
            steps = [{ tool = "iter171.simple_step" }]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("built");

    // Run MAX_CHAIN_DEPTH + 5 times sequentially. If the counter
    // were leaking by 1 per call, the (MAX+1)th call would fail.
    for i in 0..(MAX_CHAIN_DEPTH + 5) {
        let out = call_dispatcher(&f, serde_json::json!({"input": format!("{i}")})).unwrap();
        assert_eq!(
            out,
            format!("ok({i})"),
            "call #{i} unexpectedly errored — depth counter likely leaked"
        );
    }
}
