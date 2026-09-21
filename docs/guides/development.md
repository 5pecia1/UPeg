---
title: Development
description: "The upeg dev toolchain, the check/verify gates, and generated-artifact drift checks."
---

# Development

Working on upeg itself needs the full toolchain — more than the
[installation](installation.md) minimum for just running the CLI.

## Toolchain

- **Rust 1.92+** — `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **`wasm32-unknown-unknown` target** — `rustup target add wasm32-unknown-unknown`
- **`just`** — `cargo install just --locked`
- **python3** — `apt install python3` / `brew install python3`
- **Flutter SDK** — <https://docs.flutter.dev/get-started/install> (for the
  desktop/PWA lanes)
- **flutter_rust_bridge_codegen 2.12.0** —
  `cargo install flutter_rust_bridge_codegen --locked --version 2.12.0`
- **wasm-pack** — `cargo install wasm-pack --locked`
- **cargo-deny 0.18.9** — `cargo install cargo-deny --locked --version 0.18.9` (license lane)
- **actionlint** — for the workflow lint recipe
- **Chrome/Chromium** — the Rust E2E tests and headless Controlled Embed
  need a real browser binary (`UPEG_BROWSER_PATH`/`CHROME_EXECUTABLE`)
- **node** — the dependency-free Chrome-extension tests
  (`node --test chrome-ext/tests/*.test.js`)

## Gates

Two tiers:

| Tier | Command | When | Cost |
| --- | --- | --- | --- |
| pre-commit | `just check` | before every commit | ~2–3 min warm |
| full closure | `just verify` | before pushing / opening a PR | one CI run |

```bash
just check     # fmt-check, clippy-native, file-size-budget, lexicon-check,
               # cargo test --workspace --lib — read-only, Rust-only
just verify    # the full public CI lanes — see below
just fix       # automated fixes only (cargo fix + clippy --fix + fmt)
just           # = fix → verify
just smoke     # release build + CLI smoke assertions only
```

`just verify` runs the four verification lanes — the same ones public CI
runs via `scripts/verify_public.sh`:

| Lane | Covers |
| --- | --- |
| `rust` | fmt, workspace build, clippy `-D warnings`, workspace tests (needs Chrome/Chromium for E2E) |
| `wasm` | `wasm32-unknown-unknown` clippy for `upeg-tools`, `upeg-core`, `upeg_frb` |
| `flutter` | `pub get --enforce-lockfile`, analyze, test, Linux + web builds |
| `licenses` | `cargo deny --locked --all-features check advisories licenses sources` |

Run one lane directly with `bash scripts/verify_public.sh <lane>`. Pinned
tool versions (Rust 1.92.0, Flutter 3.44.0, cargo-deny 0.18.9) are in that
script; native package deps are in `.github/actions/verify/action.yml`.

## Generated artifacts and drift gates

Generated artifacts — Tool manifests, interface metadata, schemas, FRB
bindings, golden baselines — are checked for drift by `*-check` recipes.
After changing a tool, Tool, or UI, run the matching regeneration recipe and
commit the result.

| Drift gate | Check | Regenerate |
|---|---|---|
| Tool execution baseline | `just test-baseline-check` | `just test-baseline` |
| Interface metadata | `just interface-inventory-check` | `just interface-inventory` |
| Toolkit schema | `just toolkit-schema-check` | `just toolkit-schema` |
| FRB bindings | `just frb-codegen-check` | `just frb-codegen` |
| UI parity (goldens) | `just ui-parity-check` | `just ui-parity` |

`test-baseline-check` runs `cargo test --workspace --all-targets`,
`cargo test --workspace --doc`, and `flutter test` (goldens included)
exactly once, then diffs against `fixtures/test-baseline.json` — so the
standalone `test-workspace`/`test-docs`/`flutter-test` recipes exist for
iterating on a failure, not as extra gates. On failure it prints the failed
test name and an output excerpt directly; you don't need a second
`cargo test` run to see a panic.

The generation lanes run hermetically: they point the runtime source dirs at
empty paths and set `UPEG_PROJECT_MANIFEST_PATH=off` so your personal
toolkits and this repo's own `upeg.toml` don't leak into generated fixtures.

## Conventions

- **Test names are descriptive English.** Write what the test proves.
- Keep responsibilities separated and complexity low; use the type system so
  illegal states aren't representable.
- Hand-written Rust and Dart files stay at or under 1000 lines
  (`file-size-budget`); generated FRB/build_runner output is excluded.
- Vocabulary is fixed in [`docs/LEXICON.md`](../LEXICON.md) — check it before
  naming or renaming anything user-visible (`lexicon-check` enforces it).

## Where things live

| Area | Path |
|---|---|
| CLI + all headless surfaces (TUI/MCP/HTTP) | `upeg-cli/` |
| Core domain model and `#[tool]` macro surface | `upeg-core/`, `upeg-macros/` |
| Loader for TOML/manifest/plugin sources | `upeg-loader/`, `upeg-sources/` |
| Runtime dispatch | `upeg-runtime/` |
| Built-in tools | `upeg-tools/` |
| Desktop/PWA app | `flutter_app/` |
| Chrome extension | `chrome-ext/` |
| WASM plugin API | `upeg-plugin-api/`, `upeg-plugin-macros/`, `upeg-wasm/` |
| Examples | `examples/tools/`, `examples/plugins/greet/`, `examples/mcp-imports/`, `examples/project-manifest/` |

Architecture contracts live in [`docs/architecture/`](../architecture/index.md);
crate boundaries and the layer rules are the right first read.
