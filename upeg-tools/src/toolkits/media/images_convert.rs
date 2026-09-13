use std::collections::BTreeSet;
use std::io::Write;

use upeg_core::{FileContent, FileValue, tool};

use super::image_conversion::{
    INPUT_EXTENSIONS, ImageConversionOptions, decode, encode, pixel_count,
};
use super::limits::{
    LatchedCappedBuffer, MAX_ENCODED_OUTPUT_BYTES, over_cap_error, validate_max_output_bytes,
};
use super::{
    IMAGES_DEFAULT_STEM, IMAGES_ZIP_MIME, dedupe_zip_name, entry_has_extension, file_stem,
    images_zip_name,
};

mod metadata;

const MAX_BATCH_IMAGES: usize = 100;
const MAX_BATCH_FILE_BYTES: usize = 50 << 20;
const MAX_BATCH_INPUT_BYTES: usize = 50 << 20;
const MAX_BATCH_WORKING_BYTES: usize = 512 << 20;
const MAX_RASTER_WORKING_BYTES_PER_PIXEL: usize = 12;
const MAX_JSON_BYTES_PER_OUTPUT_BYTE: usize = 4;
const BATCH_FIXED_OVERHEAD_BYTES: usize = 8 << 20;
const EMPTY_ZIP_END_RECORD_BYTES: usize = 22;
const BATCH_IMAGE_DEFAULT_STEM: &str = "image";
const OUTPUT_ZIP_LABEL: &str = "output zip";

pub const IMAGES_CONVERT_MIN_OUTPUT_BYTES: usize = 1;
pub const IMAGES_CONVERT_MAX_OUTPUT_BYTES: usize = MAX_ENCODED_OUTPUT_BYTES;
pub const IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES: usize = IMAGES_CONVERT_MAX_OUTPUT_BYTES;

#[derive(Clone, Copy)]
pub(super) struct BatchMemoryLimits {
    pub(super) working_bytes: usize,
    pub(super) encoded_image_bytes: usize,
    pub(super) zip_bytes: usize,
}

const DEFAULT_MEMORY_LIMITS: BatchMemoryLimits = BatchMemoryLimits {
    working_bytes: MAX_BATCH_WORKING_BYTES,
    encoded_image_bytes: IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES,
    zip_bytes: IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES,
};

#[tool(
    id = "media.images_convert",
    display_label = "Batch convert images",
    description = "Convert selected images to a ZIP. SVG is rasterized; animation and multi-page TIFF use the first frame/page. WebP output is lossless.",
    toolkit = "media",
    inputs = [
        required images: File(
            extensions = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "ico", "qoi", "svg"],
            max_count = 100,
            max_file_bytes = 52428800,
            max_total_bytes = 52428800
        ) = "Images to convert (up to 100 files)",
        required output_format: Options(["png", "jpeg", "webp", "gif", "bmp", "tiff", "ico", "qoi"]) = "Output image format",
        optional jpeg_quality: Integer(min=1, max=100, default=90) = "JPEG quality (1–100)",
        optional background: String(regex="^#[0-9a-fA-F]{6}$", default="#FFFFFF", placeholder="#FFFFFF") = "JPEG background color",
        optional svg_width: Integer(min=0, max=8192, default=0) = "SVG output width in pixels (0 = original size)",
        optional max_output_bytes: Integer(min=1, max=67108864, default=67108864)
            = "Maximum output zip bytes",
    ],
    outputs = [
        result: File = "Zip of converted images",
    ],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Desktop, Mcp, Http, Pwa, Ext],
)]
/// Convert a flat image directory to a deterministic zip, preserving order.
pub fn images_convert(
    images: &FileValue,
    output_format: &str,
    jpeg_quality: usize,
    background: &str,
    svg_width: usize,
    max_output_bytes: usize,
) -> Result<String, String> {
    images_convert_configured(
        images,
        ImageConversionOptions::new(output_format, jpeg_quality, background, svg_width)?,
        max_output_bytes,
    )
}

/// Batch entry point with the same settings as single-image conversion.
pub fn images_convert_configured(
    images: &FileValue,
    options: ImageConversionOptions,
    max_output_bytes: usize,
) -> Result<String, String> {
    let max_output_bytes = validate_max_output_bytes(max_output_bytes)?;
    convert_with_options_and_limits(
        images,
        options,
        BatchMemoryLimits {
            encoded_image_bytes: max_output_bytes,
            zip_bytes: max_output_bytes,
            ..DEFAULT_MEMORY_LIMITS
        },
    )
}

#[cfg(test)]
pub(super) fn images_convert_with_limits(
    images: &FileValue,
    output_format: &str,
    limits: BatchMemoryLimits,
) -> Result<String, String> {
    convert_with_options_and_limits(
        images,
        ImageConversionOptions::defaults(output_format)?,
        limits,
    )
}

fn convert_with_options_and_limits(
    images: &FileValue,
    options: ImageConversionOptions,
    limits: BatchMemoryLimits,
) -> Result<String, String> {
    let output_format = options.format;
    let entries = match &images.content {
        FileContent::Bytes(_) => return Err("images input must be a directory".to_string()),
        FileContent::Directory(entries) if entries.is_empty() => {
            return Err("images directory must not be empty".to_string());
        }
        FileContent::Directory(entries) => entries,
    };
    if entries.len() > MAX_BATCH_IMAGES {
        return Err(format!(
            "images directory holds {} files, more than the {MAX_BATCH_IMAGES}-file limit",
            entries.len()
        ));
    }

    let metadata_working_bytes = metadata::validate_and_measure(images, entries, output_format)?;
    let payload_bytes = validate_entries(entries)?;
    let input_working_bytes = payload_bytes
        .checked_add(metadata_working_bytes)
        .ok_or_else(|| "images input working set byte count overflowed".to_string())?;
    ensure_serialization_budget(input_working_bytes, limits)?;
    if limits.zip_bytes < EMPTY_ZIP_END_RECORD_BYTES {
        return Err(format!(
            "output zip requires at least {EMPTY_ZIP_END_RECORD_BYTES} bytes, over the {}-byte limit",
            limits.zip_bytes
        ));
    }

    let zip = stream_images_zip(entries, options, input_working_bytes, limits)?;
    let output = FileValue {
        name: images_zip_name(&images.name, IMAGES_DEFAULT_STEM),
        content: FileContent::Bytes(zip),
        mime: Some(IMAGES_ZIP_MIME.to_string()),
    };
    serde_json::to_string(&output)
        .map_err(|error| format!("failed to serialize output file: {error}"))
}

fn stream_images_zip(
    entries: &[FileValue],
    options: ImageConversionOptions,
    input_working_bytes: usize,
    limits: BatchMemoryLimits,
) -> Result<Vec<u8>, String> {
    let output_format = options.format;
    let buffer = LatchedCappedBuffer::new(limits.zip_bytes, OUTPUT_ZIP_LABEL);
    let mut writer = zip::ZipWriter::new(buffer);
    let zip_options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut used = BTreeSet::<String>::new();
    let mut remaining_encoded_bytes = limits.encoded_image_bytes;
    for entry in entries {
        let bytes = entry_bytes(entry)?;
        let pixels = pixel_count(&entry.name, bytes, options)?;
        ensure_conversion_budget(input_working_bytes, pixels, limits)?;
        let raster = decode(&entry.name, bytes, options)?;
        let encoded = encode(&raster, options, remaining_encoded_bytes)?;
        remaining_encoded_bytes = remaining_encoded_bytes
            .checked_sub(encoded.len())
            .ok_or_else(|| "encoded image byte accounting underflowed".to_string())?;
        let basename = format!(
            "{}.{}",
            file_stem(&entry.name, BATCH_IMAGE_DEFAULT_STEM),
            output_format.extension()
        );
        let unique = dedupe_zip_name(&basename, &mut used);
        writer
            .start_file(&unique, zip_options)
            .map_err(|error| format!("could not add `{unique}` to output zip: {error}"))?;
        writer
            .write_all(&encoded)
            .map_err(|error| format!("could not write `{unique}` to output zip: {error}"))?;
    }

    let buffer = writer
        .finish()
        .map_err(|error| format!("could not finalize output zip: {error}"))?;
    buffer.into_bytes()
}

fn validate_entries(entries: &[FileValue]) -> Result<usize, String> {
    let mut total_input_bytes = 0_usize;
    for entry in entries {
        let bytes = entry_bytes(entry)?;
        if !entry_has_extension(&entry.name, INPUT_EXTENSIONS) {
            return Err(format!(
                "`{}` must have a supported image file extension",
                entry.name
            ));
        }
        if bytes.len() > MAX_BATCH_FILE_BYTES {
            return Err(over_cap_error(
                &format!("`{}`", entry.name),
                MAX_BATCH_FILE_BYTES,
            ));
        }
        total_input_bytes = total_input_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| "images input byte count overflowed".to_string())?;
        if total_input_bytes > MAX_BATCH_INPUT_BYTES {
            return Err(format!(
                "images input exceeds the {MAX_BATCH_INPUT_BYTES}-byte total limit"
            ));
        }
    }
    Ok(total_input_bytes)
}

fn entry_bytes(entry: &FileValue) -> Result<&[u8], String> {
    match &entry.content {
        FileContent::Bytes(bytes) => Ok(bytes),
        FileContent::Directory(_) => Err(format!(
            "nested directory `{}` is not allowed in images input",
            entry.name
        )),
    }
}

fn ensure_conversion_budget(
    input_bytes: usize,
    pixels: usize,
    limits: BatchMemoryLimits,
) -> Result<(), String> {
    let raster_bytes = pixels
        .checked_mul(MAX_RASTER_WORKING_BYTES_PER_PIXEL)
        .ok_or_else(|| "image working set byte count overflowed".to_string())?;
    ensure_working_budget(
        [
            input_bytes,
            limits.zip_bytes,
            limits.encoded_image_bytes,
            raster_bytes,
            BATCH_FIXED_OVERHEAD_BYTES,
        ],
        limits.working_bytes,
    )
}

fn ensure_serialization_budget(
    input_bytes: usize,
    limits: BatchMemoryLimits,
) -> Result<(), String> {
    let json_bytes = limits
        .zip_bytes
        .checked_mul(MAX_JSON_BYTES_PER_OUTPUT_BYTE)
        .ok_or_else(|| "output JSON working set byte count overflowed".to_string())?;
    ensure_working_budget(
        [
            input_bytes,
            limits.zip_bytes,
            json_bytes,
            BATCH_FIXED_OVERHEAD_BYTES,
        ],
        limits.working_bytes,
    )
}

fn ensure_working_budget(
    allocations: impl IntoIterator<Item = usize>,
    working_bytes: usize,
) -> Result<(), String> {
    let required = allocations.into_iter().try_fold(0_usize, |total, bytes| {
        total
            .checked_add(bytes)
            .ok_or_else(|| "images conversion working set byte count overflowed".to_string())
    })?;
    if required > working_bytes {
        return Err(format!(
            "images conversion working set requires {required} bytes, over the {working_bytes}-byte limit"
        ));
    }
    Ok(())
}
