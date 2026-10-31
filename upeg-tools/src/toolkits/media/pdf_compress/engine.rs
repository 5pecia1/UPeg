//! Bounded PDF image-XObject recompression. Every profile starts from the input.
use std::collections::BTreeSet;
use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageEncoder, imageops::FilterType};
use lopdf::{Dictionary, Document, LoadOptions, Object, ObjectId, Stream};
use serde::Serialize;

use super::super::limits::{
    CappedBuffer, MAX_DECODED_BYTES, MAX_ENCODED_OUTPUT_BYTES,
    MAX_PDF_COMPRESSION_DECODED_TOTAL_BYTES as MAX_DECODED_TOTAL_BYTES,
    MAX_PDF_COMPRESSION_IMAGE_WORK_BYTES as MAX_IMAGE_WORK_BYTES,
    MAX_PDF_COMPRESSION_IMAGES as MAX_IMAGES, MAX_PDF_COMPRESSION_INPUT_BYTES as MAX_INPUT_BYTES,
    MAX_PDF_COMPRESSION_OBJECT_STREAM_BYTES as MAX_OBJECT_STREAM_BYTES,
    MAX_PDF_COMPRESSION_OBJECTS as MAX_OBJECTS, MAX_PDF_PAGES, checked_dimension,
    checked_pixel_area, image_decode_limits, read_capped,
};
use super::super::pdf_extract::unpredict_png;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionMode {
    Balanced,
    HighQuality,
}

impl CompressionMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "balanced" => Ok(Self::Balanced),
            "high_quality" => Ok(Self::HighQuality),
            _ => Err(format!("unknown PDF compression mode `{value}`")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionStatus {
    AlreadyWithinTarget,
    TargetMet,
    TargetUnmet,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompressionAttempt {
    pub profile: String,
    pub size_bytes: Option<usize>,
    pub images_reencoded: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkippedImage {
    pub object_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompressionReport {
    pub status: CompressionStatus,
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub target_bytes: usize,
    pub saved_bytes: usize,
    pub reduction_percent: f64,
    pub selected_profile: Option<String>,
    pub attempts: Vec<CompressionAttempt>,
    pub skipped_images: Vec<SkippedImage>,
    pub page_count: usize,
    pub images_total: usize,
}

pub struct EngineOutput {
    pub bytes: Vec<u8>,
    pub report: CompressionReport,
}

#[derive(Clone, Copy)]
struct Profile {
    name: &'static str,
    quality: Option<u8>,
    scale_percent: u32,
}

const LOSSLESS: Profile = Profile {
    name: "lossless",
    quality: None,
    scale_percent: 100,
};
const BALANCED: [Profile; 3] = [
    Profile {
        name: "balanced_q75_75pct",
        quality: Some(75),
        scale_percent: 75,
    },
    Profile {
        name: "balanced_q65_50pct",
        quality: Some(65),
        scale_percent: 50,
    },
    Profile {
        name: "balanced_q55_35pct",
        quality: Some(55),
        scale_percent: 35,
    },
];
const HIGH_QUALITY: [Profile; 5] = [
    Profile {
        name: "hq_q92_original",
        quality: Some(92),
        scale_percent: 100,
    },
    Profile {
        name: "hq_q85_75pct",
        quality: Some(85),
        scale_percent: 75,
    },
    BALANCED[0],
    BALANCED[1],
    BALANCED[2],
];

/// Recompress only simple 8-bit RGB/gray image XObjects; leave all other
/// objects and unsupported images intact. A target is a goal, not a hard cap.
pub fn compress(
    bytes: &[u8],
    target_bytes: usize,
    mode: CompressionMode,
    max_output_bytes: usize,
) -> Result<EngineOutput, String> {
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "PDF input must be between 1 and {MAX_INPUT_BYTES} bytes"
        ));
    }
    if !(1..=MAX_ENCODED_OUTPUT_BYTES).contains(&target_bytes) {
        return Err(format!(
            "target_bytes must be between 1 and {MAX_ENCODED_OUTPUT_BYTES}"
        ));
    }
    if !(1..=MAX_ENCODED_OUTPUT_BYTES).contains(&max_output_bytes) {
        return Err(format!(
            "max_output_bytes must be between 1 and {MAX_ENCODED_OUTPUT_BYTES}"
        ));
    }

    let original = load_pdf(bytes)?;
    if original.is_encrypted() || original.was_encrypted() {
        return Err("encrypted PDFs cannot be compressed".to_string());
    }
    if has_signature(&original) {
        return Err("signed PDFs cannot be compressed without invalidating signatures".to_string());
    }
    let page_count = original.get_pages().len();
    if page_count == 0 {
        return Err("PDF has no readable pages".to_string());
    }
    if page_count > MAX_PDF_PAGES {
        return Err(format!("PDF has more than {MAX_PDF_PAGES} pages"));
    }
    if original.objects.len() > MAX_OBJECTS {
        return Err(format!("PDF has more than {MAX_OBJECTS} objects"));
    }
    let image_ids = image_ids(&original)?;
    let protected_masks = mask_image_ids(&original);
    let images_total = image_ids.len();
    let mut skipped_images = Vec::new();
    let mut eligible_ids = Vec::new();
    let needs_work = bytes.len() > target_bytes || bytes.len() > max_output_bytes;
    let mut decoded_total = 0_usize;
    for &id in &image_ids {
        if protected_masks.contains(&id) {
            skipped_images.push(SkippedImage {
                object_id: object_id(id),
                reason: "image is referenced as a transparency mask".to_string(),
            });
            continue;
        }
        let stream = original
            .get_object(id)
            .and_then(Object::as_stream)
            .map_err(|e| format!("image {id:?} cannot be read: {e}"))?;
        let checked = inspect_image(stream).and_then(|spec| {
            if needs_work {
                let component_count = if spec.gray { 1 } else { 3 };
                let decoded_bytes = checked_pixel_area("PDF image", spec.width, spec.height)?
                    .checked_mul(component_count)
                    .ok_or_else(|| "PDF image byte count overflow".to_string())?;
                decoded_total = decoded_total
                    .checked_add(decoded_bytes)
                    .ok_or_else(|| "PDF decoded image budget overflow".to_string())?;
                if decoded_total > MAX_DECODED_TOTAL_BYTES {
                    return Err("PDF decoded images exceed aggregate budget".to_string());
                }
                decode_image(stream, spec)?;
            }
            Ok(())
        });
        if let Err(reason) = checked {
            skipped_images.push(SkippedImage {
                object_id: object_id(id),
                reason,
            });
        } else {
            eligible_ids.push(id);
        }
    }
    drop(original);

    let mut report = CompressionReport {
        status: CompressionStatus::TargetUnmet,
        input_bytes: bytes.len(),
        output_bytes: bytes.len(),
        target_bytes,
        saved_bytes: 0,
        reduction_percent: 0.0,
        selected_profile: None,
        attempts: Vec::new(),
        skipped_images,
        page_count,
        images_total,
    };
    if bytes.len() <= target_bytes && bytes.len() <= max_output_bytes {
        report.status = CompressionStatus::AlreadyWithinTarget;
        return Ok(EngineOutput {
            bytes: bytes.to_vec(),
            report,
        });
    }

    let mut best = if bytes.len() <= max_output_bytes {
        Some(bytes.to_vec())
    } else {
        None
    };
    let mut best_size = best.as_ref().map_or(usize::MAX, Vec::len);
    let profiles: &[Profile] = match mode {
        CompressionMode::Balanced => &BALANCED,
        CompressionMode::HighQuality => &HIGH_QUALITY,
    };
    for profile in std::iter::once(&LOSSLESS).chain(profiles.iter()) {
        // Only one parsed document, one encoded candidate and the current best
        // are retained. The loaded original above is dropped before attempts.
        let outcome = run_profile(bytes, &eligible_ids, *profile, max_output_bytes);
        match outcome {
            Ok((candidate, changed)) => {
                let size = candidate.len();
                report.attempts.push(CompressionAttempt {
                    profile: profile.name.to_string(),
                    size_bytes: Some(size),
                    images_reencoded: changed,
                    error: None,
                });
                if size < best_size && size <= bytes.len() {
                    best_size = size;
                    best = Some(candidate);
                    report.selected_profile = Some(profile.name.to_string());
                }
                if best_size <= target_bytes {
                    report.status = CompressionStatus::TargetMet;
                    break;
                }
            }
            Err(error) => report.attempts.push(CompressionAttempt {
                profile: profile.name.to_string(),
                size_bytes: None,
                images_reencoded: 0,
                error: Some(error),
            }),
        }
    }
    let result = best.ok_or_else(|| "no PDF candidate fits max_output_bytes".to_string())?;
    report.output_bytes = result.len();
    report.saved_bytes = bytes.len().saturating_sub(result.len());
    report.reduction_percent = report.saved_bytes as f64 * 100.0 / bytes.len() as f64;
    Ok(EngineOutput {
        bytes: result,
        report,
    })
}

fn load_pdf(bytes: &[u8]) -> Result<Document, String> {
    let document = Document::load_mem_with_options(
        bytes,
        LoadOptions::with_max_decompressed_size(MAX_OBJECT_STREAM_BYTES),
    )
    .map_err(|e| format!("could not open PDF: {e}"))?;
    if document.is_encrypted() || document.was_encrypted() {
        return Err("encrypted PDFs cannot be compressed".to_string());
    }
    if document.objects.len() > MAX_OBJECTS {
        return Err(format!("PDF has more than {MAX_OBJECTS} objects"));
    }
    // lopdf can silently omit malformed or oversized object streams. Refuse
    // dangling references rather than rewriting a document with missing data.
    let mut pending: Vec<&Object> = document.objects.values().collect();
    pending.extend(document.trailer.iter().map(|(_, object)| object));
    while let Some(object) = pending.pop() {
        match object {
            Object::Reference(id) if !document.has_object(*id) => {
                return Err(format!(
                    "PDF has an unreadable object reference {} {}",
                    id.0, id.1
                ));
            }
            Object::Array(values) => pending.extend(values),
            Object::Dictionary(dict) => pending.extend(dict.iter().map(|(_, value)| value)),
            Object::Stream(stream) => pending.extend(stream.dict.iter().map(|(_, value)| value)),
            _ => {}
        }
    }
    document
        .catalog()
        .and_then(|catalog| catalog.get(b"Pages"))
        .and_then(Object::as_reference)
        .and_then(|id| document.get_object(id))
        .and_then(Object::as_dict)
        .map_err(|error| format!("PDF has no readable page tree: {error}"))?;
    Ok(document)
}

fn has_signature(document: &Document) -> bool {
    if document.trailer.get(b"Perms").is_ok() {
        return true;
    }
    document.objects.values().any(|object| {
        let dict = match object {
            Object::Dictionary(dict) => dict,
            Object::Stream(stream) => &stream.dict,
            _ => return false,
        };
        dict.get(b"ByteRange").is_ok()
            || dict
                .get(b"Type")
                .and_then(Object::as_name)
                .is_ok_and(|name| name == b"Sig")
            || dict
                .get(b"FT")
                .and_then(Object::as_name)
                .is_ok_and(|name| name == b"Sig")
    })
}

fn image_ids(document: &Document) -> Result<Vec<ObjectId>, String> {
    let ids: Vec<_> = document
        .objects
        .iter()
        .filter_map(|(&id, object)| {
            let stream = object.as_stream().ok()?;
            (stream.dict.get(b"Subtype").and_then(Object::as_name).ok()? == b"Image").then_some(id)
        })
        .collect();
    if ids.len() > MAX_IMAGES {
        return Err(format!("PDF has more than {MAX_IMAGES} image objects"));
    }
    Ok(ids)
}

fn object_id(id: ObjectId) -> String {
    format!("{} {}", id.0, id.1)
}

fn mask_image_ids(document: &Document) -> BTreeSet<ObjectId> {
    let mut masks = BTreeSet::new();
    for object in document.objects.values() {
        let Ok(stream) = object.as_stream() else {
            continue;
        };
        for key in [b"SMask".as_slice(), b"Mask"] {
            let Ok(value) = stream.dict.get(key) else {
                continue;
            };
            if let Ok(id) = value.as_reference() {
                masks.insert(id);
            }
            if let Ok((Some(id), _)) = document.dereference(value) {
                masks.insert(id);
            }
        }
    }
    masks
}

#[derive(Clone, Copy)]
struct ImageSpec {
    width: u32,
    height: u32,
    gray: bool,
    filter: Filter,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    Dct,
    Flate,
    Raw,
}

fn inspect_image(stream: &Stream) -> Result<ImageSpec, String> {
    let dict = &stream.dict;
    for key in [
        b"Mask".as_slice(),
        b"SMask",
        b"ImageMask",
        b"SMaskInData",
        b"Alternates",
        b"DP",
        b"F",
        b"FFilter",
        b"FDecodeParms",
    ] {
        if dict.get(key).is_ok() {
            return Err(format!(
                "/{key_name} is unsupported",
                key_name = String::from_utf8_lossy(key)
            ));
        }
    }
    let color = dict
        .get(b"ColorSpace")
        .and_then(Object::as_name)
        .map_err(|_| "only direct DeviceRGB/DeviceGray colorspace is supported".to_string())?;
    let gray = match color {
        b"DeviceGray" => true,
        b"DeviceRGB" => false,
        _ => return Err("only DeviceRGB/DeviceGray colorspace is supported".to_string()),
    };
    let bits = dict
        .get(b"BitsPerComponent")
        .and_then(Object::as_i64)
        .map_err(|_| "missing BitsPerComponent".to_string())?;
    if bits != 8 {
        return Err("only 8 bits per component is supported".to_string());
    }
    if !identity_decode(dict, if gray { 1 } else { 3 }) {
        return Err("nonidentity /Decode is unsupported".to_string());
    }
    let width = checked_dimension(
        "Width",
        dict.get(b"Width")
            .and_then(Object::as_i64)
            .map_err(|_| "missing Width".to_string())?,
    )?;
    let height = checked_dimension(
        "Height",
        dict.get(b"Height")
            .and_then(Object::as_i64)
            .map_err(|_| "missing Height".to_string())?,
    )?;
    checked_pixel_area("PDF image", width, height)?;
    let name = match dict.get(b"Filter") {
        Err(_) => None,
        Ok(Object::Name(name)) => Some(name.as_slice()),
        Ok(Object::Array(filters)) if filters.len() == 1 => Some(
            filters[0]
                .as_name()
                .map_err(|_| "unsupported image filter reference".to_string())?,
        ),
        _ => return Err("unsupported image filter or filter chain".to_string()),
    };
    let filter = match name {
        None => Filter::Raw,
        Some(b"DCTDecode") => Filter::Dct,
        Some(b"FlateDecode") => Filter::Flate,
        _ => return Err("unsupported image filter or filter chain".to_string()),
    };
    match filter {
        Filter::Flate => validate_flate_params(dict, width, gray)?,
        Filter::Dct | Filter::Raw if dict.get(b"DecodeParms").is_ok() => {
            return Err("/DecodeParms is unsupported for this filter".to_string());
        }
        _ => {}
    }
    Ok(ImageSpec {
        width,
        height,
        gray,
        filter,
    })
}

fn identity_decode(dict: &Dictionary, components: usize) -> bool {
    let Ok(value) = dict.get(b"Decode") else {
        return true;
    };
    let Ok(values) = value.as_array() else {
        return false;
    };
    values.len() == components * 2
        && values
            .chunks_exact(2)
            .all(|pair| number(&pair[0]) == Some(0.0) && number(&pair[1]) == Some(1.0))
}

fn number(value: &Object) -> Option<f64> {
    match value {
        Object::Integer(v) => Some(*v as f64),
        Object::Real(v) => Some(f64::from(*v)),
        _ => None,
    }
}

fn validate_flate_params(dict: &Dictionary, width: u32, gray: bool) -> Result<(), String> {
    let Ok(parms) = dict.get(b"DecodeParms") else {
        return Ok(());
    };
    let parms = parms
        .as_dict()
        .map_err(|_| "indirect /DecodeParms is unsupported".to_string())?;
    let predictor = parms
        .get(b"Predictor")
        .and_then(Object::as_i64)
        .unwrap_or(1);
    if predictor != 1 && !(10..=15).contains(&predictor) {
        return Err("unsupported Flate predictor".to_string());
    }
    if predictor != 1 {
        let columns = parms.get(b"Columns").and_then(Object::as_i64).unwrap_or(1);
        let colors = parms.get(b"Colors").and_then(Object::as_i64).unwrap_or(1);
        let bits = parms
            .get(b"BitsPerComponent")
            .and_then(Object::as_i64)
            .unwrap_or(8);
        if columns != i64::from(width) || colors != if gray { 1 } else { 3 } || bits != 8 {
            return Err("Flate predictor dimensions do not match image".to_string());
        }
    }
    Ok(())
}

fn run_profile(
    bytes: &[u8],
    ids: &[ObjectId],
    profile: Profile,
    output_cap: usize,
) -> Result<(Vec<u8>, usize), String> {
    let mut document = load_pdf(bytes)?;
    let mut changed = 0;
    let mut decoded_total = 0_usize;
    if let Some(quality) = profile.quality {
        for &id in ids {
            let stream = document
                .get_object(id)
                .and_then(Object::as_stream)
                .map_err(|e| format!("image {id:?} cannot be read: {e}"))?;
            let Ok(spec) = inspect_image(stream) else {
                continue;
            };
            let decoded_bytes = checked_pixel_area("PDF image", spec.width, spec.height)?
                .checked_mul(if spec.gray { 1 } else { 3 })
                .ok_or_else(|| "PDF image byte count overflow".to_string())?;
            decoded_total = decoded_total
                .checked_add(decoded_bytes)
                .ok_or_else(|| "PDF decoded image budget overflow".to_string())?;
            if decoded_total > MAX_DECODED_TOTAL_BYTES {
                return Err("PDF decoded images exceed aggregate budget".to_string());
            }
            let (width, height) = scaled_size(spec, profile.scale_percent);
            check_image_work(spec, height, width != spec.width || height != spec.height)?;
            let mut raster = decode_image(stream, spec)?;
            if width != spec.width || height != spec.height {
                raster = raster.resize_exact(width, height, FilterType::CatmullRom);
            }
            let encoded = encode_jpeg(&raster, spec.gray, quality)?;
            // Replacing a small stream with a larger one cannot improve PDF size.
            if encoded.len() >= stream.content.len() {
                continue;
            }
            let stream = document
                .get_object_mut(id)
                .and_then(Object::as_stream_mut)
                .map_err(|e| format!("image {id:?} cannot be updated: {e}"))?;
            stream.dict.set("Width", i64::from(width));
            stream.dict.set("Height", i64::from(height));
            stream.dict.set("Filter", "DCTDecode");
            stream.dict.remove(b"DecodeParms");
            stream.dict.remove(b"DL");
            stream.set_content(encoded);
            changed += 1;
        }
    }
    // Compress page content streams only. Unknown images and font streams must
    // retain their original filters and encoded bytes.
    let content_ids: Vec<_> = document
        .get_pages()
        .into_values()
        .flat_map(|page_id| document.get_page_contents(page_id))
        .collect();
    for id in content_ids {
        if let Ok(stream) = document.get_object_mut(id).and_then(Object::as_stream_mut) {
            if stream.dict.get(b"F").is_ok() {
                continue;
            }
            stream
                .compress()
                .map_err(|e| format!("could not compress page content: {e}"))?;
        }
    }
    document.prune_objects();
    let mut writer = CappedBuffer::new(output_cap, "compressed PDF");
    document
        .save_to(&mut writer)
        .map_err(|e| format!("could not save PDF: {e}"))?;
    Ok((writer.into_bytes(), changed))
}

fn scaled_size(spec: ImageSpec, percent: u32) -> (u32, u32) {
    let scale = |value: u32| value.saturating_mul(percent).div_ceil(100).max(1);
    (scale(spec.width), scale(spec.height))
}

fn decode_image(stream: &Stream, spec: ImageSpec) -> Result<DynamicImage, String> {
    check_image_work(spec, spec.height, false)?;
    let pixels = checked_pixel_area("PDF image", spec.width, spec.height)?;
    let expected = pixels
        .checked_mul(if spec.gray { 1 } else { 3 })
        .ok_or_else(|| "PDF image byte count overflow".to_string())?;
    if expected > MAX_DECODED_BYTES {
        return Err("PDF image exceeds decoded byte limit".to_string());
    }
    if spec.filter == Filter::Dct {
        let mut reader =
            image::ImageReader::with_format(Cursor::new(&stream.content), image::ImageFormat::Jpeg);
        reader.limits(image_decode_limits());
        let decoder = reader
            .into_decoder()
            .map_err(|e| format!("could not decode PDF JPEG: {e}"))?;
        // Validate the encoded header before allocating decoded pixels. PDF
        // dictionary dimensions/colors may lie about the JPEG working set.
        if decoder.dimensions() != (spec.width, spec.height) {
            return Err("PDF JPEG dimensions disagree with image dictionary".to_string());
        }
        if spec.gray && decoder.color_type().channel_count() != 1 {
            return Err("PDF JPEG color channels disagree with DeviceGray".to_string());
        }
        if !spec.gray && decoder.color_type().channel_count() != 3 {
            return Err("PDF JPEG color channels disagree with DeviceRGB".to_string());
        }
        return DynamicImage::from_decoder(decoder)
            .map_err(|e| format!("could not decode PDF JPEG: {e}"));
    }
    // A PNG predictor adds one filter selector per row before inflation.
    // Constrain expansion to this image's declared shape.
    let inflate_cap = if spec.filter == Filter::Flate {
        expected
            .checked_add(spec.height as usize)
            .ok_or_else(|| "PDF image inflated byte count overflow".to_string())?
    } else {
        expected
    };
    let samples = if spec.filter == Filter::Flate {
        let samples = read_capped(
            flate2::read::ZlibDecoder::new(stream.content.as_slice()),
            inflate_cap,
            "PDF image samples",
        )?;
        let predictor = stream
            .dict
            .get(b"DecodeParms")
            .and_then(Object::as_dict)
            .and_then(|parms| parms.get(b"Predictor"))
            .and_then(Object::as_i64)
            .unwrap_or(1);
        if (10..=15).contains(&predictor) {
            let components = if spec.gray { 1 } else { 3 };
            unpredict_png(&samples, spec.width as usize * components, components)?
        } else {
            samples
        }
    } else {
        if stream.content.len() != expected {
            return Err("PDF image sample length does not match dimensions".to_string());
        }
        stream.content.clone()
    };
    if samples.len() != expected {
        return Err("PDF image sample length does not match dimensions".to_string());
    }
    if spec.gray {
        image::GrayImage::from_raw(spec.width, spec.height, samples)
            .map(DynamicImage::ImageLuma8)
            .ok_or_else(|| "could not construct grayscale PDF image".to_string())
    } else {
        image::RgbImage::from_raw(spec.width, spec.height, samples)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| "could not construct RGB PDF image".to_string())
    }
}

fn encode_jpeg(raster: &DynamicImage, gray: bool, quality: u8) -> Result<Vec<u8>, String> {
    let mut writer = CappedBuffer::new(MAX_DECODED_BYTES, "PDF JPEG image");
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
    if gray {
        let image = raster
            .as_luma8()
            .ok_or_else(|| "expected 8-bit grayscale raster".to_string())?;
        encoder.write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::L8,
        )
    } else {
        let image = raster
            .as_rgb8()
            .ok_or_else(|| "expected 8-bit RGB raster".to_string())?;
        encoder.write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgb8,
        )
    }
    .map_err(|e| format!("could not encode PDF JPEG: {e}"))?;
    Ok(writer.into_bytes())
}

fn check_image_work(spec: ImageSpec, output_height: u32, resizing: bool) -> Result<(), String> {
    let pixels = checked_pixel_area("PDF image", spec.width, spec.height)?;
    // Inflation, predictor reconstruction and the decoded raster can coexist.
    let decoded = pixels
        .checked_mul(if spec.gray { 3 } else { 9 })
        .ok_or_else(|| "PDF image working byte count overflow".to_string())?;
    // image's vertical resampling pass keeps original width, output height,
    // and four f32 channels; pixel caps alone do not bound this temporary.
    let scratch = if resizing {
        (spec.width as usize)
            .checked_mul(output_height as usize)
            .and_then(|area| area.checked_mul(16))
            .ok_or_else(|| "PDF resampling working byte count overflow".to_string())?
    } else {
        0
    };
    let working = decoded
        .checked_add(scratch)
        .ok_or_else(|| "PDF image working byte count overflow".to_string())?;
    if working > MAX_IMAGE_WORK_BYTES {
        return Err(format!(
            "PDF image work needs {working} bytes, exceeding the {MAX_IMAGE_WORK_BYTES} byte limit"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;
    use std::io::Write;

    #[test]
    fn large_resampling_scratch_is_rejected_before_allocating_a_raster() {
        let gray = ImageSpec {
            width: 6000,
            height: 6666,
            gray: true,
            filter: Filter::Raw,
        };
        assert!(check_image_work(gray, gray.height, false).is_ok());
        assert!(
            check_image_work(gray, 5000, true)
                .unwrap_err()
                .contains("exceeding")
        );
        let rgb = ImageSpec {
            gray: false,
            ..gray
        };
        assert!(check_image_work(rgb, rgb.height, false).is_err());
        let ordinary = ImageSpec {
            width: 1920,
            height: 1080,
            gray: false,
            filter: Filter::Dct,
        };
        assert!(check_image_work(ordinary, 810, true).is_ok());
    }

    #[test]
    fn png_average_predictor_restores_known_pixels_before_reencoding() {
        // PNG Average: raw = encoded + floor((left + above) / 2).
        // These literal rows catch the upstream decoder's left + above/2 bug.
        let encoded = [3, 10, 20, 30, 75, 90, 105, 3, 15, 30, 45, 110, 120, 130];
        let expected = [10, 20, 30, 80, 100, 120, 20, 40, 60, 160, 190, 220];
        let mut writer = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        writer.write_all(&encoded).unwrap();
        let compressed = writer.finish().unwrap();
        for predictor in [13, 15] {
            let stream = Stream::new(
                dictionary! {
                    "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 2,
                    "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "FlateDecode",
                    "DecodeParms" => dictionary! { "Predictor" => predictor, "Colors" => 3, "Columns" => 2, "BitsPerComponent" => 8 },
                },
                compressed.clone(),
            );
            let raster = decode_image(&stream, inspect_image(&stream).unwrap()).unwrap();
            assert_eq!(raster.to_rgb8().as_raw(), &expected);
        }
    }

    #[test]
    fn png_sub_up_and_paeth_predictors_restore_known_pixels() {
        let expected = [10, 20, 30, 80, 100, 120, 20, 40, 60, 160, 190, 220];
        // Two 2-pixel RGB rows. Each vector uses literal filtered samples so
        // the decoder is checked against independently computed values.
        let cases = [
            (
                11,
                [1, 10, 20, 30, 70, 80, 90, 1, 20, 40, 60, 140, 150, 160],
            ),
            (
                12,
                [2, 10, 20, 30, 80, 100, 120, 2, 10, 20, 30, 80, 90, 100],
            ),
            (14, [4, 10, 20, 30, 70, 80, 90, 4, 10, 20, 30, 80, 90, 100]),
            (15, [1, 10, 20, 30, 70, 80, 90, 4, 10, 20, 30, 80, 90, 100]),
        ];
        for (predictor, encoded) in cases {
            let mut writer =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
            writer.write_all(&encoded).unwrap();
            let stream = Stream::new(
                dictionary! {
                    "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 2,
                    "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "FlateDecode",
                    "DecodeParms" => dictionary! { "Predictor" => predictor, "Colors" => 3, "Columns" => 2, "BitsPerComponent" => 8 },
                },
                writer.finish().unwrap(),
            );
            let raster = decode_image(&stream, inspect_image(&stream).unwrap()).unwrap();
            assert_eq!(
                raster.to_rgb8().as_raw(),
                &expected,
                "predictor {predictor}"
            );
        }
    }
}
