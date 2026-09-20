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
    .expect("test file policy must be valid")
}

fn field(policy: FileInputPolicy) -> InputFieldSpec {
    InputFieldSpec::new(
        InputName::new(FILE_INPUT_NAME).expect("test input name must be valid"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("test input field must be valid")
}

fn cli_path(path: &Path) -> String {
    format!("@{}", path.display())
}

fn load(field: &InputFieldSpec, path: &Path) -> Result<Option<InputValue>, super::CliError> {
    let InputKind::File(policy) = &field.kind else {
        panic!("test input must be a File policy");
    };
    file_value_from_cli_path(field, policy, &cli_path(path))
}

fn file_names(value: InputValue) -> Vec<String> {
    let InputValue::File(file) = value else {
        panic!("must be a File input");
    };
    let FileContent::Directory(entries) = file.content else {
        panic!("a multi File input must be a Directory");
    };
    entries.into_iter().map(|entry| entry.name).collect()
}

#[test]
fn multi_file_input_builds_a_flat_directory_sorted_by_name() {
    let dir = TempDir::new().expect("must create a temp directory");
    std::fs::write(dir.path().join("z.png"), b"z").expect("must write the test file");
    std::fs::write(dir.path().join("a.png"), b"a").expect("must write the test file");
    let field = field(policy(2, &["png"], None, None));

    let value = load(&field, dir.path())
        .expect("must read the directory input")
        .expect("input must be non-empty");

    assert_eq!(file_names(value), ["a.png", "z.png"]);
}

#[test]
fn single_file_policy_rejects_a_directory_input() {
    let dir = TempDir::new().expect("must create a temp directory");
    std::fs::write(dir.path().join("a.png"), b"a").expect("must write the test file");
    let field = field(policy(1, &["png"], None, None));

    let error = load(&field, dir.path()).expect_err("a single File policy must reject a directory");

    assert!(error.message().contains("max_count"));
}

#[test]
fn file_input_rejects_a_disallowed_extension_before_file_size() {
    let dir = TempDir::new().expect("must create a temp directory");
    let rejected = dir.path().join("huge.txt");
    let file = std::fs::File::create(&rejected).expect("must create the test file");
    file.set_len(2).expect("must size the sparse test file");
    let field = field(policy(2, &["png"], Some(1), Some(1)));

    let error = load(&field, dir.path()).expect_err("extension policy must apply first");

    assert!(error.message().contains("extension"));
    assert!(!error.message().contains("bytes"));
}

#[test]
fn multi_file_input_rejects_nested_directories() {
    let dir = TempDir::new().expect("must create a temp directory");
    std::fs::create_dir(dir.path().join("nested")).expect("must create the nested directory");
    let field = field(policy(2, &[], None, None));

    let error = load(&field, dir.path()).expect_err("nested directories must be rejected");

    assert!(error.message().contains("nested director"));
}

#[cfg(unix)]
#[test]
fn multi_file_input_rejects_symlinks() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().expect("must create a temp directory");
    let target = dir.path().join("target.png");
    std::fs::write(&target, b"png").expect("must write the test file");
    symlink(&target, dir.path().join("link.png")).expect("must create the symlink");
    let field = field(policy(3, &["png"], None, None));

    let error = load(&field, dir.path()).expect_err("a symlink must be rejected");

    assert!(error.message().contains("symlink"));
}

#[test]
fn multi_file_input_enforces_the_policys_file_count_limit() {
    let dir = TempDir::new().expect("must create a temp directory");
    for name in ["a.png", "b.png", "c.png"] {
        std::fs::write(dir.path().join(name), b"x").expect("must write the test file");
    }
    let field = field(policy(2, &["png"], None, None));

    let error =
        load(&field, dir.path()).expect_err("exceeding the file count limit must be rejected");

    assert!(error.message().contains("max_count"));
}

#[test]
fn multi_file_input_rejects_an_empty_directory() {
    let dir = TempDir::new().expect("must create a temp directory");
    let field = field(policy(2, &[], None, None));

    let error = load(&field, dir.path()).expect_err("an empty directory must be rejected");

    assert!(error.message().contains("empty"));
}

#[test]
fn multi_file_input_enforces_per_file_and_total_size_limits_separately() {
    let per_file_dir = TempDir::new().expect("must create a temp directory");
    std::fs::write(per_file_dir.path().join("large.png"), b"123")
        .expect("must write the test file");
    let per_file_field = field(policy(2, &["png"], Some(2), Some(10)));

    let per_file_error = load(&per_file_field, per_file_dir.path())
        .expect_err("exceeding the per-file limit must be rejected");
    assert!(per_file_error.message().contains("max_file_bytes"));

    let total_dir = TempDir::new().expect("must create a temp directory");
    std::fs::write(total_dir.path().join("a.png"), b"12").expect("must write the test file");
    std::fs::write(total_dir.path().join("b.png"), b"34").expect("must write the test file");
    let total_field = field(policy(2, &["png"], Some(2), Some(3)));

    let total_error = load(&total_field, total_dir.path())
        .expect_err("exceeding the total limit must be rejected");
    assert!(total_error.message().contains("max_total_bytes"));
}

#[test]
fn multi_file_input_rejects_before_reading_when_names_plus_derived_mime_exceed_metadata_limit() {
    let temp = TempDir::new().expect("must create a temp directory");
    let input = temp.path().join("root");
    std::fs::create_dir(&input).expect("must create the input directory");
    let filler = "a".repeat(METADATA_TEST_FILENAME_FILLER_BYTES);
    for index in 0..MAX_FILE_INPUT_COUNT {
        let name = format!("{index:03}-{filler}.png");
        std::fs::write(input.join(name), []).expect("must write the metadata test file");
    }
    let field = field(policy(MAX_FILE_INPUT_COUNT, &["png"], None, None));

    let error = load(&field, &input).expect_err("exceeding the metadata limit must be rejected");

    assert!(error.message().contains("metadata"));
}
