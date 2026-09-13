//! Media tools: `#[tool]` entry points plus shared File→zip helpers.
//!
//! Per-tool implementations live in submodules to stay within the workspace's
//! 1000-line file budget:
//!
//! - [`pptx`] — `media.pptx_extract_images`
//! - [`pdf_extract`] — `media.pdf_extract_images` engine
//! - [`image_pdf`] — `media.image_to_pdf`
//! - [`images_convert`] — `media.images_convert`
//! - [`pdf_render`] — `media.pdf_to_images` (hayro rasterizer)
//! - [`limits`] — the resource caps every tool above enforces on its untrusted
//!   `File` input
//!
//! The shared File→zip helpers stay here. The `#[cfg(test)] mod tests;` block
//! lives in `media/tests.rs`. All media tools are pure Rust and compile to
//! wasm32 — the former PDFium dependency is gone.

use std::io::{Seek, Write};

use upeg_core::{FileContent, FileValue, tool};

/// Default DPI for PDF to image conversion.
pub const PDF_TO_IMAGES_DEFAULT_DPI: f64 = 144.0;

/// Minimum allowed DPI for PDF to image conversion.
pub const PDF_TO_IMAGES_MIN_DPI: f64 = 36.0;

/// Maximum allowed DPI for PDF to image conversion.
pub const PDF_TO_IMAGES_MAX_DPI: f64 = 600.0;

/// Hard safety ceiling and default for encoded media `File` outputs.
pub const MAX_MEDIA_OUTPUT_BYTES: usize = limits::MAX_ENCODED_OUTPUT_BYTES;

const PDF_EXTRACTION_OUTPUT_ZIP_LABEL: &str = "PDF extraction output zip";

/// Normalize and validate a DPI value (36-600), returning it as an `f32` scale
/// input for the rasterizer.
pub fn normalize_dpi(value: f64) -> Result<f32, String> {
    if !(PDF_TO_IMAGES_MIN_DPI..=PDF_TO_IMAGES_MAX_DPI).contains(&value) {
        return Err(format!(
            "DPI must be between {PDF_TO_IMAGES_MIN_DPI} and {PDF_TO_IMAGES_MAX_DPI}, got {value}"
        ));
    }

    Ok(value as f32)
}

/// File extension for embedded images reconstructed from raw samples
/// (FlateDecode / LZWDecode → DeviceRGB/DeviceGray), encoded as PNG.
pub const PDF_EMBEDDED_IMAGE_PNG_EXT: &str = "png";

/// File extension for DCTDecode image streams: the stream bytes are already a
/// JPEG, so they are kept verbatim with no decode/re-encode round trip.
pub const PDF_EMBEDDED_IMAGE_JPEG_EXT: &str = "jpg";

/// Header introducing images that were found but skipped (unsupported filter /
/// colorspace / bit depth). Written into the output zip's notes entry.
pub const PDF_EXTRACTION_SKIPPED_LABEL: &str = "skipped";

/// Name of the plain-text notes entry added to a `pdf_extract_images` zip when
/// some images were skipped, so the reasons travel with the archive.
pub const EXTRACTION_NOTES_FILENAME: &str = "EXTRACTION-NOTES.txt";

/// Directory prefix inside an Office Open XML `.pptx` archive that holds
/// embedded media (images, etc.). Only entries under this prefix are extracted.
pub const PPTX_MEDIA_PREFIX: &str = "ppt/media/";

/// Image file extensions (lowercased, no leading dot) recognized when
/// extracting embedded media from a `.pptx` archive.
pub const PPTX_IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "tiff", "tif", "emf", "wmf", "svg", "webp",
];

/// Raster image extensions accepted inside an `image_to_pdf` input zip. Only
/// formats the `image` crate decodes (PNG/JPEG) are embeddable by krilla.
pub const IMAGE_TO_PDF_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];

/// MIME type reported on a zip archive [`FileValue`] output.
pub const IMAGES_ZIP_MIME: &str = "application/zip";

/// MIME type reported on a PDF [`FileValue`] output.
pub const PDF_MIME: &str = "application/pdf";

/// Suffix appended to the input file's stem to name an images zip, e.g.
/// `deck.pptx` → `deck-images.zip`.
pub const IMAGES_ZIP_SUFFIX: &str = "-images.zip";

/// Suffix appended to the input file's stem to name a rendered-pages zip, e.g.
/// `report.pdf` → `report-pages.zip`.
pub const PAGES_ZIP_SUFFIX: &str = "-pages.zip";

/// Suffix appended to the input file's stem to name a generated PDF.
pub const PDF_SUFFIX: &str = ".pdf";

/// Fallback stem for a `.pptx` input with no usable name stem.
pub const PPTX_DEFAULT_STEM: &str = "pptx";

/// Fallback stem for a `.pdf` input with no usable name stem.
pub const PDF_DEFAULT_STEM: &str = "pdf";

/// Fallback stem for an image-zip input with no usable name stem.
pub const IMAGES_DEFAULT_STEM: &str = "images";

/// Borrow a `File` input's raw bytes, rejecting a directory payload (the
/// media/QR tools all operate on a single archive/image file).
pub(crate) fn file_input_bytes(input: &FileValue) -> Result<&[u8], String> {
    match &input.content {
        FileContent::Bytes(bytes) => Ok(bytes),
        FileContent::Directory(_) => Err("input must be a file, not a directory".to_string()),
    }
}

/// The basename stem of `input_name` (final path component, extension dropped),
/// or `default` when the name has no usable stem.
pub(crate) fn file_stem<'a>(input_name: &'a str, default: &'a str) -> &'a str {
    let base = input_name.rsplit(['/', '\\']).next().unwrap_or(input_name);
    let stem = base.rsplit_once('.').map_or(base, |(stem, _)| stem);
    if stem.is_empty() { default } else { stem }
}

/// Name an images zip after its input: `<stem>-images.zip`.
pub(crate) fn images_zip_name(input_name: &str, default_stem: &str) -> String {
    format!("{}{IMAGES_ZIP_SUFFIX}", file_stem(input_name, default_stem))
}

/// Return `true` when a `.pptx` archive entry name refers to an embedded image:
/// it lives under [`PPTX_MEDIA_PREFIX`] and ends in a known image extension.
pub fn is_pptx_image_entry(entry_name: &str) -> bool {
    if !entry_name.starts_with(PPTX_MEDIA_PREFIX) {
        return false;
    }
    entry_has_extension(entry_name, PPTX_IMAGE_EXTENSIONS)
}

/// Return `true` when `entry_name` ends in one of `extensions` (case-insensitive).
pub(crate) fn entry_has_extension(entry_name: &str, extensions: &[&str]) -> bool {
    entry_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .is_some_and(|ext| extensions.contains(&ext.as_str()))
}

/// Reduce an archive entry name to its final path component, rejecting
/// traversal segments. Guards against Zip Slip: keeping only the basename means
/// a crafted entry name cannot influence the output archive's layout.
pub fn zip_entry_basename(entry_name: &str) -> Option<&str> {
    let base = entry_name.rsplit(['/', '\\']).next().unwrap_or(entry_name);
    if base.is_empty() || base == "." || base == ".." {
        return None;
    }
    Some(base)
}

/// Ensure a zip entry name is unique: after flattening to basenames, two source
/// images from different directories can collide, so a `-N` counter is inserted
/// before the extension on the second and later hits.
pub(crate) fn dedupe_zip_name(
    basename: &str,
    used: &mut std::collections::BTreeSet<String>,
) -> String {
    if used.insert(basename.to_string()) {
        return basename.to_string();
    }
    let (stem, ext) = basename
        .rsplit_once('.')
        .map_or((basename, None), |(stem, ext)| (stem, Some(ext)));
    let mut counter = 1_usize;
    loop {
        let candidate = match ext {
            Some(ext) => format!("{stem}-{counter}.{ext}"),
            None => format!("{stem}-{counter}"),
        };
        if used.insert(candidate.clone()) {
            return candidate;
        }
        counter += 1;
    }
}

fn pack_named_images_zip_capped(
    images: &[(String, Vec<u8>)],
    notes: Option<&str>,
    max_bytes: usize,
    label: &'static str,
) -> Result<Vec<u8>, String> {
    let buffer = limits::LatchedCappedBuffer::new(max_bytes, label);
    let buffer = write_named_images_zip(zip::ZipWriter::new(buffer), images, notes)?;
    buffer.into_bytes()
}

fn write_named_images_zip<W: Write + Seek>(
    mut writer: zip::ZipWriter<W>,
    images: &[(String, Vec<u8>)],
    notes: Option<&str>,
) -> Result<W, String> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut used = std::collections::BTreeSet::<String>::new();
    for (name, bytes) in images {
        let unique = dedupe_zip_name(name, &mut used);
        writer
            .start_file(&unique, options)
            .map_err(|e| format!("could not add `{unique}` to output zip: {e}"))?;
        writer
            .write_all(bytes)
            .map_err(|e| format!("could not write `{unique}` to output zip: {e}"))?;
    }
    if let Some(notes) = notes {
        writer
            .start_file(EXTRACTION_NOTES_FILENAME, options)
            .map_err(|e| format!("could not add notes to output zip: {e}"))?;
        writer
            .write_all(notes.as_bytes())
            .map_err(|e| format!("could not write notes to output zip: {e}"))?;
    }
    writer
        .finish()
        .map_err(|e| format!("could not finalize output zip: {e}"))
}

#[tool(
    id = "media.pdf_extract_images",
    display_label = "PDF → embedded images",
    toolkit = "media",
    inputs = [
        required input: File = "PDF file bytes",
    ],
    outputs = [
        result: File = "Zip of extracted images",
    ],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http, Pwa, Ext],
)]
/// Extract embedded raster images from a PDF's image XObjects and return them
/// bundled as a single zip [`FileValue`] (`<stem>-images.zip`).
///
/// Pure Rust (`lopdf` + per-filter codecs — no PDFium), so it compiles and runs
/// on every surface including the browser (wasm32) build. Images that use an
/// unsupported filter/colorspace/bit depth are skipped, never corrupted; their
/// reasons are recorded in an [`EXTRACTION_NOTES_FILENAME`] entry in the zip.
pub fn pdf_extract_images(input: &FileValue) -> Result<String, String> {
    let bytes = file_input_bytes(input)?;
    let stem = file_stem(&input.name, PDF_DEFAULT_STEM);
    let outcome = pdf_extract::extract_embedded_images(bytes, stem)?;

    let notes = pdf_skip_notes(outcome.skipped());
    let images: Vec<(String, Vec<u8>)> = outcome
        .into_images()
        .into_iter()
        .map(|image| (image.name, image.bytes))
        .collect();
    let zip = pack_named_images_zip_capped(
        &images,
        notes.as_deref(),
        limits::MAX_PDF_EXTRACT_ZIP_BYTES,
        PDF_EXTRACTION_OUTPUT_ZIP_LABEL,
    )?;

    let file = FileValue {
        name: images_zip_name(&input.name, PDF_DEFAULT_STEM),
        content: FileContent::Bytes(zip),
        mime: Some(IMAGES_ZIP_MIME.to_string()),
    };
    serde_json::to_string(&file).map_err(|e| format!("failed to serialize output file: {e}"))
}

/// Format the per-image skip reasons as a notes block, or `None` when nothing
/// was skipped. Each line is `p{page} i{index}: {reason}`.
fn pdf_skip_notes(skipped: &[String]) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    let mut notes = format!("{PDF_EXTRACTION_SKIPPED_LABEL} {} image(s):", skipped.len());
    for note in skipped {
        notes.push('\n');
        notes.push_str(note);
    }
    Some(notes)
}

mod image_codec;
mod image_conversion;
mod image_convert;
/// `media.image_to_pdf` — build a PDF from an input zip of images (krilla).
mod image_pdf;
mod images_convert;
/// Resource caps applied to every media tool's untrusted `File` input.
mod limits;
/// Pure-Rust embedded-image extraction backing [`pdf_extract_images`].
mod pdf_extract;
/// `media.pdf_to_images` — rasterize PDF pages to PNGs (hayro).
mod pdf_render;
/// PDF classification and native-text Markdown extraction.
mod pdf_text;
/// `media.pptx_extract_images` — extract `ppt/media/*` images to a zip.
mod pptx;

pub use image_conversion::{
    IMAGE_CONVERT_DEFAULT_BACKGROUND, IMAGE_CONVERT_DEFAULT_QUALITY,
    IMAGE_CONVERT_DEFAULT_SVG_WIDTH, IMAGE_CONVERT_MAX_SVG_WIDTH, ImageConversionOptions,
};
pub use image_convert::*;
pub use image_pdf::*;
pub use images_convert::*;
pub use pdf_render::*;
pub use pdf_text::*;
pub use pptx::*;

#[cfg(test)]
mod pdf_output_limits_tests;
#[cfg(test)]
mod tests;
