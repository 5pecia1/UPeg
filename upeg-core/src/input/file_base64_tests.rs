use serde_json::{Map, Value};

use super::file_base64::canonical_base64_decoded_len;
use super::{
    FileInputPolicy, FileInputPolicyParams, FileInputValueError, InputFieldSpec, InputKind,
    InputName, InputSpec, InputValueError, MAX_FILE_INPUT_RAW_BYTES,
};

#[test]
fn canonical_base64_경계의_디코딩_길이를_할당없이_계산한다() {
    const 유효한_벡터: &[(&str, usize)] = &[
        ("", 0),
        ("Zg==", 1),
        ("Zm8=", 2),
        ("Zm9v", 3),
        ("/w==", 1),
        ("//8=", 2),
        ("////", 3),
    ];

    for (encoded, expected) in 유효한_벡터 {
        assert_eq!(
            canonical_base64_decoded_len(encoded),
            Ok(*expected),
            "{encoded:?}"
        );
    }
}

#[test]
fn canonical_base64가_아닌_알파벳_패딩_tail_bit는_거부한다() {
    const 비정규_벡터: &[&str] = &[
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

    for encoded in 비정규_벡터 {
        assert!(
            canonical_base64_decoded_len(encoded).is_err(),
            "{encoded:?}"
        );
    }
}

#[test]
fn 구조_preflight는_base64_디코더를_호출하지_않는다() {
    const PREFLIGHT_SOURCE: &str = include_str!("file_base64.rs");
    const STRUCTURE_SOURCE: &str = include_str!("file_structure.rs");

    for source in [PREFLIGHT_SOURCE, STRUCTURE_SOURCE] {
        assert!(
            !source.contains(".decode("),
            "검증 경계에서 decoded Vec를 할당하면 안 된다"
        );
    }
}

#[test]
fn 고정_원시_바이트_한도를_넘는_encoded_입력은_디코드_할당전에_거부한다() {
    const 한도_초과_바이트: u64 = MAX_FILE_INPUT_RAW_BYTES + 1;
    const DECODED_QUANTUM_BYTES: u64 = 3;
    const ZERO_BASE64_QUANTUM: &str = "AAAA";

    let decoded_len = usize::try_from(한도_초과_바이트).expect("테스트 크기는 usize여야 한다");
    assert_eq!(한도_초과_바이트 % DECODED_QUANTUM_BYTES, 0);
    let encoded = ZERO_BASE64_QUANTUM.repeat(
        usize::try_from(한도_초과_바이트 / DECODED_QUANTUM_BYTES)
            .expect("테스트 quartet 개수는 usize여야 한다"),
    );
    assert_eq!(
        canonical_base64_decoded_len(&encoded),
        Ok(decoded_len),
        "preflight가 한도 초과 크기를 decoded Vec 없이 계산해야 한다"
    );

    let policy = FileInputPolicy::default();
    let field = InputFieldSpec::new(
        InputName::new("upload").expect("유효한 입력 이름이어야 한다"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("유효한 파일 필드여야 한다");
    let spec = InputSpec::new(vec![field]).expect("유효한 입력 명세여야 한다");
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
            actual: 한도_초과_바이트,
            ..
        }))
    ));
}

#[test]
fn 개별_정책_한도는_raw_json을_file_value로_디코딩하기전에_적용한다() {
    let policy = FileInputPolicy::try_from(FileInputPolicyParams {
        max_file_bytes: Some(3),
        ..FileInputPolicyParams::default()
    })
    .expect("유효한 파일 정책이어야 한다");
    let field = InputFieldSpec::new(
        InputName::new("upload").expect("유효한 입력 이름이어야 한다"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("유효한 파일 필드여야 한다");
    let spec = InputSpec::new(vec![field]).expect("유효한 입력 명세여야 한다");
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
