# UPeg vendored pdf-inspector

Source: https://crates.io/crates/pdf-inspector/1.17.0
Archive SHA-256: `6cdfc6057e1b38a2ae84490c5e64abc5c81738d4d5ac1ccc55cf1a2c9b87334e`.

This is the published crate, not GitHub main. The published package uses
lopdf 0.42; main used 0.44 at review time. Cargo cache metadata, the upstream
lockfile and Cargo.toml.orig are omitted. All upstream source and notices
are retained. The crate is excluded from workspace membership and UPeg's
hand-written source line budget; upstream files are kept intact for updates.

Local patches:

- `Cargo.toml`, `src/tounicode.rs`: make include_dir and the bundled CMaps
  unconditional, replacing native runtime filesystem/environment discovery.
  Installed binaries must decode CJK without a developer's Cargo cache or
  process-global environment mutation. This adds approximately 1.7 MiB of
  source assets before linker/compression effects. Actual artifact growth
  has not been measured.
- `src/tounicode/builtin_unicode.rs`, `src/tounicode.rs`: correct Unicode
  bcmap record decoding (raw first values, delta/zigzag following values,
  sequential ranges and UTF-16 surrogate pairs). Upstream misread raw values
  as varints and did not apply deltas; the Japan1 regression returned literal
  CID codepoints instead of Unicode. The separate encoding-CMap reader is
  unchanged. Reference format: https://github.com/mozilla/pdf.js/blob/master/src/core/binary_cmap.js
- `src/lib.rs`: add `process_pdf_mem_with_page_limit`. It checks the page
  count after parsing, before detection/extraction, and reuses the parsed
  document. Upstream APIs and defaults remain unchanged.
- `Cargo.toml`: `lopdf` bumped from `0.42.0` to `0.44.0` on both targets
  (matching upstream main at review time). In 0.44 `ttf-parser` moved behind
  the optional `font_embedding` feature, which nothing here enables.
- `Cargo.toml`, `src/sfnt.rs` (new), `src/tounicode.rs`, `src/detector.rs`,
  `src/extractor/fonts.rs`: `ttf-parser` (RUSTSEC-2026-0192, unmaintained)
  replaced by `skrifa` 0.44 / `read-fonts` 0.41, already in UPeg's graph via
  hayro/usvg. `src/sfnt.rs` reimplements the exact `ttf_parser` semantics
  used upstream — cmap subtable iteration and `is_unicode`, `glyph_index`,
  `codepoints`-has-mappings, post italic angle, OS/2 fsSelection italic/bold,
  maxp glyph count — and `skrifa::GlyphNames` supplies the post/CFF glyph
  names `face.glyph_name` provided (synthesized `gidNNN` names are skipped
  where `ttf_parser` returned `None`).

No OCR features are enabled. MIT license: `LICENSE`. Adobe CMap redistribution
notice: `external/bcmaps/LICENSE`; preserve both with distributions.

For updates, re-extract the published crate, compare the patched files above,
reapply the patches and run UPeg's PDF tests on native plus the FRB WASM build.
The application tests include Korea1 and Japan1 fallback decoding.
