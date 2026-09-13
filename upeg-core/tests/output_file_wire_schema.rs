#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect idiomatically"
)]

use serde_json::json;
use upeg_core::{FieldConstraints, OutputFieldSpec, OutputKind, OutputSpec};

#[test]
fn 파일_출력_schema는_정식_file_wire_계약을_노출한다() {
    let spec = OutputSpec::new(vec![OutputFieldSpec {
        name: "artifact".to_string(),
        label: None,
        description: None,
        kind: OutputKind::File,
        constraints: FieldConstraints::default(),
    }])
    .expect("File 출력 명세가 유효해야 한다");

    let schema = spec.to_json_schema_value();

    assert_eq!(
        schema["properties"]["artifact"]["x-upeg-file-wire"],
        json!({
            "version": 1,
            "bytesEncoding": "base64-rfc4648-padded",
            "legacyNumericArrays": false,
            "recursiveDirectories": true,
            "documentation": "README.md#file-input-wire"
        })
    );
}
