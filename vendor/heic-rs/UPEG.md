# UPeg vendored heic-rs

Source: https://crates.io/crates/heic-rs/0.1.1
Published archive SHA-256: `adf9dde634bb6d5b718bacd01bef80192f2334306b2f785519a0136cca7b9618`.

This directory retains the published 0.1.1 `src/` code, MIT and Apache-2.0
licenses, and `NOTICE`. Its Cargo manifest keeps the published package identity
and features but omits unused examples, benchmarks, and development dependencies.
The crate is excluded from workspace membership and UPeg's hand-written source
line budget. `tests/fixtures/flat-64.heic` and `checker-1024.heic` are from
upstream Git commit `4f0d4df474c773dfc3b14fa80e7219be6898866e`, since
the published crate excludes binary fixtures. Upstream's
`tests/fixtures/README.md` states that these generated fixtures are licensed
MIT OR Apache-2.0. Their SHA-256 hashes are, respectively,
`832f19c74d689d7b9d235f5dfe6e81ed824d60900f0360d1953b413b736816ac`
and `f24a3b6ae495605e179bd34ae282047dcc4581819eb53041db34c9fdb66648cd`.

Local patch:

- `src/decode.rs`, `src/image.rs`, `src/lib.rs`, `src/hevc/mod.rs`,
  `src/hevc/decode/mod.rs`, and `src/hevc/error.rs`: set the default and hard
  ceiling to 40 Mi-pixels. Pass the caller's limit to the selected HEVC SPS
  and check its uncropped dimensions before allocating `Geometry` or `Picture`.
  The upstream check covers container `ispe` or grid dimensions, which may
  disagree with the SPS. Apply the hard ceiling to output dimensions even when
  callers choose `max_pixels = None` or call `hevc::decode_still` directly.
  Check grid payload dimensions and the sum of every tile's largest SPS before
  decoding tiles, including in parallel. An auxiliary alpha picture shares
  the coded-pixel budget with the primary picture.
- `tests/limits.rs` and `src/hevc/mod.rs` tests: reject a real HEIC whose
  `ispe` has been reduced below its SPS, a similarly altered grid, a grid
  whose combined tile allocation exceeds the budget, an oversized `ispe`
  with `max_pixels = None`, and a synthetic 8192×8192 SPS through the public
  HEVC entry point.

Check with `cargo test --manifest-path vendor/heic-rs/Cargo.toml
--no-default-features` and `cargo check --manifest-path vendor/heic-rs/Cargo.toml
--target wasm32-unknown-unknown --no-default-features`. Re-evaluate this patch
when upstream adds a pre-allocation coded-pixel limit.
