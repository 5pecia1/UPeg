# upeg — Universal Pegboard

**English** | [한국어](README.ko.md)

Pin a tool once, call it from anywhere. upeg turns the commands, conversions,
and checks you reach for every day into **Tools** that live on a board — and
exposes each Tool on the surfaces it supports: the desktop app, the terminal,
an AI agent over MCP, or a small local HTTP API.

One definition — a Rust `#[upeg::tool]` function, a TOML manifest, a WASM
plugin, or an imported MCP server — registers a Tool once and makes it
callable across **CLI, TUI, Desktop, PWA, Chrome extension, MCP, and HTTP**,
with no per-surface code.

## What you can do with it

- **Run the built-in toolbox** — conversions, encoders, hashing, IDs, text
  tools, CSV, QR, image conversion, and PDF/Office inspection.
- **Pin Tools to Boards** — tools, saved input presets, and usage
  instructions per context; project boards live in `.upeg/project.toml`.
- **Drive everything from the keyboard** in the TUI and Desktop app, or
  script the same Tools from the CLI with stable JSON output.
- **Hand a board to an agent** — `upeg mcp` serves your Tools over stdio
  JSON-RPC, scoped to one board's pins and instructions if you want.
- **Wrap your own commands** — TOML `External` invokers, Chains, WASM
  plugins, imported MCP servers; credential values stay in environment
  variables or the OS keychain.

Local-first: the built-in toolbox runs on your machine, the HTTP host binds
loopback by default, and every route except `/healthz` requires a bearer
token. There is no cloud sync. A selected distributed Toolkit may download its
versioned pack once before its first use; built-in Tools and already cached
packs stay available offline.

## Project status

upeg is early-stage:

- **Downloads vary by release.** See [GitHub Releases](https://github.com/5pecia1/UPeg/releases)
  for published packages. They are unsigned; building from source (below)
  remains an alternative. The latest release provides Linux x86_64 and ARM64
  CLI archives, a macOS ARM64 CLI archive, a Windows x86_64 CLI archive, a
  Linux Debian package and AppImage, a universal macOS DMG, a Windows MSIX,
  and a web archive.
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
  Windows), and **Rust 1.92+**. Install Rust with the OS-specific
  instructions at <https://rustup.rs/>: on macOS/Linux use
  `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`; on
  Windows run `rustup-init.exe` from that site.

```bash
git clone https://github.com/5pecia1/UPeg.git
cd UPeg
cargo install --locked --path upeg-cli   # installs `upeg` into ~/.cargo/bin
upeg doctor                              # prints binary/feature/source-dir diagnostics
```

The Desktop app, PWA, and WASM plugins need more (Flutter SDK, `wasm-pack`,
extra targets) — but not for the CLI. The full matrix, including optional
packaging tools, is in the
[Installation guide](docs/guides/installation.md).

`upeg paths` (or `upeg paths --json`) reports paths without loading Tools or
changing files. **Unreleased storage upgrade:** new native installations use
`<profile-home>/.upeg` on every OS, with `config/` (tweaks, credentials, MCP
imports), `data/` (the single SQLite database, toolkits, WASM), `state/` (HTTP
log), `cache/toolkit-packs/`, and `runtime/` (discovery and desktop lock).
Windows resolves profile home from `USERPROFILE`, then `HOMEDRIVE` + `HOMEPATH`,
then `HOME`; XDG variables do not select the new default. Default home and
migration roots must be absolute; an invalid home never falls back to the working
directory.

Existing flat home storage or Windows `%APPDATA%/upeg` remains `legacy-v1` until
explicit migration. A custom `UPEG_HOME` remains flat even when its directory is
empty; legacy relative/empty `UPEG_HOME` values retain their getter behavior.
Pointing it at the default home root follows default selection. Lease/marker and
child-process roots are always absolute and normalized. The `storage-layout.json` marker pins migrated/new split storage and old roots can
redirect once to it. Malformed markers, ambiguous populated roots, unsafe paths,
and incomplete migrations fail closed rather than silently selecting empty data.
Per-artifact `UPEG_TOOLKITS_DIR`, `UPEG_WASM_DIR`, `UPEG_MCP_IMPORTS_DIR`, and
`UPEG_TOOLKIT_CACHE_DIR` still win verbatim (including relative/empty values).
Migration excludes overridden artifact directories; it does not move external
paths or project `.upeg` declarations.

Use `upeg storage status --json`, then `upeg storage plan --source /old/root
--target /new/root --output /outside/plan.json --json`. Source and target may be
the same root. Planning reads only and the optional output must be a new file.
Stop **all old and new** CLI, desktop, MCP, and HTTP hosts before running
`upeg storage apply --plan /outside/plan.json --yes --quiesced --json`.
`--quiesced` acknowledges that old binaries cannot honor the new storage lease.
Existing discovery/desktop-lock files block migration; investigate them and
remove stale evidence only after confirming the corresponding host has stopped.

Migration retains sources and private staging/journals in
`<target>/state/migrations/`; it uses SQLite backup, including committed WAL
rows, without splitting the schema. Reapply the same plan after an interrupted
apply. `upeg storage verify --target /new/root --json` checks preservation;
`upeg storage rollback --target /new/root --yes --quiesced --json` is refused
after durable source or destination changes, and retains copied data inactive.
There is no cleanup command. Relocating `toolkits/ecosystem.toml` requires
`--ecosystem-prefix /installation/prefix` during planning: only the exact,
owned `share/ecosystem/install.json` receipt is rebound, preserving toolkit bytes
and other receipt keys. Receipt relocation currently requires the installer's
Unix file-lock implementation; unsupported platforms refuse that operation.

For example, `upeg --project /path/to/project paths --json` selects that project
root, which must contain a `.upeg` directory, ahead of any environment override.
Without `--project`, `UPEG_PROJECT_MANIFEST_PATH` takes precedence over automatic
discovery (`off` disables discovery). Automatic discovery uses the nearest
`.upeg` marker within the existing HOME boundary; outside HOME, only the current
directory is checked. The user storage root is excluded from automatic project
discovery. When no project is found, JSON reports `"project": null`. This command
performs no migration or project edits: `.upeg/project.toml` and `.upeg/toolkits`
remain project-local.

## First success

```bash
upeg call --local num.hex_to_decimal -a input=0xff
# → 255
```

This built-in call needs no network, account, or token — `--local` forces
in-process dispatch. Without it, `upeg call` uses a running local host when
one is discoverable. A selected distributed Toolkit is different: its first
call may fetch the exact versioned pack listed by the release catalog, then
the verified cached pack can run offline. Then look around:

```bash
upeg tool list                 # every registered Tool
upeg num hex-to-decimal 0xff   # short dynamic form of the same call
upeg                           # on a terminal: the interactive TUI
```

Every surface returns the same structured `ToolResult` envelope
(`ok`, `primary_output_id`, `outputs[]`); the plain CLI prints just the
primary value. The [Quick start guide](docs/guides/quick-start.md) walks
through boards, presets, MCP client setup, and file input.

## Choose your surface

| Surface | Start it |
|---|---|
| Terminal (TUI / CLI) | `upeg` · `upeg call <toolkit>.<tool>` |
| Desktop app | `cd flutter_app && flutter run -d linux` |
| MCP (agents) | `upeg board dev connect` · `upeg mcp --board dev` |
| HTTP host | `upeg http --addr 127.0.0.1:7173` |
| PWA | `just package-web` |
| Chrome extension | `./chrome-ext/build.sh`, then load `chrome-ext/dist/` unpacked |

Sandboxed surfaces (PWA, extension) run native-runtime tools through a
paired local host. PWA deployment needs cross-origin isolation headers, and
the extension needs host endpoint/token pairing; both are covered by the
[Installation guide](docs/guides/installation.md). Wrap your own commands as
Tools — see the [Tool author guide](docs/guides/tool-author.md).

<a id="file-input-wire"></a>

File input and output use one canonical `FileValue` JSON on every surface —
the contract lives in
[`docs/architecture.md#file-wire`](docs/architecture.md#file-wire).

> **Breaking migration (beta):** the former `"bytes":[0,255]` numeric array
> is no longer supported — File values must carry a padded standard Base64
> string such as `"bytes":"AP8="`.

## Documentation

- [`docs/index.md`](docs/index.md) — documentation home
- [Guides](docs/guides/quick-start.md) — installation, quick start, tool
  authoring, development, troubleshooting
- [`docs/architecture.md`](docs/architecture.md) — the contracts every
  surface shares
- [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md) — complete Toolkit TOML
  field reference (generated)
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
[`docs/architecture.md#security-absolutes`](docs/architecture.md#security-absolutes).

UPeg's own code is [Apache-2.0](LICENSE). Plugin authors can choose MIT or
Apache-2.0 per directory for `upeg-plugin-api` and `upeg-plugin-macros`.
Third-party components such as vendored code and fonts retain their own
licenses; [NOTICE](NOTICE) records their sources and notices.
