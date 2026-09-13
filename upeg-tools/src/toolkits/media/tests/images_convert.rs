use super::*;

fn 이미지_디렉터리(entries: Vec<FileValue>) -> FileValue {
    FileValue {
        name: "photos".to_string(),
        mime: None,
        content: FileContent::Directory(entries),
    }
}

fn 이미지_파일(name: &str, bytes: Vec<u8>) -> FileValue {
    FileValue {
        name: name.to_string(),
        mime: None,
        content: FileContent::Bytes(bytes),
    }
}

#[test]
fn 이미지_일괄_변환은_입력_순서를_보존하고_중복_이름을_구분한다() {
    let first = png_bytes(2, 1, [255, 0, 0, 255]);
    let second = jpeg_bytes(1, 2, [0, 0, 255]);
    let images = 이미지_디렉터리(vec![
        이미지_파일("same.png", first),
        이미지_파일("same.jpg", second),
    ]);

    let result = 기본_이미지_변환(&images, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
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
fn 이미지_일괄_변환은_변환_결과를_모으지_않고_zip에_바로_기록한다() {
    let source = include_str!("../images_convert.rs");

    assert!(
        !source.contains("pack_named_images_zip"),
        "batch conversion must stream each encoded image directly into its capped zip"
    );
}

#[test]
fn 이미지_일괄_변환은_decode전에_전체_working_set_budget을_검사한다() {
    use super::super::images_convert::{BatchMemoryLimits, images_convert_with_limits};

    let input = 이미지_디렉터리(vec![이미지_파일(
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
fn 이미지_일괄_변환은_zip_출력_자체의_byte_cap을_지킨다() {
    use super::super::images_convert::{BatchMemoryLimits, images_convert_with_limits};

    let input = 이미지_디렉터리(vec![이미지_파일(
        "one.png",
        png_bytes(1, 1, [0, 0, 0, 255]),
    )]);
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
fn 이미지_일괄_변환은_확장자가_아닌_실제_바이트_형식을_판별한다() {
    let png = png_bytes(1, 1, [10, 20, 30, 255]);
    let images = 이미지_디렉터리(vec![이미지_파일("actually-jpeg.jpg", png)]);

    let result = 기본_이미지_변환(&images, "jpeg", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect("PNG bytes under a JPG name are supported");
    let (_file, entries) = output_zip_entries(&result);

    assert_eq!(entries[0].0, "actually-jpeg.jpeg");
    assert_eq!(
        image::guess_format(&entries[0].1).expect("encoded output format"),
        image::ImageFormat::Jpeg
    );
}

#[test]
fn jpeg_변환은_투명도를_흰색_배경에_합성한다() {
    let transparent = png_bytes(1, 1, [0, 0, 0, 0]);
    let images = 이미지_디렉터리(vec![이미지_파일("transparent.png", transparent)]);

    let result = 기본_이미지_변환(&images, "jpeg", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
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
fn 이미지_일괄_변환은_바이트_파일_입력을_거부한다() {
    let input = 이미지_파일("one.png", png_bytes(1, 1, [0, 0, 0, 255]));

    let error = 기본_이미지_변환(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("direct bytes must be rejected");

    assert!(error.contains("directory"), "got {error:?}");
}

#[test]
fn 이미지_일괄_변환은_빈_디렉터리를_거부한다() {
    let input = 이미지_디렉터리(Vec::new());

    let error = 기본_이미지_변환(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("empty directory must be rejected");

    assert!(error.contains("empty"), "got {error:?}");
}

#[test]
fn 이미지_일괄_변환은_중첩_디렉터리를_거부한다() {
    let nested = 이미지_디렉터리(vec![이미지_파일(
        "one.png",
        png_bytes(1, 1, [0, 0, 0, 255]),
    )]);
    let input = 이미지_디렉터리(vec![nested]);

    let error = 기본_이미지_변환(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("nested directory must be rejected");

    assert!(error.contains("nested"), "got {error:?}");
}

#[test]
fn 이미지_일괄_변환은_확장자와_달리_손상된_이미지_내용을_거부한다() {
    let input = 이미지_디렉터리(vec![이미지_파일("fake.png", b"GIF89a".to_vec())]);

    let error = 기본_이미지_변환(&input, "png", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("unsupported bytes must be rejected");

    assert!(error.contains("could not inspect"), "got {error:?}");
}

#[test]
fn 이미지_일괄_변환은_허용되지_않은_출력_형식을_거부한다() {
    let input = 이미지_디렉터리(vec![이미지_파일(
        "one.png",
        png_bytes(1, 1, [0, 0, 0, 255]),
    )]);

    let error = 기본_이미지_변환(&input, "heic", IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES)
        .expect_err("unknown output format must be rejected");

    assert!(error.contains("output format"), "got {error:?}");
}

#[test]
fn 이미지_일괄_변환_메타는_파일_정책과_출력_형식을_노출한다() {
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
fn 이미지_일괄_변환_메타는_파일_입력을_지원하는_surface만_노출한다() {
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
fn raster_출력_형식은_확장자와_mime을_함께_결정한다() {
    use super::super::image_conversion::ConversionFormat;

    let png = ConversionFormat::parse("png").expect("png format should parse");
    let jpeg = ConversionFormat::parse("jpeg").expect("jpeg format should parse");

    assert_eq!((png.extension(), png.mime_type()), ("png", "image/png"));
    assert_eq!((jpeg.extension(), jpeg.mime_type()), ("jpeg", "image/jpeg"));
}
