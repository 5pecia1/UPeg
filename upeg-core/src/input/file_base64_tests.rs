use serde_json::{Map, Value};

use super::file_base64::canonical_base64_decoded_len;
use super::{
    FileInputPolicy, FileInputPolicyParams, FileInputValueError, InputFieldSpec, InputKind,
    InputName, InputSpec, InputValueError, MAX_FILE_INPUT_RAW_BYTES,
};

#[test]
fn canonical_base64_decoded_length_is_computed_without_allocation() {
    const VALID_VECTORS: &[(&str, usize)] = &[
        ("", 0),
        ("Zg==", 1),
        ("Zm8=", 2),
        ("Zm9v", 3),
        ("/w==", 1),
        ("//8=", 2),
        ("////", 3),
    ];

    for (encoded, expected) in VALID_VECTORS {
        assert_eq!(
            canonical_base64_decoded_len(encoded),
            Ok(*expected),
            "{encoded:?}"
        );
    }
}

#[test]
fn non_canonical_base64_alphabet_padding_and_tail_bits_are_rejected() {
    const NON_CANONICAL_VECTORS: &[&str] = &[
        "Zg",
        "Zg===",
        "Z g==",
        "-w==",
        "_w==",
        "=AAA",
        "A=AA",
        "AA=A",
        "A===",
        "====",
        "Zh==",
        "Zm9=",
        "AA/=",
        "8J+YgA==\n",
        "가===",
    ];

    for encoded in NON_CANONICAL_VECTORS {
        assert!(
            canonical_base64_decoded_len(encoded).is_err(),
            "{encoded:?}"
        );
    }
}

#[test]
fn structural_preflight_never_invokes_the_base64_decoder() {
    const PREFLIGHT_SOURCE: &str = include_str!("file_base64.rs");
    const STRUCTURE_SOURCE: &str = include_str!("file_structure.rs");

    for source in [PREFLIGHT_SOURCE, STRUCTURE_SOURCE] {
        assert!(
            !source.contains(".decode("),
            "the validation boundary must not allocate a decoded Vec"
        );
    }
}

#[test]
fn encoded_input_over_the_fixed_raw_byte_limit_is_rejected_before_decode_allocation() {
    const OVER_LIMIT_BYTES: u64 = MAX_FILE_INPUT_RAW_BYTES + 1;
    const DECODED_QUANTUM_BYTES: u64 = 3;
    const ZERO_BASE64_QUANTUM: &str = "AAAA";

    let decoded_len = usize::try_from(OVER_LIMIT_BYTES).expect("test size must fit in usize");
    assert_eq!(OVER_LIMIT_BYTES % DECODED_QUANTUM_BYTES, 0);
    let encoded = ZERO_BASE64_QUANTUM.repeat(
        usize::try_from(OVER_LIMIT_BYTES / DECODED_QUANTUM_BYTES)
            .expect("test quartet count must fit in usize"),
    );
    assert_eq!(
        canonical_base64_decoded_len(&encoded),
        Ok(decoded_len),
        "preflight must compute the over-limit size without a decoded Vec"
    );

    let policy = FileInputPolicy::default();
    let field = InputFieldSpec::new(
        InputName::new("upload").expect("input name must be valid"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("file field must be valid");
    let spec = InputSpec::new(vec![field]).expect("input spec must be valid");
    let mut content = Map::new();
    content.insert("kind".to_string(), Value::String("bytes".to_string()));
    content.insert("bytes".to_string(), Value::String(encoded));
    let mut file = Map::new();
    file.insert("name".to_string(), Value::String("large.bin".to_string()));
    file.insert("is_dir".to_string(), Value::Bool(false));
    file.insert("content".to_string(), Value::Object(content));
    let mut args = Map::new();
    args.insert("upload".to_string(), Value::Object(file));

    let result = spec.validate_json_args(&args);

    assert!(matches!(
        result,
        Err(InputValueError::File(FileInputValueError::TotalTooLarge {
            max: MAX_FILE_INPUT_RAW_BYTES,
            actual: OVER_LIMIT_BYTES,
            ..
        }))
    ));
}

#[test]
fn per_policy_limits_apply_before_raw_json_is_decoded_into_a_file_value() {
    let policy = FileInputPolicy::try_from(FileInputPolicyParams {
        max_file_bytes: Some(3),
        ..FileInputPolicyParams::default()
    })
    .expect("file policy must be valid");
    let field = InputFieldSpec::new(
        InputName::new("upload").expect("input name must be valid"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("file field must be valid");
    let spec = InputSpec::new(vec![field]).expect("input spec must be valid");
    let mut args = Map::new();
    args.insert(
        "upload".to_string(),
        serde_json::json!({
            "name": "four.bin",
            "is_dir": false,
            "content": {"kind": "bytes", "bytes": "AQIDBA=="}
        }),
    );

    let result = spec.validate_json_args(&args);

    assert!(matches!(
        result,
        Err(InputValueError::File(FileInputValueError::FileTooLarge {
            max: 3,
            actual: 4,
            ..
        }))
    ));
}
