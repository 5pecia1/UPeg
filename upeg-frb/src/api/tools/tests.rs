//! Unit tests for the `api::tools` DTO conversions and dispatch entry
//! points.
//!
//! Split out of `tools.rs` so the module stays inside the 1000-line
//! file budget; `super::*` still resolves to `api::tools`, so the
//! bodies are unchanged from when they lived inline.

use super::*;
use upeg_core::{
    ALL_SURFACES, FieldConstraints, InputSpec, OutputFieldSpec, OutputKind, OutputSpec, Source,
};

/// Build a minimal `ToolMeta` with the requested pin/invoker/units
/// triple so tests can assert the DTO conversion picks them up.
fn fixture_meta(
    id: &'static str,
    pin: PinKind,
    invoker: Invoker,
    units: PegboardUnits,
) -> ToolMeta {
    fixture_meta_with_outputs(id, pin, invoker, units, OutputSpec::empty())
}

fn fixture_meta_with_outputs(
    id: &'static str,
    pin: PinKind,
    invoker: Invoker,
    units: PegboardUnits,
    output_spec: OutputSpec,
) -> ToolMeta {
    ToolMeta {
        id,
        toolkit: "frb_tool_test",
        local_id: id
            .strip_prefix("frb_tool_test.")
            .expect("test ToolMeta id must include the test toolkit prefix"),
        tags: &[],
        display_label: "frb tool test",
        description: "",
        input_spec: InputSpec::empty(),
        output_spec,
        primary_output_id: None,
        source: Source::UserInput,
        pin,
        pegboard_units: units,
        invoker,
        surfaces: ALL_SURFACES,
        boards: &[],
    }
}

#[test]
fn tool_dto_가_pin_kind_embed를_올바르게_노출한다() {
    let meta = fixture_meta(
        "frb_tool_test.embed_one",
        PinKind::Embed,
        Invoker::External,
        PegboardUnits::U1,
    );
    // ToolDto::from takes `&'static ToolMeta`; leak the local meta
    // so the conversion can borrow it for the duration of the test.
    let leaked: &'static ToolMeta = Box::leak(Box::new(meta));
    let dto = ToolDto::from(leaked);
    assert_eq!(dto.pin_kind, PinKindDto::Embed);
    assert_eq!(dto.invoker, InvokerDto::External);
}

#[test]
fn 승인_장벽이_없는_도구의_dto는_확인을_요구하지_않는다() {
    let meta = fixture_meta(
        "frb_tool_test.no_gate",
        PinKind::Inline,
        Invoker::Function,
        PegboardUnits::U1,
    );
    let leaked: &'static ToolMeta = Box::leak(Box::new(meta));

    let dto = ToolDto::from(leaked);

    assert!(!dto.requires_approval);
    assert!(
        dto.approval_surfaces.is_empty(),
        "승인할 것이 없으면 승인자를 지목하지 않는다"
    );
}

#[test]
fn 게이트된_도구의_dto는_확인_필요와_승인_가능_표면을_함께_싣는다() {
    // Dart가 dispatch **전에** 읽는 두 값이다: "확인을 띄워야 하나",
    // 그리고 "내 표면의 확인이 인정되나".
    let meta = fixture_meta(
        "frb_tool_test.gated",
        PinKind::Chain,
        Invoker::Chain,
        PegboardUnits::U1,
    );
    let leaked: &'static ToolMeta = Box::leak(Box::new(meta));
    upeg_runtime::set_tool_approval_policy(
        leaked.id,
        upeg_runtime::ToolApprovalPolicy::gated(vec![
            upeg_core::Surface::Cli,
            upeg_core::Surface::Desktop,
        ]),
    );

    let dto = ToolDto::from(leaked);

    assert!(dto.requires_approval);
    assert_eq!(
        dto.approval_surfaces,
        vec!["cli".to_string(), "desktop".to_string()]
    );
}

#[test]
fn tool_dto_가_pegboard_units_u2t를_올바르게_노출한다() {
    let meta = fixture_meta(
        "frb_tool_test.tall_one",
        PinKind::Inline,
        Invoker::Function,
        PegboardUnits::U2T,
    );
    let leaked: &'static ToolMeta = Box::leak(Box::new(meta));
    let dto = ToolDto::from(leaked);
    assert_eq!(dto.pegboard_units, PegboardUnitsDto::U2T);
}

#[test]
fn tool_dto는_output_fields를_타입과_라벨까지_노출한다() {
    let spec = OutputSpec::new(vec![
        OutputFieldSpec {
            name: "decimal".to_string(),
            label: Some("Decimal".to_string()),
            description: Some("base-10 result".to_string()),
            kind: OutputKind::Number,
            constraints: FieldConstraints::default(),
        },
        OutputFieldSpec {
            name: "preview".to_string(),
            label: None,
            description: None,
            kind: OutputKind::EmbeddedView {
                url: "https://example.test/preview".to_string(),
            },
            constraints: FieldConstraints::default(),
        },
        OutputFieldSpec {
            name: "payload".to_string(),
            label: Some("Payload".to_string()),
            description: None,
            kind: OutputKind::Json,
            constraints: FieldConstraints::default(),
        },
    ])
    .expect("valid output spec");
    let meta = fixture_meta_with_outputs(
        "frb_tool_test.typed_outputs",
        PinKind::Inline,
        Invoker::Function,
        PegboardUnits::U1,
        spec,
    );
    let leaked: &'static ToolMeta = Box::leak(Box::new(meta));
    let dto = ToolDto::from(leaked);

    assert_eq!(dto.output_fields.len(), 3);
    assert_eq!(dto.output_fields[0].key, "decimal");
    assert_eq!(dto.output_fields[0].label, "Decimal");
    assert_eq!(
        dto.output_fields[0].description.as_deref(),
        Some("base-10 result")
    );
    assert_eq!(dto.output_fields[0].field_type, OutputFieldType::Number);
    assert_eq!(
        dto.output_fields[1].field_type,
        OutputFieldType::EmbeddedView {
            url: "https://example.test/preview".to_string(),
        }
    );
    assert_eq!(dto.output_fields[2].field_type, OutputFieldType::Json);
}

#[test]
fn canonical_tool_result는_성공_스키마를_정규_output으로_노출한다() {
    let outcome = dispatch_tool(
        "num.hex_to_decimal".to_string(),
        r#"{"input":"0xff"}"#.to_string(),
        None,
        false,
    );

    assert!(outcome.ok, "unexpected error: {:?}", outcome.error);
    assert_eq!(outcome.primary_output_id.as_deref(), Some("result"));
    assert!(outcome.error.is_none());
    assert_eq!(outcome.outputs.len(), 1);

    let output = &outcome.outputs[0];
    assert_eq!(output.id, "result");
    assert!(output.label.is_none());
    assert_eq!(output.kind, "number");
    assert_eq!(output.value, CanonicalOutputValue::Number { value: 255.0 });
}

#[test]
fn canonical_tool_result는_실패_스키마를_정규_error로_노출한다() {
    let outcome = dispatch_tool(
        "nonexistent.tool".to_string(),
        "{}".to_string(),
        None,
        false,
    );

    assert!(!outcome.ok);
    assert!(outcome.primary_output_id.is_none());
    assert!(outcome.outputs.is_empty());
    let error = outcome.error.expect("canonical error");
    assert_eq!(error.code, TOOL_NOT_FOUND_ERROR_CODE);
    assert!(error.message.contains("not registered"));
    assert!(error.details.is_none());
}

#[test]
fn dispatch_tool은_비정규_tool_id를_거부한다() {
    let outcome = dispatch_tool(
        " num.hex_to_decimal ".to_string(),
        "{}".to_string(),
        None,
        false,
    );
    assert!(!outcome.ok);
    let error = outcome.error.expect("canonical error");
    assert_eq!(error.code, INVALID_TOOL_ID_ERROR_CODE);
    assert!(error.message.contains("tool_id validation error"));
}

#[test]
fn dispatch_tool은_output_spec이_있으면_정규_output_entry를_반환한다() {
    let outcome = dispatch_tool(
        "num.hex_to_decimal".to_string(),
        r#"{"input":"0xff"}"#.to_string(),
        None,
        false,
    );

    assert!(outcome.ok, "unexpected error: {:?}", outcome.error);
    assert_eq!(outcome.primary_output_id.as_deref(), Some("result"));
    assert_eq!(outcome.outputs[0].id, "result");
    assert_eq!(outcome.outputs[0].kind, "number");
}

#[test]
fn dispatch_tool_core는_cli없이_내장_dispatcher를_실행한다() {
    let outcome = dispatch_tool_impl("num.hex_to_decimal", r#"{"input":"0xff"}"#, None, false);

    assert!(outcome.ok, "unexpected error: {:?}", outcome.error);
    assert_eq!(
        outcome.outputs[0].value,
        CanonicalOutputValue::Number { value: 255.0 }
    );
}

#[test]
fn tool_dto는_timer_source를_노출한다() {
    let mut meta = fixture_meta(
        "frb_tool_test.timer_source",
        PinKind::Live,
        Invoker::Http,
        PegboardUnits::U1,
    );
    meta.source = Source::Timer {
        interval: std::time::Duration::from_millis(30_000),
    };
    let leaked: &'static ToolMeta = Box::leak(Box::new(meta));
    let dto = ToolDto::from(leaked);
    assert_eq!(
        dto.source,
        SourceDto::Timer {
            interval_ms: 30_000
        }
    );
}

#[test]
fn tool_dto는_user_input_source를_기본으로_노출한다() {
    let meta = fixture_meta(
        "frb_tool_test.userinput_source",
        PinKind::Inline,
        Invoker::Function,
        PegboardUnits::U1,
    );
    let leaked: &'static ToolMeta = Box::leak(Box::new(meta));
    let dto = ToolDto::from(leaked);
    assert_eq!(dto.source, SourceDto::UserInput);
}

#[test]
fn tool_dto_가_모든_invoker_변형을_매핑한다() {
    let cases = [
        (Invoker::Function, InvokerDto::Function),
        (Invoker::External, InvokerDto::External),
        (Invoker::Http, InvokerDto::Http),
        (Invoker::Embed, InvokerDto::Embed),
        (Invoker::Chain, InvokerDto::Chain),
        (Invoker::Llm, InvokerDto::Llm),
        (Invoker::Wasm, InvokerDto::Wasm),
    ];
    for (source, expected) in cases {
        assert_eq!(InvokerDto::from(source), expected);
    }
}

#[test]
fn 실패_결과는_구조화된_details를_json_문자열로_넘긴다() {
    // The External invoker packs the child's exit code and both
    // captured streams into `ToolError::details`; the bridge has to
    // carry them across so the Flutter outcome block can show the
    // diagnostics instead of only the summary line.
    let details = upeg_core::ProcessErrorDetails {
        termination: upeg_core::ProcessTermination::Exited { code: 1 },
        stdout: "warning: unused".to_string(),
        stderr: String::new(),
    }
    .to_value();
    let result = CanonicalToolResult::from(ToolResult::Failure(upeg_core::ToolFailure {
        error: upeg_core::ToolError {
            code: "tool_error".to_string(),
            message: "`cargo` exited with code 1".to_string(),
            details: Some(details.clone()),
        },
    }));

    assert!(!result.ok);
    let error = result.error.expect("failure carries a structured error");
    assert_eq!(error.message, "`cargo` exited with code 1");
    let decoded: serde_json::Value = serde_json::from_str(
        &error
            .details
            .expect("details cross the bridge as JSON text"),
    )
    .expect("details text is valid JSON");
    assert_eq!(decoded, details);
}

#[test]
fn details가_없는_실패는_널로_남는다() {
    let result = CanonicalToolResult::from(ToolResult::Failure(upeg_core::ToolFailure {
        error: upeg_core::ToolError {
            code: "invalid_args".to_string(),
            message: "bad".to_string(),
            details: None,
        },
    }));
    assert_eq!(
        result
            .error
            .expect("failure carries a structured error")
            .details,
        None
    );
}

// ─── 승인 인자 정형화 ──────────────────────────────────────────
//
// 데스크톱 dispatch가 승인되는 경로는 타입 있는 `approve` 플래그 하나뿐이다.
// Dart가 직접 만든 args 맵이 예약 키를 심어 스스로를 승인하지 못한다.

#[test]
fn approve가_거짓이면_호출자가_넣은_예약_키를_지운다() {
    let args = serde_json::json!({ "input": "0xff", APPROVE_RESERVED_ARG: true });

    let shaped = shape_approval_arg(args, false);

    assert_eq!(shaped.get(APPROVE_RESERVED_ARG), None);
    assert_eq!(
        shaped.get("input").and_then(serde_json::Value::as_str),
        Some("0xff"),
        "다른 인자는 그대로 지나간다"
    );
}

#[test]
fn approve가_참이면_예약_키를_참으로_넣는다() {
    let shaped = shape_approval_arg(serde_json::json!({ "input": "0xff" }), true);

    assert_eq!(
        shaped.get(APPROVE_RESERVED_ARG),
        Some(&serde_json::Value::Bool(true))
    );
}

#[test]
fn approve가_참이면_호출자의_거짓_예약_키를_덮어쓴다() {
    let args = serde_json::json!({ APPROVE_RESERVED_ARG: false });

    let shaped = shape_approval_arg(args, true);

    assert_eq!(
        shaped.get(APPROVE_RESERVED_ARG),
        Some(&serde_json::Value::Bool(true))
    );
}

#[test]
fn 객체가_아닌_args는_승인_정형화를_그대로_통과한다() {
    let args = serde_json::json!([1, 2, 3]);

    let shaped = shape_approval_arg(args.clone(), true);

    assert_eq!(shaped, args, "넣을 자리가 없는 값은 건드리지 않는다");
}

#[test]
fn 정형화는_호출자가_넣은_approved_steps도_지운다() {
    // 승인 레버는 둘이다. `approve`만 지우면 나머지 하나가 데이터
    // 경로로 남는다 — 그리고 `_upeg.approvedSteps`는 예약 블록의
    // wipe에서 살아남는 유일한 키다(`upeg-runtime/src/execution.rs`).
    let args = serde_json::json!({
        "input": "0xff",
        upeg_core::EXECUTION_CONTEXT_ARG: {
            upeg_core::EXECUTION_CONTEXT_APPROVED_STEPS: ["gate"],
            upeg_core::EXECUTION_CONTEXT_SURFACE: "desktop",
        },
    });

    let shaped = shape_approval_arg(args, false);

    let context = &shaped[upeg_core::EXECUTION_CONTEXT_ARG];
    assert_eq!(
        context.get(upeg_core::EXECUTION_CONTEXT_APPROVED_STEPS),
        None
    );
    assert_eq!(
        context[upeg_core::EXECUTION_CONTEXT_SURFACE],
        "desktop",
        "각인된 나머지 블록은 그대로 남는다"
    );
}

#[test]
fn approve가_참이어도_approved_steps는_다시_쓰이지_않는다() {
    // 타입 있는 플래그가 두 레버의 유일한 필자다. 그리고 그것이 쓰는
    // 것은 `approve` 하나 — GUI의 확인은 언제나 "이 호출 전체"다.
    let args = serde_json::json!({
        upeg_core::EXECUTION_CONTEXT_ARG: {
            upeg_core::EXECUTION_CONTEXT_APPROVED_STEPS: ["gate"],
        },
    });

    let shaped = shape_approval_arg(args, true);

    assert_eq!(
        shaped[upeg_core::EXECUTION_CONTEXT_ARG].get(upeg_core::EXECUTION_CONTEXT_APPROVED_STEPS),
        None
    );
    assert_eq!(
        shaped.get(APPROVE_RESERVED_ARG),
        Some(&serde_json::Value::Bool(true))
    );
}
