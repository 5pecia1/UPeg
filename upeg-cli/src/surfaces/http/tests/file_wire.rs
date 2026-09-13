use super::*;

#[tokio::test]
async fn http_openapi는_file_wire_계약을_노출한다() {
    let schema = get_ok_json(router(), "/v1/openapi.json").await;
    let file_wire = &schema["paths"]["/v1/tools/media.images_convert"]["post"]["requestBody"]["content"]
        ["application/json"]["schema"]["properties"]["images"]["x-upeg-file-wire"];

    assert_eq!(
        file_wire,
        &json!({
            "version": 1,
            "bytesEncoding": "base64-rfc4648-padded",
            "legacyNumericArrays": false,
            "recursiveDirectories": true,
            "documentation": "README.md#file-input-wire"
        })
    );
}

#[tokio::test]
async fn http_도구_목록은_파일_출력의_file_wire_계약을_노출한다() {
    const TOOL_ID: &str = "file_wire_test.http_output";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: TOOL_ID,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(TOOL_ID, "file_wire_test")
            .expect("테스트 도구 id가 정규 형식이어야 한다")
            .local(),
        tags: &[],
        display_label: "HTTP File output",
        description: "HTTP File output wire fixture",
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
        surfaces: &[upeg_core::Surface::Http],
        boards: &[],
    });

    let response = get_ok_json(router(), "/v1/tools").await;
    let tool = response["tools"]
        .as_array()
        .expect("tools 배열이어야 한다")
        .iter()
        .find(|tool| tool["name"] == TOOL_ID)
        .expect("테스트 도구가 HTTP에 노출되어야 한다");

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

#[tokio::test]
async fn http_openapi는_파일_출력의_file_wire_계약을_노출한다() {
    const TOOL_ID: &str = "file_wire_test.openapi_output";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: TOOL_ID,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(TOOL_ID, "file_wire_test")
            .expect("테스트 도구 id가 정규 형식이어야 한다")
            .local(),
        tags: &[],
        display_label: "OpenAPI File output",
        description: "OpenAPI File output wire fixture",
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
        surfaces: &[upeg_core::Surface::Http],
        boards: &[],
    });

    let schema = get_ok_json(router(), "/v1/openapi.json").await;
    let output = &schema["paths"][format!("/v1/tools/{TOOL_ID}")]["post"]["x-upeg-output-fields"];

    assert_eq!(
        output["properties"]["artifact"]["x-upeg-file-wire"],
        json!({
            "version": 1,
            "bytesEncoding": "base64-rfc4648-padded",
            "legacyNumericArrays": false,
            "recursiveDirectories": true,
            "documentation": "README.md#file-input-wire"
        })
    );
}

#[tokio::test]
async fn http_openapi는_숫자_입력_제약을_보존한다() {
    const TOOL_ID: &str = "file_wire_test.openapi_number_constraints";
    let input_spec = upeg_core::InputSpec::new(vec![
        upeg_core::InputFieldSpec::with_constraints(
            upeg_core::InputName::new("limit").expect("입력 이름이 유효해야 한다"),
            None,
            None,
            true,
            upeg_core::InputKind::Integer,
            upeg_core::FieldConstraints {
                number: Some(upeg_core::NumberConstraints {
                    min: Some(1.0),
                    max: Some(64.0),
                    default: Some(16.0),
                }),
                string: None,
            },
        )
        .expect("정수 입력 제약이 유효해야 한다"),
    ])
    .expect("입력 명세가 유효해야 한다");
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: TOOL_ID,
        toolkit: "file_wire_test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(TOOL_ID, "file_wire_test")
            .expect("테스트 도구 id가 정규 형식이어야 한다")
            .local(),
        tags: &[],
        display_label: "OpenAPI number constraints",
        description: "OpenAPI number constraint fixture",
        input_spec,
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Http],
        boards: &[],
    });

    let schema = get_ok_json(router(), "/v1/openapi.json").await;
    let limit = &schema["paths"][format!("/v1/tools/{TOOL_ID}")]["post"]["requestBody"]["content"]
        ["application/json"]["schema"]["properties"]["limit"];

    assert_eq!(
        limit,
        &json!({"type": "integer", "minimum": 1, "maximum": 64, "default": 16})
    );
}
