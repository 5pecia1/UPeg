//! Render the first page of a PDF-compatible Illustrator file as a still image.

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_interpret::hayro_syntax::{
    Filter, LoadPdfError, Pdf,
    content::UntypedIter,
    object::{Dict, Name, Object, Stream},
    page::{Page, Resources},
};
use hayro::{RenderCache, RenderSettings, render};
use image::{DynamicImage, RgbaImage};

use super::super::limits::{
    MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS_F64, checked_pixel_area, over_cap_error, read_capped,
};
use super::super::normalize_dpi;
use super::IMAGE_CONVERT_MAX_INPUT_BYTES;

const POINTS_PER_INCH: f32 = 72.0;
const PDF_PLACEHOLDER_TEXT: &[u8] = b"savedwithoutpdfcontent";
const PDF_COMPATIBLE_OPTION: &[u8] = b"createpdfcompatiblefile";
const MAX_PLACEHOLDER_TEXT_BYTES: usize = 16 << 10;
const MAX_SCANNED_CONTENT_BYTES: usize = 16 << 20;
const MAX_FORM_DEPTH: usize = 16;
const MAX_SCANNED_FORMS: usize = 256;
const RESAVE_GUIDANCE: &str =
    "Illustrator file has no usable PDF artwork; resave it with Create PDF Compatible File enabled";

pub(super) fn pixel_count(bytes: &[u8], dpi: f64) -> Result<usize, String> {
    let scale = normalize_dpi(dpi)? / POINTS_PER_INCH;
    let pdf = open_pdf(bytes)?;
    let page = first_page(&pdf)?;
    let (width, height) = checked_dimensions(page, scale)?;
    reject_placeholder(page)?;
    checked_pixel_area("Illustrator page", width, height)
}

pub(super) fn decode(bytes: &[u8], dpi: f64) -> Result<DynamicImage, String> {
    let scale = normalize_dpi(dpi)? / POINTS_PER_INCH;
    let pdf = open_pdf(bytes)?;
    let page = first_page(&pdf)?;
    let (width, height) = checked_dimensions(page, scale)?;
    reject_placeholder(page)?;
    let settings = RenderSettings {
        x_scale: scale,
        y_scale: scale,
        width: Some(u16::try_from(width).map_err(|_| "Illustrator page width is too large")?),
        height: Some(u16::try_from(height).map_err(|_| "Illustrator page height is too large")?),
        ..Default::default() // Keep the PDF canvas transparent for PNG and JPEG compositing.
    };
    let cache = RenderCache::new();
    let interpreter = InterpreterSettings::default();
    let pixmap = render(page, &cache, &interpreter, &settings);
    let mut rgba = pixmap.data_as_u8_slice().to_vec();
    // vello_cpu stores premultiplied RGBA; image's encoders take straight RGBA.
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = if alpha == 0 {
                0
            } else {
                ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8
            };
        }
    }
    RgbaImage::from_raw(width, height, rgba)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "invalid Illustrator page raster dimensions".to_string())
}

fn open_pdf(bytes: &[u8]) -> Result<Pdf, String> {
    if bytes.len() > IMAGE_CONVERT_MAX_INPUT_BYTES {
        return Err(over_cap_error(
            "Illustrator input",
            IMAGE_CONVERT_MAX_INPUT_BYTES,
        ));
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(RESAVE_GUIDANCE.to_string());
    }
    Pdf::new(bytes.to_vec()).map_err(|error| match error {
        LoadPdfError::Decryption(_) => "password-protected Illustrator PDF is unsupported".into(),
        LoadPdfError::Invalid => "could not open Illustrator PDF representation".into(),
    })
}

fn first_page(pdf: &Pdf) -> Result<&Page<'_>, String> {
    pdf.pages()
        .first()
        .ok_or_else(|| "Illustrator PDF has no pages".to_string())
}

fn reject_placeholder(page: &Page<'_>) -> Result<(), String> {
    // Adobe's no-PDF-content placeholder can be a valid, renderable PDF page.
    let mut shown_text = Vec::new();
    let mut remaining_bytes = MAX_SCANNED_CONTENT_BYTES;
    let mut forms = 0;
    match page.raw().get::<Object<'_>>(b"Contents") {
        Some(Object::Stream(stream)) => scan_content(
            &stream,
            page.resources(),
            0,
            &mut remaining_bytes,
            &mut forms,
            &mut shown_text,
        )?,
        Some(Object::Array(streams)) => {
            for stream in streams.iter::<Stream<'_>>() {
                scan_content(
                    &stream,
                    page.resources(),
                    0,
                    &mut remaining_bytes,
                    &mut forms,
                    &mut shown_text,
                )?;
            }
        }
        None => {}
        _ => return Err("invalid Illustrator PDF page content".into()),
    }
    if contains(&shown_text, PDF_PLACEHOLDER_TEXT) && contains(&shown_text, PDF_COMPATIBLE_OPTION) {
        return Err(RESAVE_GUIDANCE.to_string());
    }
    Ok(())
}

fn scan_content(
    stream: &Stream<'_>,
    resources: &Resources<'_>,
    depth: usize,
    remaining_bytes: &mut usize,
    forms: &mut usize,
    shown_text: &mut Vec<u8>,
) -> Result<(), String> {
    let content = decoded_content(stream, remaining_bytes)?;
    let mut in_text = false;
    let mut operations = UntypedIter::new(&content);
    while let Some(instruction) = operations.next() {
        match &**instruction.operator {
            b"BT" => in_text = true,
            b"ET" => in_text = false,
            b"Tj" | b"TJ" | b"'" | b"\"" if in_text => {
                for operand in instruction.operands() {
                    match operand {
                        Object::String(value) => append_text(shown_text, value.as_bytes()),
                        Object::Array(values) => {
                            let mut items = values.flex_iter();
                            while let Some(value) = items.next::<Object<'_>>() {
                                if let Object::String(value) = value {
                                    append_text(shown_text, value.as_bytes());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            b"Do" => {
                let Some(Object::Name(name)) = instruction.operands().next() else {
                    continue;
                };
                let Some(form) = resources.get_x_object(name) else {
                    continue;
                };
                if form
                    .dict()
                    .get::<Name<'_>>(b"Subtype")
                    .is_none_or(|subtype| subtype.as_ref() != b"Form")
                {
                    continue;
                }
                if depth >= MAX_FORM_DEPTH || *forms >= MAX_SCANNED_FORMS {
                    return Err("Illustrator PDF has too many nested forms".into());
                }
                *forms += 1;
                let form_resources = Resources::from_parent(
                    form.dict()
                        .get::<Dict<'_>>(b"Resources")
                        .unwrap_or_default(),
                    (*resources).clone(),
                );
                scan_content(
                    &form,
                    &form_resources,
                    depth + 1,
                    remaining_bytes,
                    forms,
                    shown_text,
                )?;
            }
            _ => {}
        }
        if shown_text.len() >= MAX_PLACEHOLDER_TEXT_BYTES {
            break;
        }
    }
    Ok(())
}

fn decoded_content(stream: &Stream<'_>, remaining_bytes: &mut usize) -> Result<Vec<u8>, String> {
    let raw = stream.raw_data();
    let filters = stream.filters();
    let content = match filters.as_slice() {
        [] if stream.dict().get::<Object<'_>>(b"Filter").is_none() => {
            if raw.len() > *remaining_bytes {
                return Err("Illustrator PDF content exceeds the scan limit".into());
            }
            raw.into_owned()
        }
        [Filter::FlateDecode] => read_capped(
            flate2::read::ZlibDecoder::new(raw.as_ref()),
            *remaining_bytes,
            "Illustrator PDF content",
        )?,
        _ => return Err("unsupported Illustrator PDF content filter".into()),
    };
    *remaining_bytes -= content.len();
    Ok(content)
}

fn checked_dimensions(page: &Page<'_>, scale: f32) -> Result<(u32, u32), String> {
    let (width, height) = page.render_dimensions();
    let width = f64::from(width * scale).floor();
    let height = f64::from(height * scale).floor();
    if !width.is_finite()
        || !height.is_finite()
        || width < 1.0
        || height < 1.0
        || width > f64::from(MAX_IMAGE_DIMENSION)
        || height > f64::from(MAX_IMAGE_DIMENSION)
        || width * height > MAX_IMAGE_PIXELS_F64
    {
        return Err("Illustrator page dimensions exceed the raster limit".into());
    }
    let dimensions = (width as u32, height as u32);
    checked_pixel_area("Illustrator page", dimensions.0, dimensions.1)?;
    Ok(dimensions)
}

fn append_text(out: &mut Vec<u8>, text: &[u8]) {
    let remaining = MAX_PLACEHOLDER_TEXT_BYTES.saturating_sub(out.len());
    out.extend(
        text.iter()
            .copied()
            .filter(u8::is_ascii_alphanumeric)
            .map(|byte| byte.to_ascii_lowercase())
            .take(remaining),
    );
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests;
