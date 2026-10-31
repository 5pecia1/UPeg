# UPeg vendored wasmtime cache

Source: https://crates.io/crates/wasmtime-internal-cache/36.0.20
Archive SHA-256: `65f09c789d1bb2e7ab1618ac8336d783c1ff0babc38430c7f1940d01c546722b`.
License: Apache-2.0 WITH LLVM-exception; see `LICENSE`.

The published Rust source is unchanged from 36.0.19; only the package version
differs. Cargo cache metadata, the upstream lockfile and original manifest are
omitted. The normalized manifest changes zstd from 0.13 to 0.14.3, resolving
RustSec advisories in the old compression stack while retaining Wasmtime 36's
compilation cache. Remove this patch when a supported upstream version no
longer depends on affected zstd releases.
