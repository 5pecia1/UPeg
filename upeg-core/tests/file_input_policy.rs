#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    reason = "integration tests use unwrap/expect/panic idiomatically"
)]

use upeg_core::{
    FileContent, FileInputPolicy, FileInputPolicyError, FileInputPolicyParams, FileInputValueError,
    FilePolicySchemaError, FileValue, InputAdapterError, InputFieldSpec, InputKind, InputName,
    InputSpec, InputValue, InputValueError,
};

fn policy(
    max_count: u32,
    extensions: &[&str],
    max_file_bytes: Option<u64>,
    max_total_bytes: Option<u64>,
) -> FileInputPolicy {
    FileInputPolicy::try_from(FileInputPolicyParams {
        max_count,
        extensions: extensions
            .iter()
            .map(|extension| (*extension).to_string())
            .collect(),
        max_file_bytes,
        max_total_bytes,
    })
    .expect("file policy must be valid")
}

fn bytes_file(name: &str, bytes: &[u8]) -> FileValue {
    FileValue {
        name: name.to_string(),
        mime: None,
        content: FileContent::Bytes(bytes.to_vec()),
    }
}

fn directory(name: &str, entries: Vec<FileValue>) -> FileValue {
    FileValue {
        name: name.to_string(),
        mime: None,
        content: FileContent::Directory(entries),
    }
}

fn nested_file(depth: usize) -> FileValue {
    let mut file = bytes_file("leaf.bin", &[1]);
    for level in 1..depth {
        file = directory(&format!("level-{level}"), vec![file]);
    }
    file
}

fn file_spec(policy: FileInputPolicy) -> InputSpec {
    let field = InputFieldSpec::new(
        InputName::new("upload").expect("input name must be valid"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("file field must be valid");
    InputSpec::new(vec![field]).expect("input spec must be valid")
}

fn validate_file(policy: FileInputPolicy, file: FileValue) -> Result<(), InputValueError> {
    let spec = file_spec(policy);
    let mut args = serde_json::Map::new();
    args.insert(
        "upload".to_string(),
        InputValue::File(file).into_json_value(),
    );

    spec.validate_json_args(&args)
}

#[test]
fn file_input_kind_owns_the_policy() {
    // Given
    let expected = policy(2, &["png"], None, None);

    // When
    let kind = InputKind::File(expected.clone());

    // Then
    assert!(matches!(kind, InputKind::File(actual) if actual == expected));
}

#[test]
fn file_policy_defaults_to_max_count_1_and_no_size_limits() {
    // When
    let policy = FileInputPolicy::default();

    // Then
    assert_eq!(policy.max_count(), 1);
    assert_eq!(policy.extensions().count(), 0);
    assert_eq!(policy.max_file_bytes(), None);
    assert_eq!(policy.max_total_bytes(), None);
}

#[test]
fn file_policy_normalizes_extensions_to_lowercase_dotless_form() {
    // When
    let policy = policy(1, &[".PNG", "Tar.GZ"], None, None);

    // Then
    assert_eq!(
        policy.extensions().collect::<Vec<_>>(),
        vec!["png", "tar.gz"]
    );
}

#[test]
fn file_policy_preserves_optional_size_limits() {
    // When
    let policy = policy(3, &[], Some(7), Some(11));

    // Then
    assert_eq!(policy.max_file_bytes(), Some(7));
    assert_eq!(policy.max_total_bytes(), Some(11));
}

#[test]
fn file_policy_rejects_zero_max_count() {
    // Given
    let params = FileInputPolicyParams {
        max_count: 0,
        ..FileInputPolicyParams::default()
    };

    // When
    let result = FileInputPolicy::try_from(params);

    // Then
    assert_eq!(result, Err(FileInputPolicyError::ZeroMaxCount));
}

#[test]
fn file_policy_rejects_extensions_duplicated_after_normalization() {
    // When
    let result = FileInputPolicy::try_from(FileInputPolicyParams {
        extensions: vec![".PNG".to_string(), "png".to_string()],
        ..FileInputPolicyParams::default()
    });

    // Then
    assert_eq!(
        result,
        Err(FileInputPolicyError::DuplicateExtension {
            extension: "png".to_string(),
        })
    );
}

#[test]
fn file_policy_rejects_invalid_extensions() {
    // When
    let result = FileInputPolicy::try_from(FileInputPolicyParams {
        extensions: vec!["bad/path".to_string()],
        ..FileInputPolicyParams::default()
    });

    // Then
    assert_eq!(
        result,
        Err(FileInputPolicyError::InvalidExtension {
            extension: "bad/path".to_string(),
        })
    );
}

#[test]
fn file_input_counts_recursive_bytes_leaves() {
    // Given
    let file = directory(
        "root",
        vec![
            bytes_file("one.txt", &[1]),
            directory(
                "nested",
                vec![bytes_file("two.txt", &[2]), bytes_file("three.txt", &[3])],
            ),
        ],
    );

    // When
    let result = validate_file(policy(2, &[], None, None), file);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(FileInputValueError::CountExceeded {
            max: 2,
            actual: 3,
            ..
        }))
    ));
}

#[test]
fn file_input_accepts_values_at_exact_policy_boundaries() {
    // Given
    let file = directory(
        "root",
        vec![
            bytes_file("one.png", &[1, 2]),
            directory("nested", vec![bytes_file("two.PNG", &[3, 4, 5])]),
        ],
    );

    // When
    let result = validate_file(policy(2, &["png"], Some(3), Some(5)), file);

    // Then
    assert_eq!(result, Ok(()));
}

#[test]
fn file_input_rejects_values_exceeding_the_per_file_size_limit() {
    // Given
    let file = bytes_file("large.bin", &[1, 2, 3, 4]);

    // When
    let result = validate_file(policy(1, &[], Some(3), None), file);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(FileInputValueError::FileTooLarge {
            max: 3,
            actual: 4,
            ..
        }))
    ));
}

#[test]
fn file_input_rejects_values_exceeding_the_total_size_limit() {
    // Given
    let file = directory(
        "root",
        vec![
            bytes_file("one.bin", &[1, 2, 3]),
            bytes_file("two.bin", &[4, 5, 6]),
        ],
    );

    // When
    let result = validate_file(policy(2, &[], None, Some(5)), file);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(FileInputValueError::TotalTooLarge {
            max: 5,
            actual: 6,
            ..
        }))
    ));
}

#[test]
fn file_input_rejects_empty_directories() {
    // When
    let result = validate_file(FileInputPolicy::default(), directory("empty", vec![]));

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(
            FileInputValueError::EmptyDirectory { .. }
        ))
    ));
}

#[test]
fn file_input_validator_rejects_nesting_beyond_the_maximum_depth() {
    // Given
    const ALLOWED_NESTING_DEPTH: usize = 64;
    let file = nested_file(ALLOWED_NESTING_DEPTH + 1);

    // When
    let result = validate_file(FileInputPolicy::default(), file);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(FileInputValueError::NestingTooDeep {
            max: ALLOWED_NESTING_DEPTH,
            ..
        }))
    ));
}

#[test]
fn file_input_rejects_disallowed_extensions() {
    // Given
    let file = bytes_file("report.pdf", &[1]);

    // When
    let result = validate_file(policy(1, &["png"], None, None), file);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(
            FileInputValueError::ExtensionRejected { file_name, .. }
        )) if file_name == "report.pdf"
    ));
}

#[test]
fn file_input_rejects_is_dir_mismatch_in_recursive_children() {
    // Given
    let malformed = serde_json::json!({
        "name": "root",
        "is_dir": true,
        "content": {
            "kind": "directory",
            "entries": [{
                "name": "child.bin",
                "is_dir": true,
                "content": {"kind": "bytes", "bytes": "AQ=="}
            }]
        }
    });
    let spec = file_spec(FileInputPolicy::default());
    let mut args = serde_json::Map::new();
    args.insert("upload".to_string(), malformed);

    // When
    let result = spec.validate_json_args(&args);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(
            FileInputValueError::InvalidStructure { .. }
        ))
    ));
}

#[test]
fn file_policy_is_exposed_in_json_schema_and_imported_losslessly() {
    // Given
    let expected = policy(2, &[".PNG", "JPG"], Some(3), Some(5));
    let spec = file_spec(expected.clone());

    // When
    let schema = spec.to_json_schema_value();

    // Then
    assert_eq!(
        schema["properties"]["upload"]["x-upeg-file-policy"],
        serde_json::json!({
            "maxCount": 2,
            "extensions": ["jpg", "png"],
            "maxFileBytes": 3,
            "maxTotalBytes": 5,
        })
    );
    let imported = InputSpec::try_from(&schema).expect("must import the file policy schema");
    assert!(matches!(&imported.fields[0].kind, InputKind::File(actual) if actual == &expected));
}

#[test]
fn default_file_policy_json_schema_omits_optional_size_limits() {
    // Given
    let spec = file_spec(FileInputPolicy::default());

    // When
    let schema = spec.to_json_schema_value();

    // Then
    let policy = &schema["properties"]["upload"]["x-upeg-file-policy"];
    assert_eq!(policy["maxCount"], 1);
    assert_eq!(policy["extensions"], serde_json::json!([]));
    assert!(policy.get("maxFileBytes").is_none());
    assert!(policy.get("maxTotalBytes").is_none());
}

#[test]
fn json_schema_without_a_file_policy_uses_the_default_policy() {
    // Given
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "upload": {
                "type": "object",
                "x-upeg-kind": "file"
            }
        }
    });

    // When
    let imported = InputSpec::try_from(&schema).expect("must import with the default file policy");

    // Then
    assert!(matches!(
        &imported.fields[0].kind,
        InputKind::File(policy) if policy == &FileInputPolicy::default()
    ));
}

#[test]
fn file_policy_json_schema_rejects_non_object_values() {
    // Given
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "upload": {
                "type": "object",
                "x-upeg-kind": "file",
                "x-upeg-file-policy": []
            }
        }
    });

    // When
    let result = InputSpec::try_from(&schema);

    // Then
    assert!(matches!(
        result,
        Err(InputAdapterError::JsonSchemaFilePolicy {
            source: FilePolicySchemaError::NotObject { .. },
            ..
        })
    ));
}

#[test]
fn file_policy_json_schema_rejects_unknown_keys() {
    // Given
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "upload": {
                "type": "object",
                "x-upeg-kind": "file",
                "x-upeg-file-policy": {
                    "maxCount": 1,
                    "extensions": [],
                    "unknown": true
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
            source: FilePolicySchemaError::UnknownKey { key },
            ..
        }) if key == "unknown"
    ));
}
