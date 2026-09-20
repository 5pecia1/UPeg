---
type: Surface Contract
title: MCP — Surface and Import
description: The direction in which upeg is an MCP server (serve) and the direction in which it is an MCP client (import), and each direction's gates.
tags: [architecture, mcp, surfaces, ai]
status: stable
sources:
  - id: mcp-surface
    resource: ../../upeg-cli/src/surfaces/mcp
    title: MCP surface implementation
  - id: mcp-import-real-smoke
    resource: ../../upeg-cli/tests/mcp_import_real_server.rs
    title: Real-server (official SDK) import smoke
---

upeg's relationship with MCP is bidirectional, and the two directions never
share a bare name. Bare `mcp` always means the Surface.

![The two MCP directions — serve (mcp Surface) and import (MCP Import)](../diagrams/mcp-directions.drawio.svg)

# Serve — upeg as MCP server (the `mcp` Surface)

`upeg mcp` exposes its own Tools over stdio JSON-RPC; the host's `/mcp` does so
over HTTP. A Tool id is always `{toolkit}.{tool}`, and MCP `tools/call` uses
the same `name` + `arguments` shape as the [shared call
envelope](call-envelope.md).

## The Board gate — "Board = server"

Started as `upeg mcp --board <b>`, `tools/list` exposes the Tools pinned to
that Board and usable on the MCP surface. Calls to runnable Tools that are not
pinned are refused. The read-only `upeg.board_context` is the exception and is
always provided; that name is reserved for Board attachment. A Board that has
pinned an ordinary Tool under that name is refused at attach time with
guidance to rename it. Started without `--board`, it serves the whole
surface-filtered Toolbox.

Each pin's args preset is the default for call arguments; explicit arguments
win. The MCP input schema also marks applied defaults, and a field satisfied
by a valid pin preset drops out of `required`. A Tool declaration's own
default marking alone cannot omit a required input. The real tool's input
validation and approval rules still apply.

## Board guidance and connect readiness

`upeg board <b> context [--json]` shows the description, Markdown
instructions, execution location, project files, the actual MCP tool list with
defaults, and pre-run check results. `ready` means the known preconditions
were verified — not a guarantee the run will succeed. A missing external
executable or working directory reports `unavailable`; things checkable only
at run time, like network or auth, report `unchecked`. Saved pins whose tools
are not registered in the current process are reported separately as
`unresolved_pins`, with guidance that the attach preview is incomplete. They
may become available after MCP Import loading and are not mixed into the
currently callable list.

`upeg board <b> connect` prints the `mcpServers` JSON that reproduces the
current environment. The generated `command`/`args` pin the working directory
via the CLI's `--working-directory`, and `env.UPEG_PROJECT_MANIFEST_PATH` is
pinned to the project file's absolute path or `off`. Personal stores and
explicit tool-source paths are preserved. No secret values are copied. When a
client's config container differs, move that server's command/args/env entry
across.

The standard `instructions` at initialization carry a short description plus
guidance to query `upeg.board_context`. The query result provides the full
instructions, the real run configuration, and the config revision in both text
and structuredContent. Do not assume the client auto-interprets arbitrary
metadata as a Skill. Whether the instructions reach and are used by the model
must be verified on the real agent client.

## Change reflection and execution location

A stdio connection pinned to a Board runs tools in that process. Even if a
host running from another project exists, nothing swaps to that host's tool
list or execution environment. MCP calls apply the connection's working
directory, and a Tool's explicit `cwd` plus the project-boundary rules win
over it.

If the board instructions, pins, presets, or a project/Toolkit/MCP-import TOML
change mid-connection, the next initialize/list/call request returns error
`-32001` with `data.reconnectRequired = true`. The MCP server must be
reconnected to load the new configuration. In-flight work is not retroactively
cancelled. Changing only pin placement is not a reconnect reason. A
backgrounded import finishing is handled by the existing
`notifications/tools/list_changed`.

Project instructions are compared against the source at load time. When the
file changed or disappeared, the stale instructions are not previewed as if
new — the guidance is to restart UPeg or reconnect MCP. Per-request Board
calls over HTTP do not share the stdio connection's session revision.

For personal/project authoring steps and a full example, see [Board and agent
workflow](../product/board-agent-workflow.md).

## In-progress output — `notifications/message`

While a tool runs, `tools/call` streams its output so far as server→client log
notifications. The final response frame does not change one bit — the
notifications are added in front of it; they are not part of the result.

```json
{"jsonrpc":"2.0","method":"notifications/message","params":{
  "level":"info","logger":"upeg.tool",
  "data":{"tool":"dev.verify","stream":"stderr","seq":0,"text":"   Compiling upeg-core\n"}}}
```

| Item | Value | Why |
|---|---|---|
| `level` | always `info` | Progress output is ordinary information, not a warning. A client that filters at `info` is exactly the client that asked to see this |
| `logger` | always `upeg.tool` | A client can route or mute tool output separately |
| `data.stream` | `stdout` \| `stderr` | The stream the child actually wrote |
| `data.seq` | increases without gaps from 0 | Runs across both streams and all chain steps, so the total order can be restored |
| `data.text` | the bytes the tool wrote, verbatim | Sent line-split (see the [manifest contract](manifest.md)) |

A request that is not `tools/call` has nothing to report, so it produces no
notifications.

## The `logging` capability differs per lane

`capabilities.logging` is declared **only on lanes that can honor it.** A
server that declares the capability and sends no notifications leaves clients
waiting in front of frames that never come; a server that sends notifications
without declaring it can be treated as a protocol violation. Both are lies, so
the split is whether the lane **has somewhere to put** the frames.

| lane | Server-initiated frames | `capabilities.logging` | `logging/setLevel` |
|---|---|---|---|
| stdio in-process (`upeg mcp`, no host) | Written to stdout between responses | Declared | Yes, per session |
| HTTP `/mcp` | Written onto the SSE stream the client opened (see "Server→client push" below) | Declared | Yes, per `Mcp-Session-Id` |
| stdio proxy (attached to a host) | None on this session — the host runs the tool | Not declared | `-32601 Method not found` |

**On the HTTP lane the capability belongs to the lane, not to a request.** A
single request whose `Accept` lacks `text/event-stream` simply opened no body
that could carry frames — that is the client's choice, not a promise the
server broke. That request's progress frames are dropped; the session's next
SSE request still receives them.

When proxy mode needs in-flight output, the host's [HTTP streaming
route](http-api.md) carries the same information.

## Server→client push — SSE on `/mcp` (Streamable HTTP)

For a long time `/mcp` had one direction only: one request, one JSON response.
So two things the stdio lane did naturally were impossible over HTTP —
streaming a running tool's output, and **notifying** that an import finished
loading. Now both directions exist in exactly the shape of the MCP Streamable
HTTP transport (2025-03-26).

| Request | Condition | Response |
|---|---|---|
| `POST /mcp` | `Accept` states `text/event-stream` **explicitly** and the request has an `id` | `text/event-stream`. Progress `notifications/message` frames flow first, **the last event is the JSON-RPC response**, then the stream closes |
| `POST /mcp` | everything else | **Not one letter different** from the old single JSON response (`204 No Content` for notifications) |
| `GET /mcp` | `Accept` states `text/event-stream` | A long-lived stream for frames belonging to no request |
| `GET /mcp` | otherwise | `406 Not Acceptable` — this resource has no other representation |

```text
POST /mcp
Authorization: Bearer <token>
Accept: text/event-stream
Content-Type: application/json

{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"dev.verify","arguments":{}}}
```

```text
event: message
data: {"jsonrpc":"2.0","method":"notifications/message","params":{"level":"info","logger":"upeg.tool","data":{"tool":"dev.verify","stream":"stderr","seq":0,"text":"   Compiling upeg-core\n"}}}

event: message
data: {"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"ok"}],"structuredContent":{...}}}
```

Contract:

- **`Accept` decides, and `*/*` does not count.** A wildcard means "anything,"
  not "stream." Every existing client, `curl` included, keeps receiving the
  same response it always did.
- **The last event is the response.** The consumer's termination condition is
  seeing the frame carrying its own request `id`. A stream that ends without
  it is a broken connection.
- **A request with no response opens no stream.** A notification without an
  `id` has no response to carry at all, so it is `204` regardless of `Accept`.
- **A consumer that does not read loses notifications, not the host's
  memory.** Unwritten SSE bodies accumulate only up to a fixed **byte budget**
  per call. Past that, notification frames are dropped, and the moment room
  frees up the lost **frame count** is reported once, summed into a single
  `warning` frame on the `upeg.transport` logger. The JSON-RPC response goes
  out regardless of the budget — the contract is the response, not the
  notifications.
- **Disconnecting stops the tool.** When the consumer goes away the SSE body
  drops, and that drop is this call's cancellation — **the same contract and
  the same implementation** as `POST …/stream`
  (`upeg-cli/src/surfaces/http/cancel_on_drop.rs`). The `External` invoker
  reads the cancellation token on every tick of its wait loop and kills the
  child's process group once set. The signal is the body's drop, not "could
  not queue a frame" — a build that stays quiet for ten minutes is exactly
  the run that must stop at once. An invoker that cannot read cancellation
  (WASM, built-in functions) still runs to the end.
- **A quiet call still emits bytes.** While no frames flow, keep-alive
  comments flow at the same interval as `GET`. An SSE comment is a line
  consumers ignore, not an event, so frame order is unchanged — and an
  idle-timeout proxy in the middle does not mistake a quiet `tools/call` for
  a dead connection.
- **Auth is the same `/mcp` as before.** The bearer token applies to `GET`
  identically.
- **The caller principal is decided by the token.** The surface is `mcp` on
  both lanes, but stdio is a process the OS user launched (`local`) while HTTP
  crossed a listener, so it is whatever the bearer proves
  (`operator`/`agent`). Even on a chain with `approval_surfaces = ["mcp"]`, a
  `tools/call` arriving on an agent token cannot approve (see [Chain
  Tool](chain.md)).

### `Mcp-Session-Id` — issued but not required

The `initialize` response carries an `Mcp-Session-Id` header (same random
source as the bearer token). When a later request echoes it back, **the
severity floor `logging/setLevel` moved stays on that session.** That is the
whole reason a session id exists on this lane.

- The transport allows a server to **require** this header; upeg does not. A
  request without it is handled identically and uses the default floor
  (`info`).
- This header is **not authorization.** Authorization is the bearer token
  alone.
- The host remembers only the last 64 sessions' floors. HTTP has no
  "session ended" signal, so forgetting beats accumulating without bound, and
  a forgotten session returns to the same state as one that never called
  `logging/setLevel`.

## `logging/setLevel`

A lane that declared the capability answers this method, which moves the
session's **severity floor**.

```json
{"jsonrpc":"2.0","id":2,"method":"logging/setLevel","params":{"level":"warning"}}
```

- Only the eight values MCP fixed (syslog, RFC 5424) are allowed — `debug`,
  `info`, `notice`, `warning`, `error`, `critical`, `alert`, `emergency`.
  Anything else is refused with `-32602 Invalid params` rather than silently
  crushed, and the message carries the allowed list back.
- The default is `info`. A client that never calls it sees exactly what it
  saw when this method did not exist.
- Progress frames go out at `info`, so raising the floor to `warning` or
  above **stops progress notifications.** The final response frame is
  unaffected — severity filters notifications only.
- The setting is per session and holds until that session ends.

## The `source` field

Each `tools/list` entry states its provenance in a first-class `source` field.
The value is one of three (`upeg-runtime/src/provenance.rs`):

| Value | Meaning |
|---|---|
| `local` | Built-in inventory, TOML Toolkits in `~/.upeg/toolkits`, WASM plugins — in-process |
| `mcp-import:<server>` | An import proxied from an upstream MCP server |
| `project-manifest:<path>` | A Tool registered by a detected project `upeg.toml` (see [Project manifest](project-manifest.md)) |

MCP clients and management surfaces can tell upeg's own Tools, proxied
imports, and project Tools that exist only in the caller's directory apart
without parsing ids.

## Proxy mode

`upeg mcp` without a Board acts as a stdio ↔ HTTP `/mcp` proxy when a host is
reachable; otherwise it stands alone as an in-process handler — it never
auto-spawns a host (see [Host topology](host-topology.md)).

The proxy is not a plain pipe. It applies **the same project-manifest rules**
as CLI/TUI (see [Project manifest](project-manifest.md) — provenance and
dispatch location):

- When `tools/call`'s `params.name` carries `project-manifest:*` provenance,
  it is **dispatched in-process**, not sent to the host. The host parsed its
  own `upeg.toml` (or none), so it does not know that Tool.
- Every other `tools/call` is forwarded with the caller's absolute cwd stamped
  into `params.arguments` as `_upeg.cwd`.
- The `tools/list` response merges this process's project-manifest Tools,
  deduped by name (on a collision the host entry stays). A stdio connection
  pinned to a Board follows the standalone rule above.

# Import — upeg as MCP client (`MCP Import`)

Against MCP configuration scattered across every client, upeg becomes the
configuration hub. Declare an upstream server once in
`~/.upeg/mcp-imports/<server>.toml`; upeg spawns it as a subprocess, reads its
tool list, **normalizes each tool to typed I/O**, and registers it in the
Toolbox as `<server>.<id>`. The file stem becomes the namespace. An imported
tool is pinned to Boards and callable from CLI/TUI/Desktop/HTTP exactly like
any other Tool.

## Partial success (per tool)

- A tool whose schema cannot be converted to upeg typed I/O (`$ref`/`oneOf`
  and friends), or one with an empty `name`, is **skipped per tool** and a
  structured reason (`SkipReason`) is recorded. The server's remaining tools
  register normally. `ImportOutcome { registration, skipped }` exposes the
  skip list to the caller.
- A server import fails in only three cases: an MCP protocol violation, a
  namespace collision, or every tool skipped.
- **A namespace collision is atomic per server.** A duplicate id or a built-in
  Tool shadow is a trust/configuration problem whose fix (renaming the server
  config) applies to the whole namespace; silently skipping the colliding tool
  could mask an intentional shadowing.

## Reexport

An imported tool is **not re-exposed** on upeg's own `mcp` Surface by default.
This default-off blocks proxy chains and self-import loops that would quietly
hand another server's tools back out. Opt in per server with `reexport =
true` in that server's TOML — the opt-in lives in the very file that declares
the trust decision to spawn that server. A skipped tool registers on no
surface, so reexport never comes into play for it.

## Import loading: long-lived server processes only, different timing per lane, reload means restart

`~/.upeg/mcp-imports/*.toml` is loaded **when a long-lived server process
starts**. There are exactly three such processes, and **the three do not load
the same way**. The shared entry point is
`upeg_sources::load_mcp_imports_for_host(&RuntimeSourceConfig)`; all three
lanes pass through `upeg-cli/src/infrastructure/mcp_imports.rs`.

| Lane | When | Exposure while loading |
|---|---|---|
| `upeg host start` (foreground / `--daemon`) | Synchronous eager load before the listener opens | Requests are accepted only after loading finishes |
| desktop embedded host (`upeg-frb` host bootstrap) | Pending is marked **before** the embed thread is spawned; the load itself runs on a background thread after ready | `/healthz`'s `importsPending` is true from the moment the listener opens — see "the async window" below |
| in-process `upeg mcp` (not proxy) | Background thread + conditional | `notifications/tools/list_changed` is issued when the load completes |

One-shot CLI commands and the TUI do **not** load imports — they spawn no
subprocess and dispatch imported tools through the attached host. `upeg mcp`
in proxy mode does not load either — the host side has already loaded the
list it serves.

**Reload is a host restart.** There is no separate mid-run reload action —
that loss is accepted deliberately. The `reexport = true` opt-in rule stays
the same as above.

### In-process `upeg mcp`: conditional + lazy load

A stdio MCP server must answer its client's `initialize` immediately. A dead
upstream can stretch spawn + handshake to tens of seconds with retries, so
this lane lifts loading off the request path.

1. **Pre-scan**: `upeg_sources::mcp_import_reexport_policy` reads only the
   declaration files to see whether any `reexport = true` exists (no spawn —
   a single `read_dir`). When everything is the default `Blocked`, imported
   tools register under `ALL_SURFACES_EXCEPT_MCP` and never appear on this
   surface's `tools/list` anyway, so **the load itself is skipped**.
2. **Background load**: with at least one opt-in, the load runs on a worker
   thread.
3. **Completion notice**: after the load finishes — and after the
   `initialize` response already went out — one
   `notifications/tools/list_changed` line (a JSON-RPC notification, no `id`)
   is written to stdout. The client re-reads `tools/list` when it sees it.
   The write is best-effort — a client that already left does not kill the
   server.

### The desktop embedded host: the async window

The desktop lane loads imports in the background, but `server.json`
publication and request serving start earlier. So **for a few seconds right
after the host becomes ready, `/v1/tools` and `/mcp`'s `tools/list` may not
yet contain the imported tools.** The window itself is unchanged — the
alternative not taken was pushing boot behind the imports, stretching the
splash by the upstream timeout. What changed is **how a client learns the
window has closed**.

| Signal | Direction | Who uses it |
|---|---|---|
| `importsPending` on `/healthz` | The asking side (polling) | Anyone, no auth. `upeg host status --json`, the desktop status bar |
| `notifications/tools/list_changed` on `GET /mcp` | The telling side (push) | An MCP client that opened an SSE stream on `/mcp` |

- The push goes out **when the load finishes** (`McpImportPhase::Done`).
  `Loading` means the window is open and `Skipped`/`NotStarted` mean nothing
  was ever going to be fetched — neither is a reason to ask a client to
  re-read. A `Done` ending with 0 tools also goes out — "the window closed
  and nothing arrived" is exactly what a waiting client needs to hear.
- Phase transitions are written only in `set_import_phase`, and that one
  place is the push point (`mcp_imports::subscribe_phase_changes`). That is
  why the asking signal and the telling signal cannot state different facts.
- Recovery for a client that opened no stream stays simple: re-read
  `tools/list` a little later.

#### `importsPending` — the signal that the window is open

A client that gets no push can still **ask.** The host carries its own import
load phase as a type
(`McpImportPhase` in `upeg-cli/src/infrastructure/mcp_imports.rs`:
`NotStarted | Loading | Done{serversLoaded, serversFailed, tools} | Skipped`)
and puts it on the unauthenticated `/healthz`:

```json
{
  "name": "upeg",
  "version": "…",
  "importsPending": true,
  "mcpImports": { "state": "loading" }
}
```

- `importsPending` is true in exactly one phase: `Loading`. A client that saw
  false may trust the `tools/list` it just read as a finished list.
- `Loading` is stamped synchronously **before** the loader thread exists. A
  client that read `/healthz` between spawn and thread entry must not see
  `not-started` and conclude "nothing more is coming."
- The desktop lane goes one step further. The embed worker thread opens the
  listener and starts answering `/healthz` **before** sending the ready
  signal, so the mark is stamped before that thread is even spawned
  (`upeg_cli::mark_mcp_imports_pending`). A path where the host never comes
  up (bind failure, losing the embed-slot race) hands the mark back
  (`clear_mcp_imports_pending`) — nobody should wait for a load that was
  never scheduled. A ready timeout does **not** hand it back: the thread is
  still alive and may bind any moment, and that host is ours, so its imports
  are ours to load.
- `Done` carries `serversLoaded` / `serversFailed` / `tools` counts.
  **Counts only** — this route is unauthenticated, so which upstreams were
  declared never goes out.
- `Skipped` means this process skipped loading because it recognized itself
  as somebody's MCP-import child (self-import prevention below).
- `upeg host status --json` now re-reads this and reports it as the
  `mcpImports` block. This one-shot command is the only place that reports a
  host's **live** state rather than a *declaration*. When the host does not
  answer or the field is absent, it is `"state": "unknown"` — filling a
  missing value with false would read as "fully loaded." The block comes out
  **always**: even with no host at all or a stale `server.json`, the key is
  `unknown` instead of absent. "Key absent" and "unknown" are the same fact,
  and emitting both shapes would make consumers handle both.
- This `/healthz` read runs on **the same budget** as discovery's liveness
  probe (`RequestBudget::HEALTH_PROBE`, 300ms each for connect/response).
  Spending the 30s budget of the routes that actually run tools here would
  have `upeg host status` wait a hundred times longer on a host the
  neighboring probe already judged "dead."
- The desktop status bar receives the same signal as `McpImportPhaseDto` on
  the FRB snapshot and renders the import chip as "imports loading…" instead
  of a count while loading.

### Partial success and safe timeouts

The per-server partial-success rule (the "Partial success" section above)
applies unchanged at load time. The loading path owns subprocess/protocol
safety.

- `initialize`: 5s bounded timeout
- `tools/list`: 5s bounded timeout
- `tools/call`: 30s bounded timeout
- shutdown: 500ms bounded timeout
- The timeout path kills the child process and waits on it
- The first spawn + handshake (`initialize` + `tools/list`) is retried up to
  a fixed attempt count with fixed backoff, only for transient failures
  (timeout, pipe closing before a response)
  (`upeg-sources/src/mcp_import/spawn_retry.rs`). Failures that retry cannot
  change — a bad command, an explicit RPC error — are returned immediately; a
  permanently failing import never retries forever.

### Handshake spec compliance

A real `@modelcontextprotocol` TypeScript SDK server validates JSON-RPC
frames strictly. Skip either of these and `tools/list` is silently dropped —
it looks like a timeout:

- A request with no `params` (`tools/list`, etc.) omits the `params` member
  itself. Sending `"params": null` makes the official SDK server drop the
  request at frame validation — `null` and member-absent mean different
  things in JSON-RPC 2.0.
- Immediately after the `initialize` response arrives, before any other
  request, the `notifications/initialized` notification goes out (no `id`,
  no response). A write failure does not fail the handshake — it is
  best-effort, and a real connection problem shows on the very next request.

#### Real-server verification

These two rules **cannot be proven against a fake stdio server.** The suite's
fake server and the dogfood test that points `upeg mcp` at upeg itself both
put upeg on both ends, so a frame both sides get wrong passes as-is. That was
exactly the blind spot here.

So `upeg-cli/tests/mcp_import_real_server.rs` runs the whole import span
against **a server we did not write** —
`npx -y @modelcontextprotocol/server-filesystem <tmpdir>` (the official
TypeScript SDK). **Three tests** see the same import at three levels:

1. Handshake + `tools/list` response (raw `UpstreamMcpServer`),
2. Namespace registration and a real `tools/call` round trip through the
   registered dispatcher,
3. **The host lane** — a declaration written into a scratch `UPEG_HOME`,
   `upeg host start --daemon` launched, and a separate `upeg call` process
   calling the imported tool through that host. The path a user actually
   steps on, and where the `mcpImports` signal of `upeg host status --json`
   is verified end to end.

- All three tests are `#[ignore]`. An ordinary `cargo test` must not presume
  a Node toolchain and the npm registry.
- `just mcp-import-real-smoke` runs them, and `just ci-smoke` calls that.
- On a dev machine without `npx` or where the package cannot be fetched, the
  outcome is a **skip, not a failure**, with the reason printed (no toolchain
  / package fetch failure / abnormal pre-warm exit are recorded separately).
  Missing Node is not a upeg regression.
- **On CI a skip is a failure.** `just mcp-import-real-smoke` turns on
  `UPEG_REQUIRE_REAL_MCP=1` when `CI` is set, and then the not-ready reasons
  above become panic messages as-is. A runner that lost Node showing "three
  greens" would erase this lane's reason to exist.
- Timing: a test first launches the server once with no stdin to warm the
  `npx` cache — otherwise the import layer's 5s `initialize` budget would be
  spent on a package download and a network problem would look like a upeg
  timeout bug. The pre-warm runs once per process (`OnceLock`) — if each of
  the three tests fetched, the cold-cache cost would triple. On an empty
  cache this warmup alone takes tens of seconds (network-bound); afterwards
  the three tests together take seconds. A CI runner's per-job cache volume
  covers only cargo, so every job fetches once again.

### Self-import recursion prevention

When upeg's own `mcp` Surface is declared as an upstream (like
`examples/mcp-imports/local.toml` with `command = "upeg", args = ["mcp"]`),
the spawned child is itself a `upeg mcp` process. Unless this recursion is
blocked, the child eager-loads `~/.upeg/mcp-imports` on its own startup path,
spawning a grandchild `upeg mcp`, which spawns a great-grandchild — a fork
bomb until the OS process/fd limits are hit.

upeg plants a marker environment variable on every MCP-import upstream
subprocess it spawns (`upeg_sources::mcp_import::MCP_IMPORT_CHILD_ENV =
"UPEG_MCP_IMPORT_CHILD"`, set in `spawn_child`). Every long-lived server
entry point (`upeg host start`, the desktop host, in-process `upeg mcp`)
checks the marker with
`upeg_sources::mcp_import::is_mcp_import_child()` before eager import loading
— `true` skips the load. All three entry points pass through
`upeg-cli/src/infrastructure/mcp_imports.rs::load_and_report_for_host`, so the
guard living in that one function is sufficient. The marker's meaning is its
existence, not its value — `is_mcp_import_child()` is exposed as a typed
predicate so no CLI-side code ever needs to compare the value.
