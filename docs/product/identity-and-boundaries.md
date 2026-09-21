---
type: Product Definition
title: Product identity and boundaries
description: "What upeg is and what it is not — the one-line definition, what it does / does not do / its limits."
tags: [product, scope, boundaries]
status: stable
---

# In one line

> Pin the tools you use often (single tools or chains) on one board, and let
> anyone — a person, an AI, an app — call them instantly from any
> environment, without searching, tab-switching, or reopening anything.

It is neither a utility collection nor a workflow builder. It is **the layer
that connects arbitrary tools to arbitrary callers and pins the result on a
pegboard for instant re-invocation**. A single tool is a Tool, and **a chain
is itself one Tool** — however complex, a chain is consumed as a single pin.

# What it does

- Pins frequently used Tools onto Boards.
- Lets people, AI, and apps call the same Tool from the surfaces it
  supports (CLI/TUI/Desktop/PWA/Ext/MCP/HTTP).
- Exposes a Tool on its declared surfaces from one Toolkit + Tool manifest.
- Wraps external tools (TOML / WASM / upstream MCP servers). Zero-line
  migration.
- Embeds external SaaS two ways: Passive Embed (the webview is the
  experience) and Controlled Embed (drives it through selectors, presented
  as a native form/result).
- Chain Tool — connects several Tools declaratively, with conditional
  branching and approval steps.
- Trigger — runs automatically on clipboard, hotkey, schedule, file,
  directory, or webhook conditions.
- Credential — keeps external API auth material separated as typed schema +
  OS keychain references.
- Execution Log — records metadata for every Tool call locally (values
  excluded).
- Full pegboard control from the keyboard alone.
- Lightweight: a single binary, lazy loading, processes start only when
  needed.
- Local-first. Cloud is optional — and designed end-to-end encrypted when it
  arrives. Sync itself is not shipped yet.
- Mobile friendly — not mobile first.

# What it does not do

- Its own password vault, its own crypto, its own payments.
- Replace best-in-class external tools (Notion / Obsidian / 1Password /
  Dropbox / Figma / VSCode).
- Store user data in plaintext on a server.
- Its own IDE / code editor, or real-time collaborative editing.
- Mobile-first design.
- Build profession-specific pins itself.
- Track SaaS UI changes automatically (selector auto-refresh).
- Enable network interfaces without user consent.
- Heavy dependencies (Electron, JVM).
- A visual workflow editor built into the core.
- OS-level mouse/keyboard control. upeg hotkeys are limited to Tool-call
  triggers.
- Language/runtime version management (mise/asdf territory).

# Limits

- PWA and the Chrome extension are `wasm32`/sandboxed hosts. Without the
  loader runtime and the native-only dispatchers, those tools render as an
  honestly unsupported state, or execute remotely through a paired local
  host (see the [surface contract](../ui-ux-surface-contract.md)).
- When a SaaS UI changes, a Controlled Embed selector breaks. There is no
  pre-flight check — the call errors and the user re-maps.
- SaaS terms-of-service gray area — limited to the user's own use.
- Credential values live only in the OS keychain and are not a cloud sync
  target.
- The `hotkey` trigger depends on the platform global-hotkey adapter and a
  graphical session. On hosts without them, the trigger list shows the
  unsupported diagnostic as-is.
- `Invoker::Llm` needs LLM API networking. A local LLM is connected through
  a credential.
- Types outside the closed I/O type set are not supported.

