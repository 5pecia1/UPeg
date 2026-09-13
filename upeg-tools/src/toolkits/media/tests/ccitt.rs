//! Round-trip tests for CCITTFaxDecode (`media/pdf_extract/ccitt.rs`).
//!
//! Each test builds a bilevel pattern, encodes it with [`ccitt_encode`] under
//! one `/DecodeParms` shape, embeds it in a PDF, and asserts the extracted PNG
//! is the pattern it started as. The encoder is an independent implementation
//! of T.4/T.6, so these pin the whole `/DecodeParms` mapping, not just that the
//! decoder is self-consistent.
//!
//! Every coding here except Group 4 was refused outright by the decoder that
//! `hayro-ccitt` replaced.

use lopdf::dictionary;

use super::ccitt_encode::{CcittEncoder, Coding};
use super::*;

/// The 8-bit grey levels a decoded CCITT pixel takes.
const CCITT_LUMA_WHITE: u8 = 255;
const CCITT_LUMA_BLACK: u8 = 0;

/// Fixture extent. The width stays inside the terminating-code runs the test
/// encoder implements, and is deliberately not a multiple of 8 so that
/// `/EncodedByteAlign` has padding to actually skip.
const FIXTURE_WIDTH: u32 = 20;
const FIXTURE_HEIGHT: u32 = 6;

/// `/K` for a mixed 1-D/2-D stream: every 3rd line is 1-D coded.
const MIXED_K: i64 = 3;

/// A pattern of bands that shift one pixel per row, so a 2-D coding has to use
/// vertical and horizontal modes rather than repeating the reference line.
fn fixture_rows() -> Vec<Vec<bool>> {
    (0..FIXTURE_HEIGHT)
        .map(|y| (0..FIXTURE_WIDTH).map(|x| (x + y) % 5 < 2).collect())
        .collect()
}

/// The grey samples `rows` (true = black) must decode to.
fn expected_luma(rows: &[Vec<bool>]) -> Vec<u8> {
    rows.iter()
        .flat_map(|row| {
            row.iter().map(|&black| {
                if black {
                    CCITT_LUMA_BLACK
                } else {
                    CCITT_LUMA_WHITE
                }
            })
        })
        .collect()
}

/// Embed `encoded` as the one CCITTFaxDecode image of a PDF and return the
/// grayscale samples `pdf_extract_images` gets back out of it.
fn extracted_luma(stem: &str, parms: lopdf::Dictionary, encoded: Vec<u8>) -> Vec<u8> {
    let pdf = build_image_pdf_fixture_with_parms(
        i64::from(FIXTURE_WIDTH),
        i64::from(FIXTURE_HEIGHT),
        "DeviceGray",
        "CCITTFaxDecode",
        Some(parms),
        encoded,
    );
    let result = pdf_extract_images(&file_input(&format!("{stem}.pdf"), pdf))
        .expect("a CCITT image we can decode should extract");
    let bytes = single_extracted_image(&result, &format!("{stem}-p1-i1.png"));
    let decoded = image::load_from_memory(&bytes)
        .expect("PNG should decode")
        .to_luma8();
    assert_eq!(decoded.dimensions(), (FIXTURE_WIDTH, FIXTURE_HEIGHT));
    decoded.into_raw()
}

/// The `/DecodeParms` every fixture shares, on top of which each test sets the
/// one parameter it is about.
fn base_parms(k: i64) -> lopdf::Dictionary {
    dictionary! {
        "K" => k,
        "Columns" => i64::from(FIXTURE_WIDTH),
        "Rows" => i64::from(FIXTURE_HEIGHT),
    }
}

#[test]
fn ccitt_group4_스트림은_원본_픽셀로_돌아온다() {
    let rows = fixture_rows();
    let encoded = CcittEncoder {
        coding: Coding::Group4,
        byte_align: false,
        emit_eol: false,
    }
    .encode(&rows);

    let luma = extracted_luma("ccitt_g4", base_parms(-1), encoded);

    assert_eq!(luma, expected_luma(&rows), "G4 round trip must match");
}

#[test]
fn ccitt_eol_없는_group3_1d_스트림은_원본_픽셀로_돌아온다() {
    // `/K 0` with `/EndOfLine` left at its `false` default — the shape the old
    // decoder refused, because it ate an EOL that was not there and so
    // desynchronized from bit zero.
    let rows = fixture_rows();
    let encoded = CcittEncoder {
        coding: Coding::Group3OneDimensional,
        byte_align: false,
        emit_eol: false,
    }
    .encode(&rows);

    let luma = extracted_luma("ccitt_g3", base_parms(0), encoded);

    assert_eq!(luma, expected_luma(&rows), "G3 1-D round trip must match");
}

#[test]
fn ccitt_eol_붙은_group3_1d_스트림도_원본_픽셀로_돌아온다() {
    // The other side of `/EndOfLine`: the same rows, this time with an EOL
    // before every line and `/EndOfLine true` to declare it.
    //
    // Note this pins the stream shape, not the flag. `hayro-ccitt` 0.3 takes
    // `end_of_line` in its settings but never reads it — it consumes EOLs
    // whenever it finds them, on the grounds that PDF producers set the flag
    // unreliably. So both shapes decode either way; `/EndOfLine` is mapped for
    // when that changes.
    let rows = fixture_rows();
    let encoded = CcittEncoder {
        coding: Coding::Group3OneDimensional,
        byte_align: false,
        emit_eol: true,
    }
    .encode(&rows);
    let mut parms = base_parms(0);
    parms.set("EndOfLine", true);

    let luma = extracted_luma("ccitt_g3_eol", parms, encoded);

    assert_eq!(luma, expected_luma(&rows), "G3 1-D with EOL must match");
}

#[test]
fn ccitt_혼합_1d_2d_group3_스트림은_원본_픽셀로_돌아온다() {
    // `/K > 0`: each line carries a tag bit choosing 1-D or 2-D. Previously
    // refused outright as "not implemented".
    let rows = fixture_rows();
    let encoded = CcittEncoder {
        coding: Coding::Group3Mixed { k: MIXED_K as u32 },
        byte_align: false,
        emit_eol: false,
    }
    .encode(&rows);

    let luma = extracted_luma("ccitt_mixed", base_parms(MIXED_K), encoded);

    assert_eq!(luma, expected_luma(&rows), "G3 mixed round trip must match");
}

#[test]
fn ccitt_encoded_byte_align_스트림은_원본_픽셀로_돌아온다() {
    // `/EncodedByteAlign true` pads every row out to a byte boundary. The
    // fixture width is not a multiple of 8, so the padding is real: a decoder
    // ignoring the flag would read it as image data. Previously refused.
    let rows = fixture_rows();
    let encoded = CcittEncoder {
        coding: Coding::Group4,
        byte_align: true,
        emit_eol: false,
    }
    .encode(&rows);
    let mut parms = base_parms(-1);
    parms.set("EncodedByteAlign", true);

    let luma = extracted_luma("ccitt_aligned", parms, encoded);

    assert_eq!(
        luma,
        expected_luma(&rows),
        "EncodedByteAlign round trip must match"
    );
}

#[test]
fn ccitt_black_is_1은_출력_명암을_뒤집는다() {
    // `/BlackIs1 true` says the coded black runs are the 1 samples, and 1 is
    // white in DeviceGray — so the same stream must come back inverted. This is
    // the polarity check on `invert_black`.
    let rows = fixture_rows();
    let encoded = CcittEncoder {
        coding: Coding::Group4,
        byte_align: false,
        emit_eol: false,
    }
    .encode(&rows);
    let mut parms = base_parms(-1);
    parms.set("BlackIs1", true);

    let luma = extracted_luma("ccitt_blackis1", parms, encoded);

    let inverted: Vec<u8> = expected_luma(&rows).iter().map(|&v| !v).collect();
    assert_eq!(luma, inverted, "BlackIs1 must invert the decoded polarity");
    assert_ne!(
        luma,
        expected_luma(&rows),
        "the fixture must not be symmetric, or the test proves nothing"
    );
}
