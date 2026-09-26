---
okf_version: "0.2"
title: Universal Pegboard
description: "Pin a tool once, call it from anywhere — one Tool definition on CLI, TUI, Desktop, PWA, Chrome extension, MCP, and HTTP."
template: home.html
---

# Universal Pegboard

upeg turns the commands, conversions, and checks you reach for every day into
**Tools** pinned on a board — then exposes each Tool on the surfaces it
supports: CLI, TUI, Desktop, PWA, Chrome extension, MCP, and HTTP. Define a
Tool once (Rust macro, TOML manifest, WASM plugin, or imported MCP server)
and its declared surfaces call it with the same inputs, outputs, and result
envelope.

## What it is

- Pins frequently used Tools onto Boards — a Chain is itself one Tool — so
  people, AI agents, and apps call them instantly.
- Wraps external tools as TOML manifests, WASM plugins, or imported MCP
  servers; embeds SaaS as a passive view or a selector-driven form.
- Triggers, credential references resolved through environment variables or
  the OS keychain,
  metadata-only execution logs, keyboard-first control.
- Local-first and lightweight; sync is not shipped.

## What it is not

Not a password vault, crypto/payment infrastructure, IDE, mobile-first app,
visual workflow editor, runtime/version manager, server-side plaintext
store, or OS-level input automation — it wraps and orchestrates specialized
applications rather than replacing them.

## Limits

- PWA and the Chrome extension are sandboxed — native-runtime tools run
  through a paired local host or show an honest unsupported state. A
  Controlled Embed selector breaks on SaaS redesigns — no auto-tracking.
- Credential values live only in the OS keychain/environment; `hotkey`
  needs a graphical session; `Invoker::Llm` needs LLM networking; types
  outside the closed I/O set are unsupported.

## Guides

- [Installation](guides/installation.md) — build the CLI, desktop app, PWA,
  and extension; optional packaging tools.
- [Quick start](guides/quick-start.md) — first call, boards and pins, MCP
  and HTTP, media tools, shell completion.
- [Tool author guide](guides/tool-author.md) — the `#[tool(...)]` macro,
  TOML/External/WASM/MCP-import authoring, project boards.
- [Development](guides/development.md) — the dev toolchain, `just` gates,
  and generated-artifact drift checks.
- [Troubleshooting](guides/troubleshooting.md) — `upeg doctor`, hosts,
  tokens, manifest detection, headless browsers.

## Architecture

- [Architecture](architecture.md) — one Tool on seven surfaces: layers, the
  call envelope, file wire, hosts, HTTP, MCP, and the security absolutes.
  Contracts live in module rustdoc.

## Reference

- [External Tool Manifest Guide](TOOL_MANIFEST.md) — every Toolkit TOML
  field, generated from `upeg-loader` (do not hand-edit).
- [Lexicon](LEXICON.md) — the single vocabulary shared by product, UI, CLI,
  manifests, and code. Read it before renaming anything.

## Diagrams

`diagrams/*.drawio.svg` files are editable SVGs — they render directly in a
browser or on GitHub, and opening one in draw.io restores the original
diagram. There are no separate source files; export back over the same name
after editing.

