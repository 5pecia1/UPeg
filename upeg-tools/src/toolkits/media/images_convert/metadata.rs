use upeg_core::FileValue;

use super::super::image_conversion::ConversionFormat;
use super::super::{IMAGES_DEFAULT_STEM, IMAGES_ZIP_SUFFIX, file_stem};

const MAX_BATCH_NAME_BYTES: usize = 4 << 10;
const MAX_BATCH_MIME_BYTES: usize = 1 << 10;
const MAX_BATCH_METADATA_BYTES: usize = 3 << 20;
const ZIP_ENTRY_NAME_ALLOCATION_COPIES: usize = 6;
const OUTPUT_NAME_ALLOCATION_COPIES: usize = 2;
const MAX_DEDUP_SUFFIX_BYTES: usize = 4;

pub(super) fn validate_and_measure(
    images: &FileValue,
    entries: &[FileValue],
    output_format: ConversionFormat,
) -> Result<usize, String> {
    validate_length("root name", images.name.len(), MAX_BATCH_NAME_BYTES)?;
    validate_optional_mime("root MIME", images.mime.as_deref())?;

    let mut total = images.name.len();
    add_bytes(&mut total, images.mime.as_deref().map_or(0, str::len))?;
    let output_name_bytes = file_stem(&images.name, IMAGES_DEFAULT_STEM)
        .len()
        .checked_add(IMAGES_ZIP_SUFFIX.len())
        .ok_or_else(metadata_overflow)?;
    add_copies(
        &mut total,
        output_name_bytes.max(images.name.len()),
        OUTPUT_NAME_ALLOCATION_COPIES,
    )?;

    for (index, entry) in entries.iter().enumerate() {
        validate_length("entry name", entry.name.len(), MAX_BATCH_NAME_BYTES)
            .map_err(|error| format!("{error} at index {index}"))?;
        validate_optional_mime("entry MIME", entry.mime.as_deref())
            .map_err(|error| format!("{error} at index {index}"))?;

        add_bytes(&mut total, entry.name.len())?;
        add_bytes(&mut total, entry.mime.as_deref().map_or(0, str::len))?;
        let zip_name_bytes = file_stem(&entry.name, super::BATCH_IMAGE_DEFAULT_STEM)
            .len()
            .checked_add(1)
            .and_then(|bytes| bytes.checked_add(output_format.extension().len()))
            .and_then(|bytes| bytes.checked_add(MAX_DEDUP_SUFFIX_BYTES))
            .ok_or_else(metadata_overflow)?;
        add_copies(
            &mut total,
            zip_name_bytes.max(entry.name.len()),
            ZIP_ENTRY_NAME_ALLOCATION_COPIES,
        )?;
    }
    Ok(total)
}

fn validate_optional_mime(label: &str, mime: Option<&str>) -> Result<(), String> {
    validate_length(label, mime.map_or(0, str::len), MAX_BATCH_MIME_BYTES)
}

fn validate_length(label: &str, bytes: usize, limit: usize) -> Result<(), String> {
    if bytes > limit {
        return Err(format!(
            "images {label} exceeds the {limit}-byte metadata limit"
        ));
    }
    Ok(())
}

fn add_copies(total: &mut usize, bytes: usize, copies: usize) -> Result<(), String> {
    let allocation = bytes.checked_mul(copies).ok_or_else(metadata_overflow)?;
    add_bytes(total, allocation)
}

fn add_bytes(total: &mut usize, bytes: usize) -> Result<(), String> {
    *total = total.checked_add(bytes).ok_or_else(metadata_overflow)?;
    if *total > MAX_BATCH_METADATA_BYTES {
        return Err(format!(
            "images metadata exceeds the {MAX_BATCH_METADATA_BYTES}-byte limit"
        ));
    }
    Ok(())
}

fn metadata_overflow() -> String {
    "images metadata byte count overflowed".to_string()
}
