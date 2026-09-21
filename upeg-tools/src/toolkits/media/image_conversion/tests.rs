use std::io::Cursor;

use super::super::{MAX_MEDIA_OUTPUT_BYTES, image_convert, images_convert};
use super::*;
use image::{DynamicImage, ImageFormat};
use upeg_core::{FileContent, FileValue};

const FORMATS: &[&str] = &["png", "jpeg", "webp", "gif", "bmp", "tiff", "ico", "qoi"];
const TRANSLUCENT_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#ff0000" fill-opacity="0.5"/></svg>"##;

fn named_file(name: &str, bytes: Vec<u8>) -> FileValue {
    FileValue {
        name: name.into(),
        content: FileContent::Bytes(bytes),
        mime: None,
    }
}
fn fixture() -> FileValue {
    let raster = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        8,
        4,
        image::Rgba([120, 70, 30, 128]),
    ));
    let mut bytes = Cursor::new(Vec::new());
    raster.write_to(&mut bytes, ImageFormat::Png).unwrap();
    named_file("source.png", bytes.into_inner())
}
fn output(result: String) -> FileValue {
    serde_json::from_str(&result).unwrap()
}
fn file_bytes(file: &FileValue) -> &[u8] {
    let FileContent::Bytes(bytes) = &file.content else {
        panic!("expected file bytes")
    };
    bytes
}
fn convert_with_defaults(input: &FileValue, format: &str) -> Result<String, String> {
    image_convert(
        input,
        format,
        IMAGE_CONVERT_DEFAULT_QUALITY,
        IMAGE_CONVERT_DEFAULT_BACKGROUND,
        IMAGE_CONVERT_DEFAULT_SVG_WIDTH,
        MAX_MEDIA_OUTPUT_BYTES,
    )
}

#[test]
fn every_output_format_reopens_as_image_with_matching_name_and_mime() {
    for format in FORMATS {
        let converted = output(convert_with_defaults(&fixture(), format).unwrap());
        let expected = ConversionFormat::parse(format).unwrap();
        assert_eq!(converted.name, format!("source.{format}"));
        assert_eq!(converted.mime.as_deref(), Some(expected.mime_type()));
        let raster = image::load_from_memory(file_bytes(&converted)).unwrap();
        assert_eq!((raster.width(), raster.height()), (8, 4));
        let png = output(convert_with_defaults(&converted, "png").unwrap());
        assert_eq!(
            image::load_from_memory(file_bytes(&png))
                .unwrap()
                .dimensions(),
            (8, 4)
        );
    }
}

#[test]
fn lossless_webp_preserves_translucent_pixels() {
    let input = fixture();
    let result = output(convert_with_defaults(&input, "webp").unwrap());
    assert_eq!(
        image::load_from_memory(file_bytes(&result))
            .unwrap()
            .to_rgba8(),
        image::load_from_memory(file_bytes(&input))
            .unwrap()
            .to_rgba8()
    );
}

#[test]
fn jpeg_background_color_is_selectable_for_transparent_images() {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image::RgbaImage::new(8, 8))
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    let input = named_file("transparent.png", bytes.into_inner());
    let result =
        output(image_convert(&input, "jpeg", 100, "#000000", 0, MAX_MEDIA_OUTPUT_BYTES).unwrap());
    let rgb = image::load_from_memory(file_bytes(&result))
        .unwrap()
        .to_rgb8();
    assert!(rgb.as_raw().iter().all(|&channel| channel <= 1));
}

#[test]
fn svg_preserves_aspect_ratio_and_translucent_colors() {
    let input = named_file("shape.svg", TRANSLUCENT_SVG.to_vec());
    let result =
        output(image_convert(&input, "png", 90, "#FFFFFF", 80, MAX_MEDIA_OUTPUT_BYTES).unwrap());
    let raster = image::load_from_memory(file_bytes(&result))
        .unwrap()
        .to_rgba8();
    assert_eq!(raster.dimensions(), (80, 40));
    let pixel = raster.get_pixel(20, 20).0;
    assert_eq!(&pixel[..3], &[255, 0, 0]);
    assert!((127..=128).contains(&pixel[3]));
}

#[test]
fn svg_hangul_text_renders_with_bundled_font() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="60"><text x="4" y="35" font-size="30">한글 변환</text></svg>"#;
    let result = output(
        convert_with_defaults(&named_file("text.svg", svg.as_bytes().to_vec()), "png").unwrap(),
    );
    let raster = image::load_from_memory(file_bytes(&result))
        .unwrap()
        .to_rgba8();
    assert!(raster.pixels().filter(|pixel| pixel[3] > 0).count() > 100);
}

#[test]
fn svg_rejects_external_references_and_huge_output() {
    let external = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image href="file:///tmp/private.png" width="20" height="20"/></svg>"#;
    assert!(
        convert_with_defaults(&named_file("external.svg", external.to_vec()), "png")
            .unwrap_err()
            .contains("external")
    );
    let huge = br#"<svg xmlns="http://www.w3.org/2000/svg" width="65535" height="65535"><rect width="100%" height="100%"/></svg>"#;
    assert!(convert_with_defaults(&named_file("huge.svg", huge.to_vec()), "png").is_err());
}

#[test]
fn invalid_options_and_oversized_output_return_errors() {
    assert!(ImageConversionOptions::new("png", 0, "#FFFFFF", 0).is_err());
    assert!(ImageConversionOptions::new("png", 90, "red", 0).is_err());
    assert!(
        ImageConversionOptions::new("png", 90, "#FFFFFF", IMAGE_CONVERT_MAX_SVG_WIDTH + 1).is_err()
    );
    for format in FORMATS {
        assert!(
            image_convert(&fixture(), format, 90, "#FFFFFF", 0, 1).is_err(),
            "{format}"
        );
    }
    let input = named_file("shape.svg", TRANSLUCENT_SVG.to_vec());
    assert!(
        image_convert(&input, "ico", 90, "#FFFFFF", 300, MAX_MEDIA_OUTPUT_BYTES)
            .unwrap_err()
            .contains("256")
    );
}

#[test]
fn batch_converts_multiple_formats_and_svg_with_same_settings() {
    let webp = output(convert_with_defaults(&fixture(), "webp").unwrap());
    let batch = FileValue {
        name: "images".into(),
        content: FileContent::Directory(vec![
            webp,
            named_file("shape.svg", TRANSLUCENT_SVG.to_vec()),
        ]),
        mime: None,
    };
    let result =
        output(images_convert(&batch, "png", 90, "#FFFFFF", 80, MAX_MEDIA_OUTPUT_BYTES).unwrap());
    let mut zip = zip::ZipArchive::new(Cursor::new(file_bytes(&result))).unwrap();
    assert_eq!(zip.len(), 2);
    use std::io::Read as _;
    let mut bytes = Vec::new();
    zip.by_name("shape.png")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(
        image::load_from_memory(&bytes).unwrap().dimensions(),
        (80, 40)
    );
}
