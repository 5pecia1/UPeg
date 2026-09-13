use super::*;

#[test]
fn 출력_preflight는_base64_decoder를_호출하지_않는다() {
    const JSON_PREFLIGHT_SOURCE: &str = include_str!("file_output_preflight/json.rs");

    assert!(
        !JSON_PREFLIGHT_SOURCE.contains(".decode("),
        "출력 preflight는 decoded Vec를 할당하면 안 된다"
    );
}

#[test]
fn raw_누적_산술이_overflow하면_한도_초과로_거부한다() {
    let mut summary = FileOutputSummary {
        raw_bytes: u64::MAX,
        ..FileOutputSummary::default()
    };

    assert_eq!(
        summary.add_raw(1),
        Err(FileOutputPreflightError::RawBytesTooLarge {
            max: MAX_FILE_OUTPUT_RAW_BYTES,
            actual: u64::MAX,
        })
    );
}

#[test]
fn metadata_누적_산술이_overflow하면_한도_초과로_거부한다() {
    let mut summary = FileOutputSummary {
        metadata_bytes: u64::MAX,
        ..FileOutputSummary::default()
    };

    assert_eq!(
        summary.add_metadata(1),
        Err(FileOutputPreflightError::MetadataTooLarge {
            max: MAX_FILE_OUTPUT_METADATA_BYTES,
            actual: u64::MAX,
        })
    );
}

#[test]
fn node_누적_산술이_overflow하면_한도_초과로_거부한다() {
    let mut summary = FileOutputSummary {
        nodes: u64::MAX,
        ..FileOutputSummary::default()
    };

    assert_eq!(
        summary.add_node(),
        Err(FileOutputPreflightError::NodeCountExceeded {
            max: MAX_FILE_OUTPUT_NODES,
            actual: u64::MAX,
        })
    );
}
