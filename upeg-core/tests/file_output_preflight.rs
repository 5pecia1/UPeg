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

fn 영_바이트_base64(decoded_len: u64) -> String {
    let full_quantums = decoded_len / DECODED_QUANTUM_BYTES;
    let mut encoded = ZERO_BASE64_QUANTUM
        .repeat(usize::try_from(full_quantums).expect("테스트 base64 길이는 usize에 맞아야 한다"));
    let remainder = decoded_len % DECODED_QUANTUM_BYTES;
    if remainder == 1 {
        encoded.push_str("AA==");
    } else if remainder == 2 {
        encoded.push_str("AAA=");
    }
    encoded
}

fn 바이트_노드(name: String, mime: Option<String>, bytes: String) -> Value {
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

fn 디렉터리_노드(name: String, entries: Vec<Value>) -> Value {
    json!({
        "name": name,
        "is_dir": true,
        "content": {
            "kind": "directory",
            "entries": entries
        }
    })
}

fn 깊이_트리(depth: usize) -> Value {
    let mut tree = 바이트_노드("f".to_string(), None, String::new());
    for _ in 1..depth {
        tree = 디렉터리_노드("d".to_string(), vec![tree]);
    }
    tree
}

#[test]
fn 파일_출력_json은_여러_leaf의_raw_합계가_상한이면_허용하고_한_바이트를_더하면_거부한다() {
    let first_leaf_bytes = MAX_FILE_OUTPUT_RAW_BYTES / 2;
    let second_leaf_bytes = MAX_FILE_OUTPUT_RAW_BYTES - first_leaf_bytes;
    let mut tree = 디렉터리_노드(
        "root".to_string(),
        vec![
            바이트_노드(
                "first.bin".to_string(),
                None,
                영_바이트_base64(first_leaf_bytes),
            ),
            바이트_노드(
                "second.bin".to_string(),
                None,
                영_바이트_base64(second_leaf_bytes),
            ),
        ],
    );

    assert_eq!(preflight_file_output_json(&tree), Ok(()));

    tree["content"]["entries"]
        .as_array_mut()
        .expect("entries는 배열이어야 한다")
        .push(바이트_노드(
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
fn 파일_출력_json은_aggregate_노드_상한을_넘으면_거부한다() {
    let leaf = || 바이트_노드("f".to_string(), None, String::new());
    let mut entries = (1..MAX_FILE_OUTPUT_NODES).map(|_| leaf()).collect();
    let mut tree = 디렉터리_노드("r".to_string(), entries);
    assert_eq!(preflight_file_output_json(&tree), Ok(()));

    entries = tree["content"]["entries"]
        .as_array_mut()
        .expect("entries는 배열이어야 한다")
        .drain(..)
        .collect();
    entries.push(leaf());
    tree = 디렉터리_노드("r".to_string(), entries);
    assert_eq!(
        preflight_file_output_json(&tree),
        Err(FileOutputPreflightError::NodeCountExceeded {
            max: MAX_FILE_OUTPUT_NODES,
            actual: MAX_FILE_OUTPUT_NODES + 1,
        })
    );
}

#[test]
fn 파일_출력_json은_utf8_메타데이터_aggregate_상한을_넘으면_거부한다() {
    const ROOT_METADATA_BYTES: u64 = 1;
    const UTF8_CHARACTER: &str = "가";
    let leaf_metadata_bytes = MAX_FILE_OUTPUT_METADATA_BYTES - ROOT_METADATA_BYTES;
    let utf8_character_bytes =
        u64::try_from(UTF8_CHARACTER.len()).expect("문자 길이는 u64에 맞아야 한다");
    let repeated_characters = leaf_metadata_bytes / utf8_character_bytes;
    let remainder = leaf_metadata_bytes % utf8_character_bytes;
    let name = UTF8_CHARACTER.repeat(
        usize::try_from(repeated_characters).expect("테스트 문자열 길이는 usize에 맞아야 한다"),
    );
    let mime = (remainder > 0).then(|| {
        "x".repeat(usize::try_from(remainder).expect("나머지 길이는 usize에 맞아야 한다"))
    });
    let mut tree = 디렉터리_노드(
        "r".to_string(),
        vec![바이트_노드(name, mime, String::new())],
    );

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
fn 파일_출력_json은_깊이_상한을_넘으면_거부한다() {
    let allowed = 깊이_트리(MAX_FILE_OUTPUT_NESTING_DEPTH);
    assert_eq!(preflight_file_output_json(&allowed), Ok(()));

    let exceeded = 디렉터리_노드("d".to_string(), vec![allowed]);
    assert_eq!(
        preflight_file_output_json(&exceeded),
        Err(FileOutputPreflightError::NestingTooDeep {
            max: MAX_FILE_OUTPUT_NESTING_DEPTH,
        })
    );
}

#[test]
fn 파일_출력_json은_비정규_base64와_잘못된_패딩을_할당전에_거부한다() {
    for encoded in ["A===", "AA=A", "Zh==", "Zm9="] {
        let tree = 바이트_노드("invalid.bin".to_string(), None, encoded.to_string());

        assert!(
            matches!(
                preflight_file_output_json(&tree),
                Err(FileOutputPreflightError::InvalidStructure { .. })
            ),
            "{encoded:?}는 canonical padded base64가 아니다"
        );
    }
}

#[test]
fn 파일_출력_json은_빈_디렉터리를_허용한다() {
    let tree = 디렉터리_노드("empty".to_string(), Vec::new());

    assert_eq!(preflight_file_output_json(&tree), Ok(()));
}

#[test]
fn typed_파일_출력도_aggregate_노드_예산을_공유한다() {
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
