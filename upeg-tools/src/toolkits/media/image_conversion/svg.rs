//! SVG rendering with bundled fonts and explicit resource resolution.
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use image::DynamicImage;
use resvg::{tiny_skia, usvg};

use super::super::limits::{
    MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS, checked_pixel_area, over_cap_error,
};

const MAX_SVG_BYTES: usize = 4 << 20;
const SVG_LABEL: &str = "SVG raster";
const DEFAULT_FONT_FAMILY: &str = "D2Coding";
static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();

fn fonts() -> Arc<usvg::fontdb::Database> {
    Arc::clone(FONTS.get_or_init(|| {
        let mut fonts = usvg::fontdb::Database::new();
        // Reuse the fonts already distributed with UPeg, with their OFL notices.
        fonts.load_font_data(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../flutter_app/fonts/D2Coding.ttf"
            ))
            .to_vec(),
        );
        fonts.set_sans_serif_family(DEFAULT_FONT_FAMILY);
        fonts.set_serif_family(DEFAULT_FONT_FAMILY);
        fonts.set_monospace_family(DEFAULT_FONT_FAMILY);
        Arc::new(fonts)
    }))
}

fn tree(bytes: &[u8]) -> Result<usvg::Tree, String> {
    if bytes.len() > MAX_SVG_BYTES {
        return Err(over_cap_error("SVG input", MAX_SVG_BYTES));
    }
    let rejected = AtomicBool::new(false);
    let embedded_pixels = AtomicUsize::new(0);
    let default_data = usvg::ImageHrefResolver::default_data_resolver();
    let options = usvg::Options {
        font_family: DEFAULT_FONT_FAMILY.into(),
        fontdb: fonts(),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_string: Box::new(|_, _| {
                rejected.store(true, Ordering::Relaxed);
                None
            }),
            resolve_data: Box::new(|mime, data, options| {
                // Embedded SVG recursion is excluded. Bound embedded rasters before resvg decodes them.
                let format = image::guess_format(&data).ok();
                if !matches!(
                    format,
                    Some(
                        image::ImageFormat::Png
                            | image::ImageFormat::Jpeg
                            | image::ImageFormat::Gif
                            | image::ImageFormat::WebP
                    )
                ) {
                    rejected.store(true, Ordering::Relaxed);
                    return None;
                }
                let conversion = super::ImageConversionOptions::defaults("png").ok()?;
                let pixels = super::pixel_count("SVG embedded image", &data, conversion).ok();
                if pixels.is_none_or(|pixels| {
                    embedded_pixels
                        .fetch_add(pixels, Ordering::Relaxed)
                        .saturating_add(pixels)
                        > MAX_IMAGE_PIXELS
                }) {
                    rejected.store(true, Ordering::Relaxed);
                    return None;
                }
                default_data(mime, data, options)
            }),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options)
        .map_err(|error| format!("could not parse SVG: {error}"))?;
    if rejected.load(Ordering::Relaxed) {
        return Err("SVG references an external or unsupported image; embed bounded PNG/JPEG/GIF/WebP images instead".into());
    }
    Ok(tree)
}

fn dimensions(tree: &usvg::Tree, requested_width: u32) -> Result<(u32, u32), String> {
    let size = tree.size();
    let width = if requested_width == 0 {
        f64::from(size.width()).ceil()
    } else {
        f64::from(requested_width)
    };
    let height = (f64::from(size.height()) * width / f64::from(size.width())).ceil();
    if !width.is_finite()
        || !height.is_finite()
        || width < 1.0
        || height < 1.0
        || width > f64::from(MAX_IMAGE_DIMENSION)
        || height > f64::from(MAX_IMAGE_DIMENSION)
    {
        return Err("SVG output dimensions are outside the supported range".into());
    }
    let dimensions = (width as u32, height as u32);
    checked_pixel_area(SVG_LABEL, dimensions.0, dimensions.1)?;
    Ok(dimensions)
}

pub(super) fn pixel_count(bytes: &[u8], width: u32) -> Result<usize, String> {
    let tree = tree(bytes)?;
    let (width, height) = dimensions(&tree, width)?;
    checked_pixel_area(SVG_LABEL, width, height)
}

pub(super) fn decode(bytes: &[u8], width: u32) -> Result<DynamicImage, String> {
    let tree = tree(bytes)?;
    let (width, height) = dimensions(&tree, width)?;
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or("could not allocate SVG raster")?;
    let scale = width as f32 / tree.size().width();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia stores premultiplied RGBA; image encoders expect straight RGBA.
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(width, height, rgba)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "invalid SVG raster size".into())
}
