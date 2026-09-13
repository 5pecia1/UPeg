//! `media.pdf_to_images` — rasterize each PDF page to a PNG with `hayro`, the
//! pure-Rust PDF renderer (Apache-2.0 OR MIT, same author as `krilla`).
//!
//! This replaces the former PDFium backend: no native dynamic library, no
//! bundling/signing/licensing burden, and it compiles to wasm32 so the tool
//! runs on every surface including the browser build.

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_interpret::hayro_syntax::Pdf;
use hayro::hayro_interpret::hayro_syntax::page::Page;
use hayro::{RenderCache, RenderSettings, render};
use upeg_core::{FileContent, FileValue, tool};

use super::image_codec::{RasterFormat, encode_rgba};
use super::limits::{
    MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS, MAX_IMAGE_PIXELS_F64, MAX_PDF_PAGES, over_cap_error,
    validate_max_output_bytes,
};
use super::{
    IMAGES_ZIP_MIME, PAGES_ZIP_SUFFIX, PDF_DEFAULT_STEM, file_input_bytes, file_stem,
    normalize_dpi, pack_named_images_zip_capped,
};

/// PDF user-space units per inch: a PDF point is 1/72 inch, so rasterizing at
/// `dpi` means scaling the page by `dpi / POINTS_PER_INCH`.
const POINTS_PER_INCH: f64 = 72.0;
const PDF_PAGES_OUTPUT_ZIP_LABEL: &str = "PDF pages output zip";

#[tool(
    id = "media.pdf_to_images",
    display_label = "PDF → PNG images",
    toolkit = "media",
    inputs = [
        required input: File = "PDF file bytes",
        optional dpi: Number = "Render DPI, default 144, valid 36..=600",
        optional max_output_bytes: Integer(min=1, max=67108864, default=67108864)
            = "Maximum encoded page and zip bytes",
    ],
    outputs = [
        result: File = "Zip of page PNGs",
    ],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http, Pwa, Ext],
)]
/// Rasterize every page of the input PDF at `dpi` (default 144, valid 36..=600)
/// and return the page PNGs bundled as a single zip [`FileValue`]
/// (`<stem>-pages.zip`, entries `<stem>-p{page}.png`).
///
/// Pure Rust (`hayro`), so it compiles and runs on every surface including the
/// browser (wasm32) build — no PDFium, no native dynamic library.
pub fn pdf_to_images(
    input: &FileValue,
    dpi: f64,
    max_output_bytes: usize,
) -> Result<String, String> {
    let max_output_bytes = validate_max_output_bytes(max_output_bytes)?;
    let bytes = file_input_bytes(input)?;
    let scale = f64::from(normalize_dpi(dpi)?) / POINTS_PER_INCH;
    let pages = render_pdf_pages(bytes, scale as f32, max_output_bytes)?;

    let stem = file_stem(&input.name, PDF_DEFAULT_STEM);
    let images: Vec<(String, Vec<u8>)> = pages
        .into_iter()
        .enumerate()
        .map(|(index, png)| (format!("{stem}-p{}.png", index + 1), png))
        .collect();
    let zip =
        pack_named_images_zip_capped(&images, None, max_output_bytes, PDF_PAGES_OUTPUT_ZIP_LABEL)?;

    let file = FileValue {
        name: format!("{stem}{PAGES_ZIP_SUFFIX}"),
        content: FileContent::Bytes(zip),
        mime: Some(IMAGES_ZIP_MIME.to_string()),
    };
    serde_json::to_string(&file).map_err(|e| format!("failed to serialize output file: {e}"))
}

/// Render each PDF page to a PNG byte buffer at the given scale factor.
///
/// Every page's PNG is held until the caller packs the zip, so both the page
/// count and each page's raster are capped: the input PDF is untrusted, and a
/// handful of bytes of `/MediaBox` and `/Count` otherwise decide how much
/// memory we allocate.
fn render_pdf_pages(
    bytes: &[u8],
    scale: f32,
    max_output_bytes: usize,
) -> Result<Vec<Vec<u8>>, String> {
    let pdf = Pdf::new(bytes.to_vec()).map_err(|e| format!("could not open PDF: {e:?}"))?;
    let cache = RenderCache::new();
    let interpreter = InterpreterSettings::default();

    let pdf_pages = pdf.pages();
    if pdf_pages.len() > MAX_PDF_PAGES {
        return Err(format!(
            "PDF has {} pages, more than the {MAX_PDF_PAGES}-page limit",
            pdf_pages.len()
        ));
    }

    let mut pages = Vec::with_capacity(pdf_pages.len());
    let mut total_bytes = 0_usize;
    for (index, page) in pdf_pages.iter().enumerate() {
        check_page_render_budget(page, scale, index + 1)?;
        let settings = RenderSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: hayro::vello_cpu::color::palette::css::WHITE,
            ..Default::default()
        };
        let pixmap = render(page, &cache, &interpreter, &settings);
        let png = encode_pixmap_png(&pixmap)?;
        total_bytes = total_bytes
            .checked_add(png.len())
            .ok_or_else(|| over_cap_error("rendered pages", max_output_bytes))?;
        if total_bytes > max_output_bytes {
            return Err(over_cap_error("rendered pages", max_output_bytes));
        }
        pages.push(png);
    }
    Ok(pages)
}

/// Reject a page whose raster would blow the pixel caps *before* [`render`]
/// allocates the pixmap.
///
/// `render` sizes the pixmap from the page's own geometry × `scale` and clamps
/// each axis into a `u16`, so a crafted `/MediaBox` silently asks for up to
/// 65535×65535×4 = 17 GB — and the clamp means the pixmap that comes back can
/// no longer tell us what was requested. Checking the unclamped size here is
/// the only place the request is still visible.
fn check_page_render_budget(page: &Page, scale: f32, page_number: usize) -> Result<(), String> {
    let (width, height) = page.render_dimensions();
    let scaled = |extent: f32| f64::from(extent * scale).floor();
    let (width, height) = (scaled(width), scaled(height));
    let too_large = !width.is_finite()
        || !height.is_finite()
        || width > f64::from(MAX_IMAGE_DIMENSION)
        || height > f64::from(MAX_IMAGE_DIMENSION)
        || width * height > MAX_IMAGE_PIXELS_F64;
    if too_large {
        return Err(format!(
            "page {page_number} rasterizes to {width}x{height} pixels, past the \
             {MAX_IMAGE_PIXELS}-pixel limit"
        ));
    }
    Ok(())
}

/// Encode a rendered [`Pixmap`](hayro::vello_cpu::Pixmap) as PNG bytes. The page
/// is drawn over an opaque white background, so premultiplied RGBA equals
/// straight RGBA and the pixmap bytes can be handed to `image` verbatim.
fn encode_pixmap_png(pixmap: &hayro::vello_cpu::Pixmap) -> Result<Vec<u8>, String> {
    let width = u32::from(pixmap.width());
    let height = u32::from(pixmap.height());
    encode_rgba(
        (width, height),
        pixmap.data_as_u8_slice(),
        RasterFormat::Png,
    )
}
