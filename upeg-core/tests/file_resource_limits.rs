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

fn 정책(max_count: u32) -> FileInputPolicy {
    FileInputPolicy::try_from(FileInputPolicyParams {
        max_count,
        ..FileInputPolicyParams::default()
    })
    .expect("유효한 파일 정책이어야 한다")
}

fn 바이트_파일(name: String, mime: Option<String>) -> FileValue {
    FileValue {
        name,
        mime,
        content: FileContent::Bytes(Vec::new()),
    }
}

fn 디렉터리(name: String, entries: Vec<FileValue>) -> FileValue {
    FileValue {
        name,
        mime: None,
        content: FileContent::Directory(entries),
    }
}

fn 파일_검증(policy: FileInputPolicy, file: FileValue) -> Result<(), InputValueError> {
    let field = InputFieldSpec::new(
        InputName::new("upload").expect("유효한 입력 이름이어야 한다"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("유효한 파일 필드여야 한다");
    let spec = InputSpec::new(vec![field]).expect("유효한 입력 명세여야 한다");
    let mut args = serde_json::Map::new();
    args.insert(
        "upload".to_string(),
        InputValue::File(file).into_json_value(),
    );

    spec.validate_json_args(&args)
}

fn 노드_트리(root_file_count: usize) -> FileValue {
    let mut entries = (0..63)
        .map(|index| {
            디렉터리(
                format!("directory-{index}"),
                vec![바이트_파일(format!("nested-{index}.bin"), None)],
            )
        })
        .collect::<Vec<_>>();
    entries
        .extend((0..root_file_count).map(|index| 바이트_파일(format!("root-{index}.bin"), None)));
    디렉터리("root".to_string(), entries)
}

fn 메타데이터_파일(byte_count: usize) -> FileValue {
    const 한글_UTF8_바이트: usize = "가".len();
    let 한글_개수 = byte_count / 한글_UTF8_바이트;
    let 나머지_바이트 = byte_count % 한글_UTF8_바이트;
    바이트_파일(
        "가".repeat(한글_개수),
        (나머지_바이트 > 0).then(|| "x".repeat(나머지_바이트)),
    )
}

#[test]
fn 파일_정책은_최대_file_개수를_허용한다() {
    // Given
    let params = FileInputPolicyParams {
        max_count: MAX_FILE_INPUT_COUNT,
        ..FileInputPolicyParams::default()
    };

    // When
    let policy = FileInputPolicy::try_from(params).expect("정확한 상한은 허용해야 한다");

    // Then
    assert_eq!(policy.max_count(), MAX_FILE_INPUT_COUNT);
}

#[test]
fn 파일_정책은_최대값을_넘는_max_count를_typed_error로_거부한다() {
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
fn 가져온_file_정책도_최대값을_넘는_max_count를_typed_error로_거부한다() {
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
fn 파일_입력은_정확한_노드_개수_상한을_허용한다() {
    // Given
    let file = 노드_트리(1);

    // When
    let result = 파일_검증(정책(MAX_FILE_INPUT_COUNT), file);

    // Then
    assert_eq!(result, Ok(()));
}

#[test]
fn 파일_입력은_노드_개수_상한을_넘으면_거부한다() {
    // Given
    let file = 노드_트리(2);

    // When
    let result = 파일_검증(정책(MAX_FILE_INPUT_COUNT), file);

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
fn 파일_입력은_정확한_utf8_메타데이터_상한을_허용한다() {
    // Given
    let metadata_bytes = usize::try_from(MAX_FILE_INPUT_METADATA_BYTES)
        .expect("메타데이터 상한은 usize에 맞아야 한다");
    let file = 메타데이터_파일(metadata_bytes);

    // When
    let result = 파일_검증(FileInputPolicy::default(), file);

    // Then
    assert_eq!(result, Ok(()));
}

#[test]
fn 파일_입력은_utf8_메타데이터_상한을_넘으면_거부한다() {
    // Given
    let metadata_bytes = usize::try_from(MAX_FILE_INPUT_METADATA_BYTES + 1)
        .expect("메타데이터 상한은 usize에 맞아야 한다");
    let file = 메타데이터_파일(metadata_bytes);

    // When
    let result = 파일_검증(FileInputPolicy::default(), file);

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
fn 파일_입력은_core_raw_input_상한을_넘으면_거부한다() {
    // Given
    let core_raw_input_bytes =
        usize::try_from(MAX_FILE_INPUT_RAW_BYTES).expect("상한은 usize에 맞아야 한다");
    let file = FileValue {
        name: "large.bin".to_string(),
        mime: None,
        content: FileContent::Bytes(vec![0; core_raw_input_bytes + 1]),
    };

    // When
    let result = 파일_검증(FileInputPolicy::default(), file);

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
