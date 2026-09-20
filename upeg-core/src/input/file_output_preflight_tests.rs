use super::*;

#[test]
fn output_preflight_never_invokes_the_base64_decoder() {
    const JSON_PREFLIGHT_SOURCE: &str = include_str!("file_output_preflight/json.rs");

    assert!(
        !JSON_PREFLIGHT_SOURCE.contains(".decode("),
        "output preflight must not allocate a decoded Vec"
    );
}

#[test]
fn raw_accumulation_overflow_is_rejected_as_over_limit() {
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
fn metadata_accumulation_overflow_is_rejected_as_over_limit() {
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
fn node_accumulation_overflow_is_rejected_as_over_limit() {
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
