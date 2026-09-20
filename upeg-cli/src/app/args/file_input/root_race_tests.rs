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
fn directory_input_reads_only_the_inspected_directory_when_root_path_is_swapped() {
    let temp = tempdir().expect("must create the test directory");
    let root = temp.path().join("root");
    let replacement = temp.path().join("replacement");
    let archived = temp.path().join("archived");
    fs::create_dir(&root).expect("must create the directory under inspection");
    fs::create_dir(&replacement).expect("must create the replacement directory");
    fs::write(root.join("original.png"), b"original").expect("must write the original file");
    fs::write(replacement.join("replacement.png"), b"replacement")
        .expect("must write the replacement file");
    let policy = file_policy();
    let field = file_field(policy.clone());
    let opened_root = open_directory_nofollow(&root).expect("must open root no-follow");
    fs::rename(&root, &archived).expect("must set aside the inspected root");
    fs::rename(&replacement, &root).expect("must swap out root");

    let value = directory_value(&field, &policy, &root, opened_root)
        .expect("must read the already-opened directory input");

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
    .expect("test file policy must be valid")
}

fn file_field(policy: FileInputPolicy) -> InputFieldSpec {
    InputFieldSpec::new(
        InputName::new("images").expect("test input name must be valid"),
        None,
        None,
        true,
        InputKind::File(policy),
    )
    .expect("test input field must be valid")
}

fn directory_files(value: InputValue) -> Vec<(String, Vec<u8>)> {
    let InputValue::File(file) = value else {
        panic!("must be a File input");
    };
    let FileContent::Directory(entries) = file.content else {
        panic!("must be a Directory input");
    };
    entries
        .into_iter()
        .map(|entry| {
            let FileContent::Bytes(bytes) = entry.content else {
                panic!("a child input must be Bytes");
            };
            (entry.name, bytes)
        })
        .collect()
}
