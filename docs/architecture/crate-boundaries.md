---
type: Architecture Contract
title: Crate Boundaries
description: The owned scope and the prohibitions of the four layers — domain / runtime / adapter / surface.
tags: [architecture, soc, crates]
status: stable
sources:
  - id: cargo-workspace
    resource: ../../Cargo.toml
    title: Workspace member list
  - id: boundary-gate
    resource: ../../upeg-core/tests/crate_boundaries.rs
    title: Crate dependency-graph gate
---

# Layers

Dependencies always point inward: `surface → adapter → runtime → domain`.

![Crate layers — Surface / Adapter / Runtime / Domain](../diagrams/crate-layers.drawio.svg)

The diagram only shows which layer each crate belongs to; the real edge
list is owned by the gate in [Enforcement](#enforcement) below.

| Crate | Role | Owns | Does not own |
|---|---|---|---|
| `upeg-core` | Domain | Toolkit/Tool/Chain/Board value types, schema contracts, pure validation, pure positional binding, capability verdicts | Runtime toolbox, dispatch, host I/O, UX labels |
| `upeg-runtime` | Application/runtime boundary | Toolbox overlay, dispatch, Trigger binding/execution, embed binding, manifest lowering, conflict policy | Source-format parsing details, surface UI flow |
| `upeg-loader` | Adapter | Parses source formats such as `upeg.toml`, then calls runtime lowering | Its own toolbox/dispatch semantics |
| `upeg-wasm` | Adapter | Parses and hosts WASM sources, then calls runtime lowering | Its own toolbox/conflict policy |
| `upeg-sources` | Application source boundary | Runtime source discovery and registration: user Toolkits, the project manifest, WASM plugins, upstream MCP servers | Surface UI flow, parser internals |
| `upeg-cli` | Surface + application/host | CLI/TUI/HTTP/MCP entry points and user I/O, plus the **host runtime**: discovery (`server.json`), bearer auth, daemon supervision, embedded HTTP server, pause state, MCP import loading, process liveness checks (`pid_alive`) | Domain policy duplicated from core/runtime, pure path resolution (→ `upeg-core::paths`) |
| `upeg-pegboard-ui` | UI state | UI-framework-independent pegboard state (boards/layouts/tweaks/memos/backups), deep-link contract, pin chrome, i18n catalog | Rendering, framework-specific widget code, surface-crate dependencies, grid **geometry** (cell size, pointer anchors — owned by Dart in the Flutter shell), grid placement algorithm (→ `upeg-runtime`) |
| `upeg-frb` | Surface boundary | The Rust↔Dart `flutter_rust_bridge` surface, host bootstrap, instance lock | Domain/runtime policy |
| `flutter_app/` | Surface | Flutter desktop/PWA UI flow | Toolbox semantics, domain validation |

`flutter_app/` sits on top of `upeg-pegboard-ui` through `upeg-frb`. Flutter is the
only GUI surface; the earlier Dioxus `desktop-ui/` crate has been removed.

# The only edge between surfaces

`upeg-cli` has two faces. It is the user entry point (CLI/TUI/HTTP/MCP) and at the
same time the **host application layer** — because the desktop shell embeds the
host instead of spawning a separate process. That is why exactly one
surface→surface edge exists:

- **`upeg-frb → upeg-cli` (allowed, documented exception).** `embedded_http_with_ready`,
  `current_host`/`ServerInfo`/`HostOrigin`, `is_paused`/`toggle_paused`,
  `load_mcp_imports_for_host`, `pid_alive` — all of them are the host runtime
  itself. The edge stays narrow: pure path resolution (`config_root`,
  `desktop.lock`) comes from `upeg-core::paths`.
- **`upeg-frb → upeg-loader` exists only in tests.** It is declared solely under
  `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]`, so it never
  enters the shipped graph. Because the loader installs approval gates at
  registration time, proving "a deep link cannot approve itself" end to end
  requires a genuinely registered Chain, and the boot summary's skipped-tool row
  likewise needs a `SkippedTool` that this host did not actually skip by hand.
  The edge runs surface→adapter, so it does not violate the layering rule itself.
- **`upeg-pegboard-ui → upeg-cli` no longer exists.** The only reason that crate
  needed `upeg-cli` was `config_root()`, and that resolution was owned by
  `upeg-core::paths` all along. The UI-state crate now reaches only the shared
  store adapter (`upeg-sources`) and domain/runtime.

Process liveness (`pid_alive`) also has exactly one owner: `upeg-cli`'s
`infrastructure::process`. Discovery-file reaping (`server.json` staleness) and
the desktop single-instance lock (`upeg-frb`) call the same implementation —
there is no second copy whose Windows `tasklist` parsing could drift.

# Enforcement

The layering is a test, not a review convention.
`upeg-core/tests/crate_boundaries.rs` reads the `Cargo.toml` of every workspace
member (including `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`,
and `[target.*]`), builds the set of `upeg-*` edges, and checks three things.

1. The edge set matches the `ALLOWED_EDGES` table **exactly**. A new edge fails,
   and so does a stale edge that only remains in the table.
2. Every edge points at the same or an inner layer (the rank in `LAYERS`).
3. The only edges toward crates that own entry points (`upeg-cli`, `upeg-frb`)
   are the ones registered in `DOCUMENTED_EXCEPTIONS` — currently just
   `upeg-frb → upeg-cli`.

To add a dependency, update that table together with the code, and update this
document with it.

# Runtime lowering

The loader never creates its own runtime truth. It parses each source's grammar
into typed input and then calls `upeg-runtime` lowering. Overlay precedence,
static-id protection, runtime-duplicate replacement, Trigger registration,
embed-binding normalization, and manifest→toolbox conversion are applied **only
in lowering**.

# Architecture style

- The backend applies Hexagonal only at real I/O seams: CLI arguments, the file
  system, environment variables, HTTP, the WASM host, credential resolution, and
  process execution. Pure domain functions get no trait wrappers.
- The TUI uses direct TEA (update/view) on `ratatui` + `crossterm`. `tui-realm`
  was rejected: it adds a framework state machine where the benefit is thin.
- The `Justfile` is the command and CI mirror.

Related: [Toolkit and Tool](toolkit-and-tool.md)
