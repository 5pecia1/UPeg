use std::fs;

use tempfile::tempdir;
use upeg_core::{InputFieldSpec, InputKind, InputName};

use super::DirectFile;
use super::policy::read_capped;

const TEST_READ_LIMIT: u64 = 64;

#[test]
fn 파일_읽기는_검사한_파일과_열린_handle의_identity가_다르면_거부한다() {
    let temp = tempdir().expect("테스트 디렉터리를 만들어야 한다");
    let inspected_path = temp.path().join("inspected.txt");
    let replacement_path = temp.path().join("replacement.txt");
    fs::write(&inspected_path, b"inspected").expect("검사 대상 파일을 써야 한다");
    fs::write(&replacement_path, b"replacement").expect("교체 파일을 써야 한다");
    let inspected_metadata =
        fs::symlink_metadata(&inspected_path).expect("검사 대상 metadata를 읽어야 한다");
    let field = InputFieldSpec::new(
        InputName::new("document").expect("테스트 입력 이름은 유효해야 한다"),
        None,
        None,
        false,
        InputKind::String,
    )
    .expect("테스트 입력 필드는 유효해야 한다");
    let replacement = DirectFile {
        name: "replacement.txt".to_string(),
        file: fs::File::open(&replacement_path).expect("교체 파일을 열어야 한다"),
        path: replacement_path,
        size: inspected_metadata.len(),
        mime: None,
        metadata: inspected_metadata,
    };

    let error = read_capped(&field, &replacement, TEST_READ_LIMIT)
        .expect_err("검사한 파일과 열린 파일이 다르면 거부해야 한다");

    assert!(error.message().contains("changed while being read"));
}
