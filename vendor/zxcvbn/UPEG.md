# UPeg vendored zxcvbn

Source: https://crates.io/crates/zxcvbn/3.1.1
Archive SHA-256: `f9eaee90f4a795d1eb4ba6c51e1c1721d4784d550e8efa7b2600f29c867365e0`.

This is the published 3.1.1 crate, including its frequency lists and MIT
`LICENSE`, with the upstream `Cargo.lock` omitted. It is excluded from
workspace membership and UPeg's hand-written source line budget.

Local patches:

- `src/lib.rs`: the wasm `performance` import uses wasm-bindgen's documented
  `thread_local_v2` accessor and calls `performance.with(|p| p.now())` for
  both timing reads. wasm-bindgen intentionally removes `JsStatic`'s `Deref`
  when `target_feature="atomics"`; the previous direct static access therefore
  cannot compile in the Flutter Rust Bridge worker build. The replacement does
  not require `Window`, so it remains valid in a Worker, and preserves the
  upstream timing algorithm.
- `Cargo.toml`, `src/scoring.rs`: remove the WASM-only chrono dependency.
  The `custom_wasm_env` imported Unix milliseconds are converted to their UTC
  year with `time::OffsetDateTime`, already used by this crate's native clock.
  Only that opt-in feature enables `time/large-dates`; an explicit imported-clock
  bound retains the published range through `+262142-12-31T23:59:59.999Z` and
  its rejection above that limit. The browser `Date.get_full_year()` path is
  unchanged. Native unit tests cover the epoch, UTC year boundary, year 10000,
  and both sides of the published upper bound.

Remove this directory and restore the crates.io dependency once an upstream
zxcvbn release includes the atomics-compatible Worker-safe timer and removes
its chrono dependency.
