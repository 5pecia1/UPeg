//! `media.pptx_extract_images` — extract embedded `ppt/media/*` images from an
//! Office Open XML (`.pptx`/`.docx`/`.xlsx`) archive into a single zip.
//!
//! Pure Rust (`zip` only), so it compiles and runs on every surface including
//! the browser (wasm32) build — like every other media tool now that the
//! PDFium dependency is gone.

use std::io::{Cursor, Write};

use upeg_core::{FileContent, FileValue, tool};

use super::limits::{MAX_ARCHIVE_TOTAL_BYTES, MAX_DECODED_BYTES, read_capped};
use super::{
    IMAGES_ZIP_MIME, PPTX_DEFAULT_STEM, dedupe_zip_name, file_input_bytes, images_zip_name,
    is_pptx_image_entry, zip_entry_basename,
};

#[tool(
    id = "media.pptx_extract_images",
    display_label = "PPTX → embedded images",
    toolkit = "media",
    inputs = [
        required input: File = "PPTX/Office file bytes",
    ],
    outputs = [
        result: File = "Zip of extracted images",
    ],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http, Pwa, Ext],
)]
/// Extract every embedded image under `ppt/media/` from an in-memory Office
/// Open XML archive and return them bundled as a single zip [`FileValue`].
///
/// Entry names are reduced to their basename before being re-added, so a Zip
/// Slip payload (`ppt/media/../../evil.png`) can never escape into a nested
/// path.
pub fn pptx_extract_images(input: &FileValue) -> Result<String, String> {
    let file = pptx_extract_images_to_file(input)?;
    serde_json::to_string(&file).map_err(|e| format!("failed to serialize output file: {e}"))
}

/// Core of [`pptx_extract_images`]: unzip the input bytes, collect the
/// `ppt/media/*` images, and repack them into a fresh in-memory zip.
fn pptx_extract_images_to_file(input: &FileValue) -> Result<FileValue, String> {
    let bytes = file_input_bytes(input)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("could not read PPTX archive: {e}"))?;

    let mut buffer = Vec::<u8>::new();
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(&mut buffer));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut used = std::collections::BTreeSet::<String>::new();
        let mut total_bytes = 0_u64;

        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|e| format!("could not read archive entry {index}: {e}"))?;
            if !entry.is_file() {
                continue;
            }
            let name = entry.name().to_string();
            if !is_pptx_image_entry(&name) {
                continue;
            }
            // Zip Slip defense: flatten to the basename so a crafted entry name
            // cannot influence the output archive's directory layout.
            let Some(basename) = zip_entry_basename(&name) else {
                continue;
            };
            // Read through a cap rather than `io::copy`, which streams whatever
            // the entry decompresses to: an entry's declared size is just as
            // attacker-controlled as its bytes, so the input archive alone can
            // hold a zip bomb.
            let bytes = read_capped(entry, MAX_DECODED_BYTES, &format!("`{name}`"))?;
            total_bytes += bytes.len() as u64;
            if total_bytes > MAX_ARCHIVE_TOTAL_BYTES {
                return Err(format!(
                    "archive's images exceed the {MAX_ARCHIVE_TOTAL_BYTES}-byte total limit"
                ));
            }
            let unique = dedupe_zip_name(basename, &mut used);
            writer
                .start_file(&unique, options)
                .map_err(|e| format!("could not add `{unique}` to output zip: {e}"))?;
            writer
                .write_all(&bytes)
                .map_err(|e| format!("could not copy `{name}`: {e}"))?;
        }

        writer
            .finish()
            .map_err(|e| format!("could not finalize output zip: {e}"))?;
    }

    Ok(FileValue {
        name: images_zip_name(&input.name, PPTX_DEFAULT_STEM),
        content: FileContent::Bytes(buffer),
        mime: Some(IMAGES_ZIP_MIME.to_string()),
    })
}
