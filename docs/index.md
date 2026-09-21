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

**New here?** Start with the
[README](https://github.com/5pecia1/UPeg/blob/main/README.md), then the
[quick start](guides/quick-start.md).

## Use cases

- **Everyday utilities without context switching** — convert, hash, diff,
  generate, and inspect files from the terminal, a desktop board, or a
  browser page.
- **A personal board of repeatable commands** — wrap `git`, `npm`, or your
  own scripts in TOML, save input presets, and re-run them from any surface.
- **Tools for AI agents** — serve a prepared board over MCP so an agent gets
  exactly the tools and guidance you chose.
- **A small local API** — call the same Tools over HTTP from scripts, apps,
  or the browser extension.
- **Project-level tooling** — a repo's `upeg.toml` declares project boards
  and tools that exist only while you work inside it.

## Guides

- [Installation](guides/installation.md) — build the CLI, desktop app, PWA,
  and extension; optional packaging tools.
- [Quick start](guides/quick-start.md) — first call, boards and pins, MCP and
  HTTP in a few minutes.
- [Tool author guide](guides/tool-author.md) — the `#[tool(...)]` macro keys
  and variants, and how to validate.
- [Development](guides/development.md) — the dev toolchain, `just` gates, and
  generated-artifact drift checks.
- [Troubleshooting](guides/troubleshooting.md) — `upeg doctor`, hosts,
  tokens, manifest detection, headless browsers.
- [PDF tools](pdf-tools.md) — `media.pdf_inspect` / `media.pdf_to_markdown`
  status, limits, and extraction contract.

## Reference

- [External Tool Manifest Guide](TOOL_MANIFEST.md) — every Toolkit TOML
  field, generated from `upeg-loader` (do not hand-edit).
- [Lexicon](LEXICON.md) — the single vocabulary shared by product, UI, CLI,
  manifests, and code. Read it before renaming anything.
- [UI/UX surface contract](ui-ux-surface-contract.md) — the Tool lifecycle,
  key bindings, and capability rendering shared by TUI, Desktop/PWA, and the
  Chrome extension.
- [File wire contract](architecture/file-wire.md) — the canonical
  `FileValue` JSON every surface sends and receives, with its size budgets.

## Product

- [Identity and boundaries](product/identity-and-boundaries.md) — what upeg
  is, what it is not, and its limits.
- [Boards and agent workflow](product/board-agent-workflow.md) — prepare a
  board's tools and guidance, then reuse it from MCP agents.
- [Security absolutes](product/security-absolutes.md) — the non-negotiable
  rules for secrets, the network, and embeds.

## Architecture

- [Crate boundaries](architecture/crate-boundaries.md) — domain / runtime /
  adapter / surface layers and what each owns.
- [Toolkit and Tool](architecture/toolkit-and-tool.md) — the two-level call
  hierarchy, Invokers, Tag inheritance, the single dispatch boundary.
- [Manifest contract](architecture/manifest.md) — Toolkit TOML structure,
  per-invoker required fields, credential reference rules.
- [I/O type system](architecture/io-types.md) — the closed input/output type
  set and its inline constraints.
- [Result presentation](architecture/result-presentation.md) — JSON collection views and typed follow-up Tool forms.
- [File wire](architecture/file-wire.md) — the canonical `FileValue` JSON,
  `x-upeg-file-wire`, and size budgets.
- [Chain Tool](architecture/chain.md) — node/connection model, expression
  grammar, execution rules.
- [Call envelope and reserved context](architecture/call-envelope.md) — the
  shared call envelope, `_upeg` context, CLI positional binding, shell
  completion.
- [Project manifest](architecture/project-manifest.md) — `upeg.toml`
  auto-detection and merge precedence.
- [HTTP API](architecture/http-api.md) — the `/v1/*` resource model,
  response rules, CORS and bearer auth.
- [Host topology and precedence](architecture/host-topology.md) — L1–L4 host
  ranks, the discovery file, tokens, lifecycle.
- [MCP — surface and import](architecture/mcp.md) — upeg as an MCP server
  and as an MCP client, and the eager-load import rules.
- [Technology stack](architecture/stack.md) — adopted and rejected
  technologies, and the license gate.

## Diagrams

`diagrams/*.drawio.svg` files are editable SVGs — they render directly in a
browser or on GitHub, and opening one in draw.io restores the original
diagram. There are no separate source files; export back over the same name
after editing.
