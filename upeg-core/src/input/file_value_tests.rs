use super::*;

fn bytes_file() -> FileValue {
    FileValue {
        name: "a.png".to_string(),
        mime: Some("image/png".to_string()),
        content: FileContent::Bytes(vec![0, 1, 255]),
    }
}

fn directory(entries: Vec<FileValue>) -> FileValue {
    FileValue {
        name: "dir".to_string(),
        mime: None,
        content: FileContent::Directory(entries),
    }
}

/// Nest `depth` file nodes on one root-to-leaf path (the root counts as 1).
fn nested_tree(depth: usize) -> FileValue {
    let mut node = bytes_file();
    for _ in 1..depth {
        node = directory(vec![node]);
    }
    node
}

#[test]
fn serde_and_manual_encodings_are_byte_identical() {
    for file in [
        bytes_file(),
        directory(vec![bytes_file(), directory(vec![])]),
    ] {
        let via_serde = serde_json::to_value(&file).expect("serde encodes FileValue");
        let via_manual = file_value_to_json(file);
        assert_eq!(via_serde, via_manual);
        assert_eq!(
            serde_json::to_string(&via_serde).expect("serde value re-encodes"),
            serde_json::to_string(&via_manual).expect("manual value re-encodes"),
        );
    }
}

#[test]
fn bytes_content_encodes_as_a_padded_standard_base64_string() {
    // Given
    let file = bytes_file();

    // When
    let via_serde = serde_json::to_value(&file).expect("must serialize the FileValue");
    let via_manual = file_value_to_json(file);

    // Then
    assert_eq!(via_serde["content"]["bytes"], "AAH/");
    assert_eq!(via_manual["content"]["bytes"], "AAH/");
}

#[test]
fn empty_bytes_content_encodes_as_an_empty_base64_string() {
    // Given
    let file = FileValue {
        content: FileContent::Bytes(Vec::new()),
        ..bytes_file()
    };

    // When
    let json = serde_json::to_value(file).expect("must serialize the FileValue");

    // Then
    assert_eq!(json["content"]["bytes"], "");
}

#[test]
fn padded_standard_base64_string_decodes_to_bytes() {
    // Given
    let json = r#"{"name":"a","is_dir":false,"content":{"kind":"bytes","bytes":"AP8="}}"#;

    // When
    let file = serde_json::from_str::<FileValue>(json).expect("must decode canonical base64");

    // Then
    assert_eq!(file.content, FileContent::Bytes(vec![0, 255]));
}

#[test]
fn legacy_byte_arrays_are_rejected() {
    // Given
    let json = r#"{"name":"a","is_dir":false,"content":{"kind":"bytes","bytes":[0,255]}}"#;

    // When
    let result = serde_json::from_str::<FileValue>(json);

    // Then
    result.expect_err("must reject the legacy byte array");
}

#[test]
fn noncanonical_base64_strings_are_all_rejected() {
    const NONCANONICAL_BODIES: &[&str] = &["Z g==", "-w==", "Zg", "Zg===", "A===", "Zh=="];

    for bytes in NONCANONICAL_BODIES {
        // Given
        let json = format!(
            r#"{{"name":"a","is_dir":false,"content":{{"kind":"bytes","bytes":"{bytes}"}}}}"#
        );

        // When
        let result = serde_json::from_str::<FileValue>(&json);

        // Then
        result.expect_err("must reject noncanonical base64");
    }
}

#[test]
fn is_dir_tracks_the_content_variant_exactly() {
    assert!(!bytes_file().is_dir());
    assert!(directory(vec![]).is_dir());
}

#[test]
fn key_order_is_name_is_dir_mime_content() {
    let json = serde_json::to_string(&bytes_file()).expect("serde encodes FileValue");
    assert!(
        json.starts_with(r#"{"name":"a.png","is_dir":false,"mime":"image/png","content":"#),
        "unexpected key order: {json}"
    );
}

#[test]
fn roundtrip_returns_the_original_value() {
    for file in [bytes_file(), directory(vec![bytes_file()]), nested_tree(8)] {
        let json = serde_json::to_string(&file).expect("serde encodes FileValue");
        let decoded: FileValue = serde_json::from_str(&json).expect("serde decodes FileValue");
        assert_eq!(decoded, file);
    }
}

#[test]
fn absent_mime_drops_the_key_entirely() {
    let file = FileValue {
        mime: None,
        ..bytes_file()
    };
    let json = serde_json::to_value(&file).expect("serde encodes FileValue");
    assert_eq!(json, file_value_to_json(file));
    assert!(json.get(FILE_KEY_MIME).is_none());
}

#[test]
fn is_dir_true_with_bytes_content_is_rejected() {
    let json = r#"{"name":"a","is_dir":true,"content":{"kind":"bytes","bytes":""}}"#;
    let error = serde_json::from_str::<FileValue>(json).expect_err("inconsistent is_dir");
    assert!(error.to_string().contains(FILE_KEY_IS_DIR), "{error}");
}

#[test]
fn is_dir_false_with_directory_content_is_rejected() {
    let json = r#"{"name":"a","is_dir":false,"content":{"kind":"directory","entries":[]}}"#;
    let error = serde_json::from_str::<FileValue>(json).expect_err("inconsistent is_dir");
    assert!(error.to_string().contains(FILE_KEY_IS_DIR), "{error}");
}

#[test]
fn unknown_keys_are_rejected() {
    let json = r#"{"name":"a","is_dir":false,"content":{"kind":"bytes","bytes":""},"x":1}"#;
    serde_json::from_str::<FileValue>(json).expect_err("unknown FileValue key");

    let json = r#"{"name":"a","is_dir":false,"content":{"kind":"bytes","bytes":"","x":1}}"#;
    serde_json::from_str::<FileValue>(json).expect_err("unknown FileContent key");
}

#[test]
fn non_base64_string_content_is_rejected() {
    for bytes in ["256", "-1", "null", "true"] {
        let json = format!(
            r#"{{"name":"a","is_dir":false,"content":{{"kind":"bytes","bytes":{bytes}}}}}"#
        );
        serde_json::from_str::<FileValue>(&json).expect_err("must reject non-string content");
    }
}

// The depth guard is exercised through `from_value`, not `from_str`:
// `from_str` applies serde_json's own 128-*JSON*-level recursion limit,
// and one file level costs three JSON levels, so it rejects deep trees
// before this guard ever sees them. `from_value` has no such limit, which
// is precisely why the guard has to exist.
#[test]
fn decodes_up_to_max_depth_and_rejects_one_level_deeper() {
    let allowed = file_value_to_json(nested_tree(MAX_FILE_NESTING_DEPTH));
    serde_json::from_value::<FileValue>(allowed).expect("max depth decodes");

    let exceeded = file_value_to_json(nested_tree(MAX_FILE_NESTING_DEPTH + 1));
    let error = serde_json::from_value::<FileValue>(exceeded).expect_err("over max depth");
    assert!(error.to_string().contains("depth"), "{error}");
}

#[test]
fn a_depth_failure_does_not_affect_the_next_decode() {
    let exceeded = file_value_to_json(nested_tree(MAX_FILE_NESTING_DEPTH + 1));
    serde_json::from_value::<FileValue>(exceeded).expect_err("over max depth");

    // The thread-local level counter must have unwound; otherwise this
    // shallow decode would inherit the failed run's depth.
    let json = file_value_to_json(bytes_file());
    serde_json::from_value::<FileValue>(json).expect("counter unwound after failure");
}
