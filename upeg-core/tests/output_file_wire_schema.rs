#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect idiomatically"
)]

use serde_json::json;
use upeg_core::{FieldConstraints, OutputFieldSpec, OutputKind, OutputSpec};

#[test]
fn file_output_schema_exposes_the_canonical_file_wire_contract() {
    let spec = OutputSpec::new(vec![OutputFieldSpec {
        name: "artifact".to_string(),
        label: None,
        description: None,
        kind: OutputKind::File,
        constraints: FieldConstraints::default(),
    }])
    .expect("File output spec must be valid");

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
