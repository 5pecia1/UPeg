//! User-facing transport surfaces.
//!
//! # `mcp` — the serve contract
//!
//! (`surfaces::mcp` itself sits at the file-size budget, so the contract
//! lives here.) `upeg mcp` exposes the Toolbox over stdio JSON-RPC 2.0;
//! the host's `/mcp` does so over HTTP. `tools/call` takes the shared
//! envelope's `name` + `arguments`; a Tool id is always
//! `{toolkit}.{tool}`. Bare `mcp` always means this surface — the
//! opposite direction (upeg as MCP *client*) is `upeg-sources`' MCP
//! Import.
//!
//! * **Board gate ("Board = server")** — `upeg mcp --board <b>` lists
//!   only Tools pinned to that Board and usable on `mcp`, and refuses
//!   calls to unpinned ones. `upeg.board_context` is reserved for Board
//!   attachment and always served; a Board pinning a Tool under that
//!   name is refused at attach. Pin args presets become call-argument
//!   defaults — explicit args win, and a preset-satisfied field drops
//!   out of `required`.
//! * **Reconnect on config change** — a change to board instructions,
//!   pins/presets, or a project/Toolkit/MCP-import TOML makes the next
//!   initialize/list/call fail `-32001` with
//!   `data.reconnectRequired = true`; in-flight work is not cancelled.
//!   Pin placement alone is not a reason, and a finished background
//!   import uses `notifications/tools/list_changed` instead.
//! * **In-progress output** — `notifications/message` frames at
//!   `level = "info"`, `logger = "upeg.tool"`,
//!   `data = { tool, stream, seq, text }`; `seq` is gapless per call
//!   across both streams and all chain steps. The final response frame
//!   is untouched — notifications are added in front of it.
//! * **`capabilities.logging` only where it can be honored** — declared
//!   on in-process stdio and on HTTP `/mcp` (frames ride the SSE
//!   stream), not on the stdio→host proxy (`logging/setLevel` →
//!   `-32601`). `logging/setLevel` accepts only the eight RFC 5424
//!   levels (`-32602` otherwise); the default is `info`, progress goes
//!   out at `info`, and the floor is per session — `Mcp-Session-Id`
//!   ties it to one (issued on `initialize`, never required, not
//!   authorization; only the last 64 sessions' floors are remembered).
//! * **`source` provenance** on every `tools/list` entry: `local`,
//!   `mcp-import:<server>`, `project-manifest:<path>`
//!   (`upeg_runtime`'s provenance module).
//! * **Proxy mode** — `upeg mcp` without `--board` proxies stdio↔HTTP
//!   `/mcp` when a host is reachable and never auto-spawns one. It is
//!   not a plain pipe: a `tools/call` whose `params.name` carries
//!   `project-manifest:*` provenance is dispatched in-process (the host
//!   parsed its own `upeg.toml`, or none), every forwarded call gets
//!   the caller's absolute cwd stamped as `_upeg.cwd`, and `tools/list`
//!   merges this process's project-manifest Tools (host wins name
//!   collisions). See `mcp::proxy`.
//!
//! Caller identity on `/mcp`: the surface is always `mcp`; the role is
//! whatever the bearer token proves (`operator`/`agent`, `agent` when
//! unauthenticated). See the `surfaces::http` module docs.

pub(crate) mod cli;
pub mod http;
pub mod mcp;
pub mod tui;
