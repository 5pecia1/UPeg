# UPeg vendored VTracer

Source: https://crates.io/crates/vtracer/0.6.5
Archive SHA-256: `c2307187c5c99e7387f3158a29b0be541b99b06c5c492598b77644bf5603bd0c`.
Source revision: `8acb6bd911e160a55cd8a5503cfac54550a429f8` (upstream `cmdapp`).

Only the upstream in-memory conversion library is retained. CLI, Python binding,
file loading/saving, and their clap/image/pyo3 dependencies were removed; UPeg
provides decoded pixels and handles bounded output. Tracing algorithms are
unchanged. A bounded conversion entry point checks pixel edge complexity before
clustering, checks cluster counts between smaller runner batches, and stops path
generation on path and accumulated point limits. A single batch or generated
path may temporarily exceed its respective limit. `SvgPath::fmt_with_precision`
is public so UPeg can stream each path with configured precision into its capped
output writer. Both upstream license texts are included. Remove this vendored
copy when a stable upstream release exposes a dependency-minimal in-memory library.
