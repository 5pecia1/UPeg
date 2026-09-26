# upeg — Flutter UI

`flutter_app/` is the Desktop/PWA UI surface for upeg (Universal Pegboard).
It renders the Pegboard/Board/Pin UI and talks to
the Rust workspace through `flutter_rust_bridge` (`upeg-frb`) — tool
registry, execution, and platform IPC stay on the Rust side; this
package owns rendering only. The Chrome extension is a separate
dependency-free JavaScript surface in `chrome-ext/`.

## Running

```bash
cd flutter_app
flutter run -d linux
```

CMake/cargokit builds the `upeg-frb` cdylib and installs it into the
Flutter bundle automatically — no separate manual `cargo build` step is
needed for the Linux desktop target. For web, run `just flutter-run-web` or
`just flutter-run-web-server` from the repository root; both build the FRB
WASM bridge first. See the top-level
[`README.md`](../README.md) for platform-specific instructions (macOS,
Web, packaging).

## Codegen and tests

From the repo root, `just` recipes wrap the Flutter/Dart tooling used in
CI:

```bash
just frb-codegen              # regenerate FRB Rust↔Dart bindings after upeg-frb API changes
just frb-codegen-check         # verify no FRB binding drift
just flutter-test              # cd flutter_app && flutter test
just flutter-integration-test  # cd flutter_app && flutter test integration_test/
just flutter-analyze           # cd flutter_app && flutter analyze
just ui-parity                 # regenerate UI parity goldens (flutter test --update-goldens --tags=golden)
just ui-parity-check           # verify no golden drift
just flutter-fmt               # dart format lib/ test/
just flutter-fmt-check         # dart format --set-exit-if-changed
```

See the top-level [`README.md`](../README.md) and `Justfile` for the
full list of recipes and drift-check gates.
