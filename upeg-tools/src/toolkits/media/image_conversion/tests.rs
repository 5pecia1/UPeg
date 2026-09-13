use std::io::Cursor;

use super::super::{MAX_MEDIA_OUTPUT_BYTES, image_convert, images_convert};
use super::*;
use image::{DynamicImage, ImageFormat};
use upeg_core::{FileContent, FileValue};

const FORMATS: &[&str] = &["png", "jpeg", "webp", "gif", "bmp", "tiff", "ico", "qoi"];
const TRANSLUCENT_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#ff0000" fill-opacity="0.5"/></svg>"##;

fn 파일(name: &str, bytes: Vec<u8>) -> FileValue {
    FileValue {
        name: name.into(),
        content: FileContent::Bytes(bytes),
        mime: None,
    }
}
fn 픽스처() -> FileValue {
    let raster = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        8,
        4,
        image::Rgba([120, 70, 30, 128]),
    ));
    let mut bytes = Cursor::new(Vec::new());
    raster.write_to(&mut bytes, ImageFormat::Png).unwrap();
    파일("source.png", bytes.into_inner())
}
fn 출력(result: String) -> FileValue {
    serde_json::from_str(&result).unwrap()
}
fn 바이트(file: &FileValue) -> &[u8] {
    let FileContent::Bytes(bytes) = &file.content else {
        panic!("expected file bytes")
    };
    bytes
}
fn 변환(input: &FileValue, format: &str) -> Result<String, String> {
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
fn 모든_출력_형식은_실제_이미지로_다시_열리고_파일명과_마임이_일치한다() {
    for format in FORMATS {
        let converted = 출력(변환(&픽스처(), format).unwrap());
        let expected = ConversionFormat::parse(format).unwrap();
        assert_eq!(converted.name, format!("source.{format}"));
        assert_eq!(converted.mime.as_deref(), Some(expected.mime_type()));
        let raster = image::load_from_memory(바이트(&converted)).unwrap();
        assert_eq!((raster.width(), raster.height()), (8, 4));
        let png = 출력(변환(&converted, "png").unwrap());
        assert_eq!(
            image::load_from_memory(바이트(&png)).unwrap().dimensions(),
            (8, 4)
        );
    }
}

#[test]
fn 무손실_웹피는_반투명_픽셀을_유지한다() {
    let input = 픽스처();
    let result = 출력(변환(&input, "webp").unwrap());
    assert_eq!(
        image::load_from_memory(바이트(&result)).unwrap().to_rgba8(),
        image::load_from_memory(바이트(&input)).unwrap().to_rgba8()
    );
}

#[test]
fn 투명한_이미지의_제이펙_배경색을_선택할_수_있다() {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image::RgbaImage::new(8, 8))
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    let input = 파일("transparent.png", bytes.into_inner());
    let result =
        출력(image_convert(&input, "jpeg", 100, "#000000", 0, MAX_MEDIA_OUTPUT_BYTES).unwrap());
    let rgb = image::load_from_memory(바이트(&result)).unwrap().to_rgb8();
    assert!(rgb.as_raw().iter().all(|&channel| channel <= 1));
}

#[test]
fn 에스브이지는_종횡비와_반투명_색상을_보존한다() {
    let input = 파일("shape.svg", TRANSLUCENT_SVG.to_vec());
    let result =
        출력(image_convert(&input, "png", 90, "#FFFFFF", 80, MAX_MEDIA_OUTPUT_BYTES).unwrap());
    let raster = image::load_from_memory(바이트(&result)).unwrap().to_rgba8();
    assert_eq!(raster.dimensions(), (80, 40));
    let pixel = raster.get_pixel(20, 20).0;
    assert_eq!(&pixel[..3], &[255, 0, 0]);
    assert!((127..=128).contains(&pixel[3]));
}

#[test]
fn 에스브이지의_한글_텍스트를_내장_글꼴로_렌더링한다() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="60"><text x="4" y="35" font-size="30">한글 변환</text></svg>"#;
    let result = 출력(변환(&파일("text.svg", svg.as_bytes().to_vec()), "png").unwrap());
    let raster = image::load_from_memory(바이트(&result)).unwrap().to_rgba8();
    assert!(raster.pixels().filter(|pixel| pixel[3] > 0).count() > 100);
}

#[test]
fn 에스브이지_외부_참조와_거대한_출력은_거부한다() {
    let external = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image href="file:///tmp/private.png" width="20" height="20"/></svg>"#;
    assert!(
        변환(&파일("external.svg", external.to_vec()), "png")
            .unwrap_err()
            .contains("external")
    );
    let huge = br#"<svg xmlns="http://www.w3.org/2000/svg" width="65535" height="65535"><rect width="100%" height="100%"/></svg>"#;
    assert!(변환(&파일("huge.svg", huge.to_vec()), "png").is_err());
}

#[test]
fn 잘못된_옵션과_출력_크기_초과는_오류로_반환한다() {
    assert!(ImageConversionOptions::new("png", 0, "#FFFFFF", 0).is_err());
    assert!(ImageConversionOptions::new("png", 90, "red", 0).is_err());
    assert!(
        ImageConversionOptions::new("png", 90, "#FFFFFF", IMAGE_CONVERT_MAX_SVG_WIDTH + 1).is_err()
    );
    for format in FORMATS {
        assert!(
            image_convert(&픽스처(), format, 90, "#FFFFFF", 0, 1).is_err(),
            "{format}"
        );
    }
    let input = 파일("shape.svg", TRANSLUCENT_SVG.to_vec());
    assert!(
        image_convert(&input, "ico", 90, "#FFFFFF", 300, MAX_MEDIA_OUTPUT_BYTES)
            .unwrap_err()
            .contains("256")
    );
}

#[test]
fn 여러_형식과_에스브이지를_같은_설정으로_일괄_변환한다() {
    let webp = 출력(변환(&픽스처(), "webp").unwrap());
    let batch = FileValue {
        name: "images".into(),
        content: FileContent::Directory(vec![webp, 파일("shape.svg", TRANSLUCENT_SVG.to_vec())]),
        mime: None,
    };
    let result =
        출력(images_convert(&batch, "png", 90, "#FFFFFF", 80, MAX_MEDIA_OUTPUT_BYTES).unwrap());
    let mut zip = zip::ZipArchive::new(Cursor::new(바이트(&result))).unwrap();
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
