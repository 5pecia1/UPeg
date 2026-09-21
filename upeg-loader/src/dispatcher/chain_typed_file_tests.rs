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
        .expect("valid File success result"),
    )
}

fn declared_file_adapter() -> OutputAdapter {
    let tool = toml::from_str::<ToolToml>(
        r#"id = "test.chain.typed_file"
toolkit = "test"
outputs = [{ name = "file", type = "file" }]
primary_output_id = "file""#,
    )
    .expect("valid File-output Tool");
    OutputAdapter::from_tool(&tool)
}

#[test]
fn final_file_chain_moves_64mib_buffer_without_text_projection() {
    let byte_count =
        usize::try_from(MAX_FILE_OUTPUT_RAW_BYTES).expect("File output budget fits in usize");
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
        .expect("final step applied");
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
        .expect("final File result");
    let ToolResult::Success(success) = result else {
        panic!("must be a success result");
    };
    let OutputValue::File(file) = success
        .outputs
        .into_iter()
        .next()
        .expect("File output")
        .value
    else {
        panic!("must be a typed File output");
    };
    let FileContent::Bytes(returned) = file.content else {
        panic!("must be a bytes File");
    };

    assert_eq!(returned.as_ptr(), original_pointer);
}

#[test]
fn downstream_file_request_uses_canonical_wire() {
    upeg_runtime::register_runtime_dispatcher("test.chain.file_wire.source", |_| {
        file_success(vec![0, 1, 2, 0xff])
    });
    upeg_runtime::register_single_text_runtime_dispatcher("test.chain.file_wire.sink", |args| {
        let wire = args
            .get("input")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "no File wire input".to_string())?;
        let file = serde_json::from_str::<FileValue>(wire)
            .map_err(|error| format!("File wire deserialization failed: {error}"))?;
        let FileContent::Bytes(bytes) = file.content else {
            return Err("not a bytes File".to_string());
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
    .expect("valid chain Tool");
    let dispatcher = chain_dispatcher_for(&tool).expect("chain dispatcher");
    let input = serde_json::json!({});
    let args = DispatchArgs::parse(&input).expect("valid dispatch args");

    let ToolResult::Success(success) = dispatcher(args) else {
        panic!("the chain must succeed");
    };

    assert_eq!(tool_success_primary_text(&success), "4");
}
