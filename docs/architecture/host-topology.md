---
type: Runtime Contract
title: Host Topology and Precedence
description: The L1–L4 tiers that decide which surface becomes the HTTP host, the discovery file, token auth, and lifecycle.
tags: [architecture, host, surfaces, security]
status: stable
sources:
  - id: attach
    resource: ../../upeg-cli/src/infrastructure/attach.rs
    title: Host attach client
  - id: host-tokens
    resource: ../../upeg-cli/src/infrastructure/auth.rs
    title: operator / agent token resolution
---

# Principles

1. **When UPeg surfaces share a host, they use HTTP loopback.** The stdio MCP
   link with agents is separate. Unix sockets and `upeg daemon` are retired.
2. **`server.json` is the single source** — discovery, auth, and lifecycle all.
3. **Host Precedence (L1–L4) resolves conflicts deterministically.** The user
   never has to pick.
4. **Explicit-start is preserved.** No surface implicitly spawns another
   surface.
5. **Tray is not a surface but a GUI entry mode** — it is integrated into
   `flutter_app/`.
6. **Two binaries are enough**: `upeg` (headless-capable) + the desktop GUI.
   No feature flag needed.

# Surface catalog and host tiers

At most **one** HTTP host is auto-discovered per config root.

| Tier | Surface | Host eligibility | Lifecycle |
|---|---|---|---|
| **L1** | `upeg host start --daemon` | Yes (persistent, strong intent) | Until explicit stop |
| **L2** | Desktop GUI + Tray | **Only when `Tweaks.local_http_host` is on** (default OFF) | User session |
| **L3** | `upeg host start` (foreground) | Yes (explicit intent, may be short) | Occupies the shell |
| **L4** | `upeg call`, `upeg mcp`, `upeg tui` | No (client or in-process) | Per call / shell session / parent-bound |

**L2 is conditional.** The Desktop GUI does not become a host just because no
host exists. It runs an in-process host only when the user turns on "Local HTTP
host" (`Tweaks.local_http_host`, default OFF) in Settings — no network listener
opens without an explicit decision. When it is off, Desktop comes up with no
host to attach to.

# Startup algorithm

Every surface goes through the same algorithm.

![Host startup algorithm — splits into attach / host / in-process by tier](../diagrams/host-precedence.drawio.svg)

| Existing state | Starting | Outcome |
|---|---|---|
| None | L1/L3 | That surface becomes the host |
| None | Desktop GUI, `local_http_host` ON | Runs an in-process host (L2) |
| None | Desktop GUI, `local_http_host` OFF (default) | Comes up with no host — no listener opens |
| Host present | Desktop GUI | Discovers the existing host. Desktop's Tool execution still uses its own runtime |
| Desktop hosting | `upeg host start [--daemon]` | Error — "already hosted, stop it first" |
| Host present | `upeg tui` / `upeg call` | Shareable calls attach. Project Tools self-execute |
| Host present | `upeg mcp` | No board: stdio ↔ HTTP `/mcp` proxy. `--board`: self-executes |
| None | Any L4 | In-process (no auto-spawn) |

**When a host exits, clients do not auto-take-over.** They tell the user
explicitly and enter a degraded mode or fall back to in-process. A takeover
race is low value for its complexity.

Desktop registers a Flutter WebView provider with its own Rust runtime. When
this Desktop serves the embedded HTTP host, Controlled Embed calls from an
external CLI use the same app-owned pages too. Normal runs, debug runs, and
HTTP requests all go through the same wait and result-conversion rules. A call
whose provider is gone returns `controlled_embed_unavailable` and is not
re-run headless. A standalone `upeg host` process uses its own headless
backend, so it is not an interchangeable execution target for the Desktop
provider.

The detailed contract for session lifetime and GUI debug follows
[Desktop Controlled Embed: shared execution and debug](../ui-ux-surface-contract.md#desktop-controlled-embed-shared-execution-and-debug).

# Discovery file

- Location: `~/.upeg/server.json` on Unix, `%APPDATA%\upeg\server.json` on
  Windows
- Permissions: `0600` on Unix; an ACL allowing only the current user on Windows

```json
{
  "endpoint": "http://127.0.0.1:49317",
  "mcp_endpoint": "http://127.0.0.1:49317/mcp",
  "token": "<32-byte URL-safe>",
  "pid": 12345,
  "started_at_ms": 1700000000000,
  "origin": "explicit"
}
```

`origin` is `explicit` (a `upeg host start` the user ran) or `embedded` (a host
Desktop started inside its own process). Desktop uses this value plus the pid
to tell "is the reachable host my embed or somebody else's process." A file
missing the key reads conservatively as `explicit`.

## Stale detection (two-signal check)

Reachability is decided by one signal: an endpoint health check
(`GET /healthz`). An answer means reachable.

Only when there is no answer does the second signal come in: a PID-alive check
(Unix `kill(pid, 0)`, Windows `OpenProcess` + `GetExitCodeProcess`).

- **No answer + PID dead** → stale. The file is cleaned up.
- **No answer + PID alive** → **not** stale. The host is merely still booting
  or busy, and deleting the file here would orphan a live daemon. The file is
  preserved and only "not reachable right now" is reported to the caller.

`started_at_ms` is recorded in the file but **not used for the stale verdict.**
A third signal that would compare it against the OS-reported process start
time to defeat PID reuse is not yet implemented (future hardening).

A host publishes the file at startup and cleans it via RAII on a clean exit
(deleting only while the file still points at its own pid); the next surface
sweeps up after an abnormal exit through stale detection.

# Authentication

A host accepts **one operator token** and **several agent tokens**. The shape is
the same; only the authority differs — which one was carried becomes the call's
`_upeg.principal.role` (see [Call envelope](call-envelope.md)).

## operator token

- Generated as a 32-byte URL-safe random token at host start and published in
  `server.json`.
- **Same-OS-user verification is delegated to filesystem permissions** (`0600`
  / ACL). The user never has to see or copy the token.
- Explicit injection: the `UPEG_HTTP_TOKEN` environment variable or
  `--token-file <PATH>` (for CI/scripts).
- Rotation happens only on restart.
- A request carrying this token is treated as the person who started this
  host: `X-Upeg-Origin-Surface` is honored, and it can cross a Chain's
  approval barrier.

## agent tokens (optional)

The path that opens the data plane to programs **without granting a person's
authority**.

```bash
UPEG_HTTP_AGENT_TOKENS="tok-a,tok-b" upeg host start --daemon
```

| Item | Detail |
|---|---|
| Configuration | `UPEG_HTTP_AGENT_TOKENS` — comma-separated. Empty entries are ignored and duplicates merged. Unset means no agent access |
| Values | Same shape as the operator token is recommended (`upeg` does not mint them — the operator chooses) |
| Can do | Every route except `/healthz` already needs auth; agent tokens pass it: tool execution, listings, log queries |
| Cannot do | Declare `X-Upeg-Origin-Surface` (always stamped `http`), approve a Chain (`approval_denied_for_principal`) |
| Recorded as | `agent` in the execution log's `principal` column. Which token it was is not recorded |

**Why an environment variable and not a flag.** A repeatable `--agent-token`
flag would be more visible, but the command line is world-readable through `ps`
output on every platform upeg supports — which is also why the operator token
has `--token-file` next to `--token`. On top of that, the Desktop embedded host
(L2) has no argv to attach a flag to and only inherits the environment. The
environment variable is the only answer that covers both lanes.

## Non-loopback bind

The default is `127.0.0.1:0` (ephemeral loopback). With the explicit opt-in of
`UPEG_HTTP_ALLOW_NON_LOOPBACK=1` + `--addr 0.0.0.0:7173`:

- The token **must** be supplied via `UPEG_HTTP_TOKEN` / `--token-file`
  (auto-generation is forbidden).
- The Origin allow-list follows its own policy.
- `server.json` publication is opt-in — it prevents unintended local discovery
  in server environments.

# Lifecycle commands

```bash
upeg host start                          # foreground (L3)
upeg host start --daemon                 # detached background (L1)
upeg host status [--json]                # endpoint + pid + uptime
upeg host stop [--force]                 # SIGTERM 5s grace, --force is SIGKILL
upeg host logs [--lines N]               # log tail
upeg http status --pairing               # pairing block (below)
```

- Detach: the workspace forbids `unsafe`, so `fork()`/`setsid()` are not used
  (`setsid` needs `pre_exec`). On Unix, **the current executable is re-spawned
  with the same arguments**, stdin is `null`, stdout/stderr attach to the log
  file, and the parent exits — the child leaves the session through kernel
  reparenting once the parent is gone. The child sees the marker environment
  variable (`UPEG_HTTP_DETACHED`) and does not detach again. Windows does the
  same natively with `CreationFlags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)`.
- Logs: `~/.upeg/upeg-http.log` (or `--log-file`). Honors `RUST_LOG`.
  Single-file rotation that renames to `.1` at 10MB.
- Shutdown: SIGTERM/SIGINT/Ctrl-Break grants in-flight requests a 5s grace,
  then `server.json` is cleaned up.

# Pairing display

`upeg http status --pairing` prints the running host's endpoint + token as
text. A browser/mobile client does not have to hand-copy an ephemeral port and
token.

**It is a CLI-only display.** The values are read from the local `server.json`
(file mode `0600`), and no HTTP route exposes this block or the token. There is
no `--json` counterpart, and `GET /healthz` does not show it either.

# Multi-user

Each OS user gets their own surface instances, their own `~/.upeg/` (or
`%APPDATA%\upeg\`), and their own ephemeral ports. A shared instance between
users on the same machine is out of scope.

# Per-platform UX

| Platform | Default | Notes |
|---|---|---|
| macOS | Menubar-only (`LSUIElement=true`) | A settings toggle switches Dock visibility |
| Windows | Notification area (system tray) | — |
| Linux (SNI available) | System tray | — |
| Linux (no SNI) | Tray install fails silently; use the desktop window only | Point at the AppIndicator extension |

Related: [the surface contract's host-attach section](../ui-ux-surface-contract.md#host-attach-contract-browser-surfaces-pairing-with-a-local-host)
