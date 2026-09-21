use std::fs;

use tempfile::tempdir;
use upeg_core::{InputFieldSpec, InputKind, InputName};

use super::DirectFile;
use super::policy::read_capped;

const TEST_READ_LIMIT: u64 = 64;

#[test]
fn reading_rejects_when_inspected_file_and_open_handle_have_different_identities() {
    let temp = tempdir().expect("must create the test directory");
    let inspected_path = temp.path().join("inspected.txt");
    let replacement_path = temp.path().join("replacement.txt");
    fs::write(&inspected_path, b"inspected").expect("must write the inspected file");
    fs::write(&replacement_path, b"replacement").expect("must write the replacement file");
    let inspected_metadata =
        fs::symlink_metadata(&inspected_path).expect("must read the inspected metadata");
    let field = InputFieldSpec::new(
        InputName::new("document").expect("test input name must be valid"),
        None,
        None,
        false,
        InputKind::String,
    )
    .expect("test input field must be valid");
    let replacement = DirectFile {
        name: "replacement.txt".to_string(),
        file: fs::File::open(&replacement_path).expect("must open the replacement file"),
        path: replacement_path,
        size: inspected_metadata.len(),
        mime: None,
        metadata: inspected_metadata,
    };

    let error = read_capped(&field, &replacement, TEST_READ_LIMIT)
        .expect_err("a different opened file than inspected must be rejected");

    assert!(error.message().contains("changed while being read"));
}
