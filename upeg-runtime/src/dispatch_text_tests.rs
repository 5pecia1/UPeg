use upeg_core::{FileContent, FileValue, OutputValue};

use crate::{output_value_canonical_wire_text, output_value_text};

#[test]
fn 파일_사람용_요약은_base64를_포함하지_않고_길이가_제한된다() {
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
        "사람용 요약은 224 bytes 이하여야 하지만 {} bytes였다",
        summary.len()
    );
}

#[test]
fn 명시적_canonical_wire_직렬화는_file_value를_roundtrip한다() {
    let file = FileValue {
        name: "result.bin".to_string(),
        mime: Some("application/octet-stream".to_string()),
        content: FileContent::Bytes(vec![0, 1, 2, 0xff]),
    };

    let wire = output_value_canonical_wire_text(&OutputValue::File(file.clone()))
        .expect("FileValue canonical wire 직렬화");
    let decoded =
        serde_json::from_str::<FileValue>(&wire).expect("FileValue canonical wire 역직렬화");

    assert_eq!(decoded, file);
}
