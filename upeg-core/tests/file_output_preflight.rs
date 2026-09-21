#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect idiomatically"
)]

use serde_json::{Value, json};
use upeg_core::{
    FileContent, FileOutputPreflightError, FileValue, MAX_FILE_OUTPUT_METADATA_BYTES,
    MAX_FILE_OUTPUT_NESTING_DEPTH, MAX_FILE_OUTPUT_NODES, MAX_FILE_OUTPUT_RAW_BYTES,
    preflight_file_output_json, validate_file_output_tree,
};

const DECODED_QUANTUM_BYTES: u64 = 3;
const ZERO_BASE64_QUANTUM: &str = "AAAA";

fn zero_bytes_base64(decoded_len: u64) -> String {
    let full_quantums = decoded_len / DECODED_QUANTUM_BYTES;
    let mut encoded = ZERO_BASE64_QUANTUM
        .repeat(usize::try_from(full_quantums).expect("test base64 length must fit in usize"));
    let remainder = decoded_len % DECODED_QUANTUM_BYTES;
    if remainder == 1 {
        encoded.push_str("AA==");
    } else if remainder == 2 {
        encoded.push_str("AAA=");
    }
    encoded
}

fn bytes_node(name: String, mime: Option<String>, bytes: String) -> Value {
    let mut node = json!({
        "name": name,
        "is_dir": false,
        "content": {
            "kind": "bytes",
            "bytes": bytes
        }
    });
    if let Some(mime) = mime {
        node["mime"] = Value::String(mime);
    }
    node
}

fn directory_node(name: String, entries: Vec<Value>) -> Value {
    json!({
        "name": name,
        "is_dir": true,
        "content": {
            "kind": "directory",
            "entries": entries
        }
    })
}

fn deep_tree(depth: usize) -> Value {
    let mut tree = bytes_node("f".to_string(), None, String::new());
    for _ in 1..depth {
        tree = directory_node("d".to_string(), vec![tree]);
    }
    tree
}

#[test]
fn file_output_json_accepts_a_raw_sum_at_the_limit_across_leaves_and_rejects_one_more_byte() {
    let first_leaf_bytes = MAX_FILE_OUTPUT_RAW_BYTES / 2;
    let second_leaf_bytes = MAX_FILE_OUTPUT_RAW_BYTES - first_leaf_bytes;
    let mut tree = directory_node(
        "root".to_string(),
        vec![
            bytes_node(
                "first.bin".to_string(),
                None,
                zero_bytes_base64(first_leaf_bytes),
            ),
            bytes_node(
                "second.bin".to_string(),
                None,
                zero_bytes_base64(second_leaf_bytes),
            ),
        ],
    );

    assert_eq!(preflight_file_output_json(&tree), Ok(()));

    tree["content"]["entries"]
        .as_array_mut()
        .expect("entries must be an array")
        .push(bytes_node(
            "overflow.bin".to_string(),
            None,
            "AA==".to_string(),
        ));
    assert_eq!(
        preflight_file_output_json(&tree),
        Err(FileOutputPreflightError::RawBytesTooLarge {
            max: MAX_FILE_OUTPUT_RAW_BYTES,
            actual: MAX_FILE_OUTPUT_RAW_BYTES + 1,
        })
    );
}

#[test]
fn file_output_json_rejects_exceeding_the_aggregate_node_limit() {
    let leaf = || bytes_node("f".to_string(), None, String::new());
    let mut entries = (1..MAX_FILE_OUTPUT_NODES).map(|_| leaf()).collect();
    let mut tree = directory_node("r".to_string(), entries);
    assert_eq!(preflight_file_output_json(&tree), Ok(()));

    entries = tree["content"]["entries"]
        .as_array_mut()
        .expect("entries must be an array")
        .drain(..)
        .collect();
    entries.push(leaf());
    tree = directory_node("r".to_string(), entries);
    assert_eq!(
        preflight_file_output_json(&tree),
        Err(FileOutputPreflightError::NodeCountExceeded {
            max: MAX_FILE_OUTPUT_NODES,
            actual: MAX_FILE_OUTPUT_NODES + 1,
        })
    );
}

#[test]
fn file_output_json_rejects_exceeding_the_utf8_metadata_aggregate_limit() {
    const ROOT_METADATA_BYTES: u64 = 1;
    const UTF8_CHARACTER: &str = "가";
    let leaf_metadata_bytes = MAX_FILE_OUTPUT_METADATA_BYTES - ROOT_METADATA_BYTES;
    let utf8_character_bytes =
        u64::try_from(UTF8_CHARACTER.len()).expect("character length must fit in u64");
    let repeated_characters = leaf_metadata_bytes / utf8_character_bytes;
    let remainder = leaf_metadata_bytes % utf8_character_bytes;
    let name = UTF8_CHARACTER.repeat(
        usize::try_from(repeated_characters).expect("test string length must fit in usize"),
    );
    let mime = (remainder > 0).then(|| {
        "x".repeat(usize::try_from(remainder).expect("remainder length must fit in usize"))
    });
    let mut tree = directory_node("r".to_string(), vec![bytes_node(name, mime, String::new())]);

    assert_eq!(preflight_file_output_json(&tree), Ok(()));

    tree["content"]["entries"][0]["mime"] = Value::String("x".to_string());
    assert_eq!(
        preflight_file_output_json(&tree),
        Err(FileOutputPreflightError::MetadataTooLarge {
            max: MAX_FILE_OUTPUT_METADATA_BYTES,
            actual: MAX_FILE_OUTPUT_METADATA_BYTES + 1,
        })
    );
}

#[test]
fn file_output_json_rejects_exceeding_the_depth_limit() {
    let allowed = deep_tree(MAX_FILE_OUTPUT_NESTING_DEPTH);
    assert_eq!(preflight_file_output_json(&allowed), Ok(()));

    let exceeded = directory_node("d".to_string(), vec![allowed]);
    assert_eq!(
        preflight_file_output_json(&exceeded),
        Err(FileOutputPreflightError::NestingTooDeep {
            max: MAX_FILE_OUTPUT_NESTING_DEPTH,
        })
    );
}

#[test]
fn file_output_json_rejects_noncanonical_base64_and_bad_padding_before_allocation() {
    for encoded in ["A===", "AA=A", "Zh==", "Zm9="] {
        let tree = bytes_node("invalid.bin".to_string(), None, encoded.to_string());

        assert!(
            matches!(
                preflight_file_output_json(&tree),
                Err(FileOutputPreflightError::InvalidStructure { .. })
            ),
            "{encoded:?} is not canonical padded base64"
        );
    }
}

#[test]
fn file_output_json_accepts_empty_directories() {
    let tree = directory_node("empty".to_string(), Vec::new());

    assert_eq!(preflight_file_output_json(&tree), Ok(()));
}

#[test]
fn typed_file_output_shares_the_aggregate_node_budget() {
    let entries = (0..MAX_FILE_OUTPUT_NODES)
        .map(|index| FileValue {
            name: format!("{index}.bin"),
            mime: None,
            content: FileContent::Bytes(Vec::new()),
        })
        .collect();
    let tree = FileValue {
        name: "root".to_string(),
        mime: None,
        content: FileContent::Directory(entries),
    };

    assert_eq!(
        validate_file_output_tree(&tree),
        Err(FileOutputPreflightError::NodeCountExceeded {
            max: MAX_FILE_OUTPUT_NODES,
            actual: MAX_FILE_OUTPUT_NODES + 1,
        })
    );
}
