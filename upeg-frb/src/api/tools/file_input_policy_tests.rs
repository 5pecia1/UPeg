use super::{FileInputPolicyDto, InputFieldType};
use upeg_core::{FileInputPolicy, FileInputPolicyParams, InputKind};

#[test]
fn file_input_dto_exposes_core_policys_four_fields_losslessly() {
    let policy = FileInputPolicy::try_from(FileInputPolicyParams {
        max_count: 7,
        extensions: vec!["png".to_string(), "jpeg".to_string()],
        max_file_bytes: Some(1_024),
        max_total_bytes: Some(4_096),
    })
    .expect("test file policy must be valid");

    let InputFieldType::File { policy } = InputFieldType::from(&InputKind::File(policy)) else {
        panic!("File input must convert to a File DTO carrying the policy");
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
fn file_input_dto_exposes_default_policy_max_count_1() {
    let InputFieldType::File { policy } =
        InputFieldType::from(&InputKind::File(FileInputPolicy::default()))
    else {
        panic!("File input must convert to a File DTO carrying the policy");
    };

    assert_eq!(policy.max_count, 1);
    assert!(policy.extensions.is_empty());
    assert_eq!(policy.max_file_bytes, None);
    assert_eq!(policy.max_total_bytes, None);
}
