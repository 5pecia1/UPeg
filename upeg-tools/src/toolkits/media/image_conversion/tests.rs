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

fn heic_fixture(name: &str) -> FileValue {
    named_file(
        name,
        include_bytes!("../../../../tests/fixtures/image_conversion/flat-64.heic").to_vec(),
    )
}
fn convert_with_defaults(input: &FileValue, format: &str) -> Result<String, String> {
    image_convert(
        input,
        format,
        IMAGE_CONVERT_DEFAULT_QUALITY,
        IMAGE_CONVERT_DEFAULT_BACKGROUND,
        IMAGE_CONVERT_DEFAULT_SVG_WIDTH,
        MAX_MEDIA_OUTPUT_BYTES,
        144.0,
        "poster",
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
fn heic_and_heif_inputs_convert_to_every_output_format() {
    for name in ["photo.heic", "photo.heif"] {
        let input = heic_fixture(name);
        for format in FORMATS {
            let converted = output(convert_with_defaults(&input, format).unwrap());
            assert_eq!(converted.name, format!("photo.{format}"));
            assert_eq!(
                converted.mime.as_deref(),
                Some(ConversionFormat::parse(format).unwrap().mime_type())
            );
            let raster = image::load_from_memory(file_bytes(&converted)).unwrap();
            assert_eq!(raster.dimensions(), (64, 64));
            let pixel = raster.to_rgb8().get_pixel(32, 32).0;
            assert!(
                pixel[0] > pixel[1] && pixel[0] > pixel[2],
                "{name} to {format}: {pixel:?}"
            );
        }
    }
}

#[test]
fn single_image_metadata_advertises_heic_and_heif_inputs() {
    let meta = upeg_core::inventory::iter::<upeg_core::StaticToolMeta>()
        .find(|meta| meta.id == "media.image_convert")
        .unwrap();
    let input = meta
        .input_spec
        .fields
        .iter()
        .find(|field| field.name == "input")
        .unwrap();
    match input.kind {
        upeg_core::StaticInputKind::File(policy) => {
            assert_eq!(policy.extensions, INPUT_EXTENSIONS);
            assert_eq!(
                policy.max_file_bytes,
                Some(IMAGE_CONVERT_MAX_INPUT_BYTES as u64)
            );
        }
        other => panic!("expected File policy, got {other:?}"),
    }
}

#[test]
fn batch_converts_heic_and_heif_with_expected_names() {
    let batch = FileValue {
        name: "photos".into(),
        content: FileContent::Directory(vec![heic_fixture("a.heic"), heic_fixture("b.heif")]),
        mime: None,
    };
    let result = output(
        images_convert(
            &batch,
            "png",
            90,
            "#FFFFFF",
            0,
            MAX_MEDIA_OUTPUT_BYTES,
            144.0,
            "poster",
        )
        .unwrap(),
    );
    let mut zip = zip::ZipArchive::new(Cursor::new(file_bytes(&result))).unwrap();
    for name in ["a.png", "b.png"] {
        use std::io::Read as _;
        let mut bytes = Vec::new();
        zip.by_name(name).unwrap().read_to_end(&mut bytes).unwrap();
        assert_eq!(
            image::load_from_memory(&bytes).unwrap().dimensions(),
            (64, 64)
        );
    }
}

#[test]
fn heic_grid_tiles_decode_in_their_expected_positions() {
    let input = named_file(
        "checker.heic",
        include_bytes!("../../../../tests/fixtures/image_conversion/checker-1024.heic").to_vec(),
    );
    let converted = output(convert_with_defaults(&input, "png").unwrap());
    let raster = image::load_from_memory(file_bytes(&converted))
        .unwrap()
        .to_rgb8();
    assert_eq!(raster.dimensions(), (1024, 1024));
    for (x, y, expected) in [
        (32, 32, [255, 255, 255]),
        (96, 32, [0, 0, 0]),
        (992, 992, [255, 255, 255]),
    ] {
        assert_eq!(raster.get_pixel(x, y).0, expected);
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
    let result = output(
        image_convert(
            &input,
            "jpeg",
            100,
            "#000000",
            0,
            MAX_MEDIA_OUTPUT_BYTES,
            144.0,
            "poster",
        )
        .unwrap(),
    );
    let rgb = image::load_from_memory(file_bytes(&result))
        .unwrap()
        .to_rgb8();
    assert!(rgb.as_raw().iter().all(|&channel| channel <= 1));
}

#[test]
fn svg_preserves_aspect_ratio_and_translucent_colors() {
    let input = named_file("shape.svg", TRANSLUCENT_SVG.to_vec());
    let result = output(
        image_convert(
            &input,
            "png",
            90,
            "#FFFFFF",
            80,
            MAX_MEDIA_OUTPUT_BYTES,
            144.0,
            "poster",
        )
        .unwrap(),
    );
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
            image_convert(&fixture(), format, 90, "#FFFFFF", 0, 1, 144.0, "poster").is_err(),
            "{format}"
        );
    }
    let input = named_file("shape.svg", TRANSLUCENT_SVG.to_vec());
    assert!(
        image_convert(
            &input,
            "ico",
            90,
            "#FFFFFF",
            300,
            MAX_MEDIA_OUTPUT_BYTES,
            144.0,
            "poster"
        )
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
    let result = output(
        images_convert(
            &batch,
            "png",
            90,
            "#FFFFFF",
            80,
            MAX_MEDIA_OUTPUT_BYTES,
            144.0,
            "poster",
        )
        .unwrap(),
    );
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

#[test]
fn raster_to_svg_produces_renderable_paths_and_respects_output_cap() {
    let converted = output(convert_with_defaults(&fixture(), "svg").unwrap());
    assert_eq!(converted.name, "source.svg");
    assert_eq!(converted.mime.as_deref(), Some("image/svg+xml"));
    let svg = std::str::from_utf8(file_bytes(&converted)).unwrap();
    assert!(svg.contains("<path"));
    assert!(!svg.contains("<image"));
    let png = output(convert_with_defaults(&converted, "png").unwrap());
    assert_eq!(
        image::load_from_memory(file_bytes(&png))
            .unwrap()
            .dimensions(),
        (8, 4)
    );
    assert_eq!(
        file_bytes(&converted),
        file_bytes(&output(convert_with_defaults(&fixture(), "svg").unwrap()))
    );
    assert!(image_convert(&fixture(), "svg", 90, "#FFFFFF", 0, 16, 144.0, "poster").is_err());
}

#[test]
fn svg_to_svg_is_explicitly_rejected() {
    let err = convert_with_defaults(&named_file("shape.svg", TRANSLUCENT_SVG.to_vec()), "svg")
        .unwrap_err();
    assert!(err.contains("SVG"), "{err}");
}

#[test]
fn large_raster_to_svg_preserves_original_display_size() {
    let raster = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2048,
        32,
        image::Rgba([255, 0, 0, 255]),
    ));
    let mut bytes = Cursor::new(Vec::new());
    raster.write_to(&mut bytes, ImageFormat::Png).unwrap();
    let converted =
        output(convert_with_defaults(&named_file("wide.png", bytes.into_inner()), "svg").unwrap());
    let svg = std::str::from_utf8(file_bytes(&converted)).unwrap();
    assert!(svg.contains("width=\"2048\""));
    assert!(svg.contains("height=\"32\""));
    assert!(svg.contains("viewBox=\"0 0 1024 16\""));
}

#[test]
fn batch_svg_preserves_names_and_rejects_svg_input() {
    let batch = FileValue {
        name: "pictures".into(),
        content: FileContent::Directory(vec![fixture()]),
        mime: None,
    };
    let result = output(
        images_convert(
            &batch,
            "svg",
            90,
            "#FFFFFF",
            0,
            MAX_MEDIA_OUTPUT_BYTES,
            144.0,
            "poster",
        )
        .unwrap(),
    );
    let mut zip = zip::ZipArchive::new(Cursor::new(file_bytes(&result))).unwrap();
    let mut svg = String::new();
    use std::io::Read as _;
    zip.by_name("source.svg")
        .unwrap()
        .read_to_string(&mut svg)
        .unwrap();
    assert!(svg.contains("<path"));
    let invalid = FileValue {
        name: "pictures".into(),
        content: FileContent::Directory(vec![named_file("shape.svg", TRANSLUCENT_SVG.to_vec())]),
        mime: None,
    };
    assert!(
        images_convert(
            &invalid,
            "svg",
            90,
            "#FFFFFF",
            0,
            MAX_MEDIA_OUTPUT_BYTES,
            144.0,
            "poster"
        )
        .is_err()
    );
}

#[test]
fn vectorization_composites_partial_alpha_on_selected_background() {
    let svg = output(
        image_convert(
            &fixture(),
            "svg",
            90,
            "#000000",
            0,
            MAX_MEDIA_OUTPUT_BYTES,
            144.0,
            "poster",
        )
        .unwrap(),
    );
    let png = output(convert_with_defaults(&svg, "png").unwrap());
    let raster = image::load_from_memory(file_bytes(&png)).unwrap().to_rgb8();
    let pixel = raster.get_pixel(4, 2).0;
    assert!((58..=63).contains(&pixel[0]), "{pixel:?}");
    assert!((33..=38).contains(&pixel[1]), "{pixel:?}");
    assert!((13..=18).contains(&pixel[2]), "{pixel:?}");
}

#[test]
fn pdf_compatible_ai_routes_through_single_and_batch_conversion() {
    let mut ai = output(super::super::image_to_pdf(&fixture(), MAX_MEDIA_OUTPUT_BYTES).unwrap());
    ai.name = "art.ai".into();
    let converted = output(
        image_convert(
            &ai,
            "png",
            90,
            "#FFFFFF",
            0,
            MAX_MEDIA_OUTPUT_BYTES,
            72.0,
            "poster",
        )
        .unwrap(),
    );
    assert_eq!(converted.name, "art.png");
    assert_eq!(
        image::load_from_memory(file_bytes(&converted))
            .unwrap()
            .dimensions(),
        (8, 4)
    );
    assert!(
        image_convert(
            &ai,
            "svg",
            90,
            "#FFFFFF",
            0,
            MAX_MEDIA_OUTPUT_BYTES,
            72.0,
            "poster"
        )
        .unwrap_err()
        .contains("AI")
    );
    let batch = FileValue {
        name: "art".into(),
        content: FileContent::Directory(vec![ai]),
        mime: None,
    };
    let result = output(
        images_convert(
            &batch,
            "png",
            90,
            "#FFFFFF",
            0,
            MAX_MEDIA_OUTPUT_BYTES,
            72.0,
            "poster",
        )
        .unwrap(),
    );
    let mut zip = zip::ZipArchive::new(Cursor::new(file_bytes(&result))).unwrap();
    use std::io::Read as _;
    let mut png = Vec::new();
    zip.by_name("art.png")
        .unwrap()
        .read_to_end(&mut png)
        .unwrap();
    assert_eq!(image::load_from_memory(&png).unwrap().dimensions(), (8, 4));
}

#[test]
fn extended_options_validate_trace_preset_and_ai_dpi() {
    for preset in ["poster", "photo", "bw"] {
        assert!(
            ImageConversionOptions::new_extended("svg", 90, "#FFFFFF", 0, 144.0, preset).is_ok()
        );
    }
    assert!(
        ImageConversionOptions::new_extended("svg", 90, "#FFFFFF", 0, 144.0, "unknown").is_err()
    );
    for dpi in [0.0, 35.0, 601.0, f64::NAN] {
        assert!(
            ImageConversionOptions::new_extended("png", 90, "#FFFFFF", 0, dpi, "poster").is_err()
        );
    }
}

#[test]
fn svg_downscaling_composites_transparency_before_averaging() {
    let image = image::RgbaImage::from_fn(2048, 16, |x, _| {
        if x % 2 == 0 {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        }
    });
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    let svg = output(
        convert_with_defaults(&named_file("stripes.png", bytes.into_inner()), "svg").unwrap(),
    );
    let png = output(convert_with_defaults(&svg, "png").unwrap());
    let raster = image::load_from_memory(file_bytes(&png))
        .unwrap()
        .to_rgba8();
    let p = raster.get_pixel(1024, 8).0;
    assert!(
        p[0] >= 248 && (123..=132).contains(&p[1]) && (123..=132).contains(&p[2]),
        "{p:?}"
    );
}

#[test]
fn svg_bw_keeps_the_selected_opaque_background() {
    let transparent = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        16,
        16,
        image::Rgba([0, 0, 0, 0]),
    ));
    let mut bytes = Cursor::new(Vec::new());
    transparent.write_to(&mut bytes, ImageFormat::Png).unwrap();
    let input = named_file("clear.png", bytes.into_inner());
    for (color, expected) in [
        ("#FFFFFF", [255, 255, 255, 255]),
        ("#000000", [0, 0, 0, 255]),
    ] {
        let svg = output(
            image_convert(
                &input,
                "svg",
                90,
                color,
                0,
                MAX_MEDIA_OUTPUT_BYTES,
                144.0,
                "bw",
            )
            .unwrap(),
        );
        let png = output(convert_with_defaults(&svg, "png").unwrap());
        let raster = image::load_from_memory(file_bytes(&png))
            .unwrap()
            .to_rgba8();
        assert_eq!(raster.get_pixel(8, 8).0, expected);
    }
}

#[test]
fn vectorization_rejects_high_frequency_one_megapixel_input_before_tracing() {
    let image = image::RgbaImage::from_fn(1024, 1024, |x, y| {
        let v = if (x + y) % 2 == 0 { 0 } else { 255 };
        image::Rgba([v, v, v, 255])
    });
    let raster = DynamicImage::ImageRgba8(image);
    let options = ImageConversionOptions::new("svg", 90, "#FFFFFF", 0).unwrap();
    let error = encode(&raster, options, MAX_MEDIA_OUTPUT_BYTES).unwrap_err();
    assert!(error.contains("edge tracing limit"), "{error}");
}

#[test]
fn binary_trace_rejects_high_frequency_pixels_across_threshold() {
    let image = image::RgbaImage::from_fn(1024, 1024, |x, y| {
        let v = if (x + y) % 2 == 0 { 127 } else { 128 };
        image::Rgba([v, v, v, 255])
    });
    let raster = DynamicImage::ImageRgba8(image);
    let options =
        ImageConversionOptions::new_extended("svg", 90, "#FFFFFF", 0, 144.0, "bw").unwrap();
    let error = encode(&raster, options, MAX_MEDIA_OUTPUT_BYTES).unwrap_err();
    assert!(error.contains("edge tracing limit"), "{error}");
}

#[test]
fn tracer_stops_when_path_or_point_budget_is_exhausted() {
    let pixels = vec![255; 32 * 32 * 4];
    let image = vtracer::ColorImage {
        pixels,
        width: 32,
        height: 32,
    };
    let limits = vtracer::TraceLimits {
        max_edges: usize::MAX,
        max_clusters: 1000,
        max_paths: 0,
        max_points: 1000,
    };
    let error = vtracer::convert_bounded(
        image.clone(),
        vtracer::Config::from_preset(vtracer::Preset::Poster),
        limits,
    )
    .unwrap_err();
    assert!(error.contains("path tracing limit"), "{error}");
    let error = vtracer::convert_bounded(
        image,
        vtracer::Config::from_preset(vtracer::Preset::Poster),
        vtracer::TraceLimits {
            max_paths: 10,
            max_points: 0,
            ..limits
        },
    )
    .unwrap_err();
    assert!(error.contains("point tracing limit"), "{error}");
}
