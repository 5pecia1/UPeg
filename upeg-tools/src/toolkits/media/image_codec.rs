use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageEncoder, Pixel};

use super::limits::{CappedBuffer, MAX_DECODED_BYTES, checked_pixel_area, image_decode_limits};

const JPEG_QUALITY: u8 = 90;
const JPEG_BACKGROUND: image::Rgba<u8> = image::Rgba([u8::MAX, u8::MAX, u8::MAX, u8::MAX]);
const ENCODED_RASTER_LABEL: &str = "encoded raster";
const RGBA8_BYTES_PER_PIXEL: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RasterFormat {
    Png,
    Jpeg,
}

impl RasterFormat {
    pub(super) const fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }

    const fn image_format(self) -> image::ImageFormat {
        match self {
            Self::Png => image::ImageFormat::Png,
            Self::Jpeg => image::ImageFormat::Jpeg,
        }
    }
}

/// Decode one PNG/JPEG raster after sniffing its bytes and applying allocation
/// and dimension limits before materializing a normalized pixel buffer.
pub(super) fn decode(name: &str, bytes: &[u8]) -> Result<DynamicImage, String> {
    let actual = sniff(name, bytes)?;
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), actual.image_format());
    reader.limits(image_decode_limits());
    let decoded = reader
        .decode()
        .map_err(|error| format!("could not decode `{name}`: {error}"))?;
    checked_pixel_area(name, decoded.width(), decoded.height())?;
    Ok(decoded)
}

pub(super) fn pixel_count(name: &str, bytes: &[u8]) -> Result<usize, String> {
    let actual = sniff(name, bytes)?;
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), actual.image_format());
    reader.limits(image_decode_limits());
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| format!("could not inspect `{name}` dimensions: {error}"))?;
    checked_pixel_area(name, width, height)
}

pub(super) fn encode(image: &DynamicImage, format: RasterFormat) -> Result<Vec<u8>, String> {
    encode_with_limit(image, format, MAX_DECODED_BYTES)
}

pub(super) fn encode_with_limit(
    image: &DynamicImage,
    format: RasterFormat,
    output_limit: usize,
) -> Result<Vec<u8>, String> {
    let (width, height) = image.dimensions();
    match format {
        RasterFormat::Png => {
            let rgba = image.to_rgba8();
            encode_png_rgba((width, height), rgba.as_raw(), output_limit)
        }
        RasterFormat::Jpeg => {
            let mut writer = CappedBuffer::new(output_limit, ENCODED_RASTER_LABEL);
            let rgb = image::RgbImage::from_fn(width, height, |x, y| {
                let mut background = JPEG_BACKGROUND;
                background.blend(&image.get_pixel(x, y));
                image::Rgb([background[0], background[1], background[2]])
            });
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, JPEG_QUALITY)
                .write_image(rgb.as_raw(), width, height, image::ExtendedColorType::Rgb8)
                .map_err(|error| {
                    format!("failed to encode {} output: {error}", format.mime_type())
                })?;
            Ok(writer.into_bytes())
        }
    }
}

pub(super) fn encode_rgba(
    dimensions: (u32, u32),
    rgba: &[u8],
    format: RasterFormat,
) -> Result<Vec<u8>, String> {
    let (width, height) = dimensions;
    let expected_bytes = checked_pixel_area("rendered raster", width, height)?
        .checked_mul(RGBA8_BYTES_PER_PIXEL)
        .ok_or_else(|| format!("invalid rendered page dimensions {width}x{height}"))?;
    if rgba.len() != expected_bytes {
        return Err(format!("invalid rendered page dimensions {width}x{height}"));
    }
    match format {
        RasterFormat::Png => encode_png_rgba(dimensions, rgba, MAX_DECODED_BYTES),
        RasterFormat::Jpeg => {
            let image = image::RgbaImage::from_raw(width, height, rgba.to_vec())
                .map(DynamicImage::ImageRgba8)
                .ok_or_else(|| format!("invalid rendered page dimensions {width}x{height}"))?;
            encode(&image, RasterFormat::Jpeg)
        }
    }
}

fn encode_png_rgba(
    dimensions: (u32, u32),
    rgba: &[u8],
    output_limit: usize,
) -> Result<Vec<u8>, String> {
    let (width, height) = dimensions;
    let mut writer = CappedBuffer::new(output_limit, ENCODED_RASTER_LABEL);
    image::codecs::png::PngEncoder::new(&mut writer)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| {
            format!(
                "failed to encode {} output: {error}",
                RasterFormat::Png.mime_type()
            )
        })?;
    Ok(writer.into_bytes())
}

fn sniff(name: &str, bytes: &[u8]) -> Result<RasterFormat, String> {
    match image::guess_format(bytes) {
        Ok(image::ImageFormat::Png) => Ok(RasterFormat::Png),
        Ok(image::ImageFormat::Jpeg) => Ok(RasterFormat::Jpeg),
        Ok(_) | Err(_) => Err(format!(
            "could not decode `{name}`: only PNG and JPEG image bytes are supported"
        )),
    }
}
