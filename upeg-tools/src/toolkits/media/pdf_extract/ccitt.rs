//! CCITTFaxDecode (ITU-T T.4 Group 3 / T.6 Group 4) support for
//! [`pdf_extract`](super), split into its own file to keep the parent inside
//! the workspace's 1000-line budget.
//!
//! `hayro-ccitt` does the decoding; its [`DecodeSettings`] is close enough to
//! PDF's `/DecodeParms` (Table 11) that [`ccitt_params`] is essentially a
//! transliteration. What this module adds on top is the pair of guarantees the
//! parent module makes about untrusted input:
//!
//! - the pixel budget, carried into [`GrayRaster`] because the decoder pushes
//!   pixels rather than returning a buffer we could have sized up front;
//! - the reject-don't-corrupt rule: a [`DecodeError`] skips the image, where
//!   `hayro-ccitt`'s own PDF filter keeps the rows it managed to decode.

use hayro_ccitt::{DecodeError, DecodeSettings, Decoder, DecoderContext, EncodingMode};
use lopdf::{Dictionary, Document};

use super::{
    ExtractionOutcome, ImageLocation, KEY_COLUMNS, KEY_HEIGHT, PDF_EMBEDDED_IMAGE_PNG_EXT,
    WriteContext, checked_dimension, checked_pixel_area, decode_parms_bool, decode_parms_i64,
    dict_i64, encode_png, image_entry_name, skip_note,
};
use crate::toolkits::media::limits::{MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS};

// CCITTFaxDecode parameters (PDF 32000-1:2008 Table 11) and their spec
// defaults.
const KEY_ROWS: &[u8] = b"Rows";
const KEY_K: &[u8] = b"K";
const KEY_BLACK_IS_1: &[u8] = b"BlackIs1";
const KEY_ENCODED_BYTE_ALIGN: &[u8] = b"EncodedByteAlign";
const KEY_END_OF_LINE: &[u8] = b"EndOfLine";
const KEY_END_OF_BLOCK: &[u8] = b"EndOfBlock";
const CCITT_DEFAULT_COLUMNS: i64 = 1728;
const CCITT_DEFAULT_K: i64 = 0;
const CCITT_DEFAULT_END_OF_LINE: bool = false;
const CCITT_DEFAULT_END_OF_BLOCK: bool = true;
const CCITT_DEFAULT_ENCODED_BYTE_ALIGN: bool = false;
const CCITT_DEFAULT_BLACK_IS_1: bool = false;
/// `/Rows` is optional; a non-positive value means "unset, use `/Height`".
const CCITT_ROWS_UNSET: i64 = 0;

// Grey levels an 8-bit output pixel takes for each decoded colour.
const CCITT_GRAY_WHITE: u8 = 255;
const CCITT_GRAY_BLACK: u8 = 0;

/// Pixels in one [`Decoder::push_pixel_chunk`] chunk, fixed by that trait.
const CCITT_CHUNK_PIXELS: usize = 8;

/// Map `/K` onto the coding the stream uses (PDF Table 11).
fn ccitt_encoding(k: i64) -> Result<EncodingMode, String> {
    if k < CCITT_DEFAULT_K {
        return Ok(EncodingMode::Group4);
    }
    if k == CCITT_DEFAULT_K {
        return Ok(EncodingMode::Group3_1D);
    }
    // `/K > 0` is mixed 1-D/2-D: each line carries a tag bit saying which.
    let k = u32::try_from(k).map_err(|_| format!("CCITT K {k} out of range"))?;
    Ok(EncodingMode::Group3_2D { k })
}

/// Read and validate the `/DecodeParms` a CCITT stream is decoded with.
fn ccitt_params(document: &Document, dict: &Dictionary) -> Result<DecodeSettings, String> {
    let columns = checked_dimension(
        "CCITT Columns",
        decode_parms_i64(document, dict, KEY_COLUMNS, CCITT_DEFAULT_COLUMNS),
    )?;
    Ok(DecodeSettings {
        columns,
        rows: ccitt_rows(document, dict, columns)?,
        end_of_block: decode_parms_bool(
            document,
            dict,
            KEY_END_OF_BLOCK,
            CCITT_DEFAULT_END_OF_BLOCK,
        ),
        end_of_line: decode_parms_bool(document, dict, KEY_END_OF_LINE, CCITT_DEFAULT_END_OF_LINE),
        rows_are_byte_aligned: decode_parms_bool(
            document,
            dict,
            KEY_ENCODED_BYTE_ALIGN,
            CCITT_DEFAULT_ENCODED_BYTE_ALIGN,
        ),
        encoding: ccitt_encoding(decode_parms_i64(document, dict, KEY_K, CCITT_DEFAULT_K))?,
        // `hayro-ccitt` folds `/BlackIs1` into the colour it reports, so the
        // flag maps straight across and `GrayRaster` needs no polarity of its
        // own: with `/BlackIs1 false` (the PDF default, 0 = black) a visually
        // black run arrives as `white = false`.
        invert_black: decode_parms_bool(document, dict, KEY_BLACK_IS_1, CCITT_DEFAULT_BLACK_IS_1),
    })
}

/// The row count to decode: the explicit `/Rows`, else the image `/Height`.
///
/// Both are attacker-supplied halves of a `/Columns` × `/Rows` allocation, so
/// the declared raster is measured against the caps here rather than after the
/// decoder has filled a 4 GB buffer from ~4 bytes of input. When the stream
/// declares neither, [`budget_rows`] stands in: `rows` is what stops the
/// decoder, so it cannot be left open-ended.
fn ccitt_rows(document: &Document, dict: &Dictionary, columns: u32) -> Result<u32, String> {
    let declared = decode_parms_i64(document, dict, KEY_ROWS, CCITT_ROWS_UNSET);
    let rows = if declared > CCITT_ROWS_UNSET {
        Some(declared)
    } else {
        dict_i64(document, dict, KEY_HEIGHT).filter(|&height| height > 0)
    };
    let Some(rows) = rows else {
        return Ok(budget_rows(columns));
    };
    let rows = checked_dimension("CCITT Rows", rows)?;
    checked_pixel_area("CCITT image", columns, rows)?;
    Ok(rows)
}

/// How many rows of `columns` pixels [`MAX_IMAGE_PIXELS`] affords, capped to
/// the tallest raster we accept. `columns` is always at least 1, having come
/// from [`checked_dimension`].
fn budget_rows(columns: u32) -> u32 {
    let rows = MAX_IMAGE_PIXELS / columns as usize;
    u32::try_from(rows)
        .unwrap_or(MAX_IMAGE_DIMENSION)
        .min(MAX_IMAGE_DIMENSION)
}

/// Collects the decoder's pixels into an 8-bit grayscale raster, refusing to
/// grow past [`MAX_IMAGE_PIXELS`].
///
/// The budget has to live in this push path, not only in [`ccitt_params`]:
/// `/Columns` × `/Rows` bounds a raster the stream *declares*, but a decoder
/// also pads rows out to `/Columns` from input it never read, so what is
/// pushed is not bounded by the stream's length. `Decoder`'s methods cannot
/// stop the decode, so an over-budget raster stops being collected and is
/// reported afterwards.
struct GrayRaster {
    pixels: Vec<u8>,
    over_budget: bool,
}

impl GrayRaster {
    fn new() -> Self {
        Self {
            pixels: Vec::new(),
            over_budget: false,
        }
    }

    fn extend(&mut self, white: bool, count: usize) {
        if self.over_budget {
            return;
        }
        if self.pixels.len().saturating_add(count) > MAX_IMAGE_PIXELS {
            self.over_budget = true;
            return;
        }
        let level = if white {
            CCITT_GRAY_WHITE
        } else {
            CCITT_GRAY_BLACK
        };
        self.pixels.extend(std::iter::repeat_n(level, count));
    }
}

impl Decoder for GrayRaster {
    fn push_pixel(&mut self, white: bool) {
        self.extend(white, 1);
    }

    fn push_pixel_chunk(&mut self, white: bool, chunk_count: u32) {
        // Saturating, not wrapping: `usize` is 32-bit on wasm32, and a count
        // that wrapped would come out *smaller* and slip under the budget.
        self.extend(
            white,
            (chunk_count as usize).saturating_mul(CCITT_CHUNK_PIXELS),
        );
    }

    fn next_line(&mut self) {}
}

/// Decode a CCITTFaxDecode (Group 3/4) bilevel stream to an 8-bit grayscale PNG.
pub(super) fn write_ccitt(
    document: &Document,
    dict: &Dictionary,
    content: &[u8],
    location: ImageLocation,
    context: &WriteContext,
    outcome: &mut ExtractionOutcome,
) -> Result<(), String> {
    let settings = match ccitt_params(document, dict) {
        Ok(settings) => settings,
        Err(reason) => {
            outcome.skipped.push(skip_note(location, &reason));
            return Ok(());
        }
    };
    let columns = settings.columns as usize;

    let mut raster = GrayRaster::new();
    let decoded = hayro_ccitt::decode(content, &mut raster, &mut DecoderContext::new(settings));

    if raster.over_budget {
        outcome.skipped.push(skip_note(
            location,
            &format!("CCITT image exceeds the {MAX_IMAGE_PIXELS}-pixel limit"),
        ));
        return Ok(());
    }
    if let Err(error) = decoded {
        // A failed decode means the recovered rows are only a guess at what the
        // stream said: skip, never write. Corrupt output is the one thing this
        // module promises not to produce — which is where we part company with
        // `hayro-ccitt`'s own filter, since it keeps the truncated rows.
        outcome
            .skipped
            .push(skip_note(location, &ccitt_failure(&settings, error)));
        return Ok(());
    }
    let Ok(height) = u32::try_from(raster.pixels.len() / columns) else {
        outcome
            .skipped
            .push(skip_note(location, "CCITT decode produced too many rows"));
        return Ok(());
    };
    if height == 0 {
        outcome
            .skipped
            .push(skip_note(location, &ccitt_no_rows(&settings)));
        return Ok(());
    }
    raster.pixels.truncate(height as usize * columns);

    let Some(image) = image::GrayImage::from_raw(settings.columns, height, raster.pixels) else {
        outcome
            .skipped
            .push(skip_note(location, "failed to build CCITT image buffer"));
        return Ok(());
    };
    let bytes = encode_png(
        &image::DynamicImage::ImageLuma8(image),
        outcome.remaining_encoded_bytes(),
    )?;
    outcome.push_image(
        image_entry_name(context.pdf_stem, location, PDF_EMBEDDED_IMAGE_PNG_EXT),
        bytes,
    )
}

/// Skip reason for a stream the decoder could not follow.
fn ccitt_failure(settings: &DecodeSettings, error: DecodeError) -> String {
    format!(
        "CCITT decode failed ({error}, {})",
        ccitt_coding_label(settings)
    )
}

/// Skip reason for a stream that decoded cleanly but yielded nothing to write.
fn ccitt_no_rows(settings: &DecodeSettings) -> String {
    format!(
        "CCITT decode produced no complete row ({})",
        ccitt_coding_label(settings)
    )
}

/// How a stream's coding reads in a skip note.
fn ccitt_coding_label(settings: &DecodeSettings) -> String {
    match settings.encoding {
        EncodingMode::Group4 => "Group 4".to_owned(),
        EncodingMode::Group3_1D => "Group 3 1-D".to_owned(),
        EncodingMode::Group3_2D { k } => format!("Group 3 mixed 1-D/2-D, K={k}"),
    }
}
