use upeg_core::{FileContent, FileValue, OutputValue};

use crate::{output_value_canonical_wire_text, output_value_text};

#[test]
fn file_human_summary_excludes_base64_and_is_length_limited() {
    let file = FileValue {
        name: "매우 긴 파일 이름".repeat(64),
        mime: Some("application/x-매우-긴-mime".repeat(64)),
        content: FileContent::Bytes(vec![0xfb, 0xee, 0xdd]),
    };

    let summary = output_value_text(&OutputValue::File(file));

    assert!(!summary.contains("++7d"));
    assert!(!summary.contains("\"bytes\""));
    assert!(summary.contains("3 bytes"));
    assert!(
        summary.len() <= 224,
        "human summary must be at most 224 bytes but was {} bytes",
        summary.len()
    );
}

#[test]
fn explicit_canonical_wire_serialization_round_trips_file_value() {
    let file = FileValue {
        name: "result.bin".to_string(),
        mime: Some("application/octet-stream".to_string()),
        content: FileContent::Bytes(vec![0, 1, 2, 0xff]),
    };

    let wire = output_value_canonical_wire_text(&OutputValue::File(file.clone()))
        .expect("FileValue canonical wire serialization");
    let decoded =
        serde_json::from_str::<FileValue>(&wire).expect("FileValue canonical wire deserialization");

    assert_eq!(decoded, file);
}
