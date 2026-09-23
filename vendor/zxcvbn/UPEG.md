# UPeg vendored zxcvbn

Source: https://crates.io/crates/zxcvbn/3.1.1
Archive SHA-256: `f9eaee90f4a795d1eb4ba6c51e1c1721d4784d550e8efa7b2600f29c867365e0`.

This is the complete published 3.1.1 crate, including its frequency lists and
MIT `LICENSE`. It is excluded from workspace membership and UPeg's
hand-written source line budget.

Local patch:

- `src/lib.rs`: the wasm `performance` import uses wasm-bindgen's documented
  `thread_local_v2` accessor and calls `performance.with(|p| p.now())` for
  both timing reads. wasm-bindgen intentionally removes `JsStatic`'s `Deref`
  when `target_feature="atomics"`; the previous direct static access therefore
  cannot compile in the Flutter Rust Bridge worker build. The replacement does
  not require `Window`, so it remains valid in a Worker, and preserves the
  upstream timing algorithm.

Remove this directory and restore the crates.io dependency once an upstream
zxcvbn release includes this atomics-compatible Worker-safe timer change.
