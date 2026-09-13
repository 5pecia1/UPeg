//! `media.image_to_pdf` — build a one-image-per-page PDF from PNG/JPEG input,
//! using the pure-Rust `krilla` PDF writer.
//!
//! Three input shapes are accepted, because all three are what the surfaces
//! actually hand over:
//!
//! - **one image's bytes** — a single-page PDF. Picking one file in any surface
//!   produces exactly this, and it is the most common thing a person asks for.
//! - **a flat directory** ([`FileContent::Directory`]) — one page per entry, in
//!   name order. This is the shape the CLI `@dir` path and the desktop
//!   multi-select build, and it is what the sibling `media.images_convert`
//!   already takes.
//! - **a zip's bytes** — one page per image entry, in entry-name order.
//!
//! Nothing here reads filesystem paths, so the tool still runs on every surface
//! including the browser (wasm32) build.

use std::io::Cursor;

use upeg_core::{FileContent, FileValue, tool};

use super::image_codec::{decode, pixel_count};
use super::limits::{
    MAX_ARCHIVE_TOTAL_BYTES, MAX_DECODED_BYTES, MAX_IMAGE_TO_PDF_RASTER_BYTES, MAX_PDF_PAGES,
    over_cap_error, read_capped, validate_max_output_bytes,
};
use super::{
    IMAGE_TO_PDF_EXTENSIONS, IMAGES_DEFAULT_STEM, PDF_MIME, PDF_SUFFIX, entry_has_extension,
    file_stem,
};

const RGBA8_BYTES_PER_PIXEL: usize = 4;

/// A decoded input image plus its pixel dimensions, ready to become one PDF page.
struct PdfImagePage {
    image: krilla::image::Image,
    width: u32,
    height: u32,
}

#[tool(
    id = "media.image_to_pdf",
    display_label = "Images → PDF",
    toolkit = "media",
    inputs = [
        // Only `max_count` is declared, and deliberately so.
        //
        // It must exceed 1 or every surface refuses a directory outright; 100 is
        // the ceiling Core allows (`MAX_FILE_INPUT_COUNT`), the same value the
        // sibling `media.images_convert` declares. A zip can still carry up to
        // `MAX_PDF_PAGES` images, since its entries are not `File` policy nodes.
        //
        // `extensions` stays EMPTY on purpose — an empty list accepts every name
        // (`FileInputPolicy::accepts_extension`). Declaring one would reject an
        // archive whose name is not `.zip` (a `.dat` holding zip bytes converted
        // fine before) and would reject a directory that merely *contains* a
        // stray file, since Core applies the list to entries too. What the tool
        // can actually decode is decided by the loaders below, from the bytes.
        //
        // No byte caps either: Core already bounds a `File` input's aggregate
        // bytes, and `PageBudget` bounds the decode work.
        required input: File(max_count = 100)
            = "A PNG/JPEG image, a flat directory of them, or a zip of them",
        optional max_output_bytes: Integer(min=1, max=67108864, default=67108864)
            = "Maximum encoded PDF bytes",
    ],
    outputs = [
        result: File = "Generated PDF",
    ],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http, Pwa, Ext],
)]
/// Decode the input's PNG/JPEG images and emit a PDF with one image per page as
/// a [`FileValue`] (`<stem>.pdf`). See the module docs for the three accepted
/// input shapes.
pub fn image_to_pdf(input: &FileValue, max_output_bytes: usize) -> Result<String, String> {
    image_to_pdf_with_raster_budget(input, MAX_IMAGE_TO_PDF_RASTER_BYTES, max_output_bytes)
}

pub(super) fn image_to_pdf_with_raster_budget(
    input: &FileValue,
    raster_budget: usize,
    max_output_bytes: usize,
) -> Result<String, String> {
    let max_output_bytes = validate_max_output_bytes(max_output_bytes)?;
    let pages = load_image_pages(input, raster_budget)?;
    if pages.is_empty() {
        return Err(empty_input_error(input));
    }
    let pdf = build_image_pdf(&pages)?;
    if pdf.len() > max_output_bytes {
        return Err(over_cap_error("output PDF", max_output_bytes));
    }

    let file = FileValue {
        name: format!(
            "{}{PDF_SUFFIX}",
            file_stem(&input.name, IMAGES_DEFAULT_STEM)
        ),
        content: FileContent::Bytes(pdf),
        mime: Some(PDF_MIME.to_string()),
    };
    serde_json::to_string(&file).map_err(|e| format!("failed to serialize output file: {e}"))
}

/// Extensions that mean "the caller meant this to be an archive". Used ONLY to
/// pick which error to report when the bytes turn out to be neither a readable
/// archive nor an image — never to decide which loader runs.
const ARCHIVE_EXTENSIONS: &[&str] = &["zip"];

/// Route the input to the loader for its shape.
///
/// The archive test is **whether the zip reader opens it**, not what the bytes
/// start with. A magic-prefix test would look equivalent and is not: a zip's
/// central directory lives at the *end*, so `ZipArchive::new` happily opens an
/// archive with arbitrary leading data (a self-extracting stub, a concatenated
/// payload) that starts with neither `PK\x03\x04` nor `PK\x05\x06`. Those
/// archives converted fine before this tool accepted bare images, and gating on
/// a prefix would have quietly stopped converting them.
///
/// Falling through to the single-image path is what lets one declared input
/// serve all three shapes: a caller who picks one image never has to know a zip
/// was ever involved.
fn load_image_pages(input: &FileValue, raster_budget: usize) -> Result<Vec<PdfImagePage>, String> {
    let mut budget = PageBudget::new(raster_budget);
    let bytes = match &input.content {
        FileContent::Directory(entries) => {
            return load_image_pages_from_directory(entries, &mut budget);
        }
        FileContent::Bytes(bytes) => bytes.as_slice(),
    };

    match zip::ZipArchive::new(Cursor::new(bytes)) {
        Ok(archive) => load_image_pages_from_archive(archive, &mut budget),
        Err(archive_error) => {
            let decoded = admit_then_decode(&[(input.name.as_str(), bytes)], &mut budget);
            // Neither shape worked. A caller who named the file `.zip` wants to
            // know why the archive would not open; a caller who handed over a
            // broken image does not care that a zip was attempted, so the
            // extension picks the message and nothing else.
            if decoded.is_err() && entry_has_extension(&input.name, ARCHIVE_EXTENSIONS) {
                return Err(format!("could not read input zip: {archive_error}"));
            }
            decoded
        }
    }
}

/// "Nothing to convert" reads differently per shape, so name the shape the
/// caller actually handed over instead of always blaming a zip.
///
/// Only a directory or a readable archive can reach here empty: the
/// single-image path either produces its one page or fails in the decoder, so
/// it never returns an empty page list.
fn empty_input_error(input: &FileValue) -> String {
    match &input.content {
        FileContent::Directory(_) => "input directory contains no PNG/JPEG images".to_string(),
        FileContent::Bytes(_) => "input zip contains no PNG/JPEG images".to_string(),
    }
}

/// The limits every input shape shares: total source bytes read, and total
/// bytes the decoded rasters would occupy. Holding these in one place is what
/// keeps the directory and single-image paths from escaping the caps the zip
/// path has always enforced.
struct PageBudget {
    total_source_bytes: u64,
    total_raster_bytes: usize,
    raster_budget: usize,
}

impl PageBudget {
    fn new(raster_budget: usize) -> Self {
        Self {
            total_source_bytes: 0,
            total_raster_bytes: 0,
            raster_budget,
        }
    }

    /// Charge one image's source and prospective raster bytes, refusing before
    /// the decode when either total would go over.
    fn admit(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let source_bytes = u64::try_from(bytes.len())
            .map_err(|_| "input image byte count overflowed".to_string())?;
        self.total_source_bytes = self
            .total_source_bytes
            .checked_add(source_bytes)
            .ok_or_else(|| "input image byte count overflowed".to_string())?;
        if self.total_source_bytes > MAX_ARCHIVE_TOTAL_BYTES {
            return Err(format!(
                "input's images exceed the {MAX_ARCHIVE_TOTAL_BYTES}-byte total limit"
            ));
        }

        let raster_bytes = pixel_count(name, bytes)?
            .checked_mul(RGBA8_BYTES_PER_PIXEL)
            .ok_or_else(|| "input decoded raster byte count overflowed".to_string())?;
        self.total_raster_bytes = self
            .total_raster_bytes
            .checked_add(raster_bytes)
            .ok_or_else(|| "input decoded raster byte count overflowed".to_string())?;
        if self.total_raster_bytes > self.raster_budget {
            return Err(format!(
                "input's decoded rasters require {} bytes, over the {}-byte limit",
                self.total_raster_bytes, self.raster_budget
            ));
        }
        Ok(())
    }
}

/// Charge the whole batch before decoding any of it, so an over-budget last
/// image does not cost the decode of every image before it. The zip path has
/// always worked this way; the other shapes follow it here.
fn admit_then_decode(
    images: &[(&str, &[u8])],
    budget: &mut PageBudget,
) -> Result<Vec<PdfImagePage>, String> {
    for (name, bytes) in images {
        budget.admit(name, bytes)?;
    }
    let mut pages = Vec::with_capacity(images.len());
    for (name, bytes) in images {
        pages.push(decode_image_page(name, bytes)?);
    }
    Ok(pages)
}

/// One page per directory entry, in name order.
///
/// Entries that are not PNG/JPEG by extension are skipped rather than rejected,
/// matching how the zip path ignores a `readme.txt` sitting next to the images.
/// This is a real path, not a defensive one: the declared policy leaves
/// `extensions` empty, so a directory carrying a stray `notes.txt` or a
/// `.DS_Store` reaches the tool intact and converts the images it does hold.
/// A nested directory is skipped too, for a different reason: the surfaces only
/// ever build a flat one, so a nested entry means a caller assembled a
/// `FileValue` tree by hand and there is no page to make from it.
fn load_image_pages_from_directory(
    entries: &[FileValue],
    budget: &mut PageBudget,
) -> Result<Vec<PdfImagePage>, String> {
    let mut images = Vec::<(&str, &[u8])>::new();
    for entry in entries {
        let FileContent::Bytes(bytes) = &entry.content else {
            continue;
        };
        if entry_has_extension(&entry.name, IMAGE_TO_PDF_EXTENSIONS) {
            images.push((entry.name.as_str(), bytes.as_slice()));
        }
    }
    images.sort_by(|(left, _), (right, _)| left.cmp(right));
    // Unreachable through any surface today: the declared `max_count = 100` is
    // far below `MAX_PDF_PAGES`, and Core enforces it before the tool runs. Kept
    // because the page limit belongs to the tool, not to the declaration — a
    // caller reaching this function directly, or a later `max_count` raise,
    // must still be bounded here.
    if images.len() > MAX_PDF_PAGES {
        return Err(format!(
            "input directory holds {} images, more than the {MAX_PDF_PAGES}-page limit",
            images.len()
        ));
    }
    admit_then_decode(&images, budget)
}

/// Read every image entry from the zip in sorted entry-name order and decode
/// each into a [`PdfImagePage`]. Sorting makes page order deterministic and
/// independent of the archive's internal storage order.
///
/// Entries are re-read *by index*, never by name: zip names are not unique, and
/// `by_name` resolves to the first match — so an archive holding two distinct
/// `a.png` entries would emit the first image twice and silently drop the
/// second. The index also breaks ties in the sort, keeping duplicate names in
/// their archive order.
fn load_image_pages_from_archive(
    mut archive: zip::ZipArchive<Cursor<&[u8]>>,
    budget: &mut PageBudget,
) -> Result<Vec<PdfImagePage>, String> {
    let mut entries = Vec::<(String, usize)>::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| format!("could not read zip entry {index}: {e}"))?;
        if entry.is_file() && entry_has_extension(entry.name(), IMAGE_TO_PDF_EXTENSIONS) {
            entries.push((entry.name().to_string(), index));
        }
    }
    entries.sort();
    if entries.len() > MAX_PDF_PAGES {
        return Err(format!(
            "input zip holds {} images, more than the {MAX_PDF_PAGES}-page limit",
            entries.len()
        ));
    }

    for (name, index) in &entries {
        let entry = archive
            .by_index(*index)
            .map_err(|e| format!("could not read `{name}` from zip: {e}"))?;
        let buffer = read_capped(entry, MAX_DECODED_BYTES, &format!("`{name}`"))?;
        budget.admit(name, &buffer)?;
    }

    let mut pages = Vec::with_capacity(entries.len());
    for (name, index) in &entries {
        let entry = archive
            .by_index(*index)
            .map_err(|e| format!("could not read `{name}` from zip: {e}"))?;
        let buffer = read_capped(entry, MAX_DECODED_BYTES, &format!("`{name}`"))?;
        pages.push(decode_image_page(name, &buffer)?);
    }
    Ok(pages)
}

/// Decode one PNG/JPEG image into a krilla page image, rejecting other formats.
///
/// Decoding goes through [`image_decode_limits`] rather than
/// `load_from_memory_with_format`, whose limits leave the pixel dimensions
/// unbounded: a ~1 KB PNG declaring 65535×65535 makes `to_rgba8` ask for 17 GB.
fn decode_image_page(name: &str, bytes: &[u8]) -> Result<PdfImagePage, String> {
    let decoded = decode(name, bytes)?;
    let width = decoded.width();
    let height = decoded.height();
    let rgba = decoded.to_rgba8();

    Ok(PdfImagePage {
        image: krilla::image::Image::from_rgba8(rgba.into_raw(), width, height),
        width,
        height,
    })
}

/// Assemble the decoded images into a PDF, one image per page sized to the
/// image's pixel dimensions.
fn build_image_pdf(image_pages: &[PdfImagePage]) -> Result<Vec<u8>, String> {
    let mut document = krilla::Document::new();

    for image_page in image_pages {
        let width = image_page.width as f32;
        let height = image_page.height as f32;
        let page_settings =
            krilla::page::PageSettings::from_wh(width, height).ok_or_else(|| {
                format!(
                    "invalid image dimensions: {}x{}",
                    image_page.width, image_page.height
                )
            })?;
        let draw_size = krilla::geom::Size::from_wh(width, height).ok_or_else(|| {
            format!(
                "invalid image dimensions: {}x{}",
                image_page.width, image_page.height
            )
        })?;

        let mut page = document.start_page_with(page_settings);
        let mut surface = page.surface();
        surface.draw_image(image_page.image.clone(), draw_size);
        surface.finish();
        page.finish();
    }

    document
        .finish()
        .map_err(|e| format!("failed to create PDF: {e:?}"))
}
