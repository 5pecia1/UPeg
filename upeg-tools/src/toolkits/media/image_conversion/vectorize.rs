use std::{fmt, io::Write};

use image::{DynamicImage, GenericImageView, Pixel};

use super::{ImageConversionOptions, VectorPreset};
use crate::toolkits::media::limits::CappedBuffer;

const MAX_TRACE_EDGE: u32 = 1024;
const MAX_TRACE_PATHS: usize = 50_000;
const MAX_TRACE_POINTS: usize = 500_000;
const MAX_TRACE_EDGES: usize = 300_000;
const MAX_TRACE_CLUSTERS: usize = 100_000;

struct BoundedPath<'a>(&'a vtracer::SvgPath, Option<u32>);
impl fmt::Display for BoundedPath<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_with_precision(formatter, self.1)
    }
}

pub(super) fn encode(
    raster: &DynamicImage,
    options: ImageConversionOptions,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let (width, height) = raster.dimensions();
    let mut composited = raster.to_rgba8();
    for pixel in composited.pixels_mut() {
        let mut background = options.background;
        background.blend(pixel);
        *pixel = background;
    }
    let opaque = DynamicImage::ImageRgba8(composited);
    let pixels = if width > MAX_TRACE_EDGE || height > MAX_TRACE_EDGE {
        opaque
            .thumbnail(MAX_TRACE_EDGE, MAX_TRACE_EDGE)
            .into_rgba8()
    } else {
        opaque.into_rgba8()
    };
    let img = vtracer::ColorImage {
        width: pixels.width() as usize,
        height: pixels.height() as usize,
        pixels: pixels.into_raw(),
    };
    let preset = match options.vector_preset {
        VectorPreset::Poster => vtracer::Preset::Poster,
        VectorPreset::Photo => vtracer::Preset::Photo,
        VectorPreset::Bw => vtracer::Preset::Bw,
    };
    let limits = vtracer::TraceLimits {
        max_edges: MAX_TRACE_EDGES,
        max_clusters: MAX_TRACE_CLUSTERS,
        max_paths: MAX_TRACE_PATHS,
        max_points: MAX_TRACE_POINTS,
    };
    let svg = vtracer::convert_bounded(img, vtracer::Config::from_preset(preset), limits)
        .map_err(|error| format!("could not vectorize image: {error}"))?;
    let mut output = CappedBuffer::new(limit, "vectorized SVG");
    writeln!(output, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {} {}\">", svg.width, svg.height)
        .map_err(|error| format!("could not write SVG: {error}"))?;
    let [r, g, b, _] = options.background.0;
    writeln!(
        output,
        "<path fill=\"#{r:02X}{g:02X}{b:02X}\" d=\"M0 0H{}V{}H0Z\"/>",
        svg.width, svg.height
    )
    .map_err(|error| format!("could not write SVG: {error}"))?;
    for path in &svg.paths {
        write!(output, "{}", BoundedPath(path, svg.path_precision))
            .map_err(|error| format!("could not write SVG: {error}"))?;
    }
    output
        .write_all(b"</svg>\n")
        .map_err(|error| format!("could not write SVG: {error}"))?;
    Ok(output.into_bytes())
}
