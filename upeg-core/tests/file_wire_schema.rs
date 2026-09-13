#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect idiomatically"
)]

use serde_json::{Value, json};
use upeg_core::{InputAdapterError, InputSpec};

const FILE_WIRE_KEY: &str = "x-upeg-file-wire";

fn 레거시_파일_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "upload": {
                "type": "object",
                "x-upeg-kind": "file"
            }
        },
        "required": ["upload"],
        "additionalProperties": false
    })
}

fn 지원되는_file_wire() -> Value {
    json!({
        "version": 1,
        "bytesEncoding": "base64-rfc4648-padded",
        "legacyNumericArrays": false,
        "recursiveDirectories": true,
        "documentation": "README.md#file-input-wire"
    })
}

#[test]
fn 파일_wire_확장은_schema_왕복에서_정확한_계약을_보존한다() {
    let spec = InputSpec::try_from(&레거시_파일_schema())
        .expect("확장이 없는 외부 레거시 schema를 가져와야 한다");

    let exported = spec.to_json_schema_value();
    assert_eq!(
        exported["properties"]["upload"][FILE_WIRE_KEY],
        지원되는_file_wire()
    );

    let imported =
        InputSpec::try_from(&exported).expect("내보낸 파일 wire 확장을 다시 가져와야 한다");
    assert_eq!(imported, spec);
}

#[test]
fn 파일_wire_확장은_누락되면_레거시_schema를_허용한다() {
    let imported = InputSpec::try_from(&레거시_파일_schema());

    assert!(imported.is_ok());
}

#[test]
fn 파일_wire_확장은_알수없는_키와_지원하지_않는_값을_거부한다() {
    let malformed = [
        ("객체 아님", json!([])),
        (
            "필수 키 누락",
            json!({
                "version": 1,
                "bytesEncoding": "base64-rfc4648-padded",
                "legacyNumericArrays": false,
                "recursiveDirectories": true
            }),
        ),
        (
            "알 수 없는 키",
            json!({
                "version": 1,
                "bytesEncoding": "base64-rfc4648-padded",
                "legacyNumericArrays": false,
                "recursiveDirectories": true,
                "documentation": "README.md#file-input-wire",
                "future": true
            }),
        ),
        ("지원하지 않는 버전", {
            let mut value = 지원되는_file_wire();
            value["version"] = json!(2);
            value
        }),
        ("지원하지 않는 bytes 인코딩", {
            let mut value = 지원되는_file_wire();
            value["bytesEncoding"] = json!("base64url");
            value
        }),
        ("숫자 배열 호환 활성화", {
            let mut value = 지원되는_file_wire();
            value["legacyNumericArrays"] = json!(true);
            value
        }),
        ("재귀 디렉터리 비활성화", {
            let mut value = 지원되는_file_wire();
            value["recursiveDirectories"] = json!(false);
            value
        }),
        ("문서 경로 불일치", {
            let mut value = 지원되는_file_wire();
            value["documentation"] = json!("docs/private.md");
            value
        }),
    ];

    for (case, extension) in malformed {
        let mut schema = 레거시_파일_schema();
        schema["properties"]["upload"][FILE_WIRE_KEY] = extension;

        assert!(
            matches!(
                InputSpec::try_from(&schema),
                Err(InputAdapterError::JsonSchemaFileWire { property, .. })
                    if property == "upload"
            ),
            "{case} 확장은 typed 오류로 거부해야 한다"
        );
    }
}
