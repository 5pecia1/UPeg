use super::*;

fn image_directory(entries: Vec<FileValue>) -> FileValue {
    FileValue {
        name: "photos".to_string(),
        mime: None,
        content: FileContent::Directory(entries),
    }
}

fn image_file(name: &str, bytes: Vec<u8>) -> FileValue {
    FileValue {
        name: name.to_string(),
        mime: None,
        content: FileContent::Bytes(bytes),
    }
}

#[test]
fn images_convert_preserves_input_order_and_disambiguates_duplicate_names() {
    let first = png_bytes(2, 1, [255, 0, 0, 255]);
    let second = jpeg_bytes(1, 2, [0, 0, 255]);
    let images = image_directory(vec![
        image_file("same.png", first),
        image_file("same.jpg", second),
    ]);

    let result = default_images_convert(&images, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect("flat image directory should convert");
    let (file, entries) = output_zip_entries(&result);

    assert_eq!(file.name, "photos-images.zip");
    assert_eq!(file.mime.as_deref(), Some(IMAGES_ZIP_MIME));
    assert_eq!(
        entries
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["same.png", "same-1.png"]
    );
    let first_pixel = image::load_from_memory(&entries[0].1)
        .expect("first PNG should decode")
        .to_rgba8()
        .get_pixel(0, 0)
        .0;
    let second_pixel = image::load_from_memory(&entries[1].1)
        .expect("second PNG should decode")
        .to_rgba8()
        .get_pixel(0, 0)
        .0;
    assert_eq!(first_pixel, [255, 0, 0, 255]);
    assert!(
        second_pixel[2] > second_pixel[0],
        "second entry must remain the blue input"
    );
}

#[test]
fn images_convert_streams_results_into_zip_without_collecting() {
    let source = include_str!("../images_convert.rs");

    assert!(
        !source.contains("pack_named_images_zip"),
        "batch conversion must stream each encoded image directly into its capped zip"
    );
}

#[test]
fn images_convert_checks_total_working_set_budget_before_decode() {
    use super::super::images_convert::{BatchMemoryLimits, images_convert_with_limits};

    let input = image_directory(vec![image_file(
        "one.png",
        png_bytes(1024, 1024, [0, 0, 0, 255]),
    )]);
    let limits = BatchMemoryLimits {
        working_bytes: 16 << 20,
        encoded_image_bytes: 512 << 10,
        zip_bytes: 512 << 10,
    };

    let error = images_convert_with_limits(&input, "png", limits)
        .expect_err("aggregate working set over its budget must be rejected");

    assert!(error.contains("working set"), "got {error:?}");
}

#[test]
fn images_convert_enforces_zip_output_byte_cap() {
    use super::super::images_convert::{BatchMemoryLimits, images_convert_with_limits};

    let input = image_directory(vec![image_file("one.png", png_bytes(1, 1, [0, 0, 0, 255]))]);
    let limits = BatchMemoryLimits {
        working_bytes: 16 << 20,
        encoded_image_bytes: 1 << 20,
        zip_bytes: 8,
    };

    let error = images_convert_with_limits(&input, "png", limits)
        .expect_err("zip bytes over the named output cap must be rejected");

    assert!(error.contains("output zip"), "got {error:?}");
}

#[test]
fn images_convert_detects_format_from_bytes_not_extension() {
    let png = png_bytes(1, 1, [10, 20, 30, 255]);
    let images = image_directory(vec![image_file("actually-jpeg.jpg", png)]);

    let result = default_images_convert(&images, "jpeg", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect("PNG bytes under a JPG name are supported");
    let (_file, entries) = output_zip_entries(&result);

    assert_eq!(entries[0].0, "actually-jpeg.jpeg");
    assert_eq!(
        image::guess_format(&entries[0].1).expect("encoded output format"),
        image::ImageFormat::Jpeg
    );
}

#[test]
fn jpeg_conversion_composites_transparency_on_white() {
    let transparent = png_bytes(1, 1, [0, 0, 0, 0]);
    let images = image_directory(vec![image_file("transparent.png", transparent)]);

    let result = default_images_convert(&images, "jpeg", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect("transparent PNG should convert");
    let (_file, entries) = output_zip_entries(&result);
    let pixel = image::load_from_memory(&entries[0].1)
        .expect("JPEG should decode")
        .to_rgb8()
        .get_pixel(0, 0)
        .0;

    assert!(
        pixel.iter().all(|channel| *channel >= 250),
        "transparent pixel should become white, got {pixel:?}"
    );
}

#[test]
fn images_convert_rejects_bytes_file_input() {
    let input = image_file("one.png", png_bytes(1, 1, [0, 0, 0, 255]));

    let error = default_images_convert(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("direct bytes must be rejected");

    assert!(error.contains("directory"), "got {error:?}");
}

#[test]
fn images_convert_rejects_empty_directory() {
    let input = image_directory(Vec::new());

    let error = default_images_convert(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("empty directory must be rejected");

    assert!(error.contains("empty"), "got {error:?}");
}

#[test]
fn images_convert_rejects_nested_directories() {
    let nested = image_directory(vec![image_file("one.png", png_bytes(1, 1, [0, 0, 0, 255]))]);
    let input = image_directory(vec![nested]);

    let error = default_images_convert(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("nested directory must be rejected");

    assert!(error.contains("nested"), "got {error:?}");
}

#[test]
fn images_convert_rejects_corrupt_content_despite_extension() {
    let input = image_directory(vec![image_file("fake.png", b"GIF89a".to_vec())]);

    let error = default_images_convert(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("unsupported bytes must be rejected");

    assert!(error.contains("could not inspect"), "got {error:?}");
}

#[test]
fn images_convert_rejects_disallowed_output_format() {
    let input = image_directory(vec![image_file("one.png", png_bytes(1, 1, [0, 0, 0, 255]))]);

    let error = default_images_convert(&input, "heic", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("unknown output format must be rejected");

    assert!(error.contains("output format"), "got {error:?}");
}

#[test]
fn images_convert_meta_exposes_file_policy_and_output_format() {
    let meta = upeg_core::inventory::iter::<upeg_core::StaticToolMeta>()
        .find(|meta| meta.id == "media.images_convert")
        .expect("images_convert metadata should exist");
    let images = meta
        .input_spec
        .fields
        .iter()
        .find(|field| field.name == "images")
        .expect("images field should exist");
    let output_format = meta
        .input_spec
        .fields
        .iter()
        .find(|field| field.name == "output_format")
        .expect("output_format field should exist");

    match images.kind {
        upeg_core::StaticInputKind::File(policy) => {
            assert_eq!(policy.max_count, 100);
            assert_eq!(
                policy.extensions,
                super::super::image_conversion::INPUT_EXTENSIONS
            );
            assert_eq!(policy.max_file_bytes, Some(52_428_800));
            assert_eq!(policy.max_total_bytes, Some(52_428_800));
        }
        other => panic!("expected File policy, got {other:?}"),
    }
    match output_format.kind {
        upeg_core::StaticInputKind::Options(options) => {
            assert_eq!(
                options
                    .iter()
                    .map(|option| option.value)
                    .collect::<Vec<_>>(),
                ["png", "jpeg", "webp", "gif", "bmp", "tiff", "ico", "qoi"]
            );
        }
        other => panic!("expected Options, got {other:?}"),
    }
}

#[test]
fn images_convert_meta_exposes_only_file_capable_surfaces() {
    let meta = upeg_core::inventory::iter::<upeg_core::StaticToolMeta>()
        .find(|meta| meta.id == "media.images_convert")
        .expect("images_convert metadata should exist");

    assert_eq!(
        meta.surfaces,
        &[
            upeg_core::Surface::Cli,
            upeg_core::Surface::Desktop,
            upeg_core::Surface::Mcp,
            upeg_core::Surface::Http,
            upeg_core::Surface::Pwa,
            upeg_core::Surface::Ext,
        ]
    );
}

#[test]
fn raster_output_format_determines_extension_and_mime() {
    use super::super::image_conversion::ConversionFormat;

    let png = ConversionFormat::parse("png").expect("png format should parse");
    let jpeg = ConversionFormat::parse("jpeg").expect("jpeg format should parse");

    assert_eq!((png.extension(), png.mime_type()), ("png", "image/png"));
    assert_eq!((jpeg.extension(), jpeg.mime_type()), ("jpeg", "image/jpeg"));
}
