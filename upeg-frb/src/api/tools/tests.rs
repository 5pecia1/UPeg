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
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: Source::UserInput,
        pin,
        pegboard_units: units,
        invoker,
        surfaces: ALL_SURFACES,
        boards: &[],
    }
}

#[test]
fn tool_dto_exposes_pin_kind_embed_correctly() {
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
fn ungated_tool_dto_does_not_require_confirmation() {
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
        "nothing to approve means no approver surfaces are named"
    );
}

#[test]
fn gated_tool_dto_carries_requires_approval_and_approval_surfaces() {
    // The two values Dart reads **before** dispatch: "must I show a
    // confirmation", and "is a confirmation on my surface honored".
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
fn tool_dto_exposes_pegboard_units_u2t_correctly() {
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
fn tool_dto_exposes_output_fields_with_type_and_label() {
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
fn canonical_tool_result_exposes_success_schema_as_canonical_output() {
    let outcome = dispatch_tool(
        "num.hex_to_decimal".to_string(),
        r#"{"input":"0xff"}"#.to_string(),
        None,
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
fn canonical_tool_result_exposes_failure_schema_as_canonical_error() {
    let outcome = dispatch_tool(
        "nonexistent.tool".to_string(),
        "{}".to_string(),
        None,
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
fn web_toolkit_preparation_reuses_tool_id_validation() {
    let prepared = prepare_web_toolkit_dispatch(
        "not a tool id".to_owned(),
        "{}".to_owned(),
        None,
        None,
        false,
    );

    assert!(prepared.effective_args_json.is_none());
    assert_eq!(
        prepared
            .error
            .and_then(|error| error.error)
            .map(|error| error.code),
        Some(INVALID_TOOL_ID_ERROR_CODE.to_owned())
    );
}

#[test]
fn web_toolkit_preparation_shapes_reserved_approval_after_context() {
    let prepared = prepare_web_toolkit_dispatch(
        "num.hex_to_decimal".to_owned(),
        r#"{"input":"0xff","approve":true}"#.to_owned(),
        None,
        None,
        false,
    );

    assert!(prepared.error.is_none());
    let args: serde_json::Value = serde_json::from_str(
        prepared
            .effective_args_json
            .as_deref()
            .expect("valid dispatch must prepare args"),
    )
    .expect("prepared args are JSON");
    assert_eq!(args["input"], "0xff");
    assert!(args.get(APPROVE_RESERVED_ARG).is_none());
    assert!(args.get(upeg_core::EXECUTION_CONTEXT_ARG).is_some());
}

#[test]
fn dispatch_tool_rejects_noncanonical_tool_id() {
    let outcome = dispatch_tool(
        " num.hex_to_decimal ".to_string(),
        "{}".to_string(),
        None,
        None,
        false,
    );
    assert!(!outcome.ok);
    let error = outcome.error.expect("canonical error");
    assert_eq!(error.code, INVALID_TOOL_ID_ERROR_CODE);
    assert!(error.message.contains("tool_id validation error"));
}

#[test]
fn dispatch_tool_returns_canonical_output_entry_when_output_spec_present() {
    let outcome = dispatch_tool(
        "num.hex_to_decimal".to_string(),
        r#"{"input":"0xff"}"#.to_string(),
        None,
        None,
        false,
    );

    assert!(outcome.ok, "unexpected error: {:?}", outcome.error);
    assert_eq!(outcome.primary_output_id.as_deref(), Some("result"));
    assert_eq!(outcome.outputs[0].id, "result");
    assert_eq!(outcome.outputs[0].kind, "number");
}

#[test]
fn dispatch_tool_core_runs_builtin_dispatcher_without_cli() {
    let outcome = dispatch_tool_impl("num.hex_to_decimal", r#"{"input":"0xff"}"#, None, false);

    assert!(outcome.ok, "unexpected error: {:?}", outcome.error);
    assert_eq!(
        outcome.outputs[0].value,
        CanonicalOutputValue::Number { value: 255.0 }
    );
}

#[test]
fn tool_dto_exposes_timer_source() {
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
fn tool_dto_exposes_user_input_source_by_default() {
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
fn tool_dto_maps_all_invoker_variants() {
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
fn failure_result_carries_structured_details_as_json_string() {
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
fn failure_without_details_stays_null() {
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

// ─── Approval-argument shaping ─────────────────────────────────
//
// The only way a desktop dispatch is approved is the typed `approve`
// flag. An args map Dart built itself cannot self-approve by planting
// the reserved key.

#[test]
fn approve_false_strips_caller_supplied_reserved_key() {
    let args = serde_json::json!({ "input": "0xff", APPROVE_RESERVED_ARG: true });

    let shaped = shape_approval_arg(args, false);

    assert_eq!(shaped.get(APPROVE_RESERVED_ARG), None);
    assert_eq!(
        shaped.get("input").and_then(serde_json::Value::as_str),
        Some("0xff"),
        "other args pass through unchanged"
    );
}

#[test]
fn approve_true_sets_reserved_key_true() {
    let shaped = shape_approval_arg(serde_json::json!({ "input": "0xff" }), true);

    assert_eq!(
        shaped.get(APPROVE_RESERVED_ARG),
        Some(&serde_json::Value::Bool(true))
    );
}

#[test]
fn approve_true_overrides_caller_false_reserved_key() {
    let args = serde_json::json!({ APPROVE_RESERVED_ARG: false });

    let shaped = shape_approval_arg(args, true);

    assert_eq!(
        shaped.get(APPROVE_RESERVED_ARG),
        Some(&serde_json::Value::Bool(true))
    );
}

#[test]
fn non_object_args_pass_approval_shaping_through_unchanged() {
    let args = serde_json::json!([1, 2, 3]);

    let shaped = shape_approval_arg(args.clone(), true);

    assert_eq!(
        shaped, args,
        "a value with nowhere to put it is left untouched"
    );
}

#[test]
fn shaping_also_strips_caller_supplied_approved_steps() {
    // There are two approval levers. Removing only `approve` leaves the
    // other on the data path — and `_upeg.approvedSteps` is the only
    // key that survives the reserved-block wipe
    // (`upeg-runtime/src/execution.rs`).
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
        "the rest of the recorded block remains unchanged"
    );
}

#[test]
fn approved_steps_is_not_reused_even_when_approve_is_true() {
    // The typed flag is the only writer of the two levers. And what it
    // writes is `approve` alone — a GUI confirmation always covers
    // "this entire call".
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
