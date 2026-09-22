---
type: API Contract
title: HTTP API
description: "The `/v1/*` resource model, response rules, and the CORS and bearer-auth boundary."
tags: [architecture, http, api, security]
status: stable
sources:
  - id: http-surface
    resource: ../../upeg-cli/src/surfaces/http
    title: HTTP surface implementation
---

# Resource model

```text
GET  /healthz
GET  /v1/toolkits
GET  /v1/toolkits/{toolkit}
GET  /v1/toolkits/{toolkit}/{tool}
GET  /v1/tags
GET  /v1/tags/{tag}
GET  /v1/boards
GET  /v1/boards/{board}
GET  /v1/credentials
GET  /v1/logs
GET  /v1/tools
GET  /v1/tools/{toolkit}.{tool}/readiness
GET  /v1/triggers
GET  /v1/clients
POST /v1/clients/heartbeat
POST /v1/tools/{toolkit}.{tool}
POST /v1/tools/{toolkit}.{tool}/stream
POST /v1/boards/{board}/tools/{toolkit}.{tool}
POST /v1/boards/{board}/tools/{toolkit}.{tool}/stream
POST /v1/trigger/{toolkit}.{tool}
GET  /v1/openapi.json
POST /mcp
GET  /mcp
```

# Response rules

- Browse responses are JSON and carry Tool metadata including valid tags.
- `/healthz` answers without auth and exposes only non-secret hints: name,
  version, and the MCP-import loading signal (the `importsPending` flag plus a
  `mcpImports` block holding counts only — see "importsPending" in
  [MCP](mcp.md)). It never includes the token, the port/capability list, or
  upstream server names.
- While a host process is up, `/v1/*` is **always** served — there is no
  separate desired-state gate. Starting the host is itself the explicit
  activation (see [Host topology](host-topology.md)).
- `/v1/tools` returns the HTTP-visible Tool list in the same metadata shape as
  the `tools/list` that MCP-compatible clients use.
- `GET /v1/tools/{id}/readiness` is bearer-, surface-, and optionally
  Board-gated (`?board=<key>`), and returns the non-executing
  [External readiness](external-readiness.md) contract. Attached CLI clients
  provide their execution directory so this check follows the same cwd as a
  dispatched External command. It never exposes PATH or credential values.
- `/v1/credentials`, `/v1/logs`, and `/v1/triggers` expose reference-only
  credential metadata, metadata-only Execution Log rows, and registered Trigger
  bindings respectively. None of them returns secret values or argument values.
- A successful Tool call returns `{ "result": "..." }`; a protocol, body, or
  Tool error returns `{ "error": "..." }` with 400/404/422.
- A Board Tool call rejects Tools not pinned to that Board and injects the
  `_upeg` context plus the project-manifest path.
- OpenAPI covers both the concrete Tool-call paths and the resource routes.

# Streaming calls — `POST …/stream`

`POST /v1/tools/{id}` answers with one envelope after the tool finishes. For a
ten-minute command, nothing arrives for ten minutes. The `/stream` sibling
route opens an `application/x-ndjson` body immediately and fills it with **one
JSON object per line**.

```text
POST /v1/tools/dev.verify/stream
Authorization: Bearer <token>
Content-Type: application/json

{}
```

```json
{"event":"chunk","stream":"stderr","seq":0,"data":"   Compiling upeg-core\n"}
{"event":"chunk","stream":"stdout","seq":1,"data":"ok\n"}
{"event":"result","result":{"ok":true,"primary_output_id":"result","outputs":[...]}}
```

| Line | Fields |
|---|---|
| `chunk` | `stream` (`stdout`\|`stderr`), `seq` (increases without gaps from 0 across both streams and **all chain steps** within one call), `data` (the bytes the tool wrote, verbatim) |
| `dropped` | `bytes` — how many bytes of tool output the host dropped because the consumer read too slowly |
| `result` | `result` — the [canonical envelope, not one letter different](call-envelope.md) from the non-streaming route |

Contract:

- **The last line is always `result`.** Reading until `"event":"result"` is the
  consumer's termination condition. A body that ends without `result` means the
  connection broke, not success.
- **`seq` is per call.** A Chain running three `External` steps still counts
  0, 1, 2 … onward. Because it never returns to 0 per step, the consumer can
  restore stdout, stderr, and steps as one total order.
- **A consumer that does not read loses chunks, not the host's memory.**
  Unwritten NDJSON accumulates only up to a fixed **byte budget** per call.
  Past that, chunks are dropped, and the moment room frees up the dropped
  amount is reported in a single `{"event":"dropped","bytes":N}` line. `seq`
  keeps running, so **loss shows up only in that line** — do not assume it can
  be spotted as holes in the sequence.
- **The status code is decided before the outcome.** Headers go out with the
  first byte, so a tool failure cannot become a `422` — failure arrives as the
  envelope in the last `result` line. Routing errors knowable in advance
  (unknown tool, tool not pinned to the board, malformed body) answer with a
  plain JSON `404`/`400` without opening a stream.
- **Auth, the Origin guard, the board-pin gate, and `_upeg` injection are all
  identical.** This route adds one body encoding, not a policy.
- Only the [`External` invoker](manifest.md) produces chunks. Other invokers
  emit the single `result` line, and that is normal.
- **Disconnecting stops the tool.** When the consumer goes away the response
  body drops, and that drop is this call's cancellation — the call was
  dispatched with a cancellation token installed in the first place (see
  "Cancellation" in the [manifest contract](manifest.md)). The `External`
  invoker reads the token on every tick of its wait loop, and once set, it
  kills the child's **process group** and finishes with an
  `error.code = "cancelled"` (`details.cancelled = true`) envelope. Nobody is
  left to read that envelope, but the child is dead for sure.
  - **A silent tool stops too.** The signal is not "could not queue a chunk"
    but the body's drop. A build that stays quiet for ten minutes is exactly
    the run that must stop at once.
  - An invoker that cannot read cancellation (WASM, built-in functions) still
    runs to the end. Cancellation is a request; the contract is always one
    final envelope.
  - `timeout_ms` stays useful — cancellation covers the case where **a
    consumer was there and then gone**, while a run that simply takes too long
    from the start is stopped by the budget.

The OpenAPI document (`/v1/openapi.json`) carries the two streaming routes as
templated paths — the stream body's shape does not vary per tool.

# MCP JSON-RPC — `/mcp`

`/mcp` is a JSON-RPC 2.0 contract, not the `/v1/*` resource model. The methods
and frame shapes are owned by [MCP](mcp.md); only **what it looks like as
HTTP** is written here.

| Method | `Accept` | Response |
|---|---|---|
| `POST` | `text/event-stream` stated explicitly + request has an `id` | `200 text/event-stream` — progress `notifications/message` events, the last event is the JSON-RPC response, then the stream closes |
| `POST` | anything else (including `*/*`) | One `200 application/json` response. A notification with no `id` gets `204` |
| `POST` | — | `400` when the body is not JSON, `400` for malformed headers |
| `GET` | `text/event-stream` stated explicitly | `200 text/event-stream` — server-initiated frames belonging to no request (today: `notifications/tools/list_changed`) plus keep-alive comments |
| `GET` | anything else | `406 Not Acceptable` |

- **`*/*` does not open a stream.** A wildcard means "anything," not "stream,"
  so existing clients — `curl` included — keep receiving the same JSON they
  always did.
- **Auth, the Origin guard, and the pause gate are exactly the same as
  `/v1/*`.** Both directions require a bearer token. SSE adds one body
  encoding, not a policy.
- **`Mcp-Session-Id`**: attached to the `initialize` response; when a later
  request echoes it back, the severity floor `logging/setLevel` moved stays on
  that session. It is not required and has nothing to do with authorization
  (see [MCP](mcp.md)).
- **A consumer that does not read loses notifications, not the host's
  memory.** The same per-call byte-budget rule as `/stream`; the lost **frame
  count** is reported once, summed into a single `warning` frame on the
  `upeg.transport` logger. The JSON-RPC response itself goes out regardless of
  the budget.
- **Disconnecting stops the tool.** Same contract and same implementation as
  `POST …/stream` (see [MCP](mcp.md)).
- **A quiet call still emits bytes.** While no frames flow, SSE comments
  (keep-alives) do, so an idle-timeout proxy in the middle does not mistake a
  quiet call for a dead connection. A comment is a line SSE consumers ignore,
  not an event.

**Caller identity on this lane** follows rules different from `/v1/*`.

| Half | Value | Why |
|---|---|---|
| surface | always `mcp` | An MCP client is a program. Whatever process relays it and whatever header it attaches, `mcp` is `mcp` |
| role | whatever the bearer token proves (`operator` / `agent`; `agent` when unauthenticated) | This lane crosses a listener. stdio `upeg mcp`'s `local` default cannot describe a caller here |

- The `X-Upeg-Origin-Surface` header does not reach this route (see "Origin
  surface" below).
- **No request header moves the surface.**


# Authentication

Every route except `/healthz` requires `Authorization: Bearer <token>` to
match, compared in constant time. The host issues tokens per
[Host topology](host-topology.md).

Tokens come in **two kinds** — same shape, different authority.

| Token | Source | Stamped principal |
|---|---|---|
| operator | `~/.upeg/server.json`, `--token` / `--token-file` / `UPEG_HTTP_TOKEN` | `_upeg.principal.role = "operator"` |
| agent | `UPEG_HTTP_AGENT_TOKENS` (comma-separated) | `_upeg.principal.role = "agent"` |

- **Authentication and authority are different questions.** Both tokens pass
  the bearer gate and enter the data plane. Where they split is after that.
- **Two things an agent token cannot do.** `X-Upeg-Origin-Surface` is not
  honored (the call always stamps `http`), and it cannot cross a Chain's
  approval barrier — even sent from an authorized surface it is
  `approval_denied_for_principal` (see [Chain Tool](chain.md)). Widening
  `approval_surfaces` gives the same answer.
- **Both branches stamp a principal.** `/v1/*` stamps `{role, origin surface}`;
  `/mcp` stamps `{role, mcp}`. There is no route that skips stamping — a
  skipped stamp would be filled by the surface's default, and `mcp`'s default
  is `local`, which can approve.
- **An unauthenticated caller is `agent`.** The only case is a bring-up router
  that requires no token; a host that can identify nobody promotes nobody to
  operator.
- The server stamps the principal and caller-sent values are erased (see
  "Principal" in [Call envelope](call-envelope.md)). The execution log keeps
  **only the role label** — token values are never stored.

# Origin surface — `X-Upeg-Origin-Surface`

When a host is up, `upeg call` and the TUI attach to `/v1/tools/{id}` instead of
running in-process. Stamping all of those calls `http` would **let the
transport change the caller's identity** — a person typing
`upeg call <chain> -a approve=true` in their own terminal was being refused
because `http` is not an approval surface, and the TUI had no escape hatch like
`--local` (see [Chain Tool](chain.md)).

So the attach client declares **which local surface it is calling from** in a
header.

```text
POST /v1/tools/dev.precommit
Authorization: Bearer <token>
X-Upeg-Origin-Surface: cli
```

| Rule | Detail |
|---|---|
| Allowed values | `cli`, `tui` — the two surfaces that actually attach from this binary. `desktop` never attaches (it runs in-process through FRB), and `pwa`/`ext` are remote clients arriving on a pairing token, so they stay `http` |
| Condition for honoring | The request must authenticate with **this host's operator token**. The token sits in `~/.upeg/server.json`, readable only by the same OS user — "local client" is exactly that trust boundary. An agent token authenticates a program, and a program cannot claim which human surface it sits at |
| When not honored | Not an error — `http`. A missing token, an agent token, a value outside the allowed list, or a missing header all behave exactly as if the header were absent |
| Scope | Both the `_upeg.surface` stamp **and** the surface-visibility gate, on the buffered route and the `/stream` route alike. An attached `upeg call` sees the same tool set it would see under `--local`. `_upeg.surface` and `_upeg.principal.surface` always point at the same value |
| MCP proxy excluded | When `upeg mcp` relays to `/mcp` it does not send this header. An MCP client is a program; whatever process relays it, `mcp` is `mcp` |

The header is a **declaration, not an identity.** It does not widen the auth
boundary — a client holding the operator token can already run tools on that
host, and this header only narrows which local surface it is. Per-token caller
identity is answered by [Authentication](#authentication) above.

# CORS

What a browser origin may do is decided separately from *who may act*.

- **Allowed by default**: every `chrome-extension://<id>` origin, and loopback
  origins (`http(s)://127.0.0.1:*`, `localhost:*`, `[::1]:*`).
- **Arbitrary web origins** require an exact match against `--cors-origin
  <ORIGIN>`. Repeatable; no wildcards. `*`, a missing scheme, and any
  path/query are rejected.
- **Preflight (`OPTIONS`) deliberately has no token** — the CORS layer is the
  outermost and short-circuits before the bearer check. The data plane always
  goes through bearer auth regardless of origin.
- **Allowed request headers**: `Authorization`, `Content-Type`,
  `Mcp-Session-Id`, `X-Upeg-Board`. The first two ride every call; the latter
  two are the `/mcp` lane's request contract. A browser cannot even send a
  header preflight did not allow, so dropping one would leave a browser MCP
  client unable to echo the session id it was issued.
- **Exposed response headers**: `Mcp-Session-Id`. A browser hides response
  headers that are not exposed — a session id the server issues but the client
  cannot read is no session id.
- **`X-Upeg-Origin-Surface` is not opened.** It is honored only from
  `cli`/`tui` + an operator token, and a surface delivered through a browser
  can be neither. Opening it would be advertising a lever that does not work.
- **Host anti-rebinding holds independently of CORS**: reject an `Origin`
  header that is present but not allowed, and reject a `Host` header that is
  not loopback. CORS only controls what browser JS may *read*.
