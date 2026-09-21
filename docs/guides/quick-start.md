---
title: Quick start
description: "From a built upeg binary to boards, pins, MCP, and HTTP in a few minutes."
---

# Quick start

Assumes the `upeg` binary is built or on your `PATH` — see
[Installation](installation.md). Sections 1–5 run fully in-process: no
network, account, or token needed. Sections 6–7 start a server; HTTP calls
need the bearer token it prints.

## 1. Call a Tool

```bash
upeg call num.hex_to_decimal -a input=0xff
# → 255
```

`upeg call` dispatches in-process by default. Other output forms:

```bash
upeg num hex-to-decimal 0xff                          # dynamic short route
upeg call num.hex_to_decimal -a input=0xff --json     # full JSON envelope
upeg call num.hex_to_decimal -a input=0xff --pretty   # labeled rows
upeg call num.hex_to_decimal -a input=0xff --field result
```

The canonical result is a structured `ToolResult` envelope (`ok`,
`primary_output_id`, `outputs[]` with `id`/`label`/`kind`/`value`). The plain
CLI prints only the primary value so it pipes cleanly.

## 2. Explore the toolbox

```bash
upeg tool list                 # every registered Tool (id, toolkit, pin kind)
upeg tool list --tag convert   # filter by tag
upeg tool list --surface mcp   # what an MCP client would see
upeg tool show num.hex_to_decimal --json
upeg toolkit list
upeg tag list
```

## 3. Use the TUI

```bash
upeg                           # terminal → interactive TUI
upeg --board dev               # TUI filtered to one board
```

The TUI is keyboard-first: search, tab between boards, run pins, and copy
results without a mouse.

## 4. Pin tools on a board

Boards hold tools plus saved input presets and usage guidance.

```bash
upeg board list                          # boards that exist right now
upeg board dev list                      # pins on the "dev" board
upeg board dev pin num.hex_to_decimal    # pin a tool
upeg board dev pin text.diff --at 1,0 --units U2
upeg board dev unpin text.diff
upeg board show dev --json
```

A pin written by the CLI is the same object a Desktop drag-and-drop writes —
there is one shared pegboard store.

## 5. Pass files to tools

Media tools take **bytes**, not paths: `-a <key>=@<path>` reads a file,
`--out` writes the result.

```bash
upeg call media.image_convert -a input=@photo.png -a output_format=webp --out photo.webp
upeg call media.pdf_inspect -a input=@doc.pdf --json
```

The CLI refuses to overwrite an existing destination unless you pass
`--force`. The full `FileValue` JSON wire shape (for HTTP/MCP callers) is in
[the File wire contract](../architecture/file-wire.md).

## 6. Serve tools to an agent (MCP)

```bash
upeg mcp                    # stdio JSON-RPC server, all MCP-surface tools
upeg mcp --board dev        # scope to one board's pins + guidance
upeg board dev connect      # print an MCP client config for that board
```

Add the printed config (or `{ "command": "upeg", "args": ["mcp"] }`) to your
MCP client — e.g. `claude_desktop_config.json` for Claude Desktop. See
[Boards and agent workflow](../product/board-agent-workflow.md) for how
board guidance reaches the agent.

## 7. Run the local HTTP host

Start a foreground server on an explicit port in one terminal:

```bash
upeg http --addr 127.0.0.1:7173
```

Then, in a second terminal, read the endpoint and bearer token it published
and call it:

```bash
upeg http status --pairing                            # endpoint + token
curl -H 'Authorization: Bearer <token>' http://127.0.0.1:7173/v1/tools
curl http://127.0.0.1:7173/healthz                    # no auth required
```

`upeg host start --daemon` runs the same server detached on an ephemeral
loopback port — `upeg http status --pairing` reports the actual endpoint.
If the Desktop app is already running it may already be hosting: reuse it
(`upeg http status` shows the live endpoint) rather than stopping it.

While a host runs, `upeg call` attaches to it automatically through the
discovery file (`~/.upeg/server.json`) — pass `upeg call --local` to stay
in-process. The host binds loopback by default; non-loopback binds require
explicit consent and an injected token (see
[security absolutes](../product/security-absolutes.md)).

## 8. Try a wrapped command

Drop this in `~/.upeg/toolkits/demo.toml`:

```toml
id = "demo"

[[tools]]
id = "git_log"
display_label = "git log"
invoker = "External"
pegboard_units = "U1"
command = "git"
args_template = ["log", "--oneline", "-n", "{count}"]

[[tools.inputs]]
name = "count"
type = "integer"
default = 10
```

```bash
upeg tool validate ~/.upeg/toolkits/demo.toml
upeg call demo.git_log -a count=3
```

Every field the manifest understands is in
[`docs/TOOL_MANIFEST.md`](../TOOL_MANIFEST.md); runnable examples live in
`examples/tools/`.

## Where next

- [Tool author guide](tool-author.md) — write Tools with the Rust macro or
  richer TOML.
- [Installation](installation.md) — desktop app, PWA, extension, packaging.
- [Troubleshooting](troubleshooting.md) — when something above misbehaves.
