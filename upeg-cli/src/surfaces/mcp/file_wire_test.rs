use super::*;

#[test]
fn mcp_tools_list는_file_wire_계약을_노출한다() {
    let response = handle(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    }))
    .expect("tools/list 응답이 있어야 한다");
    let tool = response["result"]["tools"]
        .as_array()
        .expect("tools 배열이어야 한다")
        .iter()
        .find(|tool| tool["name"] == "media.images_convert")
        .expect("media.images_convert가 MCP에 노출되어야 한다");

    assert_eq!(
        tool["inputSchema"]["properties"]["images"]["x-upeg-file-wire"],
        json!({
            "version": 1,
            "bytesEncoding": "base64-rfc4648-padded",
            "legacyNumericArrays": false,
            "recursiveDirectories": true,
            "documentation": "README.md#file-input-wire"
        })
    );
}

#[test]
fn mcp_tools_list는_파일_출력의_file_wire_계약을_노출한다() {
    const TOOL_ID: &str = "file_wire_test.mcp_output";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: TOOL_ID,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(TOOL_ID, "file_wire_test")
            .expect("테스트 도구 id가 정규 형식이어야 한다")
            .local(),
        tags: &[],
        display_label: "MCP File output",
        description: "MCP File output wire fixture",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::new(vec![upeg_core::OutputFieldSpec {
            name: "artifact".to_string(),
            label: None,
            description: None,
            kind: upeg_core::OutputKind::File,
            constraints: upeg_core::FieldConstraints::default(),
        }])
        .expect("File 출력 명세가 유효해야 한다"),
        primary_output_id: Some("artifact"),
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Mcp],
        boards: &[],
    });

    let response = handle(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }))
    .expect("tools/list 응답이 있어야 한다");
    let tool = response["result"]["tools"]
        .as_array()
        .expect("tools 배열이어야 한다")
        .iter()
        .find(|tool| tool["name"] == TOOL_ID)
        .expect("테스트 도구가 MCP에 노출되어야 한다");

    assert_eq!(
        tool["outputSchema"]["properties"]["artifact"]["x-upeg-file-wire"],
        json!({
            "version": 1,
            "bytesEncoding": "base64-rfc4648-padded",
            "legacyNumericArrays": false,
            "recursiveDirectories": true,
            "documentation": "README.md#file-input-wire"
        })
    );
}

#[test]
fn mcp_tools_list는_숫자_입력_제약을_보존한다() {
    const TOOL_ID: &str = "file_wire_test.mcp_number_constraints";
    let input_spec = upeg_core::InputSpec::new(vec![
        upeg_core::InputFieldSpec::with_constraints(
            upeg_core::InputName::new("limit").expect("입력 이름이 유효해야 한다"),
            None,
            None,
            true,
            upeg_core::InputKind::Number,
            upeg_core::FieldConstraints {
                number: Some(upeg_core::NumberConstraints {
                    min: Some(1.0),
                    max: Some(64.0),
                    default: Some(16.0),
                }),
                string: None,
            },
        )
        .expect("숫자 입력 제약이 유효해야 한다"),
    ])
    .expect("입력 명세가 유효해야 한다");
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: TOOL_ID,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(TOOL_ID, "file_wire_test")
            .expect("테스트 도구 id가 정규 형식이어야 한다")
            .local(),
        tags: &[],
        display_label: "MCP number constraints",
        description: "MCP number constraint fixture",
        input_spec,
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Mcp],
        boards: &[],
    });

    let response = handle(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/list"
    }))
    .expect("tools/list 응답이 있어야 한다");
    let schema = response["result"]["tools"]
        .as_array()
        .expect("tools 배열이어야 한다")
        .iter()
        .find(|tool| tool["name"] == TOOL_ID)
        .map(|tool| &tool["inputSchema"]["properties"]["limit"])
        .expect("테스트 도구가 MCP에 노출되어야 한다");

    assert_eq!(
        schema,
        &json!({"type": "number", "minimum": 1.0, "maximum": 64.0, "default": 16.0})
    );
}
