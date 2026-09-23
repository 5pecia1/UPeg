# upeg — Universal Pegboard

**English** | [한국어](README.ko.md)

Pin a tool once, call it from anywhere. upeg turns the commands, conversions,
and checks you reach for every day into **Tools** that live on a board — and
exposes each Tool on the surfaces it supports: the desktop app, the terminal,
an AI agent over MCP, or a small local HTTP API.

One definition — a Rust `#[upeg::tool]` function, a TOML manifest, a WASM
plugin, or an imported MCP server — registers a Tool once and makes it
callable across **CLI, TUI, Desktop, PWA, Chrome extension, MCP, and HTTP**,
with no per-surface code. Each Tool declares the surfaces it appears on: a
batch file Tool can skip the TUI, and browser surfaces run tools through a
paired local host.

## What you can do with it

- **Run the built-in toolbox** out of the box: numeric/base conversions,
  Base64/URL/HTML/JSON encoding and formatting, hashing, UUID/NanoID
  generation, text diff/regex/word counts, CSV select/diff/to-JSON, QR
  encode/decode, image conversion, and PDF/Office inspection and extraction.
- **Pin Tools to Boards.** A board holds the tools, saved input presets, and
  usage instructions for one context — personal boards live in your profile,
  project boards are declared in a repo's `upeg.toml`.
- **Drive everything from the keyboard** in the TUI and Desktop app, or
  script the same Tools from the CLI with stable JSON output.
- **Hand a board to an agent.** `upeg mcp` serves your Tools over stdio
  JSON-RPC to Claude Desktop, Cursor, and other MCP clients — scoped to one
  board, with that board's instructions, if you want.
- **Wrap your own commands** in TOML (`invoker = "External"`), compose
  Tools into Chains with approval steps, write WASM plugins, or import
  upstream MCP servers — all land in the same Toolbox. External, HTTP, and
  imported-MCP tools do whatever the wrapped command or server does —
  including any network access it needs.
- **Keep secrets out of config.** Manifests reference credentials by name;
  values live only in environment variables or the OS keychain.

Local-first: the built-in toolbox runs on your machine. The HTTP host binds
loopback by default and every route except `/healthz` requires a bearer
token. There is no cloud dependency — and no cloud sync yet (see
[Project status](#project-status)).

## Project status

upeg is early-stage and pre-release:

- **No published downloads yet.** There are no versioned releases or
  prebuilt binaries at this time — you build from source (below). Packaging
  recipes exist (`just package-*`) but produce unsigned artifacts.
- **This repository is a public mirror.** Development happens in a private
  source repository; updates arrive here as exported pull requests that
  merge like normal PRs. The development history is not mirrored — each
  export carries one squashed, verified commit.
- **CI:** every export passes the four public lanes — Rust build/test/clippy,
  WASM clippy, Flutter analyze/test/Linux+web builds, and license checks
  (`scripts/verify_public.sh`).
- **Expect breaking changes** while the format and surface contracts settle.
  Known breaking changes are called out in this document (e.g. the File
  wire migration note below).

Support is best-effort through GitHub Issues — see
[SUPPORT.md](SUPPORT.md). Security reports follow
[SECURITY.md](SECURITY.md).


## Install

There is no installer yet; build the CLI from source. You need:

- **Git**, a **C toolchain** for the linker (`build-essential` on
  Debian/Ubuntu, Xcode Command Line Tools on macOS, MSVC Build Tools on
  Windows), and **Rust 1.92+** —
  `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

```bash
git clone https://github.com/5pecia1/UPeg.git
cd UPeg
cargo install --locked --path upeg-cli   # installs `upeg` into ~/.cargo/bin
upeg doctor                              # prints binary/feature/source-dir diagnostics
```

`cargo install` puts `upeg` on your `PATH` (via `~/.cargo/bin`, which rustup
already adds). To build without installing, run
`cargo build --release -p upeg-cli` and use `./target/release/upeg` in place
of `upeg` below.

The Desktop app, PWA, and WASM plugins need more (Flutter SDK, `wasm-pack`,
extra targets) — but not for the CLI. The full matrix, including optional
packaging tools, is in the
[Installation guide](docs/guides/installation.md).

## First success

```bash
upeg call num.hex_to_decimal -a input=0xff
# → 255
```

This built-in call needs no network, account, or token — `upeg call`
dispatches in-process. Then look around:

```bash
upeg tool list                 # every registered Tool
upeg tool list --tag convert   # filter by tag
upeg num hex-to-decimal 0xff   # short dynamic form of the same call
upeg call num.hex_to_decimal -a input=0xff --json    # canonical envelope
upeg call num.hex_to_decimal -a input=0xff --pretty  # labeled rows
upeg                           # on a terminal: the interactive TUI
```

Every surface returns the same structured `ToolResult` envelope
(`ok`, `primary_output_id`, `outputs[]`); the plain CLI prints just the
primary value. The [Quick start guide](docs/guides/quick-start.md) walks
through boards, presets, and file input.

## Choose your surface

**Terminal.** `upeg` opens the keyboard-first TUI; `upeg call` and the
dynamic `upeg {toolkit} {tool}` route script it. Shell completions for
bash/zsh/fish/elvish/powershell: `upeg completions bash` (regenerate after
installing new Toolkits).

**Desktop app.** Flutter app with a real pegboard: drag pins, saved presets,
live tools, embedded web views.

```bash
cd flutter_app && flutter run -d linux    # macOS: just flutter-run-macos
```

CMake builds the Rust bridge library automatically — no separate cargo step.

**MCP (agents).** `upeg mcp` speaks stdio JSON-RPC. Point a client at it, or
scope the server to one prepared board:

```bash
upeg board dev connect        # prints an MCP client config for board "dev"
upeg mcp --board dev          # serve only that board's pinned tools
```

**HTTP.** A small local REST + MCP-over-HTTP API for apps and the browser
extension. Start a foreground server on an explicit port in one terminal:

```bash
upeg http --addr 127.0.0.1:7173
```

Then, in another terminal, read the endpoint and bearer token it published
and call it — `/v1/*` routes need `Authorization: Bearer`:

```bash
upeg http status --pairing                  # prints endpoint + token
curl -H 'Authorization: Bearer <token>' http://127.0.0.1:7173/v1/tools
curl http://127.0.0.1:7173/healthz          # the one unauthenticated route
```

Prefer a background host? `upeg host start --daemon` runs the same server
detached on an ephemeral loopback port — `upeg http status --pairing`
reports the actual endpoint. If the Desktop app is already running, it may
already be hosting: reuse it instead of stopping it — `upeg http status`
shows the live endpoint. `upeg call` auto-attaches to a running host through
the discovery file (`~/.upeg/server.json`) unless you pass `upeg call
--local`. Loopback and `chrome-extension://` origins are always
CORS-allowed; other web origins need a repeatable `--cors-origin`.

**PWA / Chrome extension.** `just package-web` produces a statically
hostable PWA bundle; `./chrome-ext/build.sh` stages an MV3 extension into
`chrome-ext/dist/` for `chrome://extensions` → "Load unpacked". Both are
sandboxed surfaces — tools needing a native host run through a paired local
host. Neither is currently hosted for you; you build and run them locally.

Long-running `External` tools stream their output while they run (CLI
mirrors it to stderr, HTTP exposes `POST /v1/tools/{id}/stream` as NDJSON,
MCP sends `notifications/message`), then every surface still receives the
same final envelope.

## Add your own tools

| Method | Location | Notes |
| --- | --- | --- |
| TOML Toolkit | `~/.upeg/toolkits/{toolkit_id}.toml` | Wrap commands, HTTP calls, scripts. See [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md) and `examples/tools/` |
| Project manifest | `upeg.toml` at a project root | Auto-detected (cwd → ancestors, bounded by `$HOME`); can declare project boards. See [`docs/architecture/project-manifest.md`](docs/architecture/project-manifest.md) |
| WASM plugin | `~/.upeg/wasm/*.wasm` | `upeg plugin new` scaffolds a guest crate. See `examples/plugins/greet/` |
| MCP import | `~/.upeg/mcp-imports/*.toml` | Re-expose an upstream MCP server's tools. See `examples/mcp-imports/` |
| Rust built-in | `#[upeg::tool]` in `upeg-tools/` | Compile-time registration. See the [Tool author guide](docs/guides/tool-author.md) |

Directories are overridable via `$UPEG_TOOLKITS_DIR`, `$UPEG_WASM_DIR`,
`$UPEG_MCP_IMPORTS_DIR`; disable project-manifest detection with
`$UPEG_PROJECT_MANIFEST_PATH=off`. Validate authored files with
`upeg tool validate <path>`.

A taste of the TOML form — wrap `git log` as a Tool with a default:

```toml
[[tools]]
id = "git_log"
invoker = "External"
pegboard_units = "U1"
command = "git"
args_template = ["log", "--oneline", "-n", "{count}"]

[[tools.inputs]]
name = "count"
type = "integer"
default = 10
```

## Media tools

Media Tools exchange **bytes, not paths** — pass file input with
`-a <key>=@<path>` and receive file output with `--out`, so the same Tool
works on the CLI and, through a paired host, in the browser surfaces.

```bash
upeg call media.image_convert -a input=@photo.png -a output_format=webp --out photo.webp
upeg call media.pdf_to_markdown -a input=@doc.pdf --json
```

The CLI refuses to overwrite an existing file unless you pass `--force`;
with no `--out`, the Tool writes its chosen name into the current directory.
Formats, engines, and per-tool limits: [PDF tools guide](docs/pdf-tools.md).

<a id="file-input-wire"></a>

## File input/output wire contract

File input and output use one canonical `FileValue` JSON on every surface:
`name`, `is_dir`, optional `mime`, and `content` — `content.bytes` is
standard padded RFC 4648 Base64, and directories nest recursively under
`content.entries`. The schema extension `x-upeg-file-wire` points here.

> **Breaking migration (beta):** the former `"bytes":[0,255]` numeric array
> is no longer supported — File values must carry a padded standard Base64
> string such as `"bytes":"AP8="`.

The full contract — exact JSON examples, HTTP/MCP call shapes, and all size
budgets — lives in
[`docs/architecture/file-wire.md`](docs/architecture/file-wire.md).

## Documentation

Start at [`docs/index.md`](docs/index.md). Highlights:

- [Guides](docs/guides/index.md) — installation, quick start,
  troubleshooting, development, Tool authoring
- [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md) — complete Toolkit TOML
  field reference (generated)
- [`docs/architecture/`](docs/architecture/index.md) — the contracts every
  surface shares: call envelope, manifest, I/O types, File wire, HTTP API,
  host topology, MCP
- [`docs/product/`](docs/product/index.md) — what upeg is and is not, and
  its non-negotiable security rules
- [`docs/LEXICON.md`](docs/LEXICON.md) — the single vocabulary shared by
  product, UI, CLI, manifests, and code

## Contributing, security, license

Contributions are welcome — note that development happens in a private
source repository: your pull request is reviewed here, then imported into
the source repository and published back through a later exported update
PR. The mechanics are in [CONTRIBUTING.md](CONTRIBUTING.md); support
expectations live in [SUPPORT.md](SUPPORT.md), and notable changes in
[CHANGELOG.md](CHANGELOG.md).

Report suspected vulnerabilities privately — see
[SECURITY.md](SECURITY.md). The non-negotiable security rules the
product holds itself to are in
[`docs/product/security-absolutes.md`](docs/product/security-absolutes.md).

UPeg's own code is [Apache-2.0](LICENSE). Plugin authors can choose MIT or
Apache-2.0 per directory for `upeg-plugin-api` and `upeg-plugin-macros`.
Third-party components such as vendored code and fonts retain their own
licenses; [NOTICE](NOTICE) records their sources and notices.
