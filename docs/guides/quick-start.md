---
title: Quick start
description: "From a built upeg binary to boards, pins, MCP, and HTTP in a few minutes."
type: Guide
tags: [usage]
---

# Quick start

Assumes the `upeg` binary is built or on your `PATH` — see
[Installation](installation.md). Sections 1–6 need no network, account, or
token. The `--local` calls below force in-process dispatch; without it,
`upeg call` dispatches locally when no host is discoverable and attaches to a
live host recorded in `server.json` when one exists. Sections 7–8 start a
server; HTTP calls need the bearer token it prints.

## 1. Call a Tool

```bash
upeg call --local num.hex_to_decimal -a input=0xff
# → 255
```

Other call/output forms:

```bash
upeg num hex-to-decimal 0xff                          # dynamic short route
upeg call --local num.hex_to_decimal -a input=0xff --json     # full JSON envelope
upeg call --local num.hex_to_decimal -a input=0xff --pretty   # labeled rows
upeg call --local num.hex_to_decimal -a input=0xff --field result
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
results without a mouse. Essential keys: `Enter` run · `o` open · `p` pin ·
`/` search · `Esc` back · `?` catalog (a Desktop overlay; the TUI shows a
hint bar).

## 4. Pin tools on a board

Boards hold tools plus saved input presets and usage guidance — a board is
the unit that prepares a Tool set, its defaults, and its instructions
together.

The built-in personal boards are `dev`, `trading`, and `personal`; the
commands below use `dev`.

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

Personal boards live in your profile; project boards are declared in a
repository's `.upeg/project.toml` (see [Boards for agents](#7-boards-for-agents)).
Running upeg inside a repository composes your personal configuration with
the detected project configuration.

## 5. Media tools — files in, files out

Media tools take **bytes**, not paths: `-a <key>=@<path>` reads a file (or a
flat directory for batch tools), `--out` writes the result. The CLI refuses
to overwrite an existing destination unless you pass `--force`; without
`--out` the tool writes its chosen name into the current directory.

```bash
upeg call --local media.image_convert  -a input=@photo.png -a output_format=webp --out photo.webp
upeg call --local media.image_convert  -a input=@icon.svg  -a output_format=png -a svg_width=512 --out icon.png
upeg call --local media.images_convert -a images=@photos/  -a output_format=jpeg -a jpeg_quality=85 --out photos.zip
upeg call --local media.pdf_inspect    -a input=@doc.pdf --json
upeg call --local media.pdf_to_markdown -a input=@doc.pdf --field markdown
```

- `media.image_convert` — one image; PNG/JPEG/WebP/GIF/BMP/TIFF/ICO/QOI
  output, SVG input (rasterized). `upeg tool show media.image_convert`
  prints the full option list and per-option ranges.
- `media.images_convert` — a flat directory of images → ZIP; absent from
  the TUI (batch file tool).
- `media.pdf_inspect` — per-page inspection: `pdf_type`, `page_count`,
  `pages_needing_ocr`. Input ≤ 32 MiB, ≤ 500 pages.
- `media.pdf_to_markdown` — Markdown plus a JSON `report` whose
  `extraction_status` is `complete`/`partial`/`unavailable`; check the
  status via `--json` or `--field report` — the default view shows only
  the Markdown.

Metadata (EXIF, ICC) and animation are not preserved, except that
recognizable EXIF orientation is applied to the pixels. The `FileValue`
JSON wire shape (for HTTP/MCP callers) is in
[the File wire contract](../architecture.md#file-wire).

## 6. Shell completion

```bash
mkdir -p ~/.local/share/bash-completion/completions
upeg completions bash > ~/.local/share/bash-completion/completions/upeg
mkdir -p ~/.zfunc && upeg completions zsh > ~/.zfunc/_upeg   # fpath before compinit
upeg completions fish > ~/.config/fish/completions/upeg.fish
```

The tool list is baked into the generated script — regenerate after
installing Toolkits or plugins.

## 7. Boards for agents

Prepare once, reuse repeatedly: pin the tools, save repeated inputs as
presets, and write a description (purpose) plus Markdown instructions (tool
selection, result interpretation, what to do on failure). An agent then
reads that guidance and calls the right Tool — connecting a board executes
nothing by itself.

Personal board:

```bash
upeg board dev describe --description 'Frequently used conversion and check tools'
upeg board dev describe --instructions 'Explain the conversion result together with the inputs used.'
upeg board dev context --json     # guidance + tool list + readiness
upeg board dev connect            # print an MCP client config for this board
```

Fields omitted from `describe` are preserved; `--clear-description` /
`--clear-instructions` empty them.

Project board — declared in the repository's `.upeg/project.toml` (authoring rules:
[tool author guide](tool-author.md#project-boards)):

```toml
[[boards]]
id = "project-checks"
label = "Project checks"
description = "Used for checks during development and verification before submitting a PR."
instructions = """
Pick the tool that matches the requested verification scope.
Distinguish execution failures from missing run environments,
and record unchecked scope in the result.
"""
```

```bash
upeg --working-directory /absolute/path/to/repo board project-checks context --json
upeg --working-directory /absolute/path/to/repo board project-checks connect
```

This repository ships a real example — `upeg board upeg-dev context --json`
and `upeg board upeg-dev connect`.

### Serving over MCP

```bash
upeg mcp                    # stdio JSON-RPC server, all MCP-surface tools
upeg mcp --board dev        # scope to one board's pins + guidance
upeg board dev connect      # print an MCP client config for that board
```

`connect` prints `mcpServers` JSON that reproduces the current environment —
`command`/`args` pin the working directory via `--working-directory`,
`env.UPEG_PROJECT_MANIFEST_PATH` is pinned to the project file's absolute
path (or `off`), and no secret values are copied. For Codex, translate one
entry from the printed JSON into `~/.codex/config.toml` like this, copying
the actual absolute `command` path, working directory, board name, and every
emitted `env` value from your output:

```toml
[mcp_servers.upeg-dev]
command = "/absolute/path/to/upeg"
args = ["--working-directory", "/absolute/path/to/repo", "mcp", "--board", "dev"]

[mcp_servers.upeg-dev.env]
UPEG_PROJECT_MANIFEST_PATH = "off"
```

For a project board, use its printed absolute project manifest path in
`UPEG_PROJECT_MANIFEST_PATH`. Codex accepts the `command`, `args`, and
`[mcp_servers.<name>.env]` fields in its
[MCP configuration](https://developers.openai.com/codex/mcp). For Claude Code,
`claude --mcp-config /path/to/upeg-mcp.json --strict-mcp-config`.

The agent queries `upeg.board_context`, picks the tool that fits the
request, and calls it — pin presets apply, explicit args win. When board
instructions, pins, or manifests change mid-connection, the next request
returns `-32001` with `data.reconnectRequired = true`: restart the MCP
connection. When the same explanation keeps being needed, improve the
board instructions instead.

## 8. Run the local HTTP host

Start a foreground server on an explicit port in one terminal:

```bash
upeg http --addr 127.0.0.1:7173
```

Then, in a second terminal, read the endpoint and bearer token it published
and call it:

```bash
upeg http status --pairing                            # endpoint + token
UPEG_TOKEN='<token from the pairing output>'
curl -sS -H "Authorization: Bearer $UPEG_TOKEN" http://127.0.0.1:7173/v1/tools
curl -sS -X POST http://127.0.0.1:7173/v1/tools/num.hex_to_decimal \
  -H "Authorization: Bearer $UPEG_TOKEN" \
  -H 'Content-Type: application/json' \
  --data '{"input":"0xff"}'
curl -sS -H "Authorization: Bearer $UPEG_TOKEN" http://127.0.0.1:7173/v1/openapi.json
curl http://127.0.0.1:7173/healthz                    # no auth required
```

The POST returns a `ToolResult` envelope with `ok: true`,
`primary_output_id: "result"`, and a `result` output whose JSON value is
`255`. The authenticated OpenAPI endpoint describes the HTTP routes and
their request and response schemas.

### Host lifecycle

```bash
upeg host start                  # foreground
upeg host start --daemon         # detached background
upeg host status [--json]        # endpoint + pid + uptime
upeg host stop [--force]         # SIGTERM, 5s grace; --force is SIGKILL
upeg host logs [--lines N]       # log tail (~/.upeg/upeg-http.log)
```

`upeg host` and `upeg http` share `start`, `status`, `stop`, and `logs`
lifecycle actions. `upeg http` additionally accepts HTTP-specific options
(including token/pairing) and `restart`, so the command families are not
otherwise interchangeable.

`upeg http status --pairing` reads the local discovery file
(`~/.upeg/server.json`, mode `0600`) and prints the running host's endpoint
and token for pairing a browser/mobile client — no HTTP route exposes it.

If the Desktop app is already running it may already be hosting: reuse it
(`upeg http status` shows the live endpoint) rather than stopping it.
While a host runs, `upeg call` attaches to it automatically through
`server.json` — pass `upeg call --local` to stay in-process. The host binds
loopback by default; non-loopback binds require explicit consent and an
injected token (see [security absolutes](../architecture.md#security-absolutes)).

## 9. Try a wrapped command

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
  richer TOML, WASM plugins, MCP imports, project boards.
- [Installation](installation.md) — desktop app, PWA, extension, packaging.
- [Troubleshooting](troubleshooting.md) — when something above misbehaves.
