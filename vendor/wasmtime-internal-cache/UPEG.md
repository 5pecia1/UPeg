# UPeg Wasmtime cache license provenance

Source: https://crates.io/crates/wasmtime-internal-cache/36.0.21
Archive SHA-256: `1ff2377df26271c42d9649ac4ee13656ef89a51f02f0daa7917e67c635d9bc1d`.
License: Apache-2.0 WITH LLVM-exception; see `LICENSE`.

The crate is resolved from crates.io and pinned by the root `Cargo.lock`.
This directory retains only its unmodified license for release packaging.

The former 36.0.20 source override raised zstd to 0.14.3. It is retired:
the registry cache now resolves zstd 0.13.5 and zstd-safe 7.4.1, whose release
lines include the required RustSec fixes. No upstream source or manifest is
patched locally. The distributed license filename remains
`wasmtime-internal-cache-LICENSE`.
