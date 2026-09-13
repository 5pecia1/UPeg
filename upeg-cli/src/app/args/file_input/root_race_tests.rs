use std::fs;

use tempfile::tempdir;
use upeg_core::{
    FileContent, FileInputPolicy, FileInputPolicyParams, InputFieldSpec, InputKind, InputName,
    InputValue,
};

use super::capability::open_directory_nofollow;
use super::directory_value;

#[cfg(unix)]
#[test]
fn 디렉터리_입력은_root_path가_교체되어도_검사한_디렉터리의_파일만_읽는다() {
    let temp = tempdir().expect("테스트 디렉터리를 만들어야 한다");
    let root = temp.path().join("root");
    let replacement = temp.path().join("replacement");
    let archived = temp.path().join("archived");
    fs::create_dir(&root).expect("검사 대상 디렉터리를 만들어야 한다");
    fs::create_dir(&replacement).expect("교체 디렉터리를 만들어야 한다");
    fs::write(root.join("original.png"), b"original").expect("원본 파일을 써야 한다");
    fs::write(replacement.join("replacement.png"), b"replacement").expect("교체 파일을 써야 한다");
    let policy = file_policy();
    let field = file_field(policy.clone());
    let opened_root = open_directory_nofollow(&root).expect("root를 no-follow로 열어야 한다");
    fs::rename(&root, &archived).expect("검사한 root를 보관해야 한다");
    fs::rename(&replacement, &root).expect("root를 교체해야 한다");

    let value = directory_value(&field, &policy, &root, opened_root)
        .expect("열어둔 디렉터리 입력을 읽어야 한다");

    assert_eq!(
        directory_files(value),
        [("original.png".to_string(), b"original".to_vec())]
    );
}

fn file_policy() -> FileInputPolicy {
    FileInputPolicy::try_from(FileInputPolicyParams {
        max_count: 2,
        extensions: vec!["png".to_string()],
        max_file_bytes: None,
        max_total_bytes: None,
    })
    .expect("테스트 파일 정책은 유효해야 한다")
}

fn file_field(policy: FileInputPolicy) -> InputFieldSpec {
    InputFieldSpec::new(
        InputName::new("images").expect("테스트 입력 이름은 유효해야 한다"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("테스트 입력 필드는 유효해야 한다")
}

fn directory_files(value: InputValue) -> Vec<(String, Vec<u8>)> {
    let InputValue::File(file) = value else {
        panic!("File 입력이어야 한다");
    };
    let FileContent::Directory(entries) = file.content else {
        panic!("Directory 입력이어야 한다");
    };
    entries
        .into_iter()
        .map(|entry| {
            let FileContent::Bytes(bytes) = entry.content else {
                panic!("자식 입력은 Bytes여야 한다");
            };
            (entry.name, bytes)
        })
        .collect()
}
