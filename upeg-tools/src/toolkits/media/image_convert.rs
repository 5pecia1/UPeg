//! Single-image entry point; batch conversion shares the same validated engine.
use upeg_core::{FileContent, FileValue, tool};

use super::image_conversion::{self, IMAGE_CONVERT_MAX_INPUT_BYTES, ImageConversionOptions};
use super::limits::{over_cap_error, validate_max_output_bytes};
use super::{file_input_bytes, file_stem};

#[tool(
    id = "media.image_convert",
    display_label = "Convert image",
    description = "Convert one image and save it directly. SVG is rasterized; animated images and multi-page TIFF use the first frame/page. WebP output is lossless.",
    toolkit = "media",
    inputs = [
        required input: File(extensions=["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "ico", "qoi", "svg"], max_count=1, max_file_bytes=52428800, max_total_bytes=52428800) = "Image file",
        required output_format: Options(["png", "jpeg", "webp", "gif", "bmp", "tiff", "ico", "qoi"]) = "Output format",
        optional jpeg_quality: Integer(min=1, max=100, default=90) = "JPEG quality (1–100)",
        optional background: String(regex="^#[0-9a-fA-F]{6}$", default="#FFFFFF", placeholder="#FFFFFF") = "JPEG background color",
        optional svg_width: Integer(min=0, max=8192, default=0) = "SVG output width in pixels (0 = original size)",
        optional max_output_bytes: Integer(min=1, max=67108864, default=67108864) = "Maximum output bytes",
    ],
    outputs = [result: File = "Converted image"],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http, Pwa, Ext],
)]
/// Convert a single image to a correctly named and typed file output.
pub fn image_convert(
    input: &FileValue,
    output_format: &str,
    jpeg_quality: usize,
    background: &str,
    svg_width: usize,
    max_output_bytes: usize,
) -> Result<String, String> {
    let options = ImageConversionOptions::new(output_format, jpeg_quality, background, svg_width)?;
    let limit = validate_max_output_bytes(max_output_bytes)?;
    let bytes = file_input_bytes(input)?;
    if bytes.len() > IMAGE_CONVERT_MAX_INPUT_BYTES {
        return Err(over_cap_error("image input", IMAGE_CONVERT_MAX_INPUT_BYTES));
    }
    let raster = image_conversion::decode(&input.name, bytes, options)?;
    let encoded = image_conversion::encode(&raster, options, limit)?;
    let output = FileValue {
        name: format!(
            "{}.{}",
            file_stem(&input.name, "image"),
            options.format.extension()
        ),
        mime: Some(options.format.mime_type().into()),
        content: FileContent::Bytes(encoded),
    };
    serde_json::to_string(&output).map_err(|error| format!("failed to serialize image: {error}"))
}
