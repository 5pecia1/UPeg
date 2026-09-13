use super::{call_dispatcher, call_dispatcher_result, single_tool_toml_str, tool_success};
use crate::dispatcher::{DEFAULT_APPROVAL_SURFACES, chain_dispatcher_for};
use crate::{ChainStepToml, ToolToml, load_and_register_dir};
use upeg_core::{OutputEntry, OutputKind, OutputValue, ToolResult, ToolSuccess};

fn 성공_결과(primary: &str, outputs: Vec<OutputEntry>) -> ToolResult {
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

fn 문자열_출력(id: &str, value: &str) -> OutputEntry {
    OutputEntry {
        id: id.to_string(),
        label: None,
        kind: OutputKind::String,
        value: OutputValue::String(value.to_string()),
    }
}

#[test]
fn 체인_필드가_없으면_체인_dispatcher는_없다() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y""#,
    )
    .unwrap();
    assert!(chain_dispatcher_for(&parsed).is_none());
}

#[test]
fn 체인_dispatcher는_단일_단계를_위임한다() {
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
fn 암시적_순서가_없는_체인_dispatcher의_독립_단계는_루트_인자를_받는다() {
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
fn 체인_dispatcher는_표준출력을_다음_단계_입력으로_연결한다() {
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
fn 체인_dispatcher는_기본으로_primary_output을_다음_단계에_전달한다() {
    upeg_runtime::register_runtime_dispatcher("test.chain.primary.source", |_| {
        성공_결과(
            "machine",
            vec![
                OutputEntry {
                    id: "label".into(),
                    label: Some("Rendered Label".into()),
                    kind: OutputKind::String,
                    value: OutputValue::String("사람용 라벨".into()),
                },
                문자열_출력("machine", "canonical"),
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
fn 체인_dispatcher는_명시된_출력_필드를_단계_인자로_전달한다() {
    upeg_runtime::register_runtime_dispatcher("test.chain.field.source", |_| {
        성공_결과(
            "primary",
            vec![
                문자열_출력("primary", "default"),
                문자열_출력("raw", "selected"),
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
fn 체인_dispatcher는_마지막_단계의_정규화된_결과를_그대로_반환한다() {
    upeg_runtime::register_runtime_dispatcher("test.chain.final.source", |_| {
        성공_결과(
            "primary",
            vec![
                문자열_출력("primary", "canonical"),
                문자열_출력("debug", "trace"),
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
fn 체인_dispatcher_연결은_같은_도구를_별도_노드로_허용한다() {
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
fn 체인_dispatcher_풍부한_단계는_식_분기_승인과_출력을_지원한다() {
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
fn 체인_dispatcher_방향성비순환그래프_단계는_선언된_의존성_출력을_받는다() {
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
fn 체인_dispatcher의_팬아웃_조인_꼬리_시나리오는_명시적_연결을_사용한다() {
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
fn 체인_dispatcher는_단계_오류에서_짧게_중단한다() {
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
fn 체인_dispatcher는_알수없는_단계에_오류를_반환한다() {
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
fn 체인의_체인은_두_계층을_통해_dispatch한다() {
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
fn 체인_dispatcher는_단계_id를_잘라낸다() {
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
fn 외부_호출자가_있는_단계는_충돌한다() {
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

// ─── 승인 인가 (approval authorization) ──────────────────────────────

/// `_upeg.surface`를 각인한 호출 봉투. surface는 서버 surface가 각인하므로
/// 호출자가 위조할 수 없다 — 그래서 승인 인가의 기준이 된다.
fn 표면_승인_호출(surface: &str) -> serde_json::Value {
    serde_json::json!({ "approve": true, "_upeg": { "surface": surface } })
}

fn 실패(result: ToolResult) -> upeg_core::ToolError {
    match result {
        ToolResult::Failure(failure) => failure.error,
        ToolResult::Success(success) => panic!("실패를 기대했으나 성공: {success:?}"),
    }
}

/// `requires_approval` step 하나짜리 체인. `approval_surfaces`를 그대로
/// 이어 붙여 선언 여부/내용만 바꿔 가며 재사용한다.
fn 승인_체인(id: &str, approval_surfaces: Option<&str>) -> ToolToml {
    upeg_runtime::register_single_text_runtime_dispatcher("test.approval.echo", |args| {
        Ok(args
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("ok")
            .to_string())
    });
    let 선언 =
        approval_surfaces.map_or_else(String::new, |list| format!("approval_surfaces = {list}\n"));
    toml::from_str::<ToolToml>(&format!(
        r#"id = "test.approval.{id}"
            toolkit = "test"
            invoker = "Chain"
            {선언}
            [[steps]]
            id = "gate"
            tool = "test.approval.echo"
            requires_approval = true
            args = '{{"input":"done"}}'
        "#
    ))
    .expect("유효한 승인 체인 fixture")
}

#[test]
fn cli_표면의_승인은_기본_approval_surfaces에서_인정된다() {
    let f = chain_dispatcher_for(&승인_체인("cli_ok", None)).expect("dispatcher built");
    let out = call_dispatcher(&f, 표면_승인_호출("cli")).expect("cli 표면 승인은 인정되어야 한다");
    assert_eq!(out, "done");
}

#[test]
fn mcp_표면의_승인은_표면_인가에서_거부된다() {
    let f = chain_dispatcher_for(&승인_체인("mcp_denied", None)).expect("dispatcher built");
    let error = 실패(call_dispatcher_result(&f, 표면_승인_호출("mcp")));
    assert_eq!(error.code, "approval_denied_for_surface");
    assert!(
        error.message.contains("surface `mcp`") && error.message.contains("only from cli"),
        "거부 메시지는 누가 승인할 수 있는지 알려야 한다: {}",
        error.message
    );
    assert!(
        error
            .message
            .contains("upeg call test.approval.mcp_denied -a approve=true"),
        "거부 메시지는 대신 실행할 명령을 그대로 알려야 한다: {}",
        error.message
    );
}

#[test]
fn http_표면의_approved_steps도_표면_인가에서_거부된다() {
    let f = chain_dispatcher_for(&승인_체인("http_denied", None)).expect("dispatcher built");
    let args = serde_json::json!({
        "_upeg": { "surface": "http", "approvedSteps": ["gate"] }
    });
    let error = 실패(call_dispatcher_result(&f, args));
    assert_eq!(error.code, "approval_denied_for_surface");
}

#[test]
fn approval_surfaces가_mcp를_지목하면_mcp_승인이_인정된다() {
    let f = chain_dispatcher_for(&승인_체인("mcp_allowed", Some(r#"["mcp"]"#)))
        .expect("dispatcher built");
    let out = call_dispatcher(&f, 표면_승인_호출("mcp"))
        .expect("명시적으로 지목된 surface의 승인은 인정되어야 한다");
    assert_eq!(out, "done");

    let error = 실패(call_dispatcher_result(&f, 표면_승인_호출("cli")));
    assert_eq!(
        error.code, "approval_denied_for_surface",
        "명시 선언은 기본값을 대체한다 — cli는 더 이상 포함되지 않는다"
    );
}

#[test]
fn 승인_가능한_표면이어도_승인을_보내지_않으면_승인_요구다() {
    let f = chain_dispatcher_for(&승인_체인("cli_missing", None)).expect("dispatcher built");
    let error = 실패(call_dispatcher_result(
        &f,
        serde_json::json!({ "_upeg": { "surface": "cli" } }),
    ));
    assert_eq!(error.code, "approval_required");
}

#[test]
fn 게이트된_체인은_ui가_읽을_승인_정책을_등록한다() {
    // UI가 dispatch **전에** 물어보는 값이다: 확인을 띄워야 하는가,
    // 그리고 내 표면의 확인이 인정되는가.
    let toml = 승인_체인("policy_default", None);
    let _ = chain_dispatcher_for(&toml).expect("dispatcher built");

    let policy = upeg_runtime::tool_approval_policy(&toml.id);

    assert!(policy.requires_approval());
    assert_eq!(policy.surfaces(), DEFAULT_APPROVAL_SURFACES);
}

#[test]
fn 명시된_approval_surfaces가_등록되는_정책의_실효_집합이다() {
    let toml = 승인_체인("policy_declared", Some(r#"["mcp"]"#));
    let _ = chain_dispatcher_for(&toml).expect("dispatcher built");

    let policy = upeg_runtime::tool_approval_policy(&toml.id);

    assert!(policy.honors(upeg_core::Surface::Mcp));
    assert!(!policy.honors(upeg_core::Surface::Cli));
}

#[test]
fn 승인_step이_없는_체인은_승인_정책을_등록하지_않는다() {
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
    .expect("유효한 체인 fixture");

    let _ = chain_dispatcher_for(&toml).expect("dispatcher built");

    let policy = upeg_runtime::tool_approval_policy(&toml.id);
    assert!(!policy.requires_approval());
    assert!(policy.surfaces().is_empty());
}

#[test]
fn 알수없는_approval_surfaces를_가진_dispatcher는_호출을_실패시킨다() {
    // 로더는 이 매니페스트를 거부한다. 손으로 만든 ToolToml만 여기 도달하므로
    // 기본값으로 조용히 내려앉지 않고 호출을 실패시킨다.
    let f = chain_dispatcher_for(&승인_체인("bad_surface", Some(r#"["nope"]"#)))
        .expect("dispatcher built");
    let error = 실패(call_dispatcher_result(&f, 표면_승인_호출("cli")));
    assert!(
        error.message.contains("approval_surfaces[0]"),
        "got {}",
        error.message
    );
}

// ─── step 메타데이터 ─────────────────────────────────────────────────

fn step_요약(success: &upeg_core::ToolSuccess) -> Vec<serde_json::Value> {
    let entry = success
        .outputs
        .iter()
        .find(|entry| entry.id == "steps")
        .expect("모든 chain 성공 봉투는 steps 행을 싣는다");
    let OutputValue::Json(serde_json::Value::Array(rows)) = &entry.value else {
        panic!("steps 행은 JSON 배열이어야 한다");
    };
    rows.clone()
}

fn 요약_행<'a>(rows: &'a [serde_json::Value], id: &str) -> &'a serde_json::Value {
    rows.iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("step `{id}` 요약 행이 없다: {rows:?}"))
}

#[test]
fn 체인_성공_봉투는_실행과_건너뛴_step을_모두_요약한다() {
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
    let rows = step_요약(&success);
    assert_eq!(rows.len(), 2);
    assert_eq!(요약_행(&rows, "first")["status"], "ran");
    assert_eq!(요약_행(&rows, "first")["tool"], "test.summary.ran");
    assert!(요약_행(&rows, "first")["duration_ms"].is_u64());
    assert_eq!(요약_행(&rows, "second")["status"], "skipped");
}

#[test]
fn 체인_실패_봉투는_details_steps에_step_상태를_싣는다() {
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

    let error = 실패(call_dispatcher_result(&f, serde_json::json!({})));
    let details = error.details.expect("실패 봉투도 step 요약을 실어야 한다");
    let rows = details["steps"].as_array().expect("steps 배열").clone();
    assert_eq!(요약_행(&rows, "before")["status"], "ran");
    assert_eq!(요약_행(&rows, "missing")["status"], "failed");
}

#[test]
fn 승인_거부_봉투의_step_요약은_denied를_담는다() {
    let f = chain_dispatcher_for(&승인_체인("denied_summary", None)).expect("dispatcher built");
    let error = 실패(call_dispatcher_result(&f, 표면_승인_호출("mcp")));
    let details = error.details.expect("거부 봉투도 step 요약을 실어야 한다");
    let rows = details["steps"].as_array().expect("steps 배열").clone();
    assert_eq!(요약_행(&rows, "gate")["status"], "denied");
}

#[test]
fn 자기_출력이_없는_체인은_step_요약이_primary가_된다() {
    // action-only step: 정본 봉투는 primary 없이 output row를 실을 수
    // 없으므로, 엔진 행이 유일한 행이면 그것이 primary가 된다.
    upeg_runtime::register_runtime_dispatcher("test.summary.action_only", |_| {
        ToolResult::Success(ToolSuccess::new(None, Vec::new()).expect("action-only 성공"))
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
    assert_eq!(요약_행(&step_요약(&success), "act")["status"], "ran");
}
