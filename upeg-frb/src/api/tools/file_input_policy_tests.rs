use super::{FileInputPolicyDto, InputFieldType};
use upeg_core::{FileInputPolicy, FileInputPolicyParams, InputKind};

#[test]
fn file_입력_dto는_코어_정책의_네_필드를_손실없이_노출한다() {
    let policy = FileInputPolicy::try_from(FileInputPolicyParams {
        max_count: 7,
        extensions: vec!["png".to_string(), "jpeg".to_string()],
        max_file_bytes: Some(1_024),
        max_total_bytes: Some(4_096),
    })
    .expect("테스트 파일 정책은 유효해야 한다");

    let InputFieldType::File { policy } = InputFieldType::from(&InputKind::File(policy)) else {
        panic!("File 입력은 정책을 포함한 File DTO로 변환되어야 한다");
    };

    assert_eq!(
        policy,
        FileInputPolicyDto {
            max_count: 7,
            extensions: vec!["jpeg".to_string(), "png".to_string()],
            max_file_bytes: Some(1_024),
            max_total_bytes: Some(4_096),
        }
    );
}

#[test]
fn file_입력_dto는_기본_정책의_max_count_1을_노출한다() {
    let InputFieldType::File { policy } =
        InputFieldType::from(&InputKind::File(FileInputPolicy::default()))
    else {
        panic!("File 입력은 정책을 포함한 File DTO로 변환되어야 한다");
    };

    assert_eq!(policy.max_count, 1);
    assert!(policy.extensions.is_empty());
    assert_eq!(policy.max_file_bytes, None);
    assert_eq!(policy.max_total_bytes, None);
}
