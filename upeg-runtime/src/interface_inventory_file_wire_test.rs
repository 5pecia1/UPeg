use super::*;

#[test]
fn 인터페이스_인벤토리는_file_wire_계약을_노출한다() {
    let id = "file_wire_test.upload";
    crate::toolbox_add_tool(ToolMeta {
        id,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "file_wire_test")
            .expect("테스트 도구 id가 정규 형식이어야 한다")
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
        .expect("레거시 File schema를 가져와야 한다"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
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
        .expect("HTTP File wire 인벤토리 항목이 있어야 한다");
    let schema = entry
        .contract
        .input
        .and_then(|contract| contract.schema)
        .expect("입력 schema가 있어야 한다");

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
fn 인터페이스_인벤토리는_파일_출력의_file_wire_계약을_노출한다() {
    let id = "file_wire_test.output";
    crate::toolbox_add_tool(ToolMeta {
        id,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "file_wire_test")
            .expect("테스트 도구 id가 정규 형식이어야 한다")
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
        .expect("File 출력 명세가 유효해야 한다"),
        primary_output_id: Some("artifact"),
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
        .expect("HTTP File 출력 wire 인벤토리 항목이 있어야 한다");
    let schema = entry
        .contract
        .output
        .and_then(|contract| contract.schema)
        .expect("출력 schema가 있어야 한다");

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
fn 인터페이스_인벤토리는_숫자_입력_제약을_보존한다() {
    let id = "file_wire_test.number_constraints";
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
    crate::toolbox_add_tool(ToolMeta {
        id,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "file_wire_test")
            .expect("테스트 도구 id가 정규 형식이어야 한다")
            .local(),
        tags: &[],
        display_label: "Number constraint test",
        description: "Number constraint inventory fixture",
        input_spec,
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
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
        .expect("숫자 입력 schema가 있어야 한다");

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
