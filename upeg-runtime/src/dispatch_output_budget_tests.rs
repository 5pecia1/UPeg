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

fn 노드_예산을_초과한_file_json() -> String {
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

fn 노드_예산을_초과한_file_value() -> FileValue {
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
        source: Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    }
}

fn 노드_예산을_초과한_성공_결과() -> ToolResult {
    ToolResult::Success(
        ToolSuccess::new(
            Some("archive".to_string()),
            vec![OutputEntry {
                id: "archive".to_string(),
                label: None,
                kind: OutputKind::File,
                value: OutputValue::File(노드_예산을_초과한_file_value()),
            }],
        )
        .expect("fixture success is valid"),
    )
}

#[test]
fn plugin_file_출력은_aggregate_노드_예산을_decode_전에_검사한다() {
    let error = output_value_from_text(&OutputKind::File, &노드_예산을_초과한_file_json(), true)
        .expect_err("plugin File 출력은 aggregate 노드 예산 초과를 거부해야 한다");

    assert!(error.contains("node count"), "{error}");
}

#[test]
fn external_typed_file_출력은_최종_runtime_경계에서_예산을_검사한다() {
    let _registration = crate::toolbox_add_tool_with_dispatcher_managed(
        file_tool_meta(
            EXTERNAL_TYPED_FILE_TOOL_ID,
            "output_budget.external_typed_file",
            Invoker::External,
        ),
        |_| 노드_예산을_초과한_성공_결과(),
    );

    let result = try_runtime_dispatch(EXTERNAL_TYPED_FILE_TOOL_ID, &serde_json::json!({}))
        .expect("fixture dispatcher is registered");

    let ToolResult::Failure(failure) = result else {
        panic!("External typed File 출력은 runtime 경계에서 예산 초과를 거부해야 한다");
    };
    assert_eq!(failure.error.code, OUTPUT_CONVERSION_ERROR_CODE);
    assert!(failure.error.message.contains("node count"));
}

#[test]
fn chain_typed_file_출력은_최종_runtime_경계에서_예산을_검사한다() {
    let _registration = crate::toolbox_add_tool_with_dispatcher_managed(
        file_tool_meta(
            CHAIN_TYPED_FILE_TOOL_ID,
            "output_budget.chain_typed_file",
            Invoker::Chain,
        ),
        |_| 노드_예산을_초과한_성공_결과(),
    );

    let result = try_runtime_dispatch(CHAIN_TYPED_FILE_TOOL_ID, &serde_json::json!({}))
        .expect("fixture dispatcher is registered");

    let ToolResult::Failure(failure) = result else {
        panic!("Chain typed File 출력은 runtime 경계에서 예산 초과를 거부해야 한다");
    };
    assert_eq!(failure.error.code, OUTPUT_CONVERSION_ERROR_CODE);
    assert!(failure.error.message.contains("node count"));
}

#[test]
fn trusted_native_typed_file_출력은_기존_동작을_유지한다() {
    let _registration = crate::toolbox_add_tool_with_dispatcher_managed(
        file_tool_meta(
            NATIVE_TYPED_FILE_TOOL_ID,
            "output_budget.native_typed_file",
            Invoker::Function,
        ),
        |_| 노드_예산을_초과한_성공_결과(),
    );

    let result = try_runtime_dispatch(NATIVE_TYPED_FILE_TOOL_ID, &serde_json::json!({}))
        .expect("fixture dispatcher is registered");

    assert!(
        matches!(result, ToolResult::Success(_)),
        "trusted native typed File 출력은 새 untrusted 경계 정책의 영향을 받지 않아야 한다"
    );
}
