use super::{OUTPUT_CONVERSION_ERROR_CODE, output_value_from_text, try_runtime_dispatch};
use upeg_core::{
    FieldConstraints, FileContent, FileValue, InputSpec, Invoker, OutputEntry, OutputFieldSpec,
    OutputKind, OutputSpec, OutputValue, PegboardUnits, PinKind, Source, ToolMeta, ToolResult,
    ToolSuccess,
};

const FILE_OUTPUT_NODE_BUDGET_EXCESS_CHILD_COUNT: u64 = upeg_core::MAX_FILE_OUTPUT_NODES;
const EXTERNAL_TYPED_FILE_TOOL_ID: &str = "test.output_budget.external_typed_file";
const CHAIN_TYPED_FILE_TOOL_ID: &str = "test.output_budget.chain_typed_file";
const NATIVE_TYPED_FILE_TOOL_ID: &str = "test.output_budget.native_typed_file";

fn file_json_exceeding_node_budget() -> String {
    let entries = (0..FILE_OUTPUT_NODE_BUDGET_EXCESS_CHILD_COUNT)
        .map(|index| {
            serde_json::json!({
                "name": format!("{index}.bin"),
                "is_dir": false,
                "content": {
                    "kind": "bytes",
                    "bytes": "",
                },
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "name": "bundle",
        "is_dir": true,
        "content": {
            "kind": "directory",
            "entries": entries,
        },
    })
    .to_string()
}

fn file_value_exceeding_node_budget() -> FileValue {
    let entries = (0..FILE_OUTPUT_NODE_BUDGET_EXCESS_CHILD_COUNT)
        .map(|index| FileValue {
            name: format!("{index}.bin"),
            mime: None,
            content: FileContent::Bytes(Vec::new()),
        })
        .collect();
    FileValue {
        name: "bundle".to_string(),
        mime: None,
        content: FileContent::Directory(entries),
    }
}

fn file_tool_meta(id: &'static str, local_id: &'static str, invoker: Invoker) -> ToolMeta {
    ToolMeta {
        id,
        toolkit: "test",
        local_id,
        tags: &[],
        display_label: "File output budget fixture",
        description: "",
        input_spec: InputSpec::empty(),
        output_spec: OutputSpec::new(vec![OutputFieldSpec {
            name: "archive".to_string(),
            label: None,
            description: None,
            kind: OutputKind::File,
            constraints: FieldConstraints::default(),
        }])
        .expect("fixture output spec is valid"),
        primary_output_id: Some("archive"),
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    }
}

fn success_result_exceeding_node_budget() -> ToolResult {
    ToolResult::Success(
        ToolSuccess::new(
            Some("archive".to_string()),
            vec![OutputEntry {
                id: "archive".to_string(),
                label: None,
                kind: OutputKind::File,
                value: OutputValue::File(file_value_exceeding_node_budget()),
            }],
        )
        .expect("fixture success is valid"),
    )
}

#[test]
fn plugin_file_output_checks_aggregate_node_budget_before_decode() {
    let error = output_value_from_text(&OutputKind::File, &file_json_exceeding_node_budget(), true)
        .expect_err("plugin File output must reject exceeding the aggregate node budget");

    assert!(error.contains("node count"), "{error}");
}

#[test]
fn external_typed_file_output_checks_budget_at_final_runtime_boundary() {
    let _registration = crate::toolbox_add_tool_with_dispatcher_managed(
        file_tool_meta(
            EXTERNAL_TYPED_FILE_TOOL_ID,
            "output_budget.external_typed_file",
            Invoker::External,
        ),
        |_| success_result_exceeding_node_budget(),
    );

    let result = try_runtime_dispatch(EXTERNAL_TYPED_FILE_TOOL_ID, &serde_json::json!({}))
        .expect("fixture dispatcher is registered");

    let ToolResult::Failure(failure) = result else {
        panic!("External typed File output must reject budget excess at the runtime boundary");
    };
    assert_eq!(failure.error.code, OUTPUT_CONVERSION_ERROR_CODE);
    assert!(failure.error.message.contains("node count"));
}

#[test]
fn chain_typed_file_output_checks_budget_at_final_runtime_boundary() {
    let _registration = crate::toolbox_add_tool_with_dispatcher_managed(
        file_tool_meta(
            CHAIN_TYPED_FILE_TOOL_ID,
            "output_budget.chain_typed_file",
            Invoker::Chain,
        ),
        |_| success_result_exceeding_node_budget(),
    );

    let result = try_runtime_dispatch(CHAIN_TYPED_FILE_TOOL_ID, &serde_json::json!({}))
        .expect("fixture dispatcher is registered");

    let ToolResult::Failure(failure) = result else {
        panic!("Chain typed File output must reject budget excess at the runtime boundary");
    };
    assert_eq!(failure.error.code, OUTPUT_CONVERSION_ERROR_CODE);
    assert!(failure.error.message.contains("node count"));
}

#[test]
fn trusted_native_typed_file_output_keeps_existing_behavior() {
    let _registration = crate::toolbox_add_tool_with_dispatcher_managed(
        file_tool_meta(
            NATIVE_TYPED_FILE_TOOL_ID,
            "output_budget.native_typed_file",
            Invoker::Function,
        ),
        |_| success_result_exceeding_node_budget(),
    );

    let result = try_runtime_dispatch(NATIVE_TYPED_FILE_TOOL_ID, &serde_json::json!({}))
        .expect("fixture dispatcher is registered");

    assert!(
        matches!(result, ToolResult::Success(_)),
        "trusted native typed File output must be unaffected by the new untrusted boundary policy"
    );
}
