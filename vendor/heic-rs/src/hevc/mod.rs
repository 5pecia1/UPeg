//! The HEVC (ITU-T H.265) still-picture intra decoder.
//!
//! This module decodes one intra picture — Main, Main 10 or Main Still
//! Picture, 4:2:0, 4:2:2, 4:4:4 or monochrome, 8 or 10 bits — from the
//! parameter sets and coded slices of an IRAP access unit. Like every other
//! module in the crate it performs no I/O: [`decode_still`] takes bytes and
//! returns samples. Inter prediction, P and B slices and multi-picture
//! sequences are refused with [`Error::Unsupported`], naming the tool that was
//! asked for.
//!
//! [`Error::Unsupported`]: crate::Error::Unsupported
//!
//! # Contract
//!
//! `parameter_sets` are the VPS, SPS and PPS NAL units taken from the item's
//! `hvcC` property, in that order, each without a length prefix or start code.
//! `slices` are the remaining NAL units of the item's data, already split on
//! their length prefixes. Both borrow from the caller; the decoder does not
//! assume they outlive the call.
//!
//! # Errors
//!
//! The decoder has its own, narrower notion of failure, defined in the private
//! `error` submodule. It never reaches a caller: it is converted into
//! [`crate::Error`] at this seam, carrying the same message, so that a caller
//! of [`crate::decode()`] matches on exactly one error type.

mod bits;
mod cabac;
mod decode;
mod error;
mod filter;
mod frame;
mod intra;
mod nal;
mod picture;
mod ps;
mod scan;
mod slice;
mod transform;

#[cfg(any(test, feature = "bench"))]
#[doc(hidden)]
pub mod synth;

#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench_api;

#[cfg(test)]
mod golden;

pub use frame::{ChromaFormat, Frame, Info};

#[cfg(test)]
use nal::split_annexb;

use crate::error::Result;

// Bound coded (pre-crop) HEVC pictures even when a caller removes the output
// pixel ceiling. A small `ispe` can otherwise hide an enormous SPS allocation.
const MAX_CODED_PIXELS: u64 = crate::image::DEFAULT_MAX_PIXELS;

pub(crate) const fn coded_pixel_limit(requested: u64) -> u64 {
    if requested < MAX_CODED_PIXELS {
        requested
    } else {
        MAX_CODED_PIXELS
    }
}

/// Largest uncropped SPS allocation any coded item can select.
pub(crate) fn max_coded_pixels(parameter_sets: &[&[u8]]) -> Result<u64> {
    let sets = decode::ParameterSets::parse(parameter_sets)?;
    sets.sps
        .iter()
        .map(|sps| (sps.width as u64) * (sps.height as u64))
        .max()
        .ok_or(crate::Error::MissingParameterSet("SPS"))
}

/// Decode a single still picture.
///
/// See the module documentation for the shape of the inputs.
pub fn decode_still(parameter_sets: &[&[u8]], slices: &[&[u8]]) -> Result<Frame> {
    decode_still_with_limit(parameter_sets, slices, MAX_CODED_PIXELS)
}

/// Decode while also enforcing the caller's pixel ceiling on the coded SPS.
pub(crate) fn decode_still_with_limit(
    parameter_sets: &[&[u8]],
    slices: &[&[u8]],
    max_pixels: u64,
) -> Result<Frame> {
    let sets = decode::ParameterSets::parse(parameter_sets)?;
    Ok(decode::decode(
        &sets,
        slices,
        coded_pixel_limit(max_pixels),
    )?)
}

/// Read the geometry out of a sequence parameter set without decoding.
pub fn probe(parameter_sets: &[&[u8]]) -> Result<Info> {
    let sets = decode::ParameterSets::parse(parameter_sets)?;
    let sps = sets.first_sps()?;
    let [l, r, t, b] = sps.crop;
    Ok(Info {
        width: sps.width.saturating_sub(l + r).max(1) as u32,
        height: sps.height.saturating_sub(t + b).max(1) as u32,
        bit_depth: sps.bit_depth_y,
        chroma: ChromaFormat::from_idc(sps.chroma_array_type).unwrap_or(ChromaFormat::Monochrome),
    })
}

/// Decode one still picture from an Annex-B byte stream.
///
/// Start codes are located, the NAL units split out and sorted into parameter
/// sets and slices, and [`decode_still`] is then applied. HEIF stores NAL
/// units with length prefixes rather than start codes, so this is used only by
/// the decoder's own tests, which speak the codec's native framing.
#[cfg(test)]
pub(crate) fn decode_annexb(stream: &[u8]) -> Result<Frame> {
    use alloc::vec::Vec;
    let units = split_annexb(stream);
    let mut sets: Vec<&[u8]> = Vec::new();
    let mut slices: Vec<&[u8]> = Vec::new();
    for u in units {
        let Ok(h) = nal::NalHeader::parse(u) else {
            continue;
        };
        if h.is_vcl() {
            slices.push(u);
        } else if matches!(h.nal_unit_type, 32..=34) {
            sets.push(u);
        }
    }
    decode_still(&sets, &slices)
}

#[cfg(test)]
mod limit_tests {
    use super::*;
    use crate::hevc::synth::{build_pps, build_slice, build_sps, build_vps, to_nal};

    #[test]
    fn public_decoder_rejects_oversized_sps_before_picture_allocation() {
        let vps = to_nal(32, &build_vps());
        let sps = to_nal(33, &build_sps(8192, 8192, 1, 8));
        let pps = to_nal(34, &build_pps());
        let slice = to_nal(19, &build_slice(1, 1));
        let result = decode_still(&[&vps, &sps, &pps], &[&slice]);
        assert_eq!(
            result.err(),
            Some(crate::Error::PixelLimit {
                pixels: 8192 * 8192,
                max_pixels: MAX_CODED_PIXELS,
            })
        );
    }
}
