use super::*;

#[test]
fn interface_inventory_exposes_file_wire_contract() {
    let id = "file_wire_test.upload";
    crate::toolbox_add_tool(ToolMeta {
        id,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "file_wire_test")
            .expect("test tool id must be in canonical form")
            .local(),
        tags: &[],
        display_label: "File wire test",
        description: "File wire inventory fixture",
        input_spec: upeg_core::InputSpec::try_from(&serde_json::json!({
            "type": "object",
            "properties": {
                "upload": {
                    "type": "object",
                    "x-upeg-kind": "file"
                }
            }
        }))
        .expect("legacy File schema must parse"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::External,
        surfaces: &[Surface::Http],
        boards: &[],
    });
    let entry = collect_tool_entries()
        .into_iter()
        .find(|entry| {
            entry.id == "tool.file_wire_test.upload"
                && entry.surfaces.contains(&Surface::Http)
                && entry.kind == InterfaceKind::Tool
        })
        .expect("an HTTP File wire inventory entry must exist");
    let schema = entry
        .contract
        .input
        .and_then(|contract| contract.schema)
        .expect("input schema must be present");

    assert_eq!(
        schema["properties"]["upload"]["x-upeg-file-wire"],
        serde_json::json!({
            "version": 1,
            "bytesEncoding": "base64-rfc4648-padded",
            "legacyNumericArrays": false,
            "recursiveDirectories": true,
            "documentation": "README.md#file-input-wire"
        })
    );
}

#[test]
fn interface_inventory_exposes_file_wire_contract_for_file_output() {
    let id = "file_wire_test.output";
    crate::toolbox_add_tool(ToolMeta {
        id,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "file_wire_test")
            .expect("test tool id must be in canonical form")
            .local(),
        tags: &[],
        display_label: "File output wire test",
        description: "File output wire inventory fixture",
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
        invoker: Invoker::External,
        surfaces: &[Surface::Http],
        boards: &[],
    });
    let entry = collect_tool_entries()
        .into_iter()
        .find(|entry| {
            entry.id == "tool.file_wire_test.output"
                && entry.surfaces.contains(&Surface::Http)
                && entry.kind == InterfaceKind::Tool
        })
        .expect("an HTTP File output wire inventory entry must exist");
    let schema = entry
        .contract
        .output
        .and_then(|contract| contract.schema)
        .expect("output schema must be present");

    assert_eq!(
        schema["x-upeg-output-fields"]["properties"]["artifact"]["x-upeg-file-wire"],
        serde_json::json!({
            "version": 1,
            "bytesEncoding": "base64-rfc4648-padded",
            "legacyNumericArrays": false,
            "recursiveDirectories": true,
            "documentation": "README.md#file-input-wire"
        })
    );
}

#[test]
fn interface_inventory_preserves_number_input_constraints() {
    let id = "file_wire_test.number_constraints";
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
    crate::toolbox_add_tool(ToolMeta {
        id,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "file_wire_test")
            .expect("test tool id must be in canonical form")
            .local(),
        tags: &[],
        display_label: "Number constraint test",
        description: "Number constraint inventory fixture",
        input_spec,
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::External,
        surfaces: &[Surface::Http],
        boards: &[],
    });
    let schema = collect_tool_entries()
        .into_iter()
        .find(|entry| {
            entry.id == "tool.file_wire_test.number_constraints"
                && entry.surfaces.contains(&Surface::Http)
                && entry.kind == InterfaceKind::Tool
        })
        .and_then(|entry| entry.contract.input)
        .and_then(|contract| contract.schema)
        .expect("number input schema must be present");

    assert_eq!(
        schema["properties"]["limit"],
        serde_json::json!({
            "type": "number",
            "minimum": 1.0,
            "maximum": 64.0,
            "default": 16.0
        })
    );
}
