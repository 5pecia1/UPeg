//! Shared still-image conversion engine. SVG is rasterized; animation is not preserved.
use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageEncoder, Pixel};

use super::image_codec::{self, RasterFormat};
use super::limits::{CappedBuffer, checked_pixel_area, image_decode_limits};

mod svg;

pub const IMAGE_CONVERT_DEFAULT_QUALITY: usize = 90;
pub const IMAGE_CONVERT_DEFAULT_BACKGROUND: &str = "#FFFFFF";
pub const IMAGE_CONVERT_DEFAULT_SVG_WIDTH: usize = 0;
pub const IMAGE_CONVERT_MAX_SVG_WIDTH: usize = 8192;
pub const IMAGE_CONVERT_MAX_INPUT_BYTES: usize = 50 << 20;
pub(super) const INPUT_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "ico", "qoi", "svg",
];
const ENCODED_IMAGE_LABEL: &str = "converted image";
const ICO_MAX_DIMENSION: u32 = 256;
const HEX_COLOR_DIGITS: usize = 6;
const HEX_RADIX: u32 = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConversionFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
    Bmp,
    Tiff,
    Ico,
    Qoi,
}

impl ConversionFormat {
    pub(super) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "png" => Ok(Self::Png),
            "jpeg" => Ok(Self::Jpeg),
            "webp" => Ok(Self::Webp),
            "gif" => Ok(Self::Gif),
            "bmp" => Ok(Self::Bmp),
            "tiff" => Ok(Self::Tiff),
            "ico" => Ok(Self::Ico),
            "qoi" => Ok(Self::Qoi),
            _ => Err(format!("unsupported output format `{value}`")),
        }
    }
    pub(super) const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Ico => "ico",
            Self::Qoi => "qoi",
        }
    }
    pub(super) const fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
            Self::Bmp => "image/bmp",
            Self::Tiff => "image/tiff",
            Self::Ico => "image/x-icon",
            Self::Qoi => "image/qoi",
        }
    }
    const fn image_format(self) -> image::ImageFormat {
        match self {
            Self::Png => image::ImageFormat::Png,
            Self::Jpeg => image::ImageFormat::Jpeg,
            Self::Webp => image::ImageFormat::WebP,
            Self::Gif => image::ImageFormat::Gif,
            Self::Bmp => image::ImageFormat::Bmp,
            Self::Tiff => image::ImageFormat::Tiff,
            Self::Ico => image::ImageFormat::Ico,
            Self::Qoi => image::ImageFormat::Qoi,
        }
    }
}

/// Validated options shared by single and batch conversion.
#[derive(Clone, Copy, Debug)]
pub struct ImageConversionOptions {
    pub(super) format: ConversionFormat,
    jpeg_quality: u8,
    background: image::Rgba<u8>,
    svg_width: u32,
}

impl ImageConversionOptions {
    /// Validate the wire values before opening any input image.
    pub fn new(
        format: &str,
        jpeg_quality: usize,
        background: &str,
        svg_width: usize,
    ) -> Result<Self, String> {
        if !(1..=100).contains(&jpeg_quality) {
            return Err("JPEG quality must be between 1 and 100".into());
        }
        if svg_width > IMAGE_CONVERT_MAX_SVG_WIDTH {
            return Err(format!(
                "SVG width must be between 0 and {IMAGE_CONVERT_MAX_SVG_WIDTH}"
            ));
        }
        let hex = background
            .strip_prefix('#')
            .filter(|value| value.len() == HEX_COLOR_DIGITS && value.is_ascii())
            .ok_or("background must be a color such as #FFFFFF")?;
        let rgb = u32::from_str_radix(hex, HEX_RADIX)
            .map_err(|_| "background must be a color such as #FFFFFF")?
            .to_be_bytes();
        Ok(Self {
            format: ConversionFormat::parse(format)?,
            jpeg_quality: u8::try_from(jpeg_quality).map_err(|_| "invalid JPEG quality")?,
            background: image::Rgba([rgb[1], rgb[2], rgb[3], u8::MAX]),
            svg_width: u32::try_from(svg_width).map_err(|_| "invalid SVG width")?,
        })
    }
    pub(super) fn defaults(format: &str) -> Result<Self, String> {
        Self::new(
            format,
            IMAGE_CONVERT_DEFAULT_QUALITY,
            IMAGE_CONVERT_DEFAULT_BACKGROUND,
            IMAGE_CONVERT_DEFAULT_SVG_WIDTH,
        )
    }
}

fn raster_format(name: &str, bytes: &[u8]) -> Result<image::ImageFormat, String> {
    match image::guess_format(bytes) {
        Ok(
            format @ (image::ImageFormat::Png
            | image::ImageFormat::Jpeg
            | image::ImageFormat::WebP
            | image::ImageFormat::Gif
            | image::ImageFormat::Bmp
            | image::ImageFormat::Tiff
            | image::ImageFormat::Ico
            | image::ImageFormat::Qoi),
        ) => Ok(format),
        _ => Err(format!(
            "could not decode `{name}`: unsupported image bytes"
        )),
    }
}

fn is_svg(name: &str) -> bool {
    name.rsplit('.')
        .next()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
}

pub(super) fn pixel_count(
    name: &str,
    bytes: &[u8],
    options: ImageConversionOptions,
) -> Result<usize, String> {
    if is_svg(name) {
        return svg::pixel_count(bytes, options.svg_width);
    }
    let mut reader =
        image::ImageReader::with_format(Cursor::new(bytes), raster_format(name, bytes)?);
    reader.limits(image_decode_limits());
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| format!("could not inspect `{name}` dimensions: {error}"))?;
    checked_pixel_area(name, width, height)
}

pub(super) fn decode(
    name: &str,
    bytes: &[u8],
    options: ImageConversionOptions,
) -> Result<DynamicImage, String> {
    if is_svg(name) {
        return svg::decode(bytes, options.svg_width);
    }
    // Check pixel count before decoding; some codecs only enforce axis/allocation limits.
    pixel_count(name, bytes, options)?;
    let mut reader =
        image::ImageReader::with_format(Cursor::new(bytes), raster_format(name, bytes)?);
    reader.limits(image_decode_limits());
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| format!("could not open `{name}`: {error}"))?;
    use image::ImageDecoder as _;
    let orientation = decoder
        .orientation()
        .map_err(|error| format!("could not read `{name}` orientation: {error}"))?;
    let mut raster = DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("could not decode `{name}`: {error}"))?;
    raster.apply_orientation(orientation);
    Ok(raster)
}

pub(super) fn encode(
    raster: &DynamicImage,
    options: ImageConversionOptions,
    limit: usize,
) -> Result<Vec<u8>, String> {
    if options.format == ConversionFormat::Png {
        return image_codec::encode_with_limit(raster, RasterFormat::Png, limit);
    }
    if options.format == ConversionFormat::Ico
        && (raster.width() > ICO_MAX_DIMENSION || raster.height() > ICO_MAX_DIMENSION)
    {
        return Err(format!(
            "ICO output must be at most {ICO_MAX_DIMENSION} × {ICO_MAX_DIMENSION} pixels; use a smaller image or SVG width"
        ));
    }
    let mut writer = CappedBuffer::new(limit, ENCODED_IMAGE_LABEL);
    if options.format == ConversionFormat::Jpeg {
        let rgb = image::RgbImage::from_fn(raster.width(), raster.height(), |x, y| {
            let mut background = options.background;
            background.blend(&raster.get_pixel(x, y));
            image::Rgb([background[0], background[1], background[2]])
        });
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, options.jpeg_quality)
            .write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|error| format!("could not encode JPEG: {error}"))?;
    } else {
        // Normalize to RGBA8 so every encoder has a supported color type,
        // including high-bit-depth TIFF/PNG input.
        DynamicImage::ImageRgba8(raster.to_rgba8())
            .write_to(&mut writer, options.format.image_format())
            .map_err(|error| format!("could not encode {}: {error}", options.format.extension()))?;
    }
    Ok(writer.into_bytes())
}

#[cfg(test)]
mod tests;
