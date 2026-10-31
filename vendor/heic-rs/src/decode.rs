//! [`decode`]: container to pixels.
//!
//! Resolving the primary item, splitting NAL units, decoding each coded
//! picture through [`crate::hevc::decode_still`], composing a grid, converting
//! colour, applying transforms — in that order, and with the declared size
//! checked against the caller's ceiling before any of it allocates.

use alloc::vec::Vec;

use crate::color::{self, Source};
use crate::context::Context;
use crate::error::{Error, Result};
use crate::grid::{self, Grid, Mosaic};
use crate::hevc::{self, Frame};
use crate::image::{DecodeOptions, Image, check_pixels};
use crate::parallel;
use crate::props::ItemProps;
use crate::props::colr::Nclx;
use crate::transform;

/// Decode the primary image of a HEIF file.
///
/// # Errors
///
/// Container-level failures are reported before anything is decoded, so a
/// malformed file costs nothing. A file whose bitstream uses a coding tool
/// this decoder does not implement comes back as [`Error::Unsupported`] naming
/// that tool.
pub fn decode(bytes: &[u8], options: &DecodeOptions) -> Result<Image> {
    // One thread pool for the whole decode, not one per stage. Inside, the
    // caller's choice has already been honoured, so `threads` there means
    // "the pool we are in" or "this thread" and nothing else.
    parallel::scope(options.threads, |threads| run(bytes, options, threads))
}

/// The decode itself, already inside whatever thread pool the caller asked
/// for.
fn run(bytes: &[u8], options: &DecodeOptions, threads: Option<usize>) -> Result<Image> {
    let ctx = Context::open(bytes)?;
    let id = ctx.meta.primary;
    let p = ctx.props(id)?;
    if options.strict && ctx.meta.props.unknown_essential(id).is_some() {
        return Err(Error::Unsupported(
            "the primary item carries an essential property this build does not implement",
        ));
    }
    let limit = hevc::coded_pixel_limit(options.pixel_limit());
    let (cw, ch) = ctx.coded_size(id, &p)?;
    // Refuse the declared size before decoding anything, so that a small file
    // claiming an enormous image costs nothing.
    check_pixels(cw, ch, limit)?;
    let (tw, th) = transform::transformed_size(cw, ch, &p.transforms)?;
    check_pixels(tw, th, limit)?;

    let (decoded, primary_coded_pixels) = decode_item(&ctx, id, &p, limit, threads)?;
    let alpha = alpha_frame(
        &ctx,
        id,
        options,
        limit.saturating_sub(primary_coded_pixels),
        threads,
    )?;
    let nclx = p.nclx.unwrap_or_default();
    let (layout, alpha) = (options.layout, alpha.as_ref());
    let image = match &decoded {
        Decoded::Picture(frame) => {
            color::convert_source(&Source::Frame(frame), alpha, nclx, layout, limit, threads)?
        }
        // The tiles are read in place: a canvas would be allocated, zeroed
        // and filled only for the colour pass to read it straight back.
        Decoded::Tiles(g, tiles) => {
            let mosaic = Mosaic::new(g, tiles)?;
            color::convert_source(
                &Source::Mosaic(&mosaic),
                alpha,
                nclx,
                layout,
                limit,
                threads,
            )?
        }
    };
    if options.apply_transforms {
        transform::apply_all(image, &p.transforms)
    } else {
        Ok(image)
    }
}

/// What decoding an item produced.
enum Decoded {
    /// One coded picture, validated.
    Picture(Frame),
    /// A grid's tiles in `dimg` order, each validated, not yet composed.
    Tiles(Grid, Vec<Frame>),
}

impl Decoded {
    /// One frame, composing a grid's tiles if there are any.
    fn into_frame(self, limit: u64) -> Result<Frame> {
        match self {
            Decoded::Picture(frame) => Ok(frame),
            Decoded::Tiles(g, tiles) => grid::compose(&g, &tiles, limit),
        }
    }
}

/// Decode one item, following a `grid` derivation when there is one.
///
/// `p` is the item's own properties, gathered once by the caller; a single
/// coded picture is decoded straight from them rather than reading its
/// `iprp` entry a second time. A grid's tiles are independent coded pictures:
/// nothing in one tile's bitstream refers to another, so they are decoded
/// together when the caller allows it. The tiles come back in `dimg` order,
/// which is what [`Mosaic`] and [`grid::compose`] place them by, so the result
/// does not depend on the order they happened to finish in.
fn decode_item(
    ctx: &Context<'_>,
    id: u32,
    p: &ItemProps<'_>,
    limit: u64,
    threads: Option<usize>,
) -> Result<(Decoded, u64)> {
    match ctx.grid(id)? {
        Some((g, tiles)) => {
            // `ispe` and the grid payload can disagree. The payload controls
            // the mosaic allocation, so limit that size before decoding tiles.
            check_pixels(g.output_width, g.output_height, limit)?;
            // Tiles remain resident together for mosaic conversion. Derive a
            // conservative total from every tile's uncropped SPS before any
            // tile is decoded, including on the parallel path.
            let coded_limit = hevc::coded_pixel_limit(limit);
            let mut coded_total = 0_u64;
            for tile in &tiles {
                let tile_props = ctx.props(*tile)?;
                let hvcc = tile_props.hvcc.as_ref().ok_or(Error::MissingBox("hvcC"))?;
                let pixels = hevc::max_coded_pixels(&hvcc.parameter_sets())?;
                coded_total = coded_total.saturating_add(pixels);
                if coded_total > coded_limit {
                    return Err(Error::PixelLimit {
                        pixels: coded_total,
                        max_pixels: coded_limit,
                    });
                }
            }
            let frames = parallel::try_map(&tiles, threads, |t| {
                decode_coded(ctx, *t, &ctx.props(*t)?, limit)
            })?;
            Ok((Decoded::Tiles(g, frames), coded_total))
        }
        None => {
            let hvcc = p.hvcc.as_ref().ok_or(Error::MissingBox("hvcC"))?;
            let pixels = hevc::max_coded_pixels(&hvcc.parameter_sets())?;
            let coded_limit = hevc::coded_pixel_limit(limit);
            if pixels > coded_limit {
                return Err(Error::PixelLimit {
                    pixels,
                    max_pixels: coded_limit,
                });
            }
            decode_coded(ctx, id, p, limit).map(|frame| (Decoded::Picture(frame), pixels))
        }
    }
}

/// Decode a single coded picture item through the codec seam.
fn decode_coded(ctx: &Context<'_>, id: u32, p: &ItemProps<'_>, limit: u64) -> Result<Frame> {
    let hvcc = p.hvcc.as_ref().ok_or(Error::MissingBox("hvcC"))?;
    let data = ctx.item_data(id)?;
    let nals = hvcc.split_nals(&data)?;
    let params = hvcc.parameter_sets();
    if params.is_empty() {
        return Err(Error::Malformed("hvcC carries no parameter sets"));
    }
    let frame = hevc::decode_still_with_limit(&params, &nals, limit)?;
    frame.validate()?;
    Ok(frame)
}

/// Decode the alpha auxiliary item, when the caller asked for alpha and the
/// requested layout has somewhere to put it.
fn alpha_frame(
    ctx: &Context<'_>,
    id: u32,
    options: &DecodeOptions,
    limit: u64,
    threads: Option<usize>,
) -> Result<Option<Frame>> {
    if !options.decode_alpha || !options.layout.has_alpha() {
        return Ok(None);
    }
    let Some(aux) = ctx.alpha_item(id)? else {
        return Ok(None);
    };
    let aux_props = ctx.props(aux)?;
    // The alpha kernel reads one plane, so a tiled alpha item is composed;
    // alpha grids are rare enough that the canvas is not worth avoiding.
    decode_item(ctx, aux, &aux_props, limit, threads)?
        .0
        .into_frame(limit)
        .map(Some)
}

/// The colour description that will be used for an item, including the
/// documented fallback for files that carry no `colr`.
pub fn effective_nclx(p: &ItemProps<'_>) -> Nclx {
    p.nclx.unwrap_or_default()
}
