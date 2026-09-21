use super::super::images_convert::{BatchMemoryLimits, images_convert_with_limits};
use super::*;

const TEST_WORKING_BYTES: usize = 512 << 20;
const SMALL_USER_OUTPUT_CAP_BYTES: usize = 32;
const SCHEMA_MIN_OUTPUT_BYTES: f64 = 1.0;
const SCHEMA_MAX_OUTPUT_BYTES: f64 = 67_108_864.0;

fn image_directory(colors: &[[u8; 4]]) -> FileValue {
    FileValue {
        name: "photos".to_string(),
        mime: None,
        content: FileContent::Directory(
            colors
                .iter()
                .enumerate()
                .map(|(index, color)| FileValue {
                    name: format!("image-{index}.png"),
                    mime: Some("image/png".to_string()),
                    content: FileContent::Bytes(png_bytes(1, 1, *color)),
                })
                .collect(),
        ),
    }
}

const fn default_batch_limits() -> BatchMemoryLimits {
    BatchMemoryLimits {
        working_bytes: TEST_WORKING_BYTES,
        encoded_image_bytes: IMAGES_CONVERT_MAX_OUTPUT_BYTES,
        zip_bytes: IMAGES_CONVERT_MAX_OUTPUT_BYTES,
    }
}

fn zip_byte_len(result: &str) -> usize {
    file_bytes(&output_file(result)).len()
}

fn total_encoded_image_bytes(input: &FileValue) -> usize {
    let result = images_convert_with_limits(input, "png", default_batch_limits())
        .expect("baseline conversion should succeed");
    let (_, entries) = output_zip_entries(&result);
    entries.iter().map(|(_, bytes)| bytes.len()).sum()
}

#[test]
fn images_convert_rejects_zero_byte_output_cap() {
    let input = image_directory(&[[1, 2, 3, u8::MAX]]);

    let error =
        default_images_convert(&input, "png", 0).expect_err("zero output cap must be rejected");

    assert!(error.contains("max_output_bytes"), "got {error:?}");
}

#[test]
fn images_convert_rejects_request_one_byte_over_hard_cap() {
    let input = image_directory(&[[1, 2, 3, u8::MAX]]);

    let error = default_images_convert(&input, "png", IMAGES_CONVERT_MAX_OUTPUT_BYTES + 1)
        .expect_err("a request above the hard cap must be rejected");

    assert!(error.contains("max_output_bytes"), "got {error:?}");
}

#[test]
fn images_convert_allows_hard_cap_boundary() {
    let input = image_directory(&[[1, 2, 3, u8::MAX]]);

    let result = default_images_convert(&input, "png", IMAGES_CONVERT_MAX_OUTPUT_BYTES);

    assert!(
        result.is_ok(),
        "hard cap boundary should be valid: {result:?}"
    );
}

#[test]
fn zip_output_cap_allows_exact_result_byte_boundary() {
    let input = image_directory(&[[1, 2, 3, u8::MAX]]);
    let baseline = default_images_convert(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect("baseline conversion should succeed");
    let exact_output_bytes = zip_byte_len(&baseline);

    let result = default_images_convert(&input, "png", exact_output_bytes);

    assert!(
        result.is_ok(),
        "exact output boundary should succeed: {result:?}"
    );
}

#[test]
fn zip_result_one_byte_over_output_cap_is_rejected() {
    let input = image_directory(&[[1, 2, 3, u8::MAX]]);
    let baseline = default_images_convert(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect("baseline conversion should succeed");
    let smaller_cap = zip_byte_len(&baseline) - 1;

    let error = default_images_convert(&input, "png", smaller_cap)
        .expect_err("one byte over the requested zip cap must be rejected");

    assert!(error.contains("output zip"), "got {error:?}");
}

#[test]
fn small_user_cap_applies_to_encoded_image_too() {
    let input = image_directory(&[[1, 2, 3, u8::MAX]]);

    let error = default_images_convert(&input, "png", SMALL_USER_OUTPUT_CAP_BYTES)
        .expect_err("encoded image over the user cap must be rejected");

    assert!(error.contains("encoded raster"), "got {error:?}");
}

#[test]
fn batch_encoded_budget_allows_single_image_boundary() {
    let input = image_directory(&[[1, 2, 3, u8::MAX]]);
    let exact_encoded_bytes = total_encoded_image_bytes(&input);
    let limits = BatchMemoryLimits {
        encoded_image_bytes: exact_encoded_bytes,
        ..default_batch_limits()
    };

    let result = images_convert_with_limits(&input, "png", limits);

    assert!(
        result.is_ok(),
        "exact aggregate encoded boundary should succeed: {result:?}"
    );
}

#[test]
fn batch_encoded_budget_applies_to_aggregate_not_per_image() {
    let single_input = image_directory(&[[1, 2, 3, u8::MAX]]);
    let input = image_directory(&[[1, 2, 3, u8::MAX], [1, 2, 3, u8::MAX]]);
    let single_image_budget = total_encoded_image_bytes(&single_input);
    let limits = BatchMemoryLimits {
        encoded_image_bytes: single_image_budget,
        ..default_batch_limits()
    };

    let error = images_convert_with_limits(&input, "png", limits)
        .expect_err("a second individually valid image must exceed the aggregate budget");

    assert!(error.contains("encoded raster"), "got {error:?}");
}

#[test]
fn images_convert_schema_exposes_output_byte_range_and_default() {
    let meta = upeg_core::inventory::iter::<upeg_core::StaticToolMeta>()
        .find(|meta| meta.id == "media.images_convert")
        .expect("images_convert metadata should exist");
    let max_output_bytes = meta
        .input_spec
        .fields
        .iter()
        .find(|field| field.name == "max_output_bytes")
        .expect("max_output_bytes field should exist");
    let constraints = max_output_bytes
        .constraints
        .number
        .expect("max_output_bytes should declare numeric constraints");

    assert!(!max_output_bytes.required);
    assert_eq!(max_output_bytes.kind, upeg_core::StaticInputKind::Integer);
    assert_eq!(constraints.min, Some(SCHEMA_MIN_OUTPUT_BYTES));
    assert_eq!(constraints.max, Some(SCHEMA_MAX_OUTPUT_BYTES));
    assert_eq!(constraints.default, Some(SCHEMA_MAX_OUTPUT_BYTES));
}
