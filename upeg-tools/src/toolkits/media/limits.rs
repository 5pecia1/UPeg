//! Resource caps for the media tools' untrusted inputs.
//!
//! Every media tool is exposed on the `Http`/`Pwa`/`Ext` surfaces, so a `File`
//! input is attacker-supplied: a few hundred bytes of crafted PDF or zip can
//! ask for gigabytes (decompression bombs, `/Width`×`/Height` bombs, predictor
//! rows sized `usize::MAX`). Nothing below these tools bounds an allocation, so
//! this module is the outermost defense — and on wasm32 an over-large request
//! is not a slow response but a failed `memory.grow`, i.e. an aborted tab.
//!
//! Every cap is a named constant so the whole policy reads in one place; the
//! helpers here are the only sanctioned way to size a raster or to pull bytes
//! out of a decoder.

use std::io::{Cursor, Read, Seek, SeekFrom, Write};

/// Upper bound on either axis of a raster the media tools decode, reconstruct
/// or rasterize. `u16::MAX` is the widest extent the rasterizer (whose pixmap
/// is `u16`-sized) and the PNG encoder handle in practice, so nothing that
/// could have been produced correctly is lost by rejecting more.
pub(super) const MAX_IMAGE_DIMENSION: u32 = 65_535;

/// Upper bound on a raster's pixel count (width × height). The largest
/// legitimate raster we serve is a page rendered at [`PDF_TO_IMAGES_MAX_DPI`]
/// (600) — an A4/Letter sheet at ~35M pixels — so this sits just above it,
/// while a `/Columns 65535 /Rows 65535` CCITT bomb (4.29e9 pixels from ~4
/// bytes of input) is refused.
///
/// [`PDF_TO_IMAGES_MAX_DPI`]: super::PDF_TO_IMAGES_MAX_DPI
pub(super) const MAX_IMAGE_PIXELS: usize = 40 << 20;

/// [`MAX_IMAGE_PIXELS`] as an `f64`, for the rasterizer's floating-point page
/// geometry. The value is far below 2^53, so the conversion is exact.
#[expect(
    clippy::cast_precision_loss,
    reason = "MAX_IMAGE_PIXELS is far below 2^53, so the f64 value is exact"
)]
pub(super) const MAX_IMAGE_PIXELS_F64: f64 = MAX_IMAGE_PIXELS as f64;

/// Upper bound on a single decompressed/decoded buffer: inflate or LZW output,
/// one archive entry, one decoded image. Deflate reaches ~1032:1, so without a
/// cap a 10 MB stream inflates to ~10 GB. Sized to still hold the raw samples
/// of a [`MAX_IMAGE_PIXELS`] RGB raster (~120 MB).
pub(super) const MAX_DECODED_BYTES: usize = 192 << 20;

pub(super) const MAX_ENCODED_OUTPUT_BYTES: usize = 64 << 20;

pub(super) const MAX_PDF_EXTRACTED_IMAGES: usize = 100;
pub(super) const MAX_PDF_FORM_DEPTH: usize = 64;
pub(super) const MAX_PDF_EXTRACTED_IMAGE_BYTES: usize = 64 << 20;
pub(super) const MAX_PDF_EXTRACT_ZIP_BYTES: usize = MAX_ENCODED_OUTPUT_BYTES;

/// Upper bound on the total bytes read out of one input archive, so that a zip
/// of many individually-legal entries cannot add up to a bomb.
pub(super) const MAX_ARCHIVE_TOTAL_BYTES: u64 = 512 << 20;

/// Upper bound on the RGBA8 rasters retained while `image_to_pdf` assembles its
/// pages. This is independent of archive bytes because highly compressible
/// images can occupy little input space while decoding to hundreds of MiB.
pub(super) const MAX_IMAGE_TO_PDF_RASTER_BYTES: usize = 256 << 20;

/// Upper bound on how many pages one PDF may rasterize into (or one image zip
/// may become): every page is held in memory until the output zip is packed.
pub(super) const MAX_PDF_PAGES: usize = 2_000;

/// Decoder limits handed to the `image` crate. Its default limits leave
/// `max_image_width`/`max_image_height` unset, so a ~1 KB PNG declaring
/// 65535×65535 makes `to_rgba8` ask for 17 GB.
pub(super) fn image_decode_limits() -> image::Limits {
    // `Limits` is `#[non_exhaustive]`, so start from the no-limits value and
    // set what we cap; any field the crate adds later stays at its default.
    let mut limits = image::Limits::no_limits();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODED_BYTES as u64);
    limits
}

/// Read `reader` to its end, refusing at `cap` bytes rather than growing the
/// buffer to whatever the input claims. `what` names the source in the error.
///
/// The cap is passed in (rather than read from this module) so each call site
/// states the budget it is spending; `Read::take` makes the bound real, not a
/// check applied after the allocation already happened.
pub(super) fn read_capped(reader: impl Read, cap: usize, what: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    reader
        .take(cap as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("could not read {what}: {e}"))?;
    if out.len() > cap {
        return Err(over_cap_error(what, cap));
    }
    Ok(out)
}

/// Error text for input that blew past a byte cap.
pub(super) fn over_cap_error(what: &str, cap: usize) -> String {
    format!("{what} exceeds the {cap}-byte limit")
}

pub(super) fn validate_max_output_bytes(value: usize) -> Result<usize, String> {
    if !(1..=MAX_ENCODED_OUTPUT_BYTES).contains(&value) {
        return Err(format!(
            "max_output_bytes must be between 1 and {MAX_ENCODED_OUTPUT_BYTES}, got {value}"
        ));
    }
    Ok(value)
}

/// Validate one axis of a raster declared by untrusted input.
///
/// Rejects rather than truncates: `value as u32` turns `/Width 4294967301`
/// into a 5-pixel-wide garbage image instead of a skip, and an unbounded axis
/// is one half of a `width × height` allocation bomb.
pub(super) fn checked_dimension(label: &str, value: i64) -> Result<u32, String> {
    u32::try_from(value)
        .ok()
        .filter(|&dimension| dimension > 0 && dimension <= MAX_IMAGE_DIMENSION)
        .ok_or_else(|| format!("{label} {value} out of range (1..={MAX_IMAGE_DIMENSION})"))
}

/// Validate a raster's pixel count against [`MAX_IMAGE_PIXELS`], returning it
/// for the caller to size a buffer with.
pub(super) fn checked_pixel_area(label: &str, width: u32, height: u32) -> Result<usize, String> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .filter(|&pixels| pixels <= MAX_IMAGE_PIXELS);
    pixels.ok_or_else(|| {
        format!("{label} {width}x{height} exceeds the {MAX_IMAGE_PIXELS}-pixel limit")
    })
}

/// In-memory `Write + Seek` target that refuses growth past a named byte cap.
///
/// Image encoders only need `Write`, while `zip::ZipWriter` also seeks back to
/// patch headers. Checking the furthest written offset, rather than summing
/// writes, permits those bounded rewrites without letting a seek create a
/// sparse allocation past the cap.
pub(super) struct CappedBuffer {
    cursor: Cursor<Vec<u8>>,
    cap: usize,
    label: &'static str,
}

impl CappedBuffer {
    pub(super) const fn new(cap: usize, label: &'static str) -> Self {
        Self {
            cursor: Cursor::new(Vec::new()),
            cap,
            label,
        }
    }

    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.cursor.into_inner()
    }
}

impl Write for CappedBuffer {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let buffer_len = u64::try_from(buffer.len()).map_err(std::io::Error::other)?;
        let end = self
            .cursor
            .position()
            .checked_add(buffer_len)
            .ok_or_else(|| std::io::Error::other(over_cap_error(self.label, self.cap)))?;
        let resulting_len =
            end.max(u64::try_from(self.cursor.get_ref().len()).map_err(std::io::Error::other)?);
        if resulting_len > u64::try_from(self.cap).map_err(std::io::Error::other)? {
            return Err(std::io::Error::other(over_cap_error(self.label, self.cap)));
        }
        self.cursor.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.cursor.flush()
    }
}

impl Seek for CappedBuffer {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(position)
    }
}

pub(super) struct LatchedCappedBuffer {
    buffer: CappedBuffer,
    error: Option<String>,
}

impl LatchedCappedBuffer {
    pub(super) const fn new(cap: usize, label: &'static str) -> Self {
        Self {
            buffer: CappedBuffer::new(cap, label),
            error: None,
        }
    }

    pub(super) fn into_bytes(self) -> Result<Vec<u8>, String> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.buffer.into_bytes()),
        }
    }
}

impl Write for LatchedCappedBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.error.is_some() {
            return Ok(bytes.len());
        }
        match self.buffer.write(bytes) {
            Ok(written) => Ok(written),
            Err(error) => {
                self.error = Some(error.to_string());
                Ok(bytes.len())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.error.is_some() {
            return Ok(());
        }
        self.buffer.flush()
    }
}

impl Seek for LatchedCappedBuffer {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.buffer.seek(position)
    }
}
