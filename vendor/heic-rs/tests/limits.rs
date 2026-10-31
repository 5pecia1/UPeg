use heic_rs::{DecodeOptions, Error, decode, probe};

// Upstream Git checkout fixture; the published crate intentionally omits HEIC
// files. Mutating only `ispe` makes the container claim a 1x1 image while the
// HEVC SPS still requests a much larger coded allocation.
const FLAT_64: &[u8] = include_bytes!("fixtures/flat-64.heic");
const GRID_1024: &[u8] = include_bytes!("fixtures/checker-1024.heic");

fn set_ispe_size(bytes: &mut [u8], occurrence: usize, width: u32, height: u32) {
    let ispe = bytes
        .windows(4)
        .enumerate()
        .filter(|(_, window)| *window == b"ispe")
        .nth(occurrence)
        .map(|(index, _)| index)
        .expect("fixture has requested ispe");
    bytes[ispe + 8..ispe + 12].copy_from_slice(&width.to_be_bytes());
    bytes[ispe + 12..ispe + 16].copy_from_slice(&height.to_be_bytes());
}

#[test]
fn coded_sps_is_limited_even_when_ispe_understates_size() {
    let mut bytes = FLAT_64.to_vec();
    set_ispe_size(&mut bytes, 0, 1, 1);

    let info = probe(&bytes).expect("mutated container still probes");
    assert_eq!((info.coded_width, info.coded_height), (1, 1));
    let options = DecodeOptions::default().with_max_pixels(Some(1));
    assert!(matches!(
        decode(&bytes, &options),
        Err(Error::PixelLimit {
            pixels,
            max_pixels: 1,
        }) if pixels > 1
    ));
}

#[test]
fn removing_callers_limit_keeps_library_hard_ceiling() {
    let mut bytes = FLAT_64.to_vec();
    set_ispe_size(&mut bytes, 0, 65_535, 65_535);
    let options = DecodeOptions::default().with_max_pixels(None);
    assert_eq!(
        decode(&bytes, &options),
        Err(Error::PixelLimit {
            pixels: 65_535_u64 * 65_535_u64,
            max_pixels: heic_rs::DEFAULT_MAX_PIXELS,
        })
    );
}

fn shrink_grid_output(bytes: &mut [u8]) {
    set_ispe_size(bytes, 1, 1, 1);
    let grid = bytes
        .windows(8)
        .position(|window| window == [0, 0, 1, 1, 4, 0, 4, 0])
        .expect("fixture has a 2x2 1024x1024 grid payload");
    bytes[grid + 4..grid + 8].copy_from_slice(&[0, 1, 0, 1]);
}

#[test]
fn grid_tile_sps_is_limited_even_when_primary_ispe_understates_size() {
    let mut bytes = GRID_1024.to_vec();
    // The first ispe belongs to the coded tile; the second to the grid item.
    // The grid item payload also declares its output size. Make it 1x1 so the
    // test reaches the tile SPS rather than stopping at the grid size check.
    shrink_grid_output(&mut bytes);
    let info = probe(&bytes).expect("mutated grid still probes");
    assert!(info.is_grid);
    assert_eq!((info.coded_width, info.coded_height), (1, 1));

    let options = DecodeOptions::default().with_max_pixels(Some(1));
    assert!(matches!(
        decode(&bytes, &options),
        Err(Error::PixelLimit {
            pixels: 262_144,
            max_pixels: 1,
        })
    ));
}

#[test]
fn grid_checks_combined_coded_tile_pixels_before_decoding() {
    let mut bytes = GRID_1024.to_vec();
    shrink_grid_output(&mut bytes);
    let options = DecodeOptions::default().with_max_pixels(Some(262_144));
    assert_eq!(
        decode(&bytes, &options),
        Err(Error::PixelLimit {
            pixels: 524_288,
            max_pixels: 262_144,
        })
    );
}
