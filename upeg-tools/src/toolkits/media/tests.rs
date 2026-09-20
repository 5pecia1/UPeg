//! Tests for `media`, extracted from `media.rs` to keep the parent module under
//! the 1000-line workspace budget. Mounted as a `#[cfg(test)] mod tests;`, so
//! `super::*` brings every `pub`/`pub(crate)` item of `media` (including the
//! re-exported `pptx` / `image_pdf` tool functions).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::print_stdout,
    clippy::unreachable,
    clippy::string_add,
    clippy::manual_let_else,
    clippy::float_cmp,
    reason = "test module uses unwrap/panic idiomatically; float_cmp on fixture constants is intentional"
)]

use super::*;

fn default_images_convert(
    images: &FileValue,
    format: &str,
    limit: usize,
) -> Result<String, String> {
    images_convert(
        images,
        format,
        IMAGE_CONVERT_DEFAULT_QUALITY,
        IMAGE_CONVERT_DEFAULT_BACKGROUND,
        IMAGE_CONVERT_DEFAULT_SVG_WIDTH,
        limit,
    )
}

/// Regression tests for the resource caps and the reject-don't-corrupt rules
/// that guard these tools' untrusted `File` input. Split out to keep this file
/// inside the workspace's 1000-line budget; it reuses the fixtures below, which
/// (like every fixture here) are native-only.
#[cfg(not(target_arch = "wasm32"))]
mod security;

#[cfg(not(target_arch = "wasm32"))]
#[path = "tests/images_convert.rs"]
mod batch_convert;
#[cfg(not(target_arch = "wasm32"))]
#[path = "tests/images_convert_metadata.rs"]
mod batch_convert_metadata;
#[cfg(not(target_arch = "wasm32"))]
#[path = "tests/images_convert_output_cap.rs"]
mod batch_convert_output_cap;
/// CCITTFaxDecode round trips, and the CCITT encoder they need — no dependency
/// provides one. Split out for the same budget reason; `security` builds its
/// hostile CCITT fixtures with the same encoder.
#[cfg(not(target_arch = "wasm32"))]
mod ccitt;
#[cfg(not(target_arch = "wasm32"))]
mod ccitt_encode;
#[cfg(not(target_arch = "wasm32"))]
#[path = "tests/image_codec.rs"]
mod image_codec;

// ─── in-memory image fixtures ───────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn png_bytes(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |_x, _y| image::Rgba(color));
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("PNG fixture should encode");
    bytes
}

#[cfg(not(target_arch = "wasm32"))]
fn jpeg_bytes(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
    let image = image::RgbImage::from_fn(width, height, |_x, _y| image::Rgb(color));
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Jpeg,
        )
        .expect("JPEG fixture should encode");
    bytes
}

// ─── File value + zip helpers ───────────────────────────────────────

/// Wrap raw bytes as a `File` input value, mirroring what the CLI `@path`
/// path constructs.
#[cfg(not(target_arch = "wasm32"))]
fn file_input(name: &str, bytes: Vec<u8>) -> FileValue {
    FileValue {
        name: name.to_string(),
        content: FileContent::Bytes(bytes),
        mime: None,
    }
}

/// Build an in-memory zip archive from `(entry_name, bytes)` pairs.
#[cfg(not(target_arch = "wasm32"))]
fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let mut buffer = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
        for (name, bytes) in entries {
            zip.start_file(*name, SimpleFileOptions::default())
                .expect("entry should start");
            zip.write_all(bytes).expect("entry bytes should write");
        }
        zip.finish().expect("zip fixture should finalize");
    }
    buffer
}

/// Parse a tool's returned JSON `FileValue`.
#[cfg(not(target_arch = "wasm32"))]
fn output_file(result_json: &str) -> FileValue {
    serde_json::from_str(result_json).expect("result must be a FileValue JSON string")
}

/// Extract the byte payload of a `FileValue`, panicking on a directory.
#[cfg(not(target_arch = "wasm32"))]
fn file_bytes(file: &FileValue) -> Vec<u8> {
    match &file.content {
        FileContent::Bytes(bytes) => bytes.clone(),
        FileContent::Directory(_) => panic!("output must be a byte file, got a directory"),
    }
}

/// Read every `(name, bytes)` entry out of a zip archive.
#[cfg(not(target_arch = "wasm32"))]
fn zip_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    use std::io::Read;
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).expect("output must be a zip");
    (0..archive.len())
        .map(|index| {
            let mut entry = archive.by_index(index).unwrap();
            let name = entry.name().to_string();
            let mut buffer = Vec::new();
            entry.read_to_end(&mut buffer).unwrap();
            (name, buffer)
        })
        .collect()
}

/// Parse a tool result into its `FileValue` plus the entries of its zip payload.
#[cfg(not(target_arch = "wasm32"))]
fn output_zip_entries(result_json: &str) -> (FileValue, Vec<(String, Vec<u8>)>) {
    let file = output_file(result_json);
    let entries = zip_entries(&file_bytes(&file));
    (file, entries)
}

/// The entry names of a tool result's zip payload.
#[cfg(not(target_arch = "wasm32"))]
fn output_zip_names(result_json: &str) -> (FileValue, Vec<String>) {
    let (file, entries) = output_zip_entries(result_json);
    (file, entries.into_iter().map(|(name, _)| name).collect())
}

/// Zip entries that are extracted images (i.e. not the notes text entry).
#[cfg(not(target_arch = "wasm32"))]
fn image_entries(entries: &[(String, Vec<u8>)]) -> Vec<&(String, Vec<u8>)> {
    entries
        .iter()
        .filter(|(name, _)| name != EXTRACTION_NOTES_FILENAME)
        .collect()
}

/// Count `/Type /Page` (excluding `/Type /Pages`) markers in raw PDF bytes.
#[cfg(not(target_arch = "wasm32"))]
fn pdf_page_count(bytes: &[u8]) -> usize {
    let text = String::from_utf8_lossy(bytes);
    text.matches("/Type /Page").count() - text.matches("/Type /Pages").count()
}

// ─── normalize_dpi (pdf_to_images) ──────────────────────────────────

#[test]
fn dpi_normalization_uses_default() {
    let result = normalize_dpi(PDF_TO_IMAGES_DEFAULT_DPI);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 144.0);
}

#[test]
fn dpi_normalization_allows_min_and_max() {
    assert!(normalize_dpi(PDF_TO_IMAGES_MIN_DPI).is_ok());
    assert!(normalize_dpi(PDF_TO_IMAGES_MAX_DPI).is_ok());
}

#[test]
fn dpi_normalization_rejects_out_of_range() {
    assert!(normalize_dpi(PDF_TO_IMAGES_MIN_DPI - 1.0).is_err());
    assert!(normalize_dpi(PDF_TO_IMAGES_MAX_DPI + 1.0).is_err());
}

// ─── media.image_to_pdf (image / directory / zip → PDF) ─────────────

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_makes_pdf_from_image_zip() {
    let png = png_bytes(3, 2, [255, 0, 0, 255]);
    let zip = build_zip(&[("photo.png", &png)]);

    let result = image_to_pdf(&file_input("shots.zip", zip), MAX_MEDIA_OUTPUT_BYTES)
        .expect("zip should convert to PDF");
    let file = output_file(&result);
    let pdf = file_bytes(&file);

    assert_eq!(file.name, "shots.pdf");
    assert_eq!(file.mime.as_deref(), Some(PDF_MIME));
    assert!(pdf.starts_with(b"%PDF-"), "output must be a PDF");
    assert_eq!(pdf_page_count(&pdf), 1);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_orders_multiple_images_by_name() {
    let a_png = png_bytes(5, 3, [255, 0, 0, 255]);
    let b_jpg = jpeg_bytes(2, 7, [0, 0, 255]);
    // Store out of order to prove sorting by entry name drives page order.
    let zip = build_zip(&[("b.jpg", &b_jpg), ("a.png", &a_png)]);

    let result = image_to_pdf(&file_input("deck.zip", zip), MAX_MEDIA_OUTPUT_BYTES)
        .expect("multiple images should convert");
    let pdf = file_bytes(&output_file(&result));

    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(pdf_page_count(&pdf), 2);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_errors_on_zip_without_images() {
    let zip = build_zip(&[("readme.txt", b"not an image")]);

    let err = image_to_pdf(&file_input("empty.zip", zip), MAX_MEDIA_OUTPUT_BYTES)
        .expect_err("a zip with no images should fail");
    assert!(err.contains("no PNG/JPEG images"), "got {err:?}");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_errors_on_corrupt_image() {
    let zip = build_zip(&[("corrupt.png", b"not really a png")]);

    let err = image_to_pdf(&file_input("bad.zip", zip), MAX_MEDIA_OUTPUT_BYTES)
        .expect_err("a corrupt image should fail");
    assert!(err.contains("could not decode"), "got {err:?}");
}

/// Handing this tool a single bare image is the most common thing a person
/// does. That input used to fall into the zip parser and end in `Could not
/// find EOCD`.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_makes_single_image_one_page_pdf() {
    let png = png_bytes(4, 6, [0, 128, 64, 255]);

    let result = image_to_pdf(&file_input("scan.png", png), MAX_MEDIA_OUTPUT_BYTES)
        .expect("a bare image should convert to a one-page PDF");
    let file = output_file(&result);
    let pdf = file_bytes(&file);

    assert_eq!(file.name, "scan.pdf");
    assert_eq!(file.mime.as_deref(), Some(PDF_MIME));
    assert!(pdf.starts_with(b"%PDF-"), "output must be a PDF");
    assert_eq!(pdf_page_count(&pdf), 1);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_orders_directory_pages_by_name() {
    let a_png = png_bytes(5, 3, [255, 0, 0, 255]);
    let b_jpg = jpeg_bytes(2, 7, [0, 0, 255]);
    // Store out of order to prove sorting by name drives page order.
    let dir_input = FileValue {
        name: "album".to_string(),
        content: FileContent::Directory(vec![
            file_input("b.jpg", b_jpg),
            file_input("a.png", a_png),
        ]),
        mime: None,
    };

    let result =
        image_to_pdf(&dir_input, MAX_MEDIA_OUTPUT_BYTES).expect("a flat directory should convert");
    let file = output_file(&result);
    let pdf = file_bytes(&file);

    assert_eq!(file.name, "album.pdf");
    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(pdf_page_count(&pdf), 2);

    let document = lopdf::Document::load_mem(&pdf).expect("output PDF should parse");
    let page_media_boxes: Vec<Vec<f32>> = document
        .get_pages()
        .values()
        .map(|&page_id| {
            document
                .get_dictionary(page_id)
                .expect("page dictionary should exist")
                .get(b"MediaBox")
                .expect("page should declare its dimensions")
                .as_array()
                .expect("MediaBox should contain coordinates")
                .iter()
                .map(|coordinate| coordinate.as_float().expect("coordinate should be numeric"))
                .collect()
        })
        .collect();
    let expected_media_boxes = [vec![0.0, 0.0, 5.0, 3.0], vec![0.0, 0.0, 2.0, 7.0]];
    assert_eq!(
        page_media_boxes, expected_media_boxes,
        "the 5x3 a.png page must precede the 2x7 b.jpg page"
    );
}

/// The same tolerance the zip path shows for `readme.txt` applies to
/// directories.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_skips_non_image_directory_entries() {
    let png = png_bytes(3, 3, [10, 20, 30, 255]);
    let dir_input = FileValue {
        name: "mixed".to_string(),
        content: FileContent::Directory(vec![
            file_input("notes.txt", b"not an image".to_vec()),
            file_input("shot.png", png),
        ]),
        mime: None,
    };

    let result = image_to_pdf(&dir_input, MAX_MEDIA_OUTPUT_BYTES)
        .expect("a non-image sibling must not fail the whole call");
    let pdf = file_bytes(&output_file(&result));

    assert_eq!(pdf_page_count(&pdf), 1);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_errors_on_directory_without_images() {
    let dir_input = FileValue {
        name: "dir".to_string(),
        content: FileContent::Directory(Vec::new()),
        mime: None,
    };

    let err =
        image_to_pdf(&dir_input, MAX_MEDIA_OUTPUT_BYTES).expect_err("an empty directory must fail");
    assert!(
        err.contains("input directory contains no PNG/JPEG images"),
        "error must name the directory, not a zip: {err:?}"
    );
}

/// Bytes that are neither an image nor a zip must fail with what went wrong,
/// not zip-parser internals.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_errors_with_decode_error_for_non_image_non_zip() {
    let err = image_to_pdf(
        &file_input("notes.txt", b"not an image".to_vec()),
        MAX_MEDIA_OUTPUT_BYTES,
    )
    .expect_err("bytes that are neither an image nor a zip must fail");
    assert!(err.contains("could not decode"), "got {err:?}");
    assert!(
        !err.contains("EOCD"),
        "zip internals must not leak to the user: {err:?}"
    );
}

// ─── media.pdf_to_images (hayro rasterizer, pure Rust) ──────────────
//
// Fixtures build a real PDF from an image via `image_to_pdf` (krilla), where
// each page's point size equals the source image's pixel dimensions — so
// rendering at 72 dpi (scale 1) reproduces those dimensions exactly.

#[cfg(not(target_arch = "wasm32"))]
fn pdf_from_image(name: &str, width: u32, height: u32) -> Vec<u8> {
    let png = png_bytes(width, height, [10, 120, 200, 255]);
    let zip = build_zip(&[(name, &png)]);
    let result = image_to_pdf(&file_input("doc.zip", zip), MAX_MEDIA_OUTPUT_BYTES)
        .expect("fixture PDF should build");
    file_bytes(&output_file(&result))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_to_images_rasterizes_pdf_pages_to_png() {
    let pdf = pdf_from_image("page.png", 100, 60);

    let result = pdf_to_images(&file_input("doc.pdf", pdf), 72.0, MAX_MEDIA_OUTPUT_BYTES)
        .expect("render should succeed");
    let (file, entries) = output_zip_entries(&result);

    assert_eq!(file.name, "doc-pages.zip");
    assert_eq!(file.mime.as_deref(), Some(IMAGES_ZIP_MIME));
    let images = image_entries(&entries);
    assert_eq!(images.len(), 1, "single-page PDF → one page PNG");
    let (name, bytes) = images[0];
    assert_eq!(name, "doc-p1.png");
    let decoded = image::load_from_memory(bytes).expect("page PNG should decode");
    // 72 dpi = scale 1 → the page's point size (== source pixels) verbatim.
    assert_eq!((decoded.width(), decoded.height()), (100, 60));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_to_images_scales_resolution_with_dpi() {
    let pdf = pdf_from_image("page.png", 50, 40);

    // 144 dpi = scale 2 → doubled pixel dimensions.
    let result = pdf_to_images(&file_input("doc.pdf", pdf), 144.0, MAX_MEDIA_OUTPUT_BYTES)
        .expect("render should succeed");
    let (_file, entries) = output_zip_entries(&result);
    let images = image_entries(&entries);
    let decoded = image::load_from_memory(&images[0].1).expect("page PNG should decode");
    assert_eq!((decoded.width(), decoded.height()), (100, 80));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_to_images_rejects_invalid_dpi() {
    let err = pdf_to_images(
        &file_input("x.pdf", b"%PDF-1.7".to_vec()),
        10.0,
        MAX_MEDIA_OUTPUT_BYTES,
    )
    .expect_err("dpi below range should fail");
    assert!(err.contains("DPI must be between"), "got {err:?}");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_to_images_errors_on_non_pdf_bytes() {
    let err = pdf_to_images(
        &file_input("x.pdf", b"not a pdf".to_vec()),
        144.0,
        MAX_MEDIA_OUTPUT_BYTES,
    )
    .expect_err("non-PDF bytes should fail");
    assert!(err.contains("could not open PDF"), "got {err:?}");
}

// ─── shared archive helpers ─────────────────────────────────────────

#[test]
fn pptx_image_entry_accepts_only_image_extensions_under_media() {
    assert!(is_pptx_image_entry("ppt/media/image1.png"));
    assert!(is_pptx_image_entry("ppt/media/photo.JPEG"));
    assert!(is_pptx_image_entry("ppt/media/logo.Emf"));
}

#[test]
fn pptx_image_entry_rejects_non_images_and_other_paths() {
    assert!(!is_pptx_image_entry("ppt/slides/slide1.xml"));
    assert!(!is_pptx_image_entry("ppt/media/notes.xml"));
    assert!(!is_pptx_image_entry("ppt/media/no_extension"));
    assert!(!is_pptx_image_entry("docProps/thumbnail.jpeg"));
}

#[test]
fn image_to_pdf_extensions_accept_only_png_and_jpeg() {
    assert!(entry_has_extension("a.png", IMAGE_TO_PDF_EXTENSIONS));
    assert!(entry_has_extension("b.JPG", IMAGE_TO_PDF_EXTENSIONS));
    assert!(entry_has_extension("c.jpeg", IMAGE_TO_PDF_EXTENSIONS));
    assert!(!entry_has_extension("d.gif", IMAGE_TO_PDF_EXTENSIONS));
    assert!(!entry_has_extension("noext", IMAGE_TO_PDF_EXTENSIONS));
}

#[test]
fn zip_entry_basename_keeps_only_last_path_component() {
    assert_eq!(
        zip_entry_basename("ppt/media/image1.png"),
        Some("image1.png")
    );
    assert_eq!(zip_entry_basename("image1.png"), Some("image1.png"));
    assert_eq!(zip_entry_basename("a\\b\\windows.png"), Some("windows.png"));
}

#[test]
fn zip_entry_basename_neutralizes_path_traversal() {
    // Zip Slip payloads collapse to a harmless basename (or are rejected),
    // never an absolute or parent-relative path.
    assert_eq!(zip_entry_basename("../../../etc/passwd"), Some("passwd"));
    assert_eq!(zip_entry_basename("ppt/media/.."), None);
    assert_eq!(zip_entry_basename("."), None);
    assert_eq!(zip_entry_basename(".."), None);
}

#[test]
fn images_zip_name_appends_suffix_to_stem() {
    assert_eq!(
        images_zip_name("deck.pptx", PPTX_DEFAULT_STEM),
        "deck-images.zip"
    );
    assert_eq!(
        images_zip_name("/a/b/report.pdf", PDF_DEFAULT_STEM),
        "report-images.zip"
    );
    assert_eq!(
        images_zip_name(".hidden", PPTX_DEFAULT_STEM),
        "pptx-images.zip"
    );
}

// ─── media.pptx_extract_images (File → zip) ─────────────────────────

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pptx_extract_pulls_only_media_images_and_blocks_zip_slip() {
    let pptx = build_zip(&[
        ("ppt/media/image1.png", b"\x89PNG-one"),
        ("ppt/slides/slide1.xml", b"<xml/>"),
        ("ppt/media/../../evil.png", b"\x89PNG-evil"),
    ]);

    let result = pptx_extract_images(&file_input("deck.pptx", pptx))
        .expect("pptx extraction should succeed");
    let (file, names) = output_zip_names(&result);

    assert_eq!(file.name, "deck-images.zip");
    assert_eq!(file.mime.as_deref(), Some(IMAGES_ZIP_MIME));
    // The real image and the (harmlessly renamed) slip payload both land as
    // basenames; the xml entry is skipped and no path escapes the archive.
    assert!(names.contains(&"image1.png".to_string()));
    assert!(names.contains(&"evil.png".to_string()));
    assert!(!names.iter().any(|name| name.contains("slide1.xml")));
    assert!(
        !names
            .iter()
            .any(|name| name.contains("..") || name.starts_with('/')),
        "no output entry may carry a traversal path, got {names:?}"
    );
    assert_eq!(names.len(), 2);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pptx_extract_returns_empty_zip_without_images() {
    let pptx = build_zip(&[("ppt/slides/slide1.xml", b"<xml/>")]);

    let result = pptx_extract_images(&file_input("empty.pptx", pptx))
        .expect("extraction should succeed even with no media");
    let (file, names) = output_zip_names(&result);

    assert_eq!(file.name, "empty-images.zip");
    assert!(
        names.is_empty(),
        "no media means an empty (still valid) zip, got {names:?}"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pptx_extract_distinguishes_colliding_basenames() {
    let pptx = build_zip(&[
        ("ppt/media/image1.png", b"a"),
        ("ppt/media/sub/image1.png", b"b"),
    ]);

    let result =
        pptx_extract_images(&file_input("dup.pptx", pptx)).expect("extraction should succeed");
    let (_file, names) = output_zip_names(&result);

    assert_eq!(
        names.len(),
        2,
        "colliding basenames must both survive, got {names:?}"
    );
    assert!(names.contains(&"image1.png".to_string()));
    assert!(names.contains(&"image1-1.png".to_string()));
}

// ─── media.pdf_extract_images (File → zip) ──────────────────────────
//
// The fixtures build a minimal single-page PDF drawing one image XObject with
// a chosen filter/colorspace, exercising the pure-Rust `pdf_extract` codecs
// with no PDFium dependency.

#[cfg(not(target_arch = "wasm32"))]
fn build_image_pdf_fixture(
    width: i64,
    height: i64,
    colorspace: &str,
    filter: &str,
    content: Vec<u8>,
) -> Vec<u8> {
    build_image_pdf_fixture_with_parms(width, height, colorspace, filter, None, content)
}

#[cfg(not(target_arch = "wasm32"))]
fn build_image_pdf_fixture_with_parms(
    width: i64,
    height: i64,
    colorspace: &str,
    filter: &str,
    decode_parms: Option<lopdf::Dictionary>,
    content: Vec<u8>,
) -> Vec<u8> {
    build_image_pdf_fixture_with_extras(
        width,
        height,
        colorspace,
        filter,
        decode_parms,
        &[],
        content,
    )
}

/// Same as [`build_image_pdf_fixture_with_parms`], plus arbitrary extra entries
/// on the image XObject's dictionary (`/Decode`, …).
#[cfg(not(target_arch = "wasm32"))]
fn build_image_pdf_fixture_with_extras(
    width: i64,
    height: i64,
    colorspace: &str,
    filter: &str,
    decode_parms: Option<lopdf::Dictionary>,
    extra_image_entries: &[(&str, lopdf::Object)],
    content: Vec<u8>,
) -> Vec<u8> {
    use lopdf::content::{Content, Operation};
    use lopdf::{Document, Object, Stream, dictionary};

    let mut doc = Document::with_version("1.7");
    let mut image_dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => width,
        "Height" => height,
        "ColorSpace" => colorspace,
        "BitsPerComponent" => 8_i64,
        "Filter" => filter,
    };
    if let Some(parms) = decode_parms {
        image_dict.set("DecodeParms", Object::Dictionary(parms));
    }
    for (key, value) in extra_image_entries {
        image_dict.set(*key, value.clone());
    }
    let image_id = doc.add_object(Stream::new(image_dict, content));
    let draw = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    width.into(),
                    0.into(),
                    0.into(),
                    height.into(),
                    0.into(),
                    0.into(),
                ],
            ),
            Operation::new("Do", vec![Object::Name(b"Im0".to_vec())]),
            Operation::new("Q", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        draw.encode().expect("content should encode"),
    ));
    let resources = doc.add_object(dictionary! {
        "XObject" => dictionary! { "Im0" => image_id },
    });
    let page_id = doc.new_object_id();
    let pages_id = doc.new_object_id();
    doc.objects.insert(
        page_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources,
            "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
        }),
    );
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1_i64,
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes)
        .expect("fixture PDF should serialize");
    bytes
}

#[cfg(not(target_arch = "wasm32"))]
fn zlib_compress(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).expect("zlib write should succeed");
    encoder.finish().expect("zlib finish should succeed")
}

#[cfg(not(target_arch = "wasm32"))]
fn encode_jpeg(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x * 8) as u8, (y * 8) as u8, 64])
    });
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Jpeg,
        )
        .expect("JPEG encode should succeed");
    bytes
}

// Encode raw 8-bit samples with PNG per-row predictors, cycling filter types
// None/Sub/Up/Average/Paeth across rows so decoding exercises all five.
#[cfg(not(target_arch = "wasm32"))]
fn png_predict_encode(raw: &[u8], width: usize, height: usize, colors: usize) -> Vec<u8> {
    let stride = width * colors;
    assert_eq!(raw.len(), stride * height);
    let paeth = |a: i32, b: i32, c: i32| -> u8 {
        let p = a + b - c;
        let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
        if pa <= pb && pa <= pc {
            a as u8
        } else if pb <= pc {
            b as u8
        } else {
            c as u8
        }
    };
    let mut out = Vec::with_capacity((stride + 1) * height);
    for row in 0..height {
        let filter_type = (row % 5) as u8;
        out.push(filter_type);
        for i in 0..stride {
            let cur = raw[row * stride + i];
            let a = if i >= colors {
                raw[row * stride + i - colors]
            } else {
                0
            };
            let b = if row > 0 {
                raw[(row - 1) * stride + i]
            } else {
                0
            };
            let c = if row > 0 && i >= colors {
                raw[(row - 1) * stride + i - colors]
            } else {
                0
            };
            let pred = match filter_type {
                1 => a,
                2 => b,
                3 => a.midpoint(b),
                4 => paeth(i32::from(a), i32::from(b), i32::from(c)),
                _ => 0,
            };
            out.push(cur.wrapping_sub(pred));
        }
    }
    out
}

// Encode raw 8-bit samples with the TIFF horizontal predictor (predictor 2).
#[cfg(not(target_arch = "wasm32"))]
fn tiff_predict_encode(raw: &[u8], width: usize, height: usize, colors: usize) -> Vec<u8> {
    let stride = width * colors;
    assert_eq!(raw.len(), stride * height);
    let mut out = raw.to_vec();
    for row in 0..height {
        let base = row * stride;
        for i in colors..stride {
            out[base + i] = raw[base + i].wrapping_sub(raw[base + i - colors]);
        }
    }
    out
}

// Encode bilevel rows (true = black) as a CCITT Group 4 stream, returning the
// packed bytes.
#[cfg(not(target_arch = "wasm32"))]
fn ccitt_g4_encode(rows: &[Vec<bool>]) -> Vec<u8> {
    ccitt_encode::CcittEncoder {
        coding: ccitt_encode::Coding::Group4,
        byte_align: false,
        emit_eol: false,
    }
    .encode(rows)
}

/// Decode the single extracted image out of a `pdf_extract_images` result,
/// asserting exactly one image entry with the expected name suffix.
#[cfg(not(target_arch = "wasm32"))]
fn single_extracted_image(result_json: &str, name_suffix: &str) -> Vec<u8> {
    let (_file, entries) = output_zip_entries(result_json);
    let images = image_entries(&entries);
    assert_eq!(
        images.len(),
        1,
        "expected exactly one image, got {entries:?}"
    );
    let (name, bytes) = images[0];
    assert!(name.ends_with(name_suffix), "got {name}");
    bytes.clone()
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_extract_reconstructs_flate_devicergb_image_as_png() {
    let (width, height) = (4_u32, 3_u32);
    let raw: Vec<u8> = (0..width * height)
        .flat_map(|i| [(i * 10) as u8, (i * 20) as u8, (i * 30) as u8])
        .collect();
    let pdf = build_image_pdf_fixture(
        i64::from(width),
        i64::from(height),
        "DeviceRGB",
        "FlateDecode",
        zlib_compress(&raw),
    );

    let result = pdf_extract_images(&file_input("flate.pdf", pdf))
        .expect("FlateDecode image should extract");
    assert_eq!(output_file(&result).name, "flate-images.zip");
    let bytes = single_extracted_image(&result, "flate-p1-i1.png");
    let decoded = image::load_from_memory(&bytes)
        .expect("PNG should decode")
        .to_rgb8();
    assert_eq!(decoded.dimensions(), (width, height));
    assert_eq!(decoded.into_raw(), raw, "round trip should preserve pixels");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_extract_passes_dct_image_through_as_jpg() {
    let jpeg = encode_jpeg(8, 8);
    let pdf = build_image_pdf_fixture(8, 8, "DeviceRGB", "DCTDecode", jpeg.clone());

    let result =
        pdf_extract_images(&file_input("dct.pdf", pdf)).expect("DCTDecode image should extract");
    let bytes = single_extracted_image(&result, "dct-p1-i1.jpg");
    // JPEG bytes travel verbatim (no decode/re-encode) and stay decodable.
    assert_eq!(bytes, jpeg);
    let decoded = image::load_from_memory(&bytes).expect("JPEG should decode");
    assert_eq!((decoded.width(), decoded.height()), (8, 8));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_extract_skips_unsupported_filter_and_notes_reason() {
    let pdf = build_image_pdf_fixture(4, 4, "DeviceRGB", "JPXDecode", vec![0_u8; 16]);

    let result = pdf_extract_images(&file_input("jpx.pdf", pdf))
        .expect("unsupported filter should not error the run");
    let (_file, entries) = output_zip_entries(&result);

    assert!(
        image_entries(&entries).is_empty(),
        "no image should be extracted"
    );
    let notes = entries
        .iter()
        .find(|(name, _)| name == EXTRACTION_NOTES_FILENAME)
        .expect("skipped run must carry a notes entry");
    let text = String::from_utf8_lossy(&notes.1);
    assert!(text.contains("skipped 1 image(s):"), "got {text:?}");
    assert!(text.contains("JPXDecode"), "got {text:?}");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_extract_restores_flate_png_predictor_image() {
    let (width, height, colors) = (5_usize, 4_usize, 3_usize);
    let raw: Vec<u8> = (0..width * height * colors)
        .map(|i| (i * 7 % 256) as u8)
        .collect();
    let predicted = png_predict_encode(&raw, width, height, colors);
    use lopdf::dictionary;
    let parms = dictionary! {
        "Predictor" => 15_i64,
        "Colors" => colors as i64,
        "Columns" => width as i64,
        "BitsPerComponent" => 8_i64,
    };
    let pdf = build_image_pdf_fixture_with_parms(
        width as i64,
        height as i64,
        "DeviceRGB",
        "FlateDecode",
        Some(parms),
        zlib_compress(&predicted),
    );

    let result = pdf_extract_images(&file_input("flate_png_pred.pdf", pdf))
        .expect("predicted FlateDecode image should extract");
    let bytes = single_extracted_image(&result, "flate_png_pred-p1-i1.png");
    let decoded = image::load_from_memory(&bytes)
        .expect("PNG should decode")
        .to_rgb8();
    assert_eq!(decoded.dimensions(), (width as u32, height as u32));
    assert_eq!(
        decoded.into_raw(),
        raw,
        "PNG predictor round trip must match"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_extract_restores_flate_tiff_predictor_image() {
    let (width, height, colors) = (6_usize, 3_usize, 1_usize);
    let raw: Vec<u8> = (0..width * height).map(|i| (i * 13 % 256) as u8).collect();
    let predicted = tiff_predict_encode(&raw, width, height, colors);
    use lopdf::dictionary;
    let parms = dictionary! {
        "Predictor" => 2_i64,
        "Colors" => colors as i64,
        "Columns" => width as i64,
        "BitsPerComponent" => 8_i64,
    };
    let pdf = build_image_pdf_fixture_with_parms(
        width as i64,
        height as i64,
        "DeviceGray",
        "FlateDecode",
        Some(parms),
        zlib_compress(&predicted),
    );

    let result = pdf_extract_images(&file_input("flate_tiff_pred.pdf", pdf))
        .expect("TIFF-predicted FlateDecode image should extract");
    let bytes = single_extracted_image(&result, "flate_tiff_pred-p1-i1.png");
    let decoded = image::load_from_memory(&bytes)
        .expect("PNG should decode")
        .to_luma8();
    assert_eq!(decoded.dimensions(), (width as u32, height as u32));
    assert_eq!(
        decoded.into_raw(),
        raw,
        "TIFF predictor round trip must match"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_extract_errors_on_non_pdf_bytes() {
    let err = pdf_extract_images(&file_input("notes.pdf", b"this is not a pdf".to_vec()))
        .expect_err("non-PDF bytes should fail");
    assert!(err.contains("could not open PDF"), "got {err:?}");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pdf_extract_pulls_images_from_image_to_pdf_output() {
    // image_to_pdf (krilla) → PDF → pdf_extract_images (lopdf): a full
    // pure-Rust round trip with no PDFium in sight.
    let png = png_bytes(4, 4, [10, 20, 30, 255]);
    let zip = build_zip(&[("embed.png", &png)]);

    let pdf_result = image_to_pdf(&file_input("shots.zip", zip), MAX_MEDIA_OUTPUT_BYTES)
        .expect("PDF should be created");
    let pdf_bytes = file_bytes(&output_file(&pdf_result));

    let extract_result = pdf_extract_images(&file_input("shots.pdf", pdf_bytes))
        .expect("PDF should yield an embedded image");
    let (_file, entries) = output_zip_entries(&extract_result);
    let images = image_entries(&entries);
    assert!(!images.is_empty(), "at least one image should be extracted");

    let decoded = image::load_from_memory(&images[0].1).expect("extracted image should decode");
    assert_eq!((decoded.width(), decoded.height()), (4, 4));
}
