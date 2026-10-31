//! HEVC-coded HEIC/HEIF input for the shared still-image converter.

use heic_rs::{DecodeOptions, PixelLayout};
use image::{DynamicImage, RgbaImage};

use super::super::limits::{MAX_IMAGE_PIXELS, checked_pixel_area};

pub(super) fn is_heif(bytes: &[u8]) -> bool {
    heic_rs::ftyp::parse(bytes).is_ok()
}

pub(super) fn pixel_count(name: &str, bytes: &[u8]) -> Result<usize, String> {
    let info = heic_rs::probe(bytes)
        .map_err(|error| format!("could not inspect `{name}` dimensions: {error}"))?;
    let visible = checked_pixel_area(name, info.width, info.height)?;
    let coded = checked_pixel_area(name, info.coded_width, info.coded_height)?;
    let mut largest = visible.max(coded);
    if let Some(grid) = info.grid {
        let width = grid
            .columns
            .checked_mul(grid.tile_width)
            .ok_or_else(|| format!("`{name}` grid width exceeds the supported range"))?;
        let height = grid
            .rows
            .checked_mul(grid.tile_height)
            .ok_or_else(|| format!("`{name}` grid height exceeds the supported range"))?;
        largest = largest.max(checked_pixel_area(name, width, height)?);
    }
    Ok(largest)
}

pub(super) fn decode(name: &str, bytes: &[u8]) -> Result<DynamicImage, String> {
    pixel_count(name, bytes)?;
    let options = DecodeOptions::default()
        .with_layout(PixelLayout::Rgba8)
        .with_max_pixels(Some(MAX_IMAGE_PIXELS as u64))
        .with_strict(true);
    let decoded = heic_rs::decode(bytes, &options)
        .map_err(|error| format!("could not decode `{name}`: {error}"))?;
    checked_pixel_area(name, decoded.width, decoded.height)?;
    RgbaImage::from_raw(decoded.width, decoded.height, decoded.data)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| format!("could not decode `{name}`: invalid HEIF raster size"))
}
