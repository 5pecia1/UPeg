//! Pure-Rust extraction of embedded raster images from a PDF's XObject image
//! streams — the PDFium-free implementation behind `media.pdf_extract_images`.
//!
//! `lopdf` parses the PDF object graph; per-filter pure-Rust codecs lift each
//! `/Subtype /Image` XObject stream. No page rendering, no native dynamic
//! library.
//!
//! # Stage 1 coverage
//! - **DCTDecode** (JPEG): the stream *is* a JPEG, written verbatim as `.jpg`
//!   after a header-validation pass (`zune-jpeg`); no decode/re-encode.
//! - **FlateDecode** / **LZWDecode**: inflate (`flate2`) / LZW-decode (`weezl`)
//!   to raw samples, reconstructed to PNG for `DeviceRGB` / `DeviceGray` at
//!   8 bits per component.
//! - **CCITTFaxDecode**: Group 4 (and EOL-carrying Group 3) bilevel streams
//!   decoded to grayscale PNG — see [`ccitt`].
//! - Everything else — other filters (JPX/JBIG2/RunLength), filter chains,
//!   other colorspaces (CMYK/Indexed/ICCBased), other bit depths, and any
//!   `/Decode` remapping — is recorded as a per-image skip reason, never a
//!   corrupt file.
//!
//! # Untrusted input
//! Every number here (`/Width`, `/Columns`, `/Colors`, `/Rows`) arrives from an
//! attacker-supplied PDF, and every buffer is sized from one. Dimensions go
//! through [`limits::checked_dimension`](super::limits::checked_dimension)
//! rather than an `as u32`, decoders read through a cap, and predictor rows are
//! tied to the image they claim to describe — see [`predictor_stride`].

use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object, ObjectId};

use super::limits::{
    CappedBuffer, MAX_DECODED_BYTES, MAX_PDF_EXTRACTED_IMAGE_BYTES, MAX_PDF_EXTRACTED_IMAGES,
    MAX_PDF_FORM_DEPTH, checked_dimension, checked_pixel_area, over_cap_error, read_capped,
};
use super::{PDF_EMBEDDED_IMAGE_JPEG_EXT, PDF_EMBEDDED_IMAGE_PNG_EXT};

// PDF image stream filters (PDF 32000-1:2008 §7.4).
const FILTER_FLATE: &[u8] = b"FlateDecode";
const FILTER_LZW: &[u8] = b"LZWDecode";
const FILTER_DCT: &[u8] = b"DCTDecode";
const FILTER_JPX: &[u8] = b"JPXDecode";
const FILTER_JBIG2: &[u8] = b"JBIG2Decode";
const FILTER_CCITT: &[u8] = b"CCITTFaxDecode";
const FILTER_RUNLENGTH: &[u8] = b"RunLengthDecode";

// Device colorspace names we can reconstruct from raw samples.
const CS_DEVICE_RGB: &[u8] = b"DeviceRGB";
const CS_DEVICE_GRAY: &[u8] = b"DeviceGray";

// PDF object dictionary keys.
const KEY_SUBTYPE: &[u8] = b"Subtype";
const KEY_FILTER: &[u8] = b"Filter";
const KEY_WIDTH: &[u8] = b"Width";
const KEY_HEIGHT: &[u8] = b"Height";
const KEY_COLORSPACE: &[u8] = b"ColorSpace";
const KEY_BITS_PER_COMPONENT: &[u8] = b"BitsPerComponent";
const KEY_DECODE_PARMS: &[u8] = b"DecodeParms";
const KEY_DECODE_PARMS_ABBREV: &[u8] = b"DP";
const KEY_PREDICTOR: &[u8] = b"Predictor";
const KEY_EARLY_CHANGE: &[u8] = b"EarlyChange";
const KEY_COLORS: &[u8] = b"Colors";
const KEY_COLUMNS: &[u8] = b"Columns";
const KEY_DECODE: &[u8] = b"Decode";
const KEY_XOBJECT: &[u8] = b"XObject";
const KEY_RESOURCES: &[u8] = b"Resources";

// XObject `/Subtype` values.
const SUBTYPE_IMAGE: &[u8] = b"Image";
const SUBTYPE_FORM: &[u8] = b"Form";

const SUPPORTED_BITS_PER_COMPONENT: i64 = 8;
const RGB_COMPONENTS: usize = 3;
const GRAY_COMPONENTS: usize = 1;
const LZW_DEFAULT_EARLY_CHANGE: i64 = 1;
const LZW_MIN_CODE_SIZE: u8 = 8;
/// Output buffer handed to the LZW decoder per step. Decoding through a fixed
/// chunk (rather than `weezl`'s `decode`, which grows a `Vec` to whatever the
/// stream expands to) is what makes [`MAX_DECODED_BYTES`] enforceable.
const LZW_DECODE_CHUNK_BYTES: usize = 64 * 1024;

/// A `/Decode` pair that leaves 8-bit samples unchanged. Anything else remaps
/// sample values (`/Decode [1 0]` inverts DeviceGray), which the raw-sample
/// reconstruction below does not apply — see [`decode_array_is_identity`].
const DECODE_IDENTITY_PAIR: [f64; 2] = [0.0, 1.0];

// `/DecodeParms /Predictor` values (PDF 32000-1:2008 Table 10).
const PREDICTOR_NONE: i64 = 1;
const PREDICTOR_TIFF: i64 = 2;
const PREDICTOR_PNG_MIN: i64 = 10;
const PREDICTOR_PNG_MAX: i64 = 15;
const DEFAULT_PREDICTOR_COLORS: i64 = 1;
const DEFAULT_PREDICTOR_COLUMNS: i64 = 1;

// PNG per-row filter types (PNG spec §6, embedded ahead of each predicted row).
const PNG_FILTER_NONE: u8 = 0;
const PNG_FILTER_SUB: u8 = 1;
const PNG_FILTER_UP: u8 = 2;
const PNG_FILTER_AVERAGE: u8 = 3;
const PNG_FILTER_PAETH: u8 = 4;

/// CCITTFaxDecode (Group 3/4) bilevel streams.
mod ccitt;

#[cfg(test)]
mod limits_tests;

/// One extracted image, ready to be added to the output zip: the entry name
/// (`{stem}-p{page}-i{index}.{ext}`) and its encoded bytes (JPEG passthrough or
/// reconstructed PNG).
pub(super) struct ExtractedImage {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Result of an extraction run: images collected in memory and per-image skip
/// reasons. Nothing touches the filesystem — the caller packs `images` into a
/// single zip [`upeg_core::FileValue`].
pub(super) struct ExtractionOutcome {
    images: Vec<ExtractedImage>,
    skipped: Vec<String>,
    encoded_bytes: usize,
    max_encoded_bytes: usize,
}

impl ExtractionOutcome {
    fn new(max_encoded_bytes: usize) -> Self {
        Self {
            images: Vec::new(),
            skipped: Vec::new(),
            encoded_bytes: 0,
            max_encoded_bytes,
        }
    }

    pub(super) fn skipped(&self) -> &[String] {
        &self.skipped
    }

    pub(super) fn into_images(self) -> Vec<ExtractedImage> {
        self.images
    }

    fn remaining_encoded_bytes(&self) -> usize {
        self.max_encoded_bytes - self.encoded_bytes
    }

    fn push_image(&mut self, name: String, bytes: Vec<u8>) -> Result<(), String> {
        let next = self
            .encoded_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| "PDF extracted image byte count overflowed".to_string())?;
        if next > self.max_encoded_bytes {
            return Err(format!(
                "PDF extracted images exceed the {} byte aggregate limit",
                self.max_encoded_bytes
            ));
        }
        self.images.push(ExtractedImage { name, bytes });
        self.encoded_bytes = next;
        Ok(())
    }

    fn copy_image(&mut self, name: String, bytes: &[u8]) -> Result<(), String> {
        let next = self
            .encoded_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| "PDF extracted image byte count overflowed".to_string())?;
        if next > self.max_encoded_bytes {
            return Err(format!(
                "PDF extracted images exceed the {} byte aggregate limit",
                self.max_encoded_bytes
            ));
        }
        self.images.push(ExtractedImage {
            name,
            bytes: bytes.to_vec(),
        });
        self.encoded_bytes = next;
        Ok(())
    }
}

/// Immutable per-run context (kept together to keep helper arg lists small).
struct WriteContext<'a> {
    pdf_stem: &'a str,
}

/// 1-based page/image position, used for output filenames and skip notes.
#[derive(Clone, Copy)]
struct ImageLocation {
    page: usize,
    index: usize,
}

/// Width/height/component count of a raster to reconstruct from raw samples.
#[derive(Clone, Copy)]
struct RasterSpec {
    width: u32,
    height: u32,
    components: usize,
}

#[derive(Clone, Copy)]
struct ExtractionLimits {
    max_form_depth: usize,
    max_images: usize,
    max_encoded_bytes: usize,
}

const DEFAULT_EXTRACTION_LIMITS: ExtractionLimits = ExtractionLimits {
    max_form_depth: MAX_PDF_FORM_DEPTH,
    max_images: MAX_PDF_EXTRACTED_IMAGES,
    max_encoded_bytes: MAX_PDF_EXTRACTED_IMAGE_BYTES,
};

#[derive(Clone, Copy)]
struct FormDepth(usize);

impl FormDepth {
    const ROOT: Self = Self(0);

    fn enter(self, max_depth: usize) -> Result<Self, String> {
        let next = self
            .0
            .checked_add(1)
            .ok_or_else(|| "PDF Form traversal depth overflowed".to_string())?;
        if next > max_depth {
            return Err(format!(
                "PDF Form traversal exceeds the {max_depth} level limit"
            ));
        }
        Ok(Self(next))
    }
}

/// Parse PDF `bytes`, walk every page's image XObjects (recursing through Form
/// XObjects, de-duplicating shared images), and collect each into memory as an
/// [`ExtractedImage`]. Pure — no filesystem access, so it runs on every surface
/// including wasm.
pub(super) fn extract_embedded_images(
    bytes: &[u8],
    pdf_stem: &str,
) -> Result<ExtractionOutcome, String> {
    extract_embedded_images_with_limits(bytes, pdf_stem, DEFAULT_EXTRACTION_LIMITS)
}

fn extract_embedded_images_with_limits(
    bytes: &[u8],
    pdf_stem: &str,
    limits: ExtractionLimits,
) -> Result<ExtractionOutcome, String> {
    let document = Document::load_mem(bytes).map_err(open_pdf_error)?;
    let context = WriteContext { pdf_stem };
    let mut outcome = ExtractionOutcome::new(limits.max_encoded_bytes);
    let mut seen = BTreeSet::<ObjectId>::new();

    for (page_number, page_id) in document.get_pages() {
        let image_ids = page_image_object_ids(&document, page_id, &mut seen, limits)?;
        for (offset, image_id) in image_ids.into_iter().enumerate() {
            let location = ImageLocation {
                page: page_number as usize,
                index: offset + 1,
            };
            extract_one(&document, image_id, location, &context, &mut outcome)?;
        }
    }

    Ok(outcome)
}

fn open_pdf_error(error: lopdf::Error) -> String {
    format!("could not open PDF: {error}")
}

/// Collect image XObject ids referenced by a single page's resources, marking
/// each in `seen` so an image shared across pages is extracted only once.
fn page_image_object_ids(
    document: &Document,
    page_id: ObjectId,
    seen: &mut BTreeSet<ObjectId>,
    limits: ExtractionLimits,
) -> Result<Vec<ObjectId>, String> {
    let mut images = Vec::new();
    let mut visited_forms = BTreeSet::<ObjectId>::new();
    let Ok((inline_resources, referenced_resources)) = document.get_page_resources(page_id) else {
        return Ok(images);
    };
    if let Some(resources) = inline_resources {
        collect_images_from_resources(
            document,
            resources,
            seen,
            &mut visited_forms,
            &mut images,
            FormDepth::ROOT,
            limits,
        )?;
    }
    for resource_id in referenced_resources {
        if let Ok(resources) = document.get_dictionary(resource_id) {
            collect_images_from_resources(
                document,
                resources,
                seen,
                &mut visited_forms,
                &mut images,
                FormDepth::ROOT,
                limits,
            )?;
        }
    }
    Ok(images)
}

/// Walk a `/Resources` dict's `/XObject` entries: record image XObjects (once)
/// and recurse into Form XObjects' own resources.
fn collect_images_from_resources(
    document: &Document,
    resources: &Dictionary,
    seen: &mut BTreeSet<ObjectId>,
    visited_forms: &mut BTreeSet<ObjectId>,
    images: &mut Vec<ObjectId>,
    depth: FormDepth,
    limits: ExtractionLimits,
) -> Result<(), String> {
    let Some(xobjects) = resources
        .get(KEY_XOBJECT)
        .ok()
        .and_then(|object| resolve_to_dict(document, object))
    else {
        return Ok(());
    };
    for (_name, value) in xobjects {
        let Ok(id) = value.as_reference() else {
            continue;
        };
        let Ok(stream) = document.get_object(id).and_then(Object::as_stream) else {
            continue;
        };
        let subtype = stream
            .dict
            .get(KEY_SUBTYPE)
            .ok()
            .and_then(|o| o.as_name().ok());
        match subtype {
            Some(name) if name == SUBTYPE_IMAGE => {
                let next_count = seen
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| "PDF image count overflowed".to_string())?;
                if seen.insert(id) {
                    if next_count > limits.max_images {
                        return Err(format!(
                            "PDF contains more than {} unique image XObjects",
                            limits.max_images
                        ));
                    }
                    images.push(id);
                }
            }
            Some(name) if name == SUBTYPE_FORM => {
                if visited_forms.contains(&id) {
                    continue;
                }
                let child_depth = depth.enter(limits.max_form_depth)?;
                visited_forms.insert(id);
                if let Some(form_resources) = stream
                    .dict
                    .get(KEY_RESOURCES)
                    .ok()
                    .and_then(|object| resolve_to_dict(document, object))
                {
                    collect_images_from_resources(
                        document,
                        form_resources,
                        seen,
                        visited_forms,
                        images,
                        child_depth,
                        limits,
                    )?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Extract a single image XObject, appending either a written path or a skip
/// reason to `outcome`. Only genuine IO faults (write failures, output
/// collisions without overwrite) return `Err` and abort the whole run.
fn extract_one(
    document: &Document,
    image_id: ObjectId,
    location: ImageLocation,
    context: &WriteContext,
    outcome: &mut ExtractionOutcome,
) -> Result<(), String> {
    let Ok(stream) = document.get_object(image_id).and_then(Object::as_stream) else {
        outcome
            .skipped
            .push(skip_note(location, "image XObject is not a stream"));
        return Ok(());
    };
    let dict = &stream.dict;

    // `/Decode` remaps sample values before they become colours; none of the
    // paths below apply it, so honouring the array is the difference between a
    // skip and a silently inverted image.
    if !decode_array_is_identity(document, dict) {
        outcome.skipped.push(skip_note(
            location,
            "unsupported /Decode array (sample remapping is not applied)",
        ));
        return Ok(());
    }

    let filters = filter_names(document, dict);
    if filters.len() > 1 {
        outcome.skipped.push(skip_note(
            location,
            &format!("chained filters not supported: {}", filter_list(&filters)),
        ));
        return Ok(());
    }
    let filter = filters.first().map(Vec::as_slice);

    // DCTDecode: the stream is already a JPEG — validate headers, write verbatim.
    if filter == Some(FILTER_DCT) {
        return write_jpeg_passthrough(&stream.content, location, context, outcome);
    }

    // CCITTFaxDecode: decode the Group 3/4 bilevel stream to a grayscale PNG.
    if filter == Some(FILTER_CCITT) {
        return ccitt::write_ccitt(document, dict, &stream.content, location, context, outcome);
    }

    // Named-but-unsupported filters are recorded and skipped.
    if let Some(name) = filter {
        if name == FILTER_JPX || name == FILTER_JBIG2 || name == FILTER_RUNLENGTH {
            outcome.skipped.push(skip_note(
                location,
                &format!(
                    "unsupported filter /{} (not yet implemented)",
                    String::from_utf8_lossy(name)
                ),
            ));
            return Ok(());
        }
        if name != FILTER_FLATE && name != FILTER_LZW {
            outcome.skipped.push(skip_note(
                location,
                &format!("unknown filter /{}", String::from_utf8_lossy(name)),
            ));
            return Ok(());
        }
    }

    // Remaining: FlateDecode, LZWDecode, or no filter → reconstruct raw samples.
    let bits =
        dict_i64(document, dict, KEY_BITS_PER_COMPONENT).unwrap_or(SUPPORTED_BITS_PER_COMPONENT);
    if bits != SUPPORTED_BITS_PER_COMPONENT {
        outcome.skipped.push(skip_note(
            location,
            &format!(
                "unsupported BitsPerComponent {bits} (supported: {SUPPORTED_BITS_PER_COMPONENT})"
            ),
        ));
        return Ok(());
    }
    let components = match image_components(document, dict) {
        Ok(components) => components,
        Err(reason) => {
            outcome.skipped.push(skip_note(location, &reason));
            return Ok(());
        }
    };
    let (Some(width), Some(height)) = (
        dict_i64(document, dict, KEY_WIDTH),
        dict_i64(document, dict, KEY_HEIGHT),
    ) else {
        outcome
            .skipped
            .push(skip_note(location, "missing Width/Height"));
        return Ok(());
    };
    let spec = match raster_spec(width, height, components) {
        Ok(spec) => spec,
        Err(reason) => {
            outcome.skipped.push(skip_note(location, &reason));
            return Ok(());
        }
    };

    let samples = if filter == Some(FILTER_FLATE) {
        match inflate(&stream.content) {
            Ok(samples) => samples,
            Err(reason) => {
                outcome.skipped.push(skip_note(location, &reason));
                return Ok(());
            }
        }
    } else if filter == Some(FILTER_LZW) {
        let early_change =
            decode_parms_i64(document, dict, KEY_EARLY_CHANGE, LZW_DEFAULT_EARLY_CHANGE);
        match lzw_decode(&stream.content, early_change) {
            Ok(samples) => samples,
            Err(reason) => {
                outcome.skipped.push(skip_note(location, &reason));
                return Ok(());
            }
        }
    } else {
        // No filter: content already holds raw samples.
        stream.content.clone()
    };

    // Undo any `/DecodeParms /Predictor` transform before reconstructing pixels.
    let samples = match unpredict(document, dict, samples, spec) {
        Ok(samples) => samples,
        Err(reason) => {
            outcome.skipped.push(skip_note(location, &reason));
            return Ok(());
        }
    };

    reconstruct_and_write(&samples, spec, location, context, outcome)
}

/// Validate a declared `/Width` × `/Height` and pair it with the resolved
/// component count.
///
/// Both axes are rejected — never truncated — when they leave the supported
/// range, and the pixel count is capped before anything is sized from it.
fn raster_spec(width: i64, height: i64, components: usize) -> Result<RasterSpec, String> {
    let width = checked_dimension("Width", width)?;
    let height = checked_dimension("Height", height)?;
    checked_pixel_area("image", width, height)?;
    Ok(RasterSpec {
        width,
        height,
        components,
    })
}

/// Return `false` when the image declares a `/Decode` array that remaps sample
/// values; an absent (or identity) array means the samples mean what they say.
fn decode_array_is_identity(document: &Document, dict: &Dictionary) -> bool {
    let Some(Object::Array(entries)) = dict
        .get(KEY_DECODE)
        .ok()
        .map(|object| resolve(document, object))
    else {
        return true;
    };
    entries.chunks(DECODE_IDENTITY_PAIR.len()).all(|pair| {
        pair.len() == DECODE_IDENTITY_PAIR.len() && decode_pair_is_identity(document, pair)
    })
}

/// Return `true` when a two-entry `/Decode` slice is the identity pair `[0 1]`.
fn decode_pair_is_identity(document: &Document, pair: &[Object]) -> bool {
    pair.iter()
        .zip(DECODE_IDENTITY_PAIR)
        .all(|(entry, expected)| {
            resolve(document, entry)
                .as_float()
                .is_ok_and(|value| (f64::from(value) - expected).abs() < f64::EPSILON)
        })
}

/// Build the zip entry name for an extracted image: `{stem}-p{page}-i{index}.{ext}`,
/// both indices 1-based. `ext` reflects the encoding actually produced (PNG for
/// reconstructed samples, JPG for a DCTDecode passthrough).
fn image_entry_name(stem: &str, location: ImageLocation, ext: &str) -> String {
    format!("{stem}-p{}-i{}.{ext}", location.page, location.index)
}

/// Encode a decoded raster to PNG bytes in memory (no filesystem).
fn encode_png(image: &image::DynamicImage, max_bytes: usize) -> Result<Vec<u8>, String> {
    let mut bytes = CappedBuffer::new(max_bytes, "PDF extracted image bytes");
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(bytes.into_bytes())
}

/// Validate a DCTDecode stream as a JPEG and keep its bytes verbatim as `.jpg`.
fn write_jpeg_passthrough(
    content: &[u8],
    location: ImageLocation,
    context: &WriteContext,
    outcome: &mut ExtractionOutcome,
) -> Result<(), String> {
    // zune-jpeg 0.5 reads via a `Seek` source; wrap the byte slice in a cursor.
    let mut decoder = zune_jpeg::JpegDecoder::new(std::io::Cursor::new(content));
    if decoder.decode_headers().is_err() {
        outcome.skipped.push(skip_note(
            location,
            "DCTDecode stream is not a decodable JPEG",
        ));
        return Ok(());
    }
    outcome.copy_image(
        image_entry_name(context.pdf_stem, location, PDF_EMBEDDED_IMAGE_JPEG_EXT),
        content,
    )
}

/// Reconstruct raw 8-bit samples into a PNG for the resolved colorspace.
fn reconstruct_and_write(
    samples: &[u8],
    spec: RasterSpec,
    location: ImageLocation,
    context: &WriteContext,
    outcome: &mut ExtractionOutcome,
) -> Result<(), String> {
    let expected = (spec.width as usize)
        .checked_mul(spec.height as usize)
        .and_then(|pixels| pixels.checked_mul(spec.components));
    let Some(expected) = expected else {
        outcome
            .skipped
            .push(skip_note(location, "image dimensions overflow"));
        return Ok(());
    };
    let Some(buffer) = samples.get(..expected) else {
        outcome.skipped.push(skip_note(
            location,
            &format!(
                "decoded {} sample byte(s), need {expected} for {}x{}",
                samples.len(),
                spec.width,
                spec.height
            ),
        ));
        return Ok(());
    };
    let owned = buffer.to_vec();
    let decoded = match spec.components {
        RGB_COMPONENTS => image::RgbImage::from_raw(spec.width, spec.height, owned)
            .map(image::DynamicImage::ImageRgb8),
        GRAY_COMPONENTS => image::GrayImage::from_raw(spec.width, spec.height, owned)
            .map(image::DynamicImage::ImageLuma8),
        _ => None,
    };
    let Some(decoded) = decoded else {
        outcome.skipped.push(skip_note(
            location,
            "failed to build image buffer from samples",
        ));
        return Ok(());
    };
    let bytes = encode_png(&decoded, outcome.remaining_encoded_bytes())?;
    outcome.push_image(
        image_entry_name(context.pdf_stem, location, PDF_EMBEDDED_IMAGE_PNG_EXT),
        bytes,
    )
}

/// Inflate a FlateDecode stream, refusing past [`MAX_DECODED_BYTES`]. Deflate
/// reaches ~1032:1, so a plain `read_to_end` lets a 10 MB stream claim ~10 GB.
fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    read_capped(
        flate2::read::ZlibDecoder::new(data),
        MAX_DECODED_BYTES,
        "FlateDecode output",
    )
    .map_err(|e| format!("FlateDecode inflate failed: {e}"))
}

/// LZW-decode a stream, refusing past [`MAX_DECODED_BYTES`].
///
/// PDF LZWDecode is MSB-first over 8-bit input; the default EarlyChange=1
/// matches weezl's TIFF size-switch variant. The decode runs through a fixed
/// chunk because `weezl`'s one-shot `decode` grows a `Vec` to whatever the
/// stream expands to, with no way to stop it.
fn lzw_decode(data: &[u8], early_change: i64) -> Result<Vec<u8>, String> {
    let mut decoder = if early_change == 0 {
        weezl::decode::Decoder::new(weezl::BitOrder::Msb, LZW_MIN_CODE_SIZE)
    } else {
        weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, LZW_MIN_CODE_SIZE)
    };
    let mut out = Vec::new();
    let mut chunk = vec![0_u8; LZW_DECODE_CHUNK_BYTES];
    let mut input = data;
    loop {
        let result = decoder.decode_bytes(input, &mut chunk);
        out.extend_from_slice(&chunk[..result.consumed_out]);
        if out.len() > MAX_DECODED_BYTES {
            return Err(over_cap_error("LZWDecode output", MAX_DECODED_BYTES));
        }
        input = &input[result.consumed_in..];
        match result.status {
            Ok(weezl::LzwStatus::Done) => return Ok(out),
            Ok(weezl::LzwStatus::Ok) => {}
            Ok(weezl::LzwStatus::NoProgress) => {
                return Err("LZWDecode failed: stream ends without an end-of-data code".to_owned());
            }
            Err(error) => return Err(format!("LZWDecode failed: {error}")),
        }
        if result.consumed_in == 0 && result.consumed_out == 0 {
            // A decoder that neither reads nor writes will never finish: bail
            // rather than spin.
            return Err("LZWDecode failed: decoder stalled".to_owned());
        }
    }
}

/// Resolve the image `/ColorSpace` to a component count, or an error naming the
/// unsupported colorspace so it can be reported as a skip reason.
fn image_components(document: &Document, dict: &Dictionary) -> Result<usize, String> {
    match dict.get(KEY_COLORSPACE).ok().map(|o| resolve(document, o)) {
        Some(Object::Name(name)) if name.as_slice() == CS_DEVICE_RGB => Ok(RGB_COMPONENTS),
        Some(Object::Name(name)) if name.as_slice() == CS_DEVICE_GRAY => Ok(GRAY_COMPONENTS),
        Some(Object::Name(name)) => Err(format!(
            "unsupported ColorSpace /{} (stage 1 supports DeviceRGB/DeviceGray)",
            String::from_utf8_lossy(name)
        )),
        Some(Object::Array(entries)) => {
            let head = entries
                .first()
                .and_then(|o| resolve(document, o).as_name().ok())
                .map_or_else(
                    || "?".to_owned(),
                    |name| String::from_utf8_lossy(name).into_owned(),
                );
            Err(format!(
                "unsupported ColorSpace array /{head} (e.g. Indexed/ICCBased)"
            ))
        }
        _ => Err("missing ColorSpace".to_owned()),
    }
}

fn filter_names(document: &Document, dict: &Dictionary) -> Vec<Vec<u8>> {
    match dict.get(KEY_FILTER).ok().map(|o| resolve(document, o)) {
        Some(Object::Name(name)) => vec![name.clone()],
        Some(Object::Array(entries)) => entries
            .iter()
            .filter_map(|o| resolve(document, o).as_name().ok().map(<[u8]>::to_vec))
            .collect(),
        _ => Vec::new(),
    }
}

fn filter_list(filters: &[Vec<u8>]) -> String {
    filters
        .iter()
        .map(|name| format!("/{}", String::from_utf8_lossy(name)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_parms_i64(document: &Document, dict: &Dictionary, key: &[u8], default: i64) -> i64 {
    decode_parms(document, dict)
        .and_then(|parms| parms.get(key).ok())
        .and_then(|object| resolve(document, object).as_i64().ok())
        .unwrap_or(default)
}

fn decode_parms_bool(document: &Document, dict: &Dictionary, key: &[u8], default: bool) -> bool {
    decode_parms(document, dict)
        .and_then(|parms| parms.get(key).ok())
        .and_then(|object| resolve(document, object).as_bool().ok())
        .unwrap_or(default)
}

/// Reverse a `/DecodeParms /Predictor` transform. Predictor 1 (none) returns the
/// samples untouched; predictor 2 is the TIFF horizontal predictor; predictors
/// 10-15 are the PNG per-row filters. Unsupported parameters yield a skip reason.
fn unpredict(
    document: &Document,
    dict: &Dictionary,
    samples: Vec<u8>,
    spec: RasterSpec,
) -> Result<Vec<u8>, String> {
    let predictor = decode_parms_i64(document, dict, KEY_PREDICTOR, PREDICTOR_NONE);
    if predictor == PREDICTOR_NONE {
        return Ok(samples);
    }
    let predictor_bits = decode_parms_i64(
        document,
        dict,
        KEY_BITS_PER_COMPONENT,
        SUPPORTED_BITS_PER_COMPONENT,
    );
    if predictor_bits != SUPPORTED_BITS_PER_COMPONENT {
        return Err(format!(
            "unsupported Predictor BitsPerComponent {predictor_bits}"
        ));
    }
    let colors = decode_parms_i64(document, dict, KEY_COLORS, DEFAULT_PREDICTOR_COLORS);
    let columns = decode_parms_i64(document, dict, KEY_COLUMNS, DEFAULT_PREDICTOR_COLUMNS);
    let stride = predictor_stride(spec, colors, columns)?;

    if predictor == PREDICTOR_TIFF {
        unpredict_tiff(&samples, stride, spec.components)
    } else if (PREDICTOR_PNG_MIN..=PREDICTOR_PNG_MAX).contains(&predictor) {
        unpredict_png(&samples, stride, spec.components)
    } else {
        Err(format!("unsupported Predictor {predictor}"))
    }
}

/// The predictor's row size in bytes, tied to the image the samples describe.
///
/// `/Columns` and `/Colors` are unbounded attacker-supplied `i64`s that nothing
/// else relates to the image. Left free, `Columns × Colors` reaches `usize::MAX`
/// (which is divisible by 3, so it survives a `checked_mul`), and from there
/// `stride + 1` overflows into a zero row length that divides by zero — while
/// even a merely large value has [`unpredict_png`] allocate gigabytes of row
/// scratch before it looks at a single input byte.
///
/// Requiring the pair to match `/Width` and the colourspace's component count
/// bounds both: `Width` is already capped by [`checked_dimension`], so the row
/// cannot exceed `MAX_IMAGE_DIMENSION × RGB_COMPONENTS`. It also costs nothing
/// real — a predictor row that disagrees with the image it precedes could only
/// ever have reconstructed into a garbled raster, so a skip is the honest
/// answer rather than a conservative one.
fn predictor_stride(spec: RasterSpec, colors: i64, columns: i64) -> Result<usize, String> {
    if usize::try_from(colors).ok() != Some(spec.components) {
        return Err(format!(
            "Predictor Colors {colors} does not match the {}-component ColorSpace",
            spec.components
        ));
    }
    if columns != i64::from(spec.width) {
        return Err(format!(
            "Predictor Columns {columns} does not match image Width {}",
            spec.width
        ));
    }
    (spec.width as usize)
        .checked_mul(spec.components)
        .filter(|&stride| stride > 0)
        .ok_or_else(|| "invalid Predictor row size".to_owned())
}

/// Reverse PNG per-row filtering: each row is a 1-byte filter type followed by
/// `stride` filtered sample bytes; `bpp` is the byte distance to the pixel to
/// the left (Colors, since 8-bit samples).
///
/// `stride` must come from [`predictor_stride`], which bounds it well below
/// `usize::MAX`, so the `stride + 1` row length below cannot overflow.
fn unpredict_png(data: &[u8], stride: usize, bpp: usize) -> Result<Vec<u8>, String> {
    let row_len = stride + 1;
    if !data.len().is_multiple_of(row_len) {
        return Err(format!(
            "PNG predictor data ({} bytes) is not a multiple of the {row_len}-byte row",
            data.len()
        ));
    }
    let rows = data.len() / row_len;
    let mut out = vec![0_u8; rows * stride];
    let mut previous = vec![0_u8; stride];
    for row in 0..rows {
        let filter_type = data[row * row_len];
        let encoded = &data[row * row_len + 1..(row + 1) * row_len];
        let mut current = vec![0_u8; stride];
        for i in 0..stride {
            let left = if i >= bpp { current[i - bpp] } else { 0 };
            let up = previous[i];
            let up_left = if i >= bpp { previous[i - bpp] } else { 0 };
            let raw = encoded[i];
            current[i] = match filter_type {
                PNG_FILTER_NONE => raw,
                PNG_FILTER_SUB => raw.wrapping_add(left),
                PNG_FILTER_UP => raw.wrapping_add(up),
                PNG_FILTER_AVERAGE => raw.wrapping_add(left.midpoint(up)),
                PNG_FILTER_PAETH => raw.wrapping_add(paeth_predictor(left, up, up_left)),
                other => return Err(format!("unknown PNG predictor filter type {other}")),
            };
        }
        out[row * stride..(row + 1) * stride].copy_from_slice(&current);
        previous = current;
    }
    Ok(out)
}

/// Reverse the TIFF horizontal predictor (predictor 2): each sample is stored as
/// the difference from the same colour component in the pixel to its left.
fn unpredict_tiff(data: &[u8], stride: usize, colors: usize) -> Result<Vec<u8>, String> {
    if !data.len().is_multiple_of(stride) {
        return Err(format!(
            "TIFF predictor data ({} bytes) is not a multiple of the {stride}-byte row",
            data.len()
        ));
    }
    let mut out = data.to_vec();
    let rows = out.len() / stride;
    for row in 0..rows {
        let base = row * stride;
        for i in colors..stride {
            out[base + i] = out[base + i].wrapping_add(out[base + i - colors]);
        }
    }
    Ok(out)
}

/// The PNG Paeth predictor function (PNG spec §6.6).
fn paeth_predictor(left: u8, up: u8, up_left: u8) -> u8 {
    let (a, b, c) = (i32::from(left), i32::from(up), i32::from(up_left));
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc {
        left
    } else if pb <= pc {
        up
    } else {
        up_left
    }
}

fn decode_parms<'a>(document: &'a Document, dict: &'a Dictionary) -> Option<&'a Dictionary> {
    let object = dict
        .get(KEY_DECODE_PARMS)
        .or_else(|_| dict.get(KEY_DECODE_PARMS_ABBREV))
        .ok()?;
    match resolve(document, object) {
        Object::Dictionary(parms) => Some(parms),
        Object::Array(entries) => entries
            .iter()
            .find_map(|entry| match resolve(document, entry) {
                Object::Dictionary(parms) => Some(parms),
                _ => None,
            }),
        _ => None,
    }
}

fn dict_i64(document: &Document, dict: &Dictionary, key: &[u8]) -> Option<i64> {
    let object = dict.get(key).ok()?;
    resolve(document, object).as_i64().ok()
}

fn resolve_to_dict<'a>(document: &'a Document, object: &'a Object) -> Option<&'a Dictionary> {
    match resolve(document, object) {
        Object::Dictionary(dict) => Some(dict),
        Object::Stream(stream) => Some(&stream.dict),
        _ => None,
    }
}

/// Follow a single indirect reference; returns the input unchanged when it is a
/// direct object or the reference cannot be resolved.
fn resolve<'a>(document: &'a Document, object: &'a Object) -> &'a Object {
    match object {
        Object::Reference(id) => document.get_object(*id).unwrap_or(object),
        other => other,
    }
}

fn skip_note(location: ImageLocation, reason: &str) -> String {
    format!("p{} i{}: {reason}", location.page, location.index)
}
