use super::{call_dispatcher, call_dispatcher_result, single_tool_toml_str, tool_success};
use crate::dispatcher::{DEFAULT_APPROVAL_SURFACES, chain_dispatcher_for};
use crate::{ChainStepToml, ToolToml, load_and_register_dir};
use upeg_core::{OutputEntry, OutputKind, OutputValue, ToolResult, ToolSuccess};

fn success_result(primary: &str, outputs: Vec<OutputEntry>) -> ToolResult {
    ToolResult::Success(
        ToolSuccess::new(Some(primary.to_string()), outputs).expect("valid canonical success"),
    )
}

/// A call envelope stamped the way the CLI surface stamps one: a
/// trustworthy `_upeg.surface`, plus the caller's optional approval
/// allow-list. Approval authorization reads the surface label, so a
/// chain test that wants an approval honored has to say where the call
/// came from.
fn cli_call(input: serde_json::Value, approved_steps: Option<&[&str]>) -> serde_json::Value {
    let mut context = serde_json::json!({ "surface": "cli" });
    if let Some(steps) = approved_steps {
        context["approvedSteps"] = serde_json::json!(steps);
    }
    serde_json::json!({ "input": input, "_upeg": context })
}

fn string_output(id: &str, value: &str) -> OutputEntry {
    OutputEntry {
        id: id.to_string(),
        label: None,
        kind: OutputKind::String,
        value: OutputValue::String(value.to_string()),
    }
}

#[test]
fn no_chain_fields_means_no_chain_dispatcher() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y""#,
    )
    .unwrap();
    assert!(chain_dispatcher_for(&parsed).is_none());
}

#[test]
fn chain_dispatcher_delegates_single_step() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain.step.alpha", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("alpha({s})"))
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "Chain"
            steps = [{ tool = "test.chain.step.alpha" }]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({"input": "first"})).unwrap();
    assert_eq!(out, "alpha(first)");
}

#[test]
fn independent_steps_of_chain_without_implicit_order_receive_root_args() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain.independent.first", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("first:{s}:{mode}"))
    });
    upeg_runtime::register_single_text_runtime_dispatcher(
        "test.chain.independent.second",
        |args| {
            let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
            let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("");
            Ok(format!("second:{s}:{mode}"))
        },
    );

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.chain.independent"
            toolkit = "test"
            invoker = "Chain"
            output = "{{steps.first.output}}|{{steps.second.output}}"
            steps = [
                { id = "first", tool = "test.chain.independent.first" },
                { id = "second", tool = "test.chain.independent.second" },
            ]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");

    let out = call_dispatcher(&f, serde_json::json!({"input": "root", "mode": "fanout"})).unwrap();
    assert_eq!(out, "first:root:fanout|second:root:fanout");
}

#[test]
fn chain_dispatcher_pipes_stdout_into_next_step_input() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain.step.upper", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(s.to_uppercase())
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain.step.exclaim", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("{s}!"))
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "Chain"
            connections = [{ from = "upper", to = "exclaim" }]
            steps = [
                { id = "upper", tool = "test.chain.step.upper" },
                { id = "exclaim", tool = "test.chain.step.exclaim" },
            ]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).unwrap();
    let out = call_dispatcher(&f, serde_json::json!({"input": "hello"})).unwrap();
    assert_eq!(out, "HELLO!");
}

#[test]
fn chain_dispatcher_forwards_primary_output_to_next_step_by_default() {
    upeg_runtime::register_runtime_dispatcher("test.chain.primary.source", |_| {
        success_result(
            "machine",
            vec![
                OutputEntry {
                    id: "label".into(),
                    label: Some("Rendered Label".into()),
                    kind: OutputKind::String,
                    value: OutputValue::String("human-facing label".into()),
                },
                string_output("machine", "canonical"),
            ],
        )
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain.primary.sink", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("sink:{s}"))
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.chain.primary"
            toolkit = "test"
            invoker = "Chain"
            connections = [{ from = "source", to = "sink" }]
            steps = [
                { id = "source", tool = "test.chain.primary.source" },
                { id = "sink", tool = "test.chain.primary.sink" },
            ]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).unwrap();

    let out = call_dispatcher(&f, serde_json::json!({"input": "root"})).unwrap();
    assert_eq!(out, "sink:canonical");
}

#[test]
fn chain_dispatcher_forwards_named_output_field_as_step_arg() {
    upeg_runtime::register_runtime_dispatcher("test.chain.field.source", |_| {
        success_result(
            "primary",
            vec![
                string_output("primary", "default"),
                string_output("raw", "selected"),
            ],
        )
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain.field.sink", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("sink:{s}"))
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.chain.field"
            toolkit = "test"
            invoker = "Chain"
            connections = [{ from = "source", to = "sink" }]

            [[steps]]
            id = "source"
            tool = "test.chain.field.source"

            [[steps]]
            id = "sink"
            tool = "test.chain.field.sink"
            args = '{"input":"{{steps.source.raw}}"}'
        "#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).unwrap();

    let out = call_dispatcher(&f, serde_json::json!({})).unwrap();
    assert_eq!(out, "sink:selected");
}

#[test]
fn chain_dispatcher_returns_last_steps_normalized_result_verbatim() {
    upeg_runtime::register_runtime_dispatcher("test.chain.final.source", |_| {
        success_result(
            "primary",
            vec![
                string_output("primary", "canonical"),
                string_output("debug", "trace"),
            ],
        )
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.chain.final"
            toolkit = "test"
            invoker = "Chain"
            steps = [{ id = "source", tool = "test.chain.final.source" }]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).unwrap();

    let success = tool_success(call_dispatcher_result(&f, serde_json::json!({})));
    assert_eq!(success.primary_output_id.as_deref(), Some("primary"));
    // The last step's own two rows, plus the engine-owned step summary
    // every chain result carries.
    assert_eq!(success.outputs.len(), 3);
    assert_eq!(
        success.outputs[0].value,
        OutputValue::String("canonical".into())
    );
    assert_eq!(
        success.outputs[1].value,
        OutputValue::String("trace".into())
    );
    assert_eq!(success.outputs[2].id, "steps");
}

#[test]
fn chain_dispatcher_connections_allow_same_tool_as_distinct_nodes() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.connection.reused", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        let label = args.get("label").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("{label}({s})"))
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.connection.reused_chain"
            toolkit = "test"
            invoker = "Chain"
            output = "{{steps.first.output}}|{{steps.second.output}}"
            connections = [{ from = "first", to = "second" }]

            [[steps]]
            id = "first"
            tool = "test.connection.reused"
            args = '{"input":"{{input}}","label":"first"}'

            [[steps]]
            id = "second"
            tool = "test.connection.reused"
            args = '{"input":"{{steps.first.output}}","label":"second"}'
        "#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");

    let out = call_dispatcher(&f, serde_json::json!({"input": "root"})).unwrap();
    assert_eq!(out, "first(root)|second(first(root))");
}

#[test]
fn chain_dispatcher_rich_steps_support_expression_gating_approval_and_output() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.rich.upper", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(s.to_uppercase())
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.rich.suffix", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("{s}!"))
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.rich.chain"
            toolkit = "test"
            invoker = "Chain"
            output = "{{steps.approved.output}}"
            connections = [
                { from = "upper", to = "approved" },
                { from = "skipped", to = "approved" },
            ]

            [[steps]]
            id = "upper"
            tool = "test.rich.upper"
            args = '{"input":"{{input.text}}"}'

            [[steps]]
            id = "skipped"
            tool = "test.rich.missing"
            when = "false"

            [[steps]]
            id = "approved"
            tool = "test.rich.suffix"
            requires_approval = true
            args = '{"input":"{{steps.upper.output}}"}'
        "#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");

    let denied =
        call_dispatcher(&f, cli_call(serde_json::json!({"text": "hello"}), None)).unwrap_err();
    assert!(denied.contains("requires approval"), "got {denied}");

    let legacy_denied = call_dispatcher(
        &f,
        serde_json::json!({
            "input": {"text": "hello"},
            "approved_steps": ["approved"],
            "_upeg": {"surface": "cli"}
        }),
    )
    .unwrap_err();
    assert!(
        legacy_denied.contains("requires approval"),
        "got {legacy_denied}"
    );

    let approved_by_flag = call_dispatcher(
        &f,
        serde_json::json!({
            "input": {"text": "hello"},
            "approve": true,
            "_upeg": {"surface": "cli"}
        }),
    )
    .unwrap();
    assert_eq!(approved_by_flag, "HELLO!");

    let out = call_dispatcher(
        &f,
        cli_call(serde_json::json!({"text": "hello"}), Some(&["approved"])),
    )
    .unwrap();
    assert_eq!(out, "HELLO!");
}

#[test]
fn chain_dispatcher_dag_steps_receive_declared_dependency_outputs() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.dag.a", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("a:{s}"))
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.dag.b", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("b:{s}"))
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.dag.join", |args| {
        let inputs = args.get("inputs").and_then(|v| v.as_object()).unwrap();
        Ok(format!(
            "{}|{}",
            inputs["left"].as_str().unwrap_or(""),
            inputs["right"].as_str().unwrap_or("")
        ))
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.dag.chain"
            toolkit = "test"
            invoker = "Chain"
            connections = [
                { from = "left", to = "joined" },
                { from = "right", to = "joined" },
            ]

            [[steps]]
            id = "left"
            tool = "test.dag.a"

            [[steps]]
            id = "right"
            tool = "test.dag.b"

            [[steps]]
            id = "joined"
            tool = "test.dag.join"
        "#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");

    let out = call_dispatcher(&f, serde_json::json!({"input": "root"})).unwrap();
    assert_eq!(out, "a:root|b:root");
}

#[test]
fn chain_dispatcher_fanout_join_tail_scenario_uses_explicit_connections() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.scenario.seed", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("seed({s})"))
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.scenario.left", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("left({s})"))
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.scenario.right", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("right({s})"))
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.scenario.join", |args| {
        let inputs = args.get("inputs").and_then(|v| v.as_object()).unwrap();
        Ok(format!(
            "{}+{}",
            inputs["left"].as_str().unwrap_or(""),
            inputs["right"].as_str().unwrap_or("")
        ))
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.scenario.tail", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("tail[{s}]"))
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.scenario.chain"
            toolkit = "test"
            invoker = "Chain"
            connections = [
                { from = "seed", to = "left" },
                { from = "seed", to = "right" },
                { from = "left", to = "joined" },
                { from = "right", to = "joined" },
                { from = "joined", to = "tail" },
            ]

            [[steps]]
            id = "seed"
            tool = "test.scenario.seed"

            [[steps]]
            id = "left"
            tool = "test.scenario.left"

            [[steps]]
            id = "right"
            tool = "test.scenario.right"

            [[steps]]
            id = "joined"
            tool = "test.scenario.join"

            [[steps]]
            id = "tail"
            tool = "test.scenario.tail"
        "#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");

    let out = call_dispatcher(&f, serde_json::json!({"input": "root"})).unwrap();
    assert_eq!(out, "tail[left(seed(root))+right(seed(root))]");
}

#[test]
fn chain_dispatcher_short_circuits_on_step_error() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain.step.fails", |_| {
        Err("step blew up".into())
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain.step.never_runs", |_| {
        Ok("should-not-appear".into())
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "Chain"
            connections = [{ from = "fails", to = "never_runs" }]
            steps = [
                { id = "fails", tool = "test.chain.step.fails" },
                { id = "never_runs", tool = "test.chain.step.never_runs" },
            ]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).unwrap();
    match call_dispatcher(&f, serde_json::json!({})) {
        Err(msg) => {
            assert!(msg.contains("test.chain.step.fails"));
            assert!(msg.contains("step blew up"));
            assert!(!msg.contains("should-not-appear"));
        }
        Ok(_) => panic!("expected Err, got Ok"),
    }
}

#[test]
fn chain_dispatcher_errors_on_unknown_step() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "Chain"
            steps = [{ tool = "test.chain.step.does_not_exist_xyz" }]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).unwrap();
    match call_dispatcher(&f, serde_json::json!({})) {
        Err(msg) => assert!(msg.contains("not found in registry"), "got: {msg}"),
        Ok(_) => panic!("expected error for missing step"),
    }
}

#[test]
fn chain_of_chains_dispatches_through_two_levels() {
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: "iter68.chain_a",
        toolkit: "iter68",
        local_id: "chain_a",
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher("iter68.chain_a", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(s.to_uppercase())
    });
    upeg_runtime::register_single_text_runtime_dispatcher("iter68.step_reverse", |args| {
        let s = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
        Ok(s.chars().rev().collect::<String>())
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "iter68.chain_b"
            toolkit = "iter68"
            invoker = "Chain"
            connections = [{ from = "upper", to = "reverse" }]
            steps = [
                { id = "upper", tool = "iter68.chain_a" },
                { id = "reverse", tool = "iter68.step_reverse" },
            ]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");

    let out = call_dispatcher(&f, serde_json::json!({"input": "hello"})).unwrap();
    assert_eq!(out, "OLLEH");
}

#[test]
fn chain_dispatcher_trims_step_ids() {
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: "iter240.step",
        toolkit: "iter240",
        local_id: "step",
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher("iter240.step", |_args| {
        Ok("step ran".to_string())
    });

    for (i, padded) in [" iter240.step", "iter240.step ", "  iter240.step  "]
        .iter()
        .enumerate()
    {
        let parsed = toml::from_str::<ToolToml>(&format!(
            r#"id = "iter240.outer_{i}"
                toolkit = "iter240"
                invoker = "Chain"
                steps = [{{ tool = "{padded}" }}]"#,
        ))
        .unwrap();
        let f = chain_dispatcher_for(&parsed).expect("dispatcher built");
        let result = call_dispatcher(&f, serde_json::json!({})).unwrap();
        assert_eq!(
            result, "step ran",
            "padded chain step `{padded:?}` should resolve to the trimmed registry id"
        );
    }

    let parsed_with_ws = ToolToml {
        id: "iter240.outer_ws".into(),
        toolkit: "iter240".into(),
        tags: None,
        description: None,
        pin: None,
        pegboard_units: Some("U1".into()),
        invoker: Some("Chain".into()),
        surfaces: None,
        boards: None,
        command: None,
        args_template: None,
        steps: Some(vec![ChainStepToml {
            id: None,
            tool: "\titer240.step\n".into(),
            args: None,
            when: None,
            requires_approval: None,
        }]),
        connections: None,
        embed_url: None,
        ..ToolToml::default()
    };
    let f = chain_dispatcher_for(&parsed_with_ws).expect("dispatcher built");
    let result = call_dispatcher(&f, serde_json::json!({})).unwrap();
    assert_eq!(
        result, "step ran",
        "tab/newline-padded chain step should also trim cleanly"
    );
}

#[test]
fn step_with_external_invoker_conflicts() {
    let dir = std::env::temp_dir().join("upeg_loader_chain_vs_external");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "test.chain_vs_external.conflict";
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain_vs_external.step", |_| {
        Ok("from-chain".into())
    });
    std::fs::write(
        dir.join("t.toml"),
        single_tool_toml_str(&format!(
            r#"id = "{id}"
toolkit = "test"
invoker = "External"
command = "echo"
args_template = ["from-external"]
steps = [{{ tool = "test.chain_vs_external.step" }}]"#,
        )),
    )
    .unwrap();

    let (loaded, failed) = load_and_register_dir(&dir);
    assert_eq!((loaded, failed), (0, 1));

    let r = upeg_runtime::try_runtime_dispatch(id, &serde_json::json!({}));
    assert!(
        r.is_none(),
        "conflicting invoker fields must not register a dispatcher"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ─── approval authorization ─────────────────────────────────────────

/// A call envelope stamped with `_upeg.surface`. The surface is stamped
/// by the serving surface so the caller cannot forge it — which is why
/// it is the basis for approval authorization.
fn surface_approval_call(surface: &str) -> serde_json::Value {
    serde_json::json!({ "approve": true, "_upeg": { "surface": surface } })
}

fn failure(result: ToolResult) -> upeg_core::ToolError {
    match result {
        ToolResult::Failure(failure) => failure.error,
        ToolResult::Success(success) => panic!("expected failure but succeeded: {success:?}"),
    }
}

/// A chain with a single `requires_approval` step. `approval_surfaces`
/// is spliced in verbatim so tests can vary only whether/how it is
/// declared.
fn approval_chain(id: &str, approval_surfaces: Option<&str>) -> ToolToml {
    upeg_runtime::register_single_text_runtime_dispatcher("test.approval.echo", |args| {
        Ok(args
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("ok")
            .to_string())
    });
    let declaration =
        approval_surfaces.map_or_else(String::new, |list| format!("approval_surfaces = {list}\n"));
    toml::from_str::<ToolToml>(&format!(
        r#"id = "test.approval.{id}"
            toolkit = "test"
            invoker = "Chain"
            {declaration}
            [[steps]]
            id = "gate"
            tool = "test.approval.echo"
            requires_approval = true
            args = '{{"input":"done"}}'
        "#
    ))
    .expect("valid approval chain fixture")
}

#[test]
fn cli_surface_approval_is_honored_by_default_approval_surfaces() {
    let f = chain_dispatcher_for(&approval_chain("cli_ok", None)).expect("dispatcher built");
    let out = call_dispatcher(&f, surface_approval_call("cli"))
        .expect("a cli-surface approval must be honored");
    assert_eq!(out, "done");
}

#[test]
fn mcp_surface_approval_is_denied_by_surface_authorization() {
    let f = chain_dispatcher_for(&approval_chain("mcp_denied", None)).expect("dispatcher built");
    let error = failure(call_dispatcher_result(&f, surface_approval_call("mcp")));
    assert_eq!(error.code, "approval_denied_for_surface");
    assert!(
        error.message.contains("surface `mcp`") && error.message.contains("only from cli"),
        "the denial message must say who may approve: {}",
        error.message
    );
    assert!(
        error
            .message
            .contains("upeg call test.approval.mcp_denied -a approve=true"),
        "the denial message must name the command to run instead: {}",
        error.message
    );
}

#[test]
fn http_surface_approved_steps_are_denied_by_surface_authorization() {
    let f = chain_dispatcher_for(&approval_chain("http_denied", None)).expect("dispatcher built");
    let args = serde_json::json!({
        "_upeg": { "surface": "http", "approvedSteps": ["gate"] }
    });
    let error = failure(call_dispatcher_result(&f, args));
    assert_eq!(error.code, "approval_denied_for_surface");
}

#[test]
fn approval_surfaces_naming_mcp_honor_mcp_approval() {
    let f = chain_dispatcher_for(&approval_chain("mcp_allowed", Some(r#"["mcp"]"#)))
        .expect("dispatcher built");
    let out = call_dispatcher(&f, surface_approval_call("mcp"))
        .expect("an approval from an explicitly listed surface must be honored");
    assert_eq!(out, "done");

    let error = failure(call_dispatcher_result(&f, surface_approval_call("cli")));
    assert_eq!(
        error.code, "approval_denied_for_surface",
        "an explicit declaration replaces the default — cli is no longer included"
    );
}

#[test]
fn approvable_surface_without_approval_sent_is_approval_required() {
    let f = chain_dispatcher_for(&approval_chain("cli_missing", None)).expect("dispatcher built");
    let error = failure(call_dispatcher_result(
        &f,
        serde_json::json!({ "_upeg": { "surface": "cli" } }),
    ));
    assert_eq!(error.code, "approval_required");
}

#[test]
fn gated_chain_registers_approval_policy_for_ui() {
    // This is the value a UI asks for **before** dispatch: should a
    // confirmation be shown, and does a confirmation on my surface count.
    let toml = approval_chain("policy_default", None);
    let _ = chain_dispatcher_for(&toml).expect("dispatcher built");

    let policy = upeg_runtime::tool_approval_policy(&toml.id);

    assert!(policy.requires_approval());
    assert_eq!(policy.surfaces(), DEFAULT_APPROVAL_SURFACES);
}

#[test]
fn declared_approval_surfaces_are_the_registered_policys_effective_set() {
    let toml = approval_chain("policy_declared", Some(r#"["mcp"]"#));
    let _ = chain_dispatcher_for(&toml).expect("dispatcher built");

    let policy = upeg_runtime::tool_approval_policy(&toml.id);

    assert!(policy.honors(upeg_core::Surface::Mcp));
    assert!(!policy.honors(upeg_core::Surface::Cli));
}

#[test]
fn chain_without_approval_steps_registers_no_approval_policy() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.approval.echo", |_| {
        Ok("done".to_string())
    });
    let toml = toml::from_str::<ToolToml>(
        r#"id = "test.approval.policy_none"
            toolkit = "test"
            invoker = "Chain"
            [[steps]]
            tool = "test.approval.echo"
        "#,
    )
    .expect("valid chain fixture");

    let _ = chain_dispatcher_for(&toml).expect("dispatcher built");

    let policy = upeg_runtime::tool_approval_policy(&toml.id);
    assert!(!policy.requires_approval());
    assert!(policy.surfaces().is_empty());
}

#[test]
fn dispatcher_with_unknown_approval_surfaces_fails_calls() {
    // The loader rejects this manifest. Only a hand-built ToolToml reaches
    // this point, so the call must fail rather than silently fall back
    // to defaults.
    let f = chain_dispatcher_for(&approval_chain("bad_surface", Some(r#"["nope"]"#)))
        .expect("dispatcher built");
    let error = failure(call_dispatcher_result(&f, surface_approval_call("cli")));
    assert!(
        error.message.contains("approval_surfaces[0]"),
        "got {}",
        error.message
    );
}

// ─── step metadata ───────────────────────────────────────────────────

fn step_summary(success: &upeg_core::ToolSuccess) -> Vec<serde_json::Value> {
    let entry = success
        .outputs
        .iter()
        .find(|entry| entry.id == "steps")
        .expect("every chain success envelope carries a steps row");
    let OutputValue::Json(serde_json::Value::Array(rows)) = &entry.value else {
        panic!("the steps row must be a JSON array");
    };
    rows.clone()
}

fn summary_row<'a>(rows: &'a [serde_json::Value], id: &str) -> &'a serde_json::Value {
    rows.iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no summary row for step `{id}`: {rows:?}"))
}

#[test]
fn chain_success_envelope_summarizes_ran_and_skipped_steps() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.summary.ran", |_| {
        Ok("ran".to_string())
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.summary.chain"
            toolkit = "test"
            invoker = "Chain"
            output = "{{steps.first.output}}"

            [[steps]]
            id = "first"
            tool = "test.summary.ran"

            [[steps]]
            id = "second"
            tool = "test.summary.never"
            when = "false"
        "#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");

    let success = tool_success(call_dispatcher_result(&f, serde_json::json!({})));
    let rows = step_summary(&success);
    assert_eq!(rows.len(), 2);
    assert_eq!(summary_row(&rows, "first")["status"], "ran");
    assert_eq!(summary_row(&rows, "first")["tool"], "test.summary.ran");
    assert!(summary_row(&rows, "first")["duration_ms"].is_u64());
    assert_eq!(summary_row(&rows, "second")["status"], "skipped");
}

#[test]
fn chain_failure_envelope_carries_step_states_in_details_steps() {
    upeg_runtime::register_single_text_runtime_dispatcher("test.summary.before", |_| {
        Ok("before".to_string())
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.summary.failing"
            toolkit = "test"
            invoker = "Chain"
            connections = [{ from = "before", to = "missing" }]

            [[steps]]
            id = "before"
            tool = "test.summary.before"

            [[steps]]
            id = "missing"
            tool = "test.summary.unregistered"
        "#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");

    let error = failure(call_dispatcher_result(&f, serde_json::json!({})));
    let details = error
        .details
        .expect("the failure envelope must carry the step summary too");
    let rows = details["steps"].as_array().expect("steps array").clone();
    assert_eq!(summary_row(&rows, "before")["status"], "ran");
    assert_eq!(summary_row(&rows, "missing")["status"], "failed");
}

#[test]
fn approval_denial_envelope_step_summary_marks_denied() {
    let f =
        chain_dispatcher_for(&approval_chain("denied_summary", None)).expect("dispatcher built");
    let error = failure(call_dispatcher_result(&f, surface_approval_call("mcp")));
    let details = error
        .details
        .expect("the denial envelope must carry the step summary too");
    let rows = details["steps"].as_array().expect("steps array").clone();
    assert_eq!(summary_row(&rows, "gate")["status"], "denied");
}

#[test]
fn chain_without_own_output_makes_step_summary_primary() {
    // action-only step: the canonical envelope cannot carry an output
    // row without a primary, so when the engine row is the only row it
    // becomes the primary.
    upeg_runtime::register_runtime_dispatcher("test.summary.action_only", |_| {
        ToolResult::Success(ToolSuccess::new(None, Vec::new()).expect("action-only success"))
    });

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "test.summary.action_only_chain"
            toolkit = "test"
            invoker = "Chain"
            steps = [{ id = "act", tool = "test.summary.action_only" }]"#,
    )
    .unwrap();
    let f = chain_dispatcher_for(&parsed).expect("dispatcher built");

    let success = tool_success(call_dispatcher_result(&f, serde_json::json!({})));
    assert_eq!(success.primary_output_id.as_deref(), Some("steps"));
    assert_eq!(summary_row(&step_summary(&success), "act")["status"], "ran");
}
