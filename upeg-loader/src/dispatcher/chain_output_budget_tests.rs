use super::output_value_from_text;
use crate::ToolToml;
use crate::dispatcher::external_dispatcher_for;
use upeg_core::{OutputKind, ToolResult};

const FILE_OUTPUT_NODE_BUDGET_EXCESS_CHILD_COUNT: u64 = upeg_core::MAX_FILE_OUTPUT_NODES;
const OUTPUT_CONVERSION_ERROR_CODE: &str = "output_conversion_error";

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

#[test]
fn loader_체인_file_출력은_aggregate_노드_예산을_decode_전에_검사한다() {
    let error = output_value_from_text(&OutputKind::File, &노드_예산을_초과한_file_json())
        .expect_err("loader 체인의 File 출력은 aggregate 노드 예산 초과를 거부해야 한다");

    assert!(error.contains("node count"), "{error}");
}

#[test]
fn loader_외부_프로세스_file_출력은_aggregate_예산을_넘으면_실패한다() {
    let mut parsed = toml::from_str::<ToolToml>(
        r#"id = "test.output_budget.external_process"
            toolkit = "test"
            invoker = "External"
            command = "printf"
            primary_output_id = "archive"
            outputs = [{ name = "archive", type = "file" }]"#,
    )
    .expect("fixture manifest is valid");
    parsed.args_template = Some(vec!["%s".to_string(), "{payload}".to_string()]);
    let dispatcher = external_dispatcher_for(&parsed, None).expect("External dispatcher is built");
    let args = serde_json::json!({ "payload": 노드_예산을_초과한_file_json() });

    let result = dispatcher(upeg_runtime::DispatchArgs::parse(&args).expect("object args"));

    let ToolResult::Failure(failure) = result else {
        panic!("External process File 출력은 aggregate 예산 초과를 거부해야 한다");
    };
    assert_eq!(failure.error.code, OUTPUT_CONVERSION_ERROR_CODE);
    assert!(
        failure.error.message.contains("node count"),
        "{}",
        failure.error.message
    );
}
