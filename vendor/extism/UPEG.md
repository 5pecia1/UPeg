# UPeg vendored extism

Source: https://crates.io/crates/extism/1.21.0
Archive SHA-256: `ed8c5859bdab81d2eb4cd963eeacd8031d353b1ffb2fde43ee9179a0d6295120`.
Published from https://github.com/extism/extism commit
`9afa572969bdbb089e363b898a46c24718f47ee8` (`runtime/`).

This is the published crate, not GitHub main. Cargo cache metadata, the
upstream lockfile and Cargo.toml.orig are omitted. `LICENSE` is the
repository's BSD-3-Clause file at the publish commit (the crate tarball does
not ship one). All upstream source, tests, examples and benches are retained
byte-for-byte; the crate is excluded from workspace membership and UPeg's
hand-written source line budget.

Why vendored: every published extism release pins one wasmtime major line
(1.13/1.20 → 37, 1.21 → 41, 1.30 → 43). None of those lines has a release that
clears the 2026 wasmtime RUSTSEC advisories (RUSTSEC-2026-0085…0096, 0114,
0222, 0269); the patched lines are 36.0.14+ (LTS, rustc 1.86) and 46.0.3+ /
47.0.4+ (rustc 1.94, above UPeg's pinned 1.92). Only the 36 LTS line is both
patched and buildable here, and no extism release targets it.

Local patches:

- `Cargo.toml`: `wasmtime`, `wasi-common` and `wiggle` requirements moved
  from `"41"` to `"36"`. The 1.21.0 source compiles against wasmtime 36
  unchanged: it still uses the anyhow-based `wasmtime::Error` that 36 and 41
  share (the `wasmtime::Error` migration only landed in extism 1.30, which
  targets 43). The exact patch release is pinned in the root `Cargo.lock`.
- `Cargo.toml`: `build = false` and the `cbindgen` build-dependency removed;
  `build.rs` and the generated `extism.h` are omitted. The C header serves the
  non-Rust SDKs only, and a vendored build script would rewrite a tracked
  file on every build.
- `Cargo.toml`: the `wasmtime-default-features` feature lists wasmtime 36's
  `default` features explicitly with `profiling` dropped — UPeg does not use
  guest profiling, and `profiling` pulls `fxprof-processed-profile` → `fxhash`
  (RUSTSEC-2025-0057, unmaintained).

Behaviour is otherwise upstream 1.21.0. UPeg uses `Manifest`, `Wasm::data`,
`Plugin::new(_, [], false)` (no WASI) and `Plugin::call`; the plugin wire
contract is covered by `upeg-wasm`'s unit and `tests/wasm_e2e.rs` tests.

BSD-3-Clause license: `LICENSE`; preserve it with distributions.

For updates, re-extract the published crate, reapply the two manifest
patches above, run `cargo update -p wasmtime --precise <36.x>` (with
`wasi-common` first, since it pins `wiggle` exactly) and run
`cargo test -p upeg-wasm` plus `cargo check -p upeg-cli --features wasm-plugin`.
Drop this directory once an extism release targets a patched wasmtime line
that builds on UPeg's toolchain.
