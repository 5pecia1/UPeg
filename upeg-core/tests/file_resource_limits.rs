#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect and panic idiomatically"
)]

use upeg_core::{
    FileContent, FileInputPolicy, FileInputPolicyError, FileInputPolicyParams, FileInputValueError,
    FilePolicySchemaError, FileValue, InputAdapterError, InputFieldSpec, InputKind, InputName,
    InputSpec, InputValue, InputValueError, MAX_FILE_INPUT_COUNT, MAX_FILE_INPUT_METADATA_BYTES,
    MAX_FILE_INPUT_NODES, MAX_FILE_INPUT_RAW_BYTES,
};

fn policy(max_count: u32) -> FileInputPolicy {
    FileInputPolicy::try_from(FileInputPolicyParams {
        max_count,
        ..FileInputPolicyParams::default()
    })
    .expect("file policy must be valid")
}

fn bytes_file(name: String, mime: Option<String>) -> FileValue {
    FileValue {
        name,
        mime,
        content: FileContent::Bytes(Vec::new()),
    }
}

fn directory(name: String, entries: Vec<FileValue>) -> FileValue {
    FileValue {
        name,
        mime: None,
        content: FileContent::Directory(entries),
    }
}

fn validate_file(policy: FileInputPolicy, file: FileValue) -> Result<(), InputValueError> {
    let field = InputFieldSpec::new(
        InputName::new("upload").expect("input name must be valid"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("file field must be valid");
    let spec = InputSpec::new(vec![field]).expect("input spec must be valid");
    let mut args = serde_json::Map::new();
    args.insert(
        "upload".to_string(),
        InputValue::File(file).into_json_value(),
    );

    spec.validate_json_args(&args)
}

fn node_tree(root_file_count: usize) -> FileValue {
    let mut entries = (0..63)
        .map(|index| {
            directory(
                format!("directory-{index}"),
                vec![bytes_file(format!("nested-{index}.bin"), None)],
            )
        })
        .collect::<Vec<_>>();
    entries.extend((0..root_file_count).map(|index| bytes_file(format!("root-{index}.bin"), None)));
    directory("root".to_string(), entries)
}

fn metadata_file(byte_count: usize) -> FileValue {
    const HANGUL_UTF8_BYTES: usize = "가".len();
    let hangul_count = byte_count / HANGUL_UTF8_BYTES;
    let remaining_bytes = byte_count % HANGUL_UTF8_BYTES;
    bytes_file(
        "가".repeat(hangul_count),
        (remaining_bytes > 0).then(|| "x".repeat(remaining_bytes)),
    )
}

#[test]
fn file_policy_accepts_the_maximum_file_count() {
    // Given
    let params = FileInputPolicyParams {
        max_count: MAX_FILE_INPUT_COUNT,
        ..FileInputPolicyParams::default()
    };

    // When
    let policy = FileInputPolicy::try_from(params).expect("must accept the exact limit");

    // Then
    assert_eq!(policy.max_count(), MAX_FILE_INPUT_COUNT);
}

#[test]
fn file_policy_rejects_max_count_above_the_maximum_with_a_typed_error() {
    // Given
    let actual = MAX_FILE_INPUT_COUNT + 1;
    let params = FileInputPolicyParams {
        max_count: actual,
        ..FileInputPolicyParams::default()
    };

    // When
    let result = FileInputPolicy::try_from(params);

    // Then
    assert_eq!(
        result,
        Err(FileInputPolicyError::MaxCountExceeded {
            max: MAX_FILE_INPUT_COUNT,
            actual,
        })
    );
}

#[test]
fn imported_file_policy_rejects_max_count_above_the_maximum_with_a_typed_error() {
    // Given
    let actual = MAX_FILE_INPUT_COUNT + 1;
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "upload": {
                "type": "object",
                "x-upeg-kind": "file",
                "x-upeg-file-policy": {
                    "maxCount": actual,
                    "extensions": []
                }
            }
        }
    });

    // When
    let result = InputSpec::try_from(&schema);

    // Then
    assert!(matches!(
        result,
        Err(InputAdapterError::JsonSchemaFilePolicy {
            source: FilePolicySchemaError::Policy(
                FileInputPolicyError::MaxCountExceeded {
                    max: MAX_FILE_INPUT_COUNT,
                    actual: imported_actual,
                }
            ),
            ..
        }) if imported_actual == actual
    ));
}

#[test]
fn file_input_accepts_the_exact_node_count_limit() {
    // Given
    let file = node_tree(1);

    // When
    let result = validate_file(policy(MAX_FILE_INPUT_COUNT), file);

    // Then
    assert_eq!(result, Ok(()));
}

#[test]
fn file_input_rejects_exceeding_the_node_count_limit() {
    // Given
    let file = node_tree(2);

    // When
    let result = validate_file(policy(MAX_FILE_INPUT_COUNT), file);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(
            FileInputValueError::NodeCountExceeded {
                max: MAX_FILE_INPUT_NODES,
                actual,
                ..
            }
        )) if actual == MAX_FILE_INPUT_NODES + 1
    ));
}

#[test]
fn file_input_accepts_the_exact_utf8_metadata_limit() {
    // Given
    let metadata_bytes =
        usize::try_from(MAX_FILE_INPUT_METADATA_BYTES).expect("metadata limit must fit in usize");
    let file = metadata_file(metadata_bytes);

    // When
    let result = validate_file(FileInputPolicy::default(), file);

    // Then
    assert_eq!(result, Ok(()));
}

#[test]
fn file_input_rejects_exceeding_the_utf8_metadata_limit() {
    // Given
    let metadata_bytes = usize::try_from(MAX_FILE_INPUT_METADATA_BYTES + 1)
        .expect("metadata limit must fit in usize");
    let file = metadata_file(metadata_bytes);

    // When
    let result = validate_file(FileInputPolicy::default(), file);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(
            FileInputValueError::MetadataTooLarge {
                max: MAX_FILE_INPUT_METADATA_BYTES,
                actual,
                ..
            }
        )) if actual == MAX_FILE_INPUT_METADATA_BYTES + 1
    ));
}

#[test]
fn file_input_rejects_exceeding_the_core_raw_input_limit() {
    // Given
    let core_raw_input_bytes =
        usize::try_from(MAX_FILE_INPUT_RAW_BYTES).expect("limit must fit in usize");
    let file = FileValue {
        name: "large.bin".to_string(),
        mime: None,
        content: FileContent::Bytes(vec![0; core_raw_input_bytes + 1]),
    };

    // When
    let result = validate_file(FileInputPolicy::default(), file);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(FileInputValueError::TotalTooLarge {
            max,
            actual,
            ..
        })) if max == MAX_FILE_INPUT_RAW_BYTES && actual == max + 1
    ));
}
