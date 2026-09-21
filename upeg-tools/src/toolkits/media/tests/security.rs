//! Regression tests for hostile media input.
//!
//! Every tool here takes a `File` from the `Http`/`Pwa`/`Ext` surfaces, so each
//! fixture below is a small, real, attacker-shaped payload — a PDF or zip whose
//! *numbers* are the attack. Two properties are asserted throughout:
//!
//! - a crafted document is answered with a clean `Err` or a skip note, never a
//!   panic and never a gigabyte allocation;
//! - a document we cannot decode faithfully produces no image at all, rather
//!   than a plausible-looking wrong one.
//!
//! The bombs are sized to *request* far more memory than the caps allow while
//! costing the test nothing: the point of each cap is that the request is
//! refused before anything is allocated, so a passing test is also a fast one.

use lopdf::{Object, dictionary};

use super::*;
use crate::toolkits::media::image_pdf::image_to_pdf_with_raster_budget;
use crate::toolkits::media::limits::{MAX_IMAGE_PIXELS, read_capped};

/// `usize::MAX / 3` on a 64-bit host: `/Colors 3 × /Columns` lands on exactly
/// `usize::MAX`, which is what let the product survive a `checked_mul`.
const STRIDE_OVERFLOW_COLUMNS: i64 = 6_148_914_691_236_517_205;

/// A `/Columns` that asks for a 3 GB predictor row (× `/Colors 3`) — large
/// enough to abort a wasm32 `memory.grow`, small enough to survive every
/// arithmetic check on the way there.
const HUGE_PREDICTOR_COLUMNS: i64 = 1 << 30;

/// The widest CCITT `/Columns` × `/Rows` a PDF can declare: 65535 × 65535 is
/// 4.29e9 pixels, and a decoder pads to `/Rows` with lines that read no input
/// at all — so ~4 bytes of stream ask for 4 GB.
const CCITT_BOMB_EXTENT: i64 = 65_535;

/// A `/Width` past `u32::MAX`: `width as u32` truncates it to 5 instead of
/// rejecting it, emitting a garbage 5-pixel-wide image.
const TRUNCATING_WIDTH: i64 = 4_294_967_301;

/// The number of colour components in `DeviceRGB`, as a `/DecodeParms /Colors`.
const RGB_PREDICTOR_COLORS: i64 = 3;

/// A PNG-predictor `/Predictor` value (10-15 all select PNG per-row filtering).
const PNG_PREDICTOR: i64 = 12;

const TEST_IMAGE_TO_PDF_RASTER_BYTES: usize = 2 * 256 * 256 * 4;

/// The skip notes of an extraction that must have produced no image at all.
fn skip_notes_only(result_json: &str) -> String {
    let (_file, entries) = output_zip_entries(result_json);
    assert!(
        image_entries(&entries).is_empty(),
        "a stream we cannot decode faithfully must yield no image, got {entries:?}"
    );
    let notes = entries
        .iter()
        .find(|(name, _)| name == EXTRACTION_NOTES_FILENAME)
        .expect("a skipped run must carry a notes entry");
    String::from_utf8_lossy(&notes.1).into_owned()
}

/// A single-page PDF whose one image declares the given predictor parameters
/// over an empty (but valid) FlateDecode stream.
fn pdf_with_predictor_parms(width: i64, colors: i64, columns: i64) -> Vec<u8> {
    build_image_pdf_fixture_with_parms(
        width,
        1,
        "DeviceRGB",
        "FlateDecode",
        Some(dictionary! {
            "Predictor" => PNG_PREDICTOR,
            "Colors" => colors,
            "Columns" => columns,
            "BitsPerComponent" => 8_i64,
        }),
        zlib_compress(&[]),
    )
}

/// A single-page PDF whose one image is a CCITTFaxDecode stream with the given
/// decode parameters.
fn pdf_with_ccitt_parms(extent: i64, parms: lopdf::Dictionary, content: Vec<u8>) -> Vec<u8> {
    build_image_pdf_fixture_with_parms(
        extent,
        extent,
        "DeviceGray",
        "CCITTFaxDecode",
        Some(parms),
        content,
    )
}

// ─── predictor stride: overflow + unbounded allocation ──────────────

#[test]
fn pdf_extract_skips_predictor_whose_stride_hits_usize_max() {
    // `/Colors 3 × /Columns 6148914691236517205` is exactly `usize::MAX`, so the
    // product passes `checked_mul`. The row length `stride + 1` then overflows
    // (debug) or wraps to zero and divides by zero (release) — this test must
    // therefore run under both profiles.
    let pdf = pdf_with_predictor_parms(1, RGB_PREDICTOR_COLORS, STRIDE_OVERFLOW_COLUMNS);

    let result = pdf_extract_images(&file_input("stride.pdf", pdf))
        .expect("a crafted predictor must skip the image, not fail the run");
    let notes = skip_notes_only(&result);

    assert!(
        notes.contains("Predictor Columns") && notes.contains("does not match image Width"),
        "got {notes:?}"
    );
}

#[test]
fn pdf_extract_does_not_allocate_row_buffer_for_huge_predictor_columns() {
    // The predictor's scratch rows are allocated from `/Columns × /Colors`
    // before any input byte is read, so a ~300-byte PDF with an empty stream
    // asks for 3 GB.
    let pdf = pdf_with_predictor_parms(1, RGB_PREDICTOR_COLORS, HUGE_PREDICTOR_COLUMNS);

    let result = pdf_extract_images(&file_input("huge_columns.pdf", pdf))
        .expect("an oversized predictor row must skip the image");
    let notes = skip_notes_only(&result);

    assert!(notes.contains("Predictor Columns"), "got {notes:?}");
}

#[test]
fn pdf_extract_skips_predictor_colors_mismatching_colorspace() {
    // `/Colors` that disagrees with the colourspace could only ever reconstruct
    // into a garbled raster, so it is refused rather than guessed at.
    let pdf = pdf_with_predictor_parms(1, RGB_PREDICTOR_COLORS + 1, 1);

    let result = pdf_extract_images(&file_input("colors.pdf", pdf))
        .expect("a mismatched Colors must skip the image");
    let notes = skip_notes_only(&result);

    assert!(notes.contains("Predictor Colors"), "got {notes:?}");
}

// ─── CCITT: pixel bombs and the coding we cannot follow ─────────────

#[test]
fn pdf_extract_skips_ccitt_rows_columns_bomb() {
    let pdf = pdf_with_ccitt_parms(
        CCITT_BOMB_EXTENT,
        dictionary! {
            "K" => -1_i64,
            "Columns" => CCITT_BOMB_EXTENT,
            "Rows" => CCITT_BOMB_EXTENT,
        },
        // A stream that hits end-of-data immediately: every one of the 65535
        // declared rows would be padded in with no input behind it.
        vec![0_u8; 4],
    );

    let result = pdf_extract_images(&file_input("ccitt_bomb.pdf", pdf))
        .expect("an oversized CCITT raster must skip the image");
    let notes = skip_notes_only(&result);

    assert!(
        notes.contains(&format!("{MAX_IMAGE_PIXELS}-pixel limit")),
        "got {notes:?}"
    );
}

// EOL-less Group 3 (`/K 0`), mixed 1-D/2-D (`/K > 0`) and `/EncodedByteAlign`
// used to be refused here, because the decoder in use could not follow them.
// `hayro-ccitt` decodes all three, so they are round-tripped in `super::ccitt`
// rather than refused.

#[test]
fn pdf_extract_does_not_write_failed_ccitt_decode_as_image() {
    // A G4 stream truncated mid-line: the rows recovered so far are a guess, so
    // the run must skip rather than write "the rows we got".
    let (width, height) = (16_u32, 4_u32);
    let rows: Vec<Vec<bool>> = (0..height)
        .map(|y| (0..width).map(|x| (x + y) % 3 == 0).collect())
        .collect();
    let mut encoded = ccitt_g4_encode(&rows);
    encoded.truncate(2);
    let pdf = pdf_with_ccitt_parms(
        i64::from(width),
        dictionary! {
            "K" => -1_i64,
            "Columns" => i64::from(width),
            "Rows" => i64::from(height),
        },
        encoded,
    );

    let result = pdf_extract_images(&file_input("ccitt_truncated.pdf", pdf))
        .expect("a truncated CCITT stream must not fail the run");
    let (_file, entries) = output_zip_entries(&result);

    assert!(
        image_entries(&entries).is_empty(),
        "a failed decode must never write an image, got {entries:?}"
    );
}

// ─── dimensions: rejected, not truncated ────────────────────────────

#[test]
fn pdf_extract_rejects_width_over_u32_instead_of_truncating() {
    let pdf = build_image_pdf_fixture(
        TRUNCATING_WIDTH,
        1,
        "DeviceGray",
        "FlateDecode",
        zlib_compress(&[0_u8; 8]),
    );

    let result = pdf_extract_images(&file_input("wide.pdf", pdf))
        .expect("an out-of-range Width must skip the image");
    let notes = skip_notes_only(&result);

    assert!(
        notes.contains(&format!("Width {TRUNCATING_WIDTH} out of range")),
        "got {notes:?}"
    );
}

#[test]
fn pdf_extract_skips_image_over_pixel_limit() {
    let extent = CCITT_BOMB_EXTENT;
    let pdf = build_image_pdf_fixture(
        extent,
        extent,
        "DeviceRGB",
        "FlateDecode",
        zlib_compress(&[0_u8; 8]),
    );

    let result = pdf_extract_images(&file_input("big.pdf", pdf))
        .expect("an oversized raster must skip the image");
    let notes = skip_notes_only(&result);

    assert!(
        notes.contains(&format!("{MAX_IMAGE_PIXELS}-pixel limit")),
        "got {notes:?}"
    );
}

// ─── /Decode: honoured or skipped, never ignored ────────────────────

#[test]
fn pdf_extract_skips_inverted_decode_array() {
    // `/Decode [1 0]` inverts DeviceGray. The reconstruction does not apply it,
    // so writing the image would hand back an inverted picture as if it were
    // the real one.
    let pdf = build_image_pdf_fixture_with_extras(
        2,
        1,
        "DeviceGray",
        "FlateDecode",
        None,
        &[(
            "Decode",
            Object::Array(vec![Object::Integer(1), Object::Integer(0)]),
        )],
        zlib_compress(&[0_u8, 255]),
    );

    let result = pdf_extract_images(&file_input("decode.pdf", pdf))
        .expect("a non-default /Decode must skip the image");
    let notes = skip_notes_only(&result);

    assert!(notes.contains("/Decode"), "got {notes:?}");
}

#[test]
fn pdf_extract_extracts_default_decode_array_verbatim() {
    // The identity `/Decode [0 1]` says exactly what the samples already mean,
    // so it must not turn a perfectly good image into a skip.
    let raw = [0_u8, 128, 255, 64];
    let pdf = build_image_pdf_fixture_with_extras(
        raw.len() as i64,
        1,
        "DeviceGray",
        "FlateDecode",
        None,
        &[(
            "Decode",
            Object::Array(vec![Object::Integer(0), Object::Integer(1)]),
        )],
        zlib_compress(&raw),
    );

    let result = pdf_extract_images(&file_input("identity.pdf", pdf))
        .expect("an identity /Decode should extract normally");
    let bytes = single_extracted_image(&result, "identity-p1-i1.png");
    let decoded = image::load_from_memory(&bytes)
        .expect("PNG should decode")
        .to_luma8();

    assert_eq!(decoded.into_raw(), raw);
}

// ─── pdf_to_images: page raster budget ──────────────────────────────

#[test]
fn pdf_to_images_rejects_huge_mediabox_page() {
    // The rasterizer sizes its pixmap from the page's own geometry and clamps
    // each axis into a u16, so an outsized /MediaBox quietly asks for
    // 65535×65535×4 = 17 GB.
    let extent = CCITT_BOMB_EXTENT;
    let pdf = build_image_pdf_fixture(
        extent,
        extent,
        "DeviceGray",
        "FlateDecode",
        zlib_compress(&[0_u8; 8]),
    );

    let error = pdf_to_images(
        &file_input("huge.pdf", pdf),
        PDF_TO_IMAGES_MIN_DPI,
        MAX_MEDIA_OUTPUT_BYTES,
    )
    .expect_err("an oversized page must be refused");

    assert!(
        error.contains(&format!("{MAX_IMAGE_PIXELS}-pixel limit")),
        "got {error:?}"
    );
}

#[test]
fn image_to_pdf_rejects_when_compressed_images_exceed_cumulative_raster_budget() {
    // Given: three individually valid, highly compressible PNGs whose decoded
    // RGBA8 rasters need three pages while the test budget holds only two.
    let png = png_bytes(256, 256, [0, 0, 0, 0]);
    let zip = build_zip(&[
        ("a.png", png.as_slice()),
        ("b.png", png.as_slice()),
        ("c.png", png.as_slice()),
    ]);

    // When
    let error = image_to_pdf_with_raster_budget(
        &file_input("compressed.zip", zip),
        TEST_IMAGE_TO_PDF_RASTER_BYTES,
        MAX_MEDIA_OUTPUT_BYTES,
    )
    .expect_err("the cumulative decoded raster budget must be enforced");

    // Then
    assert!(
        error.contains("decoded rasters require 786432 bytes")
            && error.contains(&format!(
                "over the {TEST_IMAGE_TO_PDF_RASTER_BYTES}-byte limit"
            )),
        "got {error:?}"
    );
}

/// The refactor that pulled the budget accumulator out of the zip loader
/// claims all three input shapes share the same limit, so the two non-zip
/// paths get regression guards too. Without this test, folding the
/// accumulator back into a single path would break nothing.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_applies_cumulative_raster_budget_to_directory_input() {
    // Given: the same image set as the zip fixture, passed as a directory.
    let png = png_bytes(256, 256, [0, 0, 0, 0]);
    let dir_input = FileValue {
        name: "compressed".to_string(),
        content: FileContent::Directory(vec![
            file_input("a.png", png.clone()),
            file_input("b.png", png.clone()),
            file_input("c.png", png),
        ]),
        mime: None,
    };

    // When
    let error = image_to_pdf_with_raster_budget(
        &dir_input,
        TEST_IMAGE_TO_PDF_RASTER_BYTES,
        MAX_MEDIA_OUTPUT_BYTES,
    )
    .expect_err("the directory path must share the cumulative raster budget");

    // Then
    assert!(
        error.contains("decoded rasters require 786432 bytes")
            && error.contains(&format!(
                "over the {TEST_IMAGE_TO_PDF_RASTER_BYTES}-byte limit"
            )),
        "got {error:?}"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_applies_raster_budget_to_single_image_input() {
    // Given: a single image that alone exceeds the test budget.
    let png = png_bytes(512, 512, [0, 0, 0, 0]);

    // When
    let error = image_to_pdf_with_raster_budget(
        &file_input("huge.png", png),
        TEST_IMAGE_TO_PDF_RASTER_BYTES,
        MAX_MEDIA_OUTPUT_BYTES,
    )
    .expect_err("the single-image path must be charged against the raster budget");

    // Then
    assert!(
        error.contains(&format!(
            "over the {TEST_IMAGE_TO_PDF_RASTER_BYTES}-byte limit"
        )),
        "got {error:?}"
    );
}

/// A zip's central directory sits at the **end** of the file, so a zip
/// reader still opens an archive with arbitrary leading data. Sniffing a
/// magic prefix would silently mistake such an archive for an image and
/// break conversions that used to work.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_reads_zip_with_leading_data_as_archive() {
    let png = png_bytes(3, 2, [255, 0, 0, 255]);
    let mut bytes = b"MZ\x90\x00self-extracting-stub".to_vec();
    bytes.extend_from_slice(&build_zip(&[("photo.png", &png)]));
    assert!(
        !bytes.starts_with(b"PK"),
        "if the fixture starts with PK the test misses what it is checking"
    );

    let result = image_to_pdf(&file_input("sfx.zip", bytes), MAX_MEDIA_OUTPUT_BYTES)
        .expect("leading data must not stop the archive from opening");

    assert!(file_bytes(&output_file(&result)).starts_with(b"%PDF-"));
}

/// An archive opens even when its name is not `.zip` — detection is whether
/// `ZipArchive::new` succeeds and never looks at the name.
///
/// This test calls the tool function directly, so it **does not pass through
/// the declared policy**: re-declaring `extensions` leaves this test green.
/// Keeping the policy empty is the job of `fixtures/interface-inventory.json`
/// and CI; what is pinned here is the loader behaviour beneath it.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_reads_archive_whose_name_is_not_zip() {
    let png = png_bytes(3, 2, [255, 0, 0, 255]);
    let zip = build_zip(&[("photo.png", &png)]);

    let result = image_to_pdf(&file_input("album.dat", zip), MAX_MEDIA_OUTPUT_BYTES)
        .expect("an archive whose name is not .zip must still open");

    assert!(file_bytes(&output_file(&result)).starts_with(b"%PDF-"));
}

/// A `.zip` name that will not open surfaces the archive error. The
/// extension only chooses the error wording, not which loader runs.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_to_pdf_reports_archive_error_for_unopenable_zip_name() {
    let error = image_to_pdf(
        &file_input("broken.zip", b"PK\x03\x04not-really-an-archive".to_vec()),
        MAX_MEDIA_OUTPUT_BYTES,
    )
    .expect_err("a .zip that will not open must report the archive error");

    assert!(error.contains("could not read input zip"), "got {error:?}");
}

// ─── the byte cap itself ────────────────────────────────────────────

#[test]
fn read_capped_rejects_input_over_the_cap() {
    const CAP: usize = 4;

    let error = read_capped(&b"12345"[..], CAP, "test input")
        .expect_err("input past the cap must be refused");

    assert!(
        error.contains(&format!("exceeds the {CAP}-byte limit")),
        "got {error:?}"
    );
}

#[test]
fn read_capped_returns_input_up_to_the_cap() {
    const CAP: usize = 4;

    let read = read_capped(&b"1234"[..], CAP, "test input").expect("input at the cap is fine");

    assert_eq!(read, b"1234");
}
