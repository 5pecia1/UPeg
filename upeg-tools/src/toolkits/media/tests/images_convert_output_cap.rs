use super::super::images_convert::{BatchMemoryLimits, images_convert_with_limits};
use super::*;

const TEST_WORKING_BYTES: usize = 512 << 20;
const SMALL_USER_OUTPUT_CAP_BYTES: usize = 32;
const SCHEMA_MIN_OUTPUT_BYTES: f64 = 1.0;
const SCHEMA_MAX_OUTPUT_BYTES: f64 = 67_108_864.0;

fn 이미지_디렉터리(colors: &[[u8; 4]]) -> FileValue {
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

const fn 기본_batch_limits() -> BatchMemoryLimits {
    BatchMemoryLimits {
        working_bytes: TEST_WORKING_BYTES,
        encoded_image_bytes: IMAGES_CONVERT_MAX_OUTPUT_BYTES,
        zip_bytes: IMAGES_CONVERT_MAX_OUTPUT_BYTES,
    }
}

fn zip_byte_len(result: &str) -> usize {
    file_bytes(&output_file(result)).len()
}

fn 전체_encoded_image_bytes(input: &FileValue) -> usize {
    let result = images_convert_with_limits(input, "png", 기본_batch_limits())
        .expect("baseline conversion should succeed");
    let (_, entries) = output_zip_entries(&result);
    entries.iter().map(|(_, bytes)| bytes.len()).sum()
}

#[test]
fn 이미지_일괄_변환은_0_byte_output_cap을_거부한다() {
    let input = 이미지_디렉터리(&[[1, 2, 3, u8::MAX]]);

    let error = 기본_이미지_변환(&input, "png", 0).expect_err("zero output cap must be rejected");

    assert!(error.contains("max_output_bytes"), "got {error:?}");
}

#[test]
fn 이미지_일괄_변환은_hard_cap보다_1_byte_큰_요청을_거부한다() {
    let input = 이미지_디렉터리(&[[1, 2, 3, u8::MAX]]);

    let error = 기본_이미지_변환(&input, "png", IMAGES_CONVERT_MAX_OUTPUT_BYTES + 1)
        .expect_err("a request above the hard cap must be rejected");

    assert!(error.contains("max_output_bytes"), "got {error:?}");
}

#[test]
fn 이미지_일괄_변환은_hard_cap_경계를_허용한다() {
    let input = 이미지_디렉터리(&[[1, 2, 3, u8::MAX]]);

    let result = 기본_이미지_변환(&input, "png", IMAGES_CONVERT_MAX_OUTPUT_BYTES);

    assert!(
        result.is_ok(),
        "hard cap boundary should be valid: {result:?}"
    );
}

#[test]
fn zip_output_cap은_정확한_결과_byte_경계를_허용한다() {
    let input = 이미지_디렉터리(&[[1, 2, 3, u8::MAX]]);
    let baseline = 기본_이미지_변환(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect("baseline conversion should succeed");
    let exact_output_bytes = zip_byte_len(&baseline);

    let result = 기본_이미지_변환(&input, "png", exact_output_bytes);

    assert!(
        result.is_ok(),
        "exact output boundary should succeed: {result:?}"
    );
}

#[test]
fn zip_결과가_output_cap보다_1_byte_크면_거부한다() {
    let input = 이미지_디렉터리(&[[1, 2, 3, u8::MAX]]);
    let baseline = 기본_이미지_변환(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect("baseline conversion should succeed");
    let smaller_cap = zip_byte_len(&baseline) - 1;

    let error = 기본_이미지_변환(&input, "png", smaller_cap)
        .expect_err("one byte over the requested zip cap must be rejected");

    assert!(error.contains("output zip"), "got {error:?}");
}

#[test]
fn 작은_user_cap은_encoded_image_생성에도_먼저_적용된다() {
    let input = 이미지_디렉터리(&[[1, 2, 3, u8::MAX]]);

    let error = 기본_이미지_변환(&input, "png", SMALL_USER_OUTPUT_CAP_BYTES)
        .expect_err("encoded image over the user cap must be rejected");

    assert!(error.contains("encoded raster"), "got {error:?}");
}

#[test]
fn batch_encoded_budget은_단일_이미지_경계를_허용한다() {
    let input = 이미지_디렉터리(&[[1, 2, 3, u8::MAX]]);
    let exact_encoded_bytes = 전체_encoded_image_bytes(&input);
    let limits = BatchMemoryLimits {
        encoded_image_bytes: exact_encoded_bytes,
        ..기본_batch_limits()
    };

    let result = images_convert_with_limits(&input, "png", limits);

    assert!(
        result.is_ok(),
        "exact aggregate encoded boundary should succeed: {result:?}"
    );
}

#[test]
fn batch_encoded_budget은_이미지별이_아닌_전체_합계에_적용된다() {
    let single_input = 이미지_디렉터리(&[[1, 2, 3, u8::MAX]]);
    let input = 이미지_디렉터리(&[[1, 2, 3, u8::MAX], [1, 2, 3, u8::MAX]]);
    let single_image_budget = 전체_encoded_image_bytes(&single_input);
    let limits = BatchMemoryLimits {
        encoded_image_bytes: single_image_budget,
        ..기본_batch_limits()
    };

    let error = images_convert_with_limits(&input, "png", limits)
        .expect_err("a second individually valid image must exceed the aggregate budget");

    assert!(error.contains("encoded raster"), "got {error:?}");
}

#[test]
fn 이미지_일괄_변환_schema는_output_byte_범위와_기본값을_노출한다() {
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
