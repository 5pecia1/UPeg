use std::fs;

use cap_std::fs::{File, Metadata};
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
    let inspected_metadata = Metadata::from_file(
        &fs::File::open(&inspected_path).expect("must open the inspected file"),
    )
    .expect("must read the inspected metadata");
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
        file: File::from_std(
            fs::File::open(&replacement_path).expect("must open the replacement file"),
        ),
        path: replacement_path,
        size: inspected_metadata.len(),
        mime: None,
        metadata: inspected_metadata,
    };

    let error = read_capped(&field, &replacement, TEST_READ_LIMIT)
        .expect_err("a different opened file than inspected must be rejected");

    assert!(error.message().contains("changed while being read"));
}

#[test]
fn reading_accepts_a_hard_link_to_the_inspected_file() {
    let temp = tempdir().expect("must create the test directory");
    let inspected_path = temp.path().join("inspected.txt");
    let linked_path = temp.path().join("linked.txt");
    fs::write(&inspected_path, b"same file").expect("must write the inspected file");
    fs::hard_link(&inspected_path, &linked_path).expect("must create a hard link");
    let inspected_metadata = Metadata::from_file(
        &fs::File::open(&inspected_path).expect("must open the inspected file"),
    )
    .expect("must read the inspected metadata");
    let field = InputFieldSpec::new(
        InputName::new("document").expect("test input name must be valid"),
        None,
        None,
        false,
        InputKind::String,
    )
    .expect("test input field must be valid");
    let linked = DirectFile {
        name: "linked.txt".to_string(),
        file: File::from_std(fs::File::open(&linked_path).expect("must open the hard link")),
        path: linked_path,
        size: inspected_metadata.len(),
        mime: None,
        metadata: inspected_metadata,
    };

    let bytes = read_capped(&field, &linked, TEST_READ_LIMIT)
        .expect("a hard link must have the same file identity")
        .expect("the hard-linked file must fit the read limit");

    assert_eq!(bytes, b"same file");
}
