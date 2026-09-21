use super::*;

const SECURITY_TEST_MAX_NAME_BYTES: usize = 4 << 10;
const SECURITY_TEST_MAX_MIME_BYTES: usize = 1 << 10;
const SECURITY_TEST_ZIP_BYTES: usize = 64 << 10;
const SECURITY_TEST_ENCODED_BYTES: usize = 64 << 10;
const SECURITY_TEST_FIXED_OVERHEAD_BYTES: usize = 8 << 20;
const SECURITY_TEST_JSON_AND_ZIP_MULTIPLIER: usize = 5;
const SECURITY_TEST_UNACCOUNTED_MARGIN_BYTES: usize = 1 << 10;

fn input_directory(entry_name: &str, entry_mime: Option<String>) -> FileValue {
    FileValue {
        name: "photos".to_string(),
        mime: None,
        content: FileContent::Directory(vec![FileValue {
            name: entry_name.to_string(),
            mime: entry_mime,
            content: FileContent::Bytes(png_bytes(1, 1, [0, 0, 0, 255])),
        }]),
    }
}

#[test]
fn images_convert_enforces_root_name_metadata_cap() {
    let mut input = input_directory("one.png", None);
    input.name = "r".repeat(SECURITY_TEST_MAX_NAME_BYTES + 1);

    let error = default_images_convert(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("oversized root name must be rejected");

    assert!(error.contains("root name"), "got {error:?}");
}

#[test]
fn images_convert_enforces_root_mime_metadata_cap() {
    let mut input = input_directory("one.png", None);
    input.mime = Some("m".repeat(SECURITY_TEST_MAX_MIME_BYTES + 1));

    let error = default_images_convert(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("oversized root MIME must be rejected");

    assert!(error.contains("root MIME"), "got {error:?}");
}

#[test]
fn images_convert_enforces_entry_name_metadata_cap() {
    let name = format!("{}.png", "n".repeat(SECURITY_TEST_MAX_NAME_BYTES));
    let input = input_directory(&name, None);

    let error = default_images_convert(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("oversized entry name must be rejected");

    assert!(error.contains("entry name"), "got {error:?}");
}

#[test]
fn images_convert_enforces_entry_mime_metadata_cap() {
    let mime = "m".repeat(SECURITY_TEST_MAX_MIME_BYTES + 1);
    let input = input_directory("one.png", Some(mime));

    let error = default_images_convert(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("oversized entry MIME must be rejected");

    assert!(error.contains("entry MIME"), "got {error:?}");
}

#[test]
fn images_convert_counts_long_name_duplication_in_working_set() {
    use super::super::images_convert::{BatchMemoryLimits, images_convert_with_limits};

    let extension = ".png";
    let name = format!(
        "{}{extension}",
        "n".repeat(SECURITY_TEST_MAX_NAME_BYTES - extension.len())
    );
    let input = input_directory(&name, None);
    let payload_bytes = match &input.content {
        FileContent::Directory(entries) => match &entries[0].content {
            FileContent::Bytes(bytes) => bytes.len(),
            FileContent::Directory(_) => unreachable!("fixture entry is a byte file"),
        },
        FileContent::Bytes(_) => unreachable!("fixture root is a directory"),
    };
    let limits = BatchMemoryLimits {
        working_bytes: SECURITY_TEST_FIXED_OVERHEAD_BYTES
            + payload_bytes
            + SECURITY_TEST_ZIP_BYTES * SECURITY_TEST_JSON_AND_ZIP_MULTIPLIER
            + SECURITY_TEST_UNACCOUNTED_MARGIN_BYTES,
        encoded_image_bytes: SECURITY_TEST_ENCODED_BYTES,
        zip_bytes: SECURITY_TEST_ZIP_BYTES,
    };

    let error = images_convert_with_limits(&input, "png", limits)
        .expect_err("name allocations over the aggregate budget must be rejected");

    assert!(error.contains("working set"), "got {error:?}");
}
