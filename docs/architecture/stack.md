---
type: Technology Decision
title: Technology Stack
description: The technologies the workspace adopted and rejected, and the rationale for each.
tags: [architecture, stack, dependencies]
status: stable
---

# Principles

1. Rust-first core/domain. The CLI/TUI/MCP/HTTP surfaces are a single Rust
   binary. UI surfaces (Desktop/Web/PWA) delegate view + platform integration
   to a Dart (Flutter) runtime while business logic stays in Rust crates. The
   ~10MB Dart AOT cost is accepted to avoid the maintenance of a direct
   muda/tao/wry integration.
2. No Electron/JVM/Node core runtime. Dart is confined to the UI layer.
3. One domain toolbox feeds every surface.
4. A dependency is added only when it removes more complexity than it brings.
5. Security uses proven OS features. No invented crypto or vault semantics.
6. Retired concepts are deleted, not kept for compatibility.

# Adopted

| Area | Choice | Rationale |
|---|---|---|
| Language | Rust 2024 | Strong types, proc macros, static inventory, WASM target |
| Toolbox | `inventory` + runtime overlay map | Static/Declarative/Project/WASM Tools share one lookup path |
| Serialization | `serde`, `serde_json`, `toml` | The standard stack for manifests/APIs |
| CLI | `clap` + `clap_complete` | Direct calls, logs, credentials, triggers, completion |
| TUI | `ratatui` + `crossterm` | A light keyboard-first surface |
| HTTP | `axum` + `tokio` | REST API and OpenAPI-compatible metadata |
| HTTP client adapter | A small sync std/adapter layer | Avoids a heavy client dependency for local deterministic dispatch tests |
| WASM host | `extism` (behind a feature flag) | A plugin model with no default-build cost |
| Desktop / Web (PWA) | Flutter + `flutter_rust_bridge` | Cross-platform UI consistency + native widget/platform integration. The Rust domain exposes the same code through two targets: cdylib (desktop) and wasm (web) |
| Chrome ext | Plain HTML/JS | The popup only handles direct dispatch and `upeg://` deep links, so no framework is needed |
| Command/CI mirror | `Justfile` | Verification commands stay discoverable and composable while light |
| Credential | Environment-variable/OS-keychain reference adapter boundary | TOML stores names only; values resolve at the execution boundary and are never logged |

# Storage

| Need | Choice |
|---|---|
| Pegboard state, memos, execution log, last results | A single WAL-mode SQLite (`upeg.db`) under the config root |
| Schema version | `PRAGMA user_version` + append-only migrations. **No version in the file name** |
| Credential | An adapter resolves name references. No plaintext persistence in manifests/logs |
| Execution log | Metadata-only events: time, tool id, invoker, surface, board, status, duration, error class |
| Network interface | explicit-start, loopback-first, remote-bind consent, visible state |

# License gate

Dependencies follow this license policy: MIT, Apache-2.0, BSD-2/3-Clause, ISC,
MPL-2.0, Zlib, Unicode-3.0, CC0-1.0. Copyleft (GPL/AGPL/CC-BY-SA/SSPL) is
rejected.

When vendoring a third-party crate, record the source, archive SHA, local
patches, and license notice in `vendor/<crate>/UPEG.md`, and exclude it from
workspace membership and the source budget (first case: pdf-inspector 1.17.0,
MIT + Adobe BSD-3 CMaps).
