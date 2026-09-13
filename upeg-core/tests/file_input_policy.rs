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

fn 정책(
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
    .expect("유효한 파일 정책이어야 한다")
}

fn 바이트_파일(name: &str, bytes: &[u8]) -> FileValue {
    FileValue {
        name: name.to_string(),
        mime: None,
        content: FileContent::Bytes(bytes.to_vec()),
    }
}

fn 디렉터리(name: &str, entries: Vec<FileValue>) -> FileValue {
    FileValue {
        name: name.to_string(),
        mime: None,
        content: FileContent::Directory(entries),
    }
}

fn 중첩_파일(depth: usize) -> FileValue {
    let mut file = 바이트_파일("leaf.bin", &[1]);
    for level in 1..depth {
        file = 디렉터리(&format!("level-{level}"), vec![file]);
    }
    file
}

fn 파일_명세(policy: FileInputPolicy) -> InputSpec {
    let field = InputFieldSpec::new(
        InputName::new("upload").expect("유효한 입력 이름이어야 한다"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("유효한 파일 필드여야 한다");
    InputSpec::new(vec![field]).expect("유효한 입력 명세여야 한다")
}

fn 파일_검증(policy: FileInputPolicy, file: FileValue) -> Result<(), InputValueError> {
    let spec = 파일_명세(policy);
    let mut args = serde_json::Map::new();
    args.insert(
        "upload".to_string(),
        InputValue::File(file).into_json_value(),
    );

    spec.validate_json_args(&args)
}

#[test]
fn file_입력_종류는_정책을_소유한다() {
    // Given
    let expected = 정책(2, &["png"], None, None);

    // When
    let kind = InputKind::File(expected.clone());

    // Then
    assert!(matches!(kind, InputKind::File(actual) if actual == expected));
}

#[test]
fn 파일_정책은_max_count_1과_선택적인_크기_제한을_기본값으로_사용한다() {
    // When
    let policy = FileInputPolicy::default();

    // Then
    assert_eq!(policy.max_count(), 1);
    assert_eq!(policy.extensions().count(), 0);
    assert_eq!(policy.max_file_bytes(), None);
    assert_eq!(policy.max_total_bytes(), None);
}

#[test]
fn 파일_정책은_확장자를_소문자_무점_형태로_정규화한다() {
    // When
    let policy = 정책(1, &[".PNG", "Tar.GZ"], None, None);

    // Then
    assert_eq!(
        policy.extensions().collect::<Vec<_>>(),
        vec!["png", "tar.gz"]
    );
}

#[test]
fn 파일_정책은_선택적인_크기_제한을_보존한다() {
    // When
    let policy = 정책(3, &[], Some(7), Some(11));

    // Then
    assert_eq!(policy.max_file_bytes(), Some(7));
    assert_eq!(policy.max_total_bytes(), Some(11));
}

#[test]
fn 파일_정책은_0인_max_count를_거부한다() {
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
fn 파일_정책은_정규화후_중복된_확장자를_거부한다() {
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
fn 파일_정책은_잘못된_확장자를_거부한다() {
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
fn 파일_입력은_재귀_bytes_leaf_개수를_센다() {
    // Given
    let file = 디렉터리(
        "root",
        vec![
            바이트_파일("one.txt", &[1]),
            디렉터리(
                "nested",
                vec![바이트_파일("two.txt", &[2]), 바이트_파일("three.txt", &[3])],
            ),
        ],
    );

    // When
    let result = 파일_검증(정책(2, &[], None, None), file);

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
fn 파일_입력은_정확히_정책_경계인_값을_허용한다() {
    // Given
    let file = 디렉터리(
        "root",
        vec![
            바이트_파일("one.png", &[1, 2]),
            디렉터리("nested", vec![바이트_파일("two.PNG", &[3, 4, 5])]),
        ],
    );

    // When
    let result = 파일_검증(정책(2, &["png"], Some(3), Some(5)), file);

    // Then
    assert_eq!(result, Ok(()));
}

#[test]
fn 파일_입력은_개별_파일_크기_제한을_초과하면_거부한다() {
    // Given
    let file = 바이트_파일("large.bin", &[1, 2, 3, 4]);

    // When
    let result = 파일_검증(정책(1, &[], Some(3), None), file);

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
fn 파일_입력은_전체_크기_제한을_초과하면_거부한다() {
    // Given
    let file = 디렉터리(
        "root",
        vec![
            바이트_파일("one.bin", &[1, 2, 3]),
            바이트_파일("two.bin", &[4, 5, 6]),
        ],
    );

    // When
    let result = 파일_검증(정책(2, &[], None, Some(5)), file);

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
fn 파일_입력은_빈_디렉터리를_거부한다() {
    // When
    let result = 파일_검증(FileInputPolicy::default(), 디렉터리("empty", vec![]));

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(
            FileInputValueError::EmptyDirectory { .. }
        ))
    ));
}

#[test]
fn 파일_입력_validator는_최대_중첩_깊이를_초과하면_거부한다() {
    // Given
    const 허용_중첩_깊이: usize = 64;
    let file = 중첩_파일(허용_중첩_깊이 + 1);

    // When
    let result = 파일_검증(FileInputPolicy::default(), file);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(FileInputValueError::NestingTooDeep {
            max: 허용_중첩_깊이,
            ..
        }))
    ));
}

#[test]
fn 파일_입력은_허용되지_않은_확장자를_거부한다() {
    // Given
    let file = 바이트_파일("report.pdf", &[1]);

    // When
    let result = 파일_검증(정책(1, &["png"], None, None), file);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(
            FileInputValueError::ExtensionRejected { file_name, .. }
        )) if file_name == "report.pdf"
    ));
}

#[test]
fn 파일_입력은_재귀_자식의_is_dir_불일치를_거부한다() {
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
    let spec = 파일_명세(FileInputPolicy::default());
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
fn 파일_정책은_json_schema에_노출되고_손실없이_가져온다() {
    // Given
    let expected = 정책(2, &[".PNG", "JPG"], Some(3), Some(5));
    let spec = 파일_명세(expected.clone());

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
    let imported = InputSpec::try_from(&schema).expect("파일 정책 schema를 가져와야 한다");
    assert!(matches!(&imported.fields[0].kind, InputKind::File(actual) if actual == &expected));
}

#[test]
fn 기본_파일_정책_json_schema는_선택적_크기_제한을_생략한다() {
    // Given
    let spec = 파일_명세(FileInputPolicy::default());

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
fn 파일_정책이_없는_json_schema는_기본_정책을_사용한다() {
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
    let imported = InputSpec::try_from(&schema).expect("기본 파일 정책으로 가져와야 한다");

    // Then
    assert!(matches!(
        &imported.fields[0].kind,
        InputKind::File(policy) if policy == &FileInputPolicy::default()
    ));
}

#[test]
fn 파일_정책_json_schema는_객체가_아닌_값을_거부한다() {
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
fn 파일_정책_json_schema는_알수없는_키를_거부한다() {
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
