#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect idiomatically"
)]

use serde_json::{Value, json};
use upeg_core::{InputAdapterError, InputSpec};

const FILE_WIRE_KEY: &str = "x-upeg-file-wire";

fn legacy_file_schema() -> Value {
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

fn supported_file_wire() -> Value {
    json!({
        "version": 1,
        "bytesEncoding": "base64-rfc4648-padded",
        "legacyNumericArrays": false,
        "recursiveDirectories": true,
        "documentation": "README.md#file-input-wire"
    })
}

#[test]
fn file_wire_extension_preserves_the_exact_contract_across_a_schema_roundtrip() {
    let spec = InputSpec::try_from(&legacy_file_schema())
        .expect("must import the external legacy schema without the extension");

    let exported = spec.to_json_schema_value();
    assert_eq!(
        exported["properties"]["upload"][FILE_WIRE_KEY],
        supported_file_wire()
    );

    let imported =
        InputSpec::try_from(&exported).expect("must re-import the exported file wire extension");
    assert_eq!(imported, spec);
}

#[test]
fn file_wire_extension_absent_accepts_a_legacy_schema() {
    let imported = InputSpec::try_from(&legacy_file_schema());

    assert!(imported.is_ok());
}

#[test]
fn file_wire_extension_rejects_unknown_keys_and_unsupported_values() {
    let malformed = [
        ("not an object", json!([])),
        (
            "missing required key",
            json!({
                "version": 1,
                "bytesEncoding": "base64-rfc4648-padded",
                "legacyNumericArrays": false,
                "recursiveDirectories": true
            }),
        ),
        (
            "unknown key",
            json!({
                "version": 1,
                "bytesEncoding": "base64-rfc4648-padded",
                "legacyNumericArrays": false,
                "recursiveDirectories": true,
                "documentation": "README.md#file-input-wire",
                "future": true
            }),
        ),
        ("unsupported version", {
            let mut value = supported_file_wire();
            value["version"] = json!(2);
            value
        }),
        ("unsupported bytes encoding", {
            let mut value = supported_file_wire();
            value["bytesEncoding"] = json!("base64url");
            value
        }),
        ("numeric array compatibility enabled", {
            let mut value = supported_file_wire();
            value["legacyNumericArrays"] = json!(true);
            value
        }),
        ("recursive directories disabled", {
            let mut value = supported_file_wire();
            value["recursiveDirectories"] = json!(false);
            value
        }),
        ("documentation path mismatch", {
            let mut value = supported_file_wire();
            value["documentation"] = json!("docs/private.md");
            value
        }),
    ];

    for (case, extension) in malformed {
        let mut schema = legacy_file_schema();
        schema["properties"]["upload"][FILE_WIRE_KEY] = extension;

        assert!(
            matches!(
                InputSpec::try_from(&schema),
                Err(InputAdapterError::JsonSchemaFileWire { property, .. })
                    if property == "upload"
            ),
            "the {case} extension must be rejected with a typed error"
        );
    }
}
