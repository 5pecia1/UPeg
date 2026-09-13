#[test]
fn pdf_render의_rgba_png_인코딩은_픽셀_buffer를_복제하지_않는다() {
    let source = include_str!("../image_codec.rs");
    let pdf_render_source = include_str!("../pdf_render.rs");
    let encode_rgba = source
        .split_once("pub(super) fn encode_rgba")
        .and_then(|(_, tail)| tail.split_once("fn sniff"))
        .map(|(body, _)| body)
        .expect("encode_rgba must remain before sniff");
    let png_arm = encode_rgba
        .split_once("RasterFormat::Png =>")
        .and_then(|(_, tail)| tail.split_once("RasterFormat::Jpeg =>"))
        .map(|(body, _)| body)
        .expect("encode_rgba must keep explicit PNG and JPEG branches");
    let png_encoder = source
        .split_once("fn encode_png_rgba")
        .and_then(|(_, tail)| tail.split_once("fn sniff"))
        .map(|(body, _)| body)
        .expect("encode_png_rgba must remain before sniff");
    let pixmap_encoder = pdf_render_source
        .split_once("fn encode_pixmap_png")
        .map(|(_, body)| body)
        .expect("PDF render must keep a dedicated pixmap encoder");

    assert!(
        png_arm.contains("encode_png_rgba"),
        "the supplied RGBA buffer must enter the bounded PNG encoder directly"
    );
    assert!(
        !png_arm.contains("DynamicImage") && !png_arm.contains("encode(&image"),
        "encode_rgba must not send PNG through DynamicImage::to_rgba8"
    );
    assert!(
        png_encoder.contains(".write_image(rgba,")
            && !png_encoder.contains("to_rgba8")
            && !png_encoder.contains("to_vec"),
        "the bounded PNG encoder must borrow the supplied RGBA slice without cloning the raster"
    );
    assert!(
        pixmap_encoder.contains("pixmap.data_as_u8_slice()")
            && !pixmap_encoder.contains("pixmap.data_as_u8_slice().to_vec()"),
        "PDF rendering must pass the pixmap's RGBA bytes to the codec by reference"
    );
}

#[test]
fn 제공된_rgba_buffer를_png로_인코딩하면_pixel이_보존된다() {
    use super::super::image_codec::{RasterFormat, encode_rgba};

    let rgba = vec![255, 0, 0, 255, 0, 0, 255, 128];

    let encoded =
        encode_rgba((2, 1), &rgba, RasterFormat::Png).expect("RGBA buffer should encode as PNG");
    let decoded = image::load_from_memory(&encoded)
        .expect("encoded PNG should decode")
        .to_rgba8();

    assert_eq!(decoded.dimensions(), (2, 1));
    assert_eq!(decoded.as_raw(), &[255, 0, 0, 255, 0, 0, 255, 128]);
}

#[test]
fn 제공된_rgba_buffer를_jpeg로_인코딩하면_이미지_변환_경로가_유지된다() {
    use super::super::image_codec::{RasterFormat, encode_rgba};

    let rgba = vec![255; 4 * 8 * 8];

    let encoded =
        encode_rgba((8, 8), &rgba, RasterFormat::Jpeg).expect("RGBA buffer should encode as JPEG");
    let decoded = image::load_from_memory(&encoded).expect("encoded JPEG should decode");

    assert_eq!(
        image::guess_format(&encoded).expect("encoded bytes should have a known image format"),
        image::ImageFormat::Jpeg
    );
    assert_eq!((decoded.width(), decoded.height()), (8, 8));
}
