use super::file_value_from_cli_path;
use std::path::Path;
use tempfile::TempDir;
use upeg_core::{
    FileContent, FileInputPolicy, FileInputPolicyParams, InputFieldSpec, InputKind, InputName,
    InputValue, MAX_FILE_INPUT_COUNT,
};

const FILE_INPUT_NAME: &str = "images";
const METADATA_TEST_FILENAME_FILLER_BYTES: usize = 155;

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
    .expect("테스트 파일 정책은 유효해야 한다")
}

fn field(policy: FileInputPolicy) -> InputFieldSpec {
    InputFieldSpec::new(
        InputName::new(FILE_INPUT_NAME).expect("테스트 입력 이름은 유효해야 한다"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("테스트 입력 필드는 유효해야 한다")
}

fn cli_path(path: &Path) -> String {
    format!("@{}", path.display())
}

fn load(field: &InputFieldSpec, path: &Path) -> Result<Option<InputValue>, super::CliError> {
    let InputKind::File(policy) = &field.kind else {
        panic!("테스트 입력은 File 정책이어야 한다");
    };
    file_value_from_cli_path(field, policy, &cli_path(path))
}

fn file_names(value: InputValue) -> Vec<String> {
    let InputValue::File(file) = value else {
        panic!("File 입력이어야 한다");
    };
    let FileContent::Directory(entries) = file.content else {
        panic!("다중 File 입력은 Directory여야 한다");
    };
    entries.into_iter().map(|entry| entry.name).collect()
}

#[test]
fn 다중_file_입력은_직접_파일을_이름순으로_정렬한_flat_directory를_만든다() {
    let dir = TempDir::new().expect("임시 디렉터리를 생성해야 한다");
    std::fs::write(dir.path().join("z.png"), b"z").expect("테스트 파일을 써야 한다");
    std::fs::write(dir.path().join("a.png"), b"a").expect("테스트 파일을 써야 한다");
    let field = field(policy(2, &["png"], None, None));

    let value = load(&field, dir.path())
        .expect("디렉터리 입력을 읽어야 한다")
        .expect("비어 있지 않은 입력이어야 한다");

    assert_eq!(file_names(value), ["a.png", "z.png"]);
}

#[test]
fn 단일_file_정책은_디렉터리_입력을_거부한다() {
    let dir = TempDir::new().expect("임시 디렉터리를 생성해야 한다");
    std::fs::write(dir.path().join("a.png"), b"a").expect("테스트 파일을 써야 한다");
    let field = field(policy(1, &["png"], None, None));

    let error = load(&field, dir.path()).expect_err("단일 File 정책은 디렉터리를 거부해야 한다");

    assert!(error.message().contains("max_count"));
}

#[test]
fn file_입력은_허용되지_않은_확장자를_파일_크기보다_먼저_거부한다() {
    let dir = TempDir::new().expect("임시 디렉터리를 생성해야 한다");
    let rejected = dir.path().join("huge.txt");
    let file = std::fs::File::create(&rejected).expect("테스트 파일을 생성해야 한다");
    file.set_len(2)
        .expect("희소 테스트 파일 크기를 정해야 한다");
    let field = field(policy(2, &["png"], Some(1), Some(1)));

    let error = load(&field, dir.path()).expect_err("확장자 정책을 먼저 적용해야 한다");

    assert!(error.message().contains("extension"));
    assert!(!error.message().contains("bytes"));
}

#[test]
fn 다중_file_입력은_중첩_디렉터리를_거부한다() {
    let dir = TempDir::new().expect("임시 디렉터리를 생성해야 한다");
    std::fs::create_dir(dir.path().join("nested")).expect("중첩 디렉터리를 생성해야 한다");
    let field = field(policy(2, &[], None, None));

    let error = load(&field, dir.path()).expect_err("중첩 디렉터리를 거부해야 한다");

    assert!(error.message().contains("nested director"));
}

#[cfg(unix)]
#[test]
fn 다중_file_입력은_symlink를_거부한다() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().expect("임시 디렉터리를 생성해야 한다");
    let target = dir.path().join("target.png");
    std::fs::write(&target, b"png").expect("테스트 파일을 써야 한다");
    symlink(&target, dir.path().join("link.png")).expect("symlink를 생성해야 한다");
    let field = field(policy(3, &["png"], None, None));

    let error = load(&field, dir.path()).expect_err("symlink를 거부해야 한다");

    assert!(error.message().contains("symlink"));
}

#[test]
fn 다중_file_입력은_정책의_파일_개수_제한을_지킨다() {
    let dir = TempDir::new().expect("임시 디렉터리를 생성해야 한다");
    for name in ["a.png", "b.png", "c.png"] {
        std::fs::write(dir.path().join(name), b"x").expect("테스트 파일을 써야 한다");
    }
    let field = field(policy(2, &["png"], None, None));

    let error = load(&field, dir.path()).expect_err("파일 개수 제한을 초과하면 거부해야 한다");

    assert!(error.message().contains("max_count"));
}

#[test]
fn 다중_file_입력은_빈_디렉터리를_거부한다() {
    let dir = TempDir::new().expect("임시 디렉터리를 생성해야 한다");
    let field = field(policy(2, &[], None, None));

    let error = load(&field, dir.path()).expect_err("빈 디렉터리를 거부해야 한다");

    assert!(error.message().contains("empty"));
}

#[test]
fn 다중_file_입력은_개별과_전체_크기_제한을_각각_지킨다() {
    let per_file_dir = TempDir::new().expect("임시 디렉터리를 생성해야 한다");
    std::fs::write(per_file_dir.path().join("large.png"), b"123").expect("테스트 파일을 써야 한다");
    let per_file_field = field(policy(2, &["png"], Some(2), Some(10)));

    let per_file_error = load(&per_file_field, per_file_dir.path())
        .expect_err("개별 파일 제한을 초과하면 거부해야 한다");
    assert!(per_file_error.message().contains("max_file_bytes"));

    let total_dir = TempDir::new().expect("임시 디렉터리를 생성해야 한다");
    std::fs::write(total_dir.path().join("a.png"), b"12").expect("테스트 파일을 써야 한다");
    std::fs::write(total_dir.path().join("b.png"), b"34").expect("테스트 파일을 써야 한다");
    let total_field = field(policy(2, &["png"], Some(2), Some(3)));

    let total_error =
        load(&total_field, total_dir.path()).expect_err("전체 파일 제한을 초과하면 거부해야 한다");
    assert!(total_error.message().contains("max_total_bytes"));
}

#[test]
fn 다중_file_입력은_이름과_파생_mime의_합계가_metadata_상한을_넘으면_읽기_전에_거부한다() {
    let temp = TempDir::new().expect("임시 디렉터리를 생성해야 한다");
    let input = temp.path().join("root");
    std::fs::create_dir(&input).expect("입력 디렉터리를 생성해야 한다");
    let filler = "a".repeat(METADATA_TEST_FILENAME_FILLER_BYTES);
    for index in 0..MAX_FILE_INPUT_COUNT {
        let name = format!("{index:03}-{filler}.png");
        std::fs::write(input.join(name), []).expect("metadata 테스트 파일을 써야 한다");
    }
    let field = field(policy(MAX_FILE_INPUT_COUNT, &["png"], None, None));

    let error = load(&field, &input).expect_err("metadata 상한을 넘으면 거부해야 한다");

    assert!(error.message().contains("metadata"));
}
