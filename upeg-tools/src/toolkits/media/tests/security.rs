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
fn pdf_추출은_stride가_usize_최대가_되는_predictor를_패닉_없이_건너뛴다() {
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
fn pdf_추출은_거대한_predictor_columns에_행_버퍼를_할당하지_않는다() {
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
fn pdf_추출은_colorspace와_어긋난_predictor_colors를_건너뛴다() {
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
fn pdf_추출은_ccitt_rows_columns_폭탄을_건너뛴다() {
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
fn pdf_추출은_디코드에_실패한_ccitt를_이미지로_쓰지_않는다() {
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
fn pdf_추출은_u32를_넘는_width를_절단하지_않고_거부한다() {
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
fn pdf_추출은_픽셀_상한을_넘는_이미지를_건너뛴다() {
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
fn pdf_추출은_반전된_decode_배열을_건너뛴다() {
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
fn pdf_추출은_기본_decode_배열을_그대로_추출한다() {
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
fn pdf_to_이미지는_거대한_mediabox_페이지를_거부한다() {
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
fn 이미지_to_pdf는_개별_상한_안의_압축_이미지들이_누적_래스터_예산을_넘으면_거부한다() {
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

/// 예산 누적기를 zip 로더 밖으로 끌어낸 리팩터의 핵심 주장이 "세 입력 모양이
/// 같은 상한을 받는다"는 것이므로, zip 말고 두 경로에도 회귀 가드를 둔다.
/// 이 테스트가 없으면 누적기를 다시 한 경로 안으로 되돌려도 아무것도 깨지지
/// 않는다.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn 이미지_to_pdf는_디렉터리_입력에도_누적_래스터_예산을_적용한다() {
    // Given: zip 판의 픽스처와 같은 이미지 셋을 디렉터리로 넘긴다.
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
fn 이미지_to_pdf는_단일_이미지_입력에도_래스터_예산을_적용한다() {
    // Given: 한 장만으로도 테스트 예산을 넘는 이미지.
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

/// zip의 중앙 디렉터리는 파일 **끝**에 있으므로, 앞에 임의의 데이터가 붙어
/// 있어도 zip 리더는 아카이브를 연다. 매직 접두어로 판별하면 이런 아카이브를
/// 조용히 이미지로 오인해 예전에 되던 변환을 멈추게 된다.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn 이미지_to_pdf는_선행_데이터가_붙은_zip도_아카이브로_읽는다() {
    let png = png_bytes(3, 2, [255, 0, 0, 255]);
    let mut bytes = b"MZ\x90\x00self-extracting-stub".to_vec();
    bytes.extend_from_slice(&build_zip(&[("photo.png", &png)]));
    assert!(
        !bytes.starts_with(b"PK"),
        "픽스처가 PK로 시작하면 이 테스트가 검증하려는 것을 놓친다"
    );

    let result = image_to_pdf(&file_input("sfx.zip", bytes), MAX_MEDIA_OUTPUT_BYTES)
        .expect("leading data must not stop the archive from opening");

    assert!(file_bytes(&output_file(&result)).starts_with(b"%PDF-"));
}

/// 이름이 `.zip`이 아니어도 아카이브로 열린다 — 판별이 `ZipArchive::new`의 성공
/// 여부이고 이름을 보지 않기 때문이다.
///
/// 이 테스트는 도구 함수를 직접 부르므로 **선언된 정책을 지나지 않는다.** 즉
/// `extensions`를 다시 선언해도 이 테스트는 초록이다. 정책이 비어 있다는 것을
/// 지키는 것은 `fixtures/interface-inventory.json`과 CI이고, 여기서 고정하는
/// 것은 그 위의 로더 동작이다.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn 이미지_to_pdf는_이름이_zip이_아닌_아카이브도_읽는다() {
    let png = png_bytes(3, 2, [255, 0, 0, 255]);
    let zip = build_zip(&[("photo.png", &png)]);

    let result = image_to_pdf(&file_input("album.dat", zip), MAX_MEDIA_OUTPUT_BYTES)
        .expect("an archive whose name is not .zip must still open");

    assert!(file_bytes(&output_file(&result)).starts_with(b"%PDF-"));
}

/// 이름이 `.zip`인데 열리지 않으면 아카이브 오류를 보여 준다. 확장자는 오류
/// 문구를 고르는 데만 쓰이고 어떤 로더가 도는지는 정하지 않는다.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn 이미지_to_pdf는_열리지_않는_zip_이름에_아카이브_오류를_낸다() {
    let error = image_to_pdf(
        &file_input("broken.zip", b"PK\x03\x04not-really-an-archive".to_vec()),
        MAX_MEDIA_OUTPUT_BYTES,
    )
    .expect_err("a .zip that will not open must report the archive error");

    assert!(error.contains("could not read input zip"), "got {error:?}");
}

// ─── the byte cap itself ────────────────────────────────────────────

#[test]
fn 상한_읽기는_상한을_넘는_입력을_거부한다() {
    const CAP: usize = 4;

    let error = read_capped(&b"12345"[..], CAP, "test input")
        .expect_err("input past the cap must be refused");

    assert!(
        error.contains(&format!("exceeds the {CAP}-byte limit")),
        "got {error:?}"
    );
}

#[test]
fn 상한_읽기는_상한까지의_입력을_그대로_돌려준다() {
    const CAP: usize = 4;

    let read = read_capped(&b"1234"[..], CAP, "test input").expect("input at the cap is fine");

    assert_eq!(read, b"1234");
}
