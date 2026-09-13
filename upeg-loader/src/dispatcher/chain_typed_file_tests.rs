use upeg_core::{
    FileContent, FileValue, MAX_FILE_OUTPUT_RAW_BYTES, OutputEntry, OutputKind, OutputValue,
    ToolResult, ToolSuccess,
};
use upeg_runtime::{DispatchArgs, tool_success_primary_text};

use super::{ChainJobResult, ChainState, OutputAdapter, RuntimeStep, chain_dispatcher_for};
use crate::ToolToml;

fn file_success(bytes: Vec<u8>) -> ToolResult {
    ToolResult::Success(
        ToolSuccess::new(
            Some("file".to_string()),
            vec![OutputEntry {
                id: "file".to_string(),
                label: None,
                kind: OutputKind::File,
                value: OutputValue::File(FileValue {
                    name: "result.bin".to_string(),
                    mime: Some("application/octet-stream".to_string()),
                    content: FileContent::Bytes(bytes),
                }),
            }],
        )
        .expect("유효한 File 성공 결과"),
    )
}

fn declared_file_adapter() -> OutputAdapter {
    let tool = toml::from_str::<ToolToml>(
        r#"id = "test.chain.typed_file"
toolkit = "test"
outputs = [{ name = "file", type = "file" }]
primary_output_id = "file""#,
    )
    .expect("유효한 File 출력 Tool");
    OutputAdapter::from_tool(&tool)
}

#[test]
fn 최종_file_체인은_64mib_버퍼를_텍스트_투영_없이_이동한다() {
    let byte_count =
        usize::try_from(MAX_FILE_OUTPUT_RAW_BYTES).expect("File 출력 예산은 usize 범위");
    let bytes = vec![0xa5; byte_count];
    let original_pointer = bytes.as_ptr();
    let mut state = ChainState::new(1);
    state
        .apply_result(ChainJobResult {
            key: "source".to_string(),
            tool: "test.chain.typed_file.source".to_string(),
            result: Some(file_success(bytes)),
            duration_ms: 0,
        })
        .expect("최종 단계 적용");
    let steps = [RuntimeStep {
        key: "source".to_string(),
        tool: "test.chain.typed_file.source".to_string(),
        args: None,
        when: None,
        upstream: Vec::new(),
        requires_approval: false,
    }];

    let result = state
        .last_step_result(&steps, &declared_file_adapter())
        .expect("최종 File 결과");
    let ToolResult::Success(success) = result else {
        panic!("성공 결과여야 한다");
    };
    let OutputValue::File(file) = success.outputs.into_iter().next().expect("File 출력").value
    else {
        panic!("typed File 출력이어야 한다");
    };
    let FileContent::Bytes(returned) = file.content else {
        panic!("bytes File이어야 한다");
    };

    assert_eq!(returned.as_ptr(), original_pointer);
}

#[test]
fn downstream이_file_출력을_요청하면_canonical_wire로_전달한다() {
    upeg_runtime::register_runtime_dispatcher("test.chain.file_wire.source", |_| {
        file_success(vec![0, 1, 2, 0xff])
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain.file_wire.sink", |args| {
        let wire = args
            .get("input")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "File wire 입력이 없다".to_string())?;
        let file = serde_json::from_str::<FileValue>(wire)
            .map_err(|error| format!("File wire 역직렬화 실패: {error}"))?;
        let FileContent::Bytes(bytes) = file.content else {
            return Err("bytes File이 아니다".to_string());
        };
        Ok(bytes.len().to_string())
    });
    let tool = toml::from_str::<ToolToml>(
        r#"id = "test.chain.file_wire"
toolkit = "test"
invoker = "Chain"
connections = [{ from = "source", to = "sink" }]
steps = [
    { id = "source", tool = "test.chain.file_wire.source" },
    { id = "sink", tool = "test.chain.file_wire.sink" },
]"#,
    )
    .expect("유효한 chain Tool");
    let dispatcher = chain_dispatcher_for(&tool).expect("chain dispatcher");
    let input = serde_json::json!({});
    let args = DispatchArgs::parse(&input).expect("유효한 dispatch 인자");

    let ToolResult::Success(success) = dispatcher(args) else {
        panic!("chain이 성공해야 한다");
    };

    assert_eq!(tool_success_primary_text(&success), "4");
}
