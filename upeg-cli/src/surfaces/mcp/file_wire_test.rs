use super::*;

#[test]
fn mcp_tools_list_exposes_file_wire_contract() {
    let response = handle(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    }))
    .expect("a tools/list response is owed");
    let tool = response["result"]["tools"]
        .as_array()
        .expect("tools must be an array")
        .iter()
        .find(|tool| tool["name"] == "media.images_convert")
        .expect("media.images_convert must be exposed on MCP");

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
fn mcp_tools_list_exposes_file_wire_contract_for_file_outputs() {
    const TOOL_ID: &str = "file_wire_test.mcp_output";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: TOOL_ID,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(TOOL_ID, "file_wire_test")
            .expect("test tool id must be canonical")
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
        .expect("File output spec must be valid"),
        primary_output_id: Some("artifact"),
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
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
    .expect("a tools/list response is owed");
    let tool = response["result"]["tools"]
        .as_array()
        .expect("tools must be an array")
        .iter()
        .find(|tool| tool["name"] == TOOL_ID)
        .expect("test tool must be exposed on MCP");

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
fn mcp_tools_list_preserves_number_input_constraints() {
    const TOOL_ID: &str = "file_wire_test.mcp_number_constraints";
    let input_spec = upeg_core::InputSpec::new(vec![
        upeg_core::InputFieldSpec::with_constraints(
            upeg_core::InputName::new("limit").expect("input name must be valid"),
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
        .expect("number input constraints must be valid"),
    ])
    .expect("input spec must be valid");
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: TOOL_ID,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(TOOL_ID, "file_wire_test")
            .expect("test tool id must be canonical")
            .local(),
        tags: &[],
        display_label: "MCP number constraints",
        description: "MCP number constraint fixture",
        input_spec,
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
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
    .expect("a tools/list response is owed");
    let schema = response["result"]["tools"]
        .as_array()
        .expect("tools must be an array")
        .iter()
        .find(|tool| tool["name"] == TOOL_ID)
        .map(|tool| &tool["inputSchema"]["properties"]["limit"])
        .expect("test tool must be exposed on MCP");

    assert_eq!(
        schema,
        &json!({"type": "number", "minimum": 1.0, "maximum": 64.0, "default": 16.0})
    );
}
