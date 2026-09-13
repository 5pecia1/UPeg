# upeg — Universal Pegboard

**English** | [한국어](README.ko.md)

A Rust workspace where one Tool definition works consistently across seven surfaces: CLI, TUI, Desktop, PWA, Chrome extension, MCP, and HTTP.

One `#[upeg::tool]` annotation exposes a Tool on every surface. TOML, WASM plugins, and external MCP servers are integrated through the same registry.

## Prerequisites

Install these tools for local development and builds:

- **Rust 1.92+**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **`wasm32-unknown-unknown` target**: `rustup target add wasm32-unknown-unknown`
- **just**: `cargo install just --locked`
- **python3**: Included with most Linux distributions (`apt install python3` or `brew install python3`)
- **Flutter SDK**: Download for Linux from [https://docs.flutter.dev/get-started/install](https://docs.flutter.dev/get-started/install). For Linux desktop dependencies, see the packaging table in [Build and run](#build-and-run).
- **flutter_rust_bridge_codegen 2.12.0**: `cargo install flutter_rust_bridge_codegen --locked --version 2.12.0`
- **wasm-pack**: `cargo install wasm-pack --locked`
- **actionlint**: `bash <(curl -s https://raw.githubusercontent.com/rhysd/actionlint/main/scripts/download-actionlint.bash)`

## Build and run

```bash
# Release build
cargo build --release -p upeg-cli --bin upeg

# List registered tools
./target/release/upeg tool list

# Run a tool
./target/release/upeg num hex-to-decimal 0xff      # → 255
./target/release/upeg call num.hex_to_decimal -a input=0xff
./target/release/upeg call num.hex_to_decimal -a input=0xff --json
./target/release/upeg call num.hex_to_decimal -a input=0xff --field result
./target/release/upeg call num.hex_to_decimal -a input=0xff --pretty

# Manage board pins (pin / unpin / move)
./target/release/upeg board dev pin num.hex_to_decimal

# Shell completions (bash / zsh / fish / elvish / powershell)
# bash — user path for bash-completion 2.x
mkdir -p ~/.local/share/bash-completion/completions
./target/release/upeg completions bash > ~/.local/share/bash-completion/completions/upeg
# zsh — place it in a directory in fpath, then run
#       fpath=(~/.zfunc $fpath) before compinit in ~/.zshrc
mkdir -p ~/.zfunc && ./target/release/upeg completions zsh > ~/.zfunc/_upeg
# fish
mkdir -p ~/.config/fish/completions
./target/release/upeg completions fish > ~/.config/fish/completions/upeg.fish
# The Tool list is embedded when the script is generated. Regenerate it after
# installing a Toolkit or plugin so that new Tools appear.
# Full contract: the "Shell completion" section of docs/architecture/call-envelope.md

# MCP server (stdio JSON-RPC) — for Claude Desktop / Cursor integration
./target/release/upeg mcp

# Inspect a board's guidance and tools, and generate MCP connection settings
./target/release/upeg board dev context --json
./target/release/upeg board dev connect

# HTTP server
./target/release/upeg http --addr 127.0.0.1:7173

# When connecting from a browser (PWA) origin: loopback and chrome-extension://
# are allowed by default. Other web origins must exactly match a repeatable --cors-origin.
./target/release/upeg http --addr 127.0.0.1:7173 --cors-origin https://upeg.example.com

# Background host (does not occupy the shell; same binary)
./target/release/upeg host start --daemon

# call automatically attaches to the running host through the discovery file
# (~/.upeg/server.json); no separate flag is needed.
./target/release/upeg call num.hex_to_decimal -a input=0xff

# HTTP REST API — bearer-token authentication
UPEG_HTTP_TOKEN=dev-token ./target/release/upeg host start --addr 127.0.0.1:7174 --daemon
curl -H 'Authorization: Bearer dev-token' http://127.0.0.1:7174/v1/tools
curl -H 'Authorization: Bearer dev-token' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' \
  http://127.0.0.1:7174/mcp

# Health check — no authentication required; importsPending reports MCP-import progress.
curl http://127.0.0.1:7174/healthz

# Pairing — prints the running host's endpoint and token as text (local operator only;
# it exists only in text status output and is not exposed by any HTTP route).
./target/release/upeg http status --pairing

# Diagnostics
./target/release/upeg doctor
```

### Tool output contract

The source of truth for a Tool result is the canonical structured `ToolResult`,
not raw stdout. A successful envelope contains `ok=true`, `primary_output_id`,
and `outputs[]`; every output row has `id`, `label`, `kind`, and `value`.
`primary_output_id` is required when there is at least one output, and is empty
for action-only Tools.

- The default CLI mode writes only the primary output value to stdout.
- `upeg call ... --json` writes the canonical success or failure JSON envelope to stdout.
- `upeg call ... --field <id>` writes only the requested output value.
- `upeg call ... --pretty` writes label-and-value rows.
- HTTP, host, and FRB return the canonical JSON envelope.
- MCP `tools/call` uses the canonical envelope as `structuredContent`; text
  `content` is a fallback for displaying the primary output.
- TUI, Desktop, PWA, Chrome extension, and Controlled Embed render the same output row
  labels and values in a presentation appropriate to each surface.
- A `Chain` Tool result contains a `steps` output row with every step's `id`, `status`,
  and `duration_ms`. An approval step is accepted only when the server-stamped
  `_upeg.surface` is authorized by `approval_surfaces`. CLI (`-a approve=true`), TUI
  (confirmation dialog), and Desktop (confirmation dialog) all send `approve` only
  after a human has confirmed it.

A failed envelope has `ok=false` plus `error`. The `External` invoker (an external
binary) additionally includes `error.details`: `exit_code` (`null` when it did not
exit normally), `signal` when it was signal-terminated, `timed_out` when it exceeded
`timeout_ms`, and **capped stdout/stderr**. `--check`-style tools write diagnostics to
stdout, so without these fields the reason for failure would disappear entirely. CLI
(human/`--json`), MCP, HTTP, TUI, and Flutter all read the same keys. See the full
contract in [`docs/architecture/manifest.md`](docs/architecture/manifest.md).

There is one unchanged envelope after execution **finishes**. Long-running `External`
Tools have an additional optional channel that streams output **during** execution;
surfaces that cannot consume it still receive only the final envelope as before.

- Default CLI and `--pretty` mirror child stdout and stderr to **stderr** while running.
  stdout is reserved for the final result. `--json` and `--field` stream nothing.
- HTTP `POST /v1/tools/{id}/stream` sends `chunk` lines followed by one `result` line as
  `application/x-ndjson`.
- MCP `tools/call` sends `notifications/message` while running (level `info`, logger
  `upeg.tool`); its final response frame is unchanged.
- TUI and Desktop show the last few lines as a live tail and cancel execution with `Esc`
  (TUI) or the Cancel button (Desktop). The same applies to a TUI attached to a host:
  it uses the NDJSON route above, so tailing and cancellation work as they do locally.
  When attached to an older host that does not know the streaming route, it falls back
  to the buffered path and writes "No live output" in the tail.

## Desktop UI

Controlled Embed runs in an app-owned WebView session. Card Run and debug Run/Re-run
use the same execution path, and opening or closing Debug does not recreate the page.
When the app starts with Desktop's **Local HTTP host** enabled, CLI calls connected to
that embedded host can use the same session. The Rust dispatcher handles input presets,
output types, and the primary result. This is distinct from locally running a project
Tool or headless execution through a separate CLI host. For the full scope, see the
[Controlled Embed session contract](docs/ui-ux-surface-contract.md#desktop-controlled-embed의-공통-실행과-디버그).

Desktop and PWA are built with **Flutter + flutter_rust_bridge**. The Rust workspace
owns the Tool registry, execution, and platform IPC; `flutter_app/` owns UI and
rendering (cutover completed 2026-05-24). The Chrome extension is a JavaScript
implementation in `chrome-ext/` that uses HTTP and the current tab's content script.
The former Dioxus-based `desktop-ui/` comparison build was removed during Phase 11
cleanup (`6597a3c`, range `dd28130..d404cec`, and follow-up closure commits).

## Running the app

### Linux

```bash
cd flutter_app
flutter run -d linux
```

CMake automatically invokes `cargo build -p upeg_frb` and installs the generated
cdylib (`libupeg_frb.so`) in the Flutter bundle's `lib/` directory. No separate
manual `cargo build` step is needed: the cargokit plugin in `flutter_app/rust_builder/`
handles it in the Flutter build pipeline.

### macOS

```bash
just flutter-run-macos
UPEG_FLUTTER='fvm flutter' just flutter-run-macos
```

The macOS runtime smoke test runs on a real Mac. `flutter-build-macos` validates a
release build, while `flutter-run-macos` directly checks GUI startup paths such as the
window manager, tray, FRB initialization, and board/tag-selection parity.

### Web

```bash
just flutter-run-web         # build-frb-wasm + flutter run -d chrome
just flutter-run-web-server  # build-frb-wasm + flutter run -d web-server
just flutter-web-smoke       # release web build + headless Chromium boot check
```

The Web target requires the wasm-pack output from `upeg_frb`, so use these `just`
recipes. `RustLib.init()` loads `web/pkg/upeg_frb.js` and `upeg_frb_bg.wasm` through
the FRB Web loader.

The PWA path also uses the real FRB wasm implementation for local-first features:
Tool listing, built-in Tool execution, pegboard/tweaks/backup storage, embed URL
resolution, and opening an external browser. Only features requiring a host process,
such as background-host control, are desktop-only.

### Manual build (alternative)

```bash
cargo build --release -p upeg_frb
cd flutter_app && flutter run -d linux
```

Because the CMake integration already does the same work, the manual `cargo build`
prerequisite above is redundant but harmless. It **does not replace the cargokit
bundling step**: without it, the cdylib builds but is not included in the Flutter bundle.

### Flutter build

```bash
cd flutter_app
flutter pub get
flutter run -d linux            # or -d chrome, -d web-server
```

Build the Chrome MV3 extension with `./chrome-ext/build.sh`. Its investment direction
is **in-page features possible only in the extension** ([surface contract](docs/ui-ux-surface-contract.md#chrome-extension-contract)):

- **In-page detectors** — a frozen table in `chrome-ext/detectors.js`: hex →
  `num.hex_to_decimal`, base64 → `convert.base64_decode`, epoch → offline ISO preview
  (no corresponding Tool). On hover, the host fills the tooltip with the real result;
  without a host it falls back to the `upeg://open?...` deep link.
- **In-page selector adapter** — applies a `ControlledEmbed` pin's selector binding to
  the tab currently being viewed. Desktop controls its own WebView; the extension
  controls the user's tab.
- **Per-site enablement** — target sites are a saved allow-list, not manifest constants.
  "Enable on this site" in the popup requests permission and the service worker
  registers it through `chrome.scripting.registerContentScripts`. etherscan and
  polygonscan are enabled on installation.

Tools that the popup cannot dispatch (`static` invokers and Controlled Embeds with no
page to open) open the Flutter desktop instance through the `upeg://open?...` deep link.

### Packaging (creating installable artifacts)

```bash
# All artifacts for the host OS (only what the current OS can create)
just package

# Individual artifacts
just package-linux-deb        # → target/packages/linux/upeg_<ver>.deb
just package-linux-appimage   # → target/packages/linux/upeg-<ver>-x86_64.AppImage
just package-macos-dmg        # → target/packages/macos/upeg-<ver>.dmg     (macOS only)
just package-windows-msix     # → target/packages/windows/*.msix          (Windows only)
just package-web              # → target/packages/web/ (PWA bundle for static hosting; .js/.css pre-gzipped)
```

All artifacts are in `target/packages/<platform>/`. Linux desktop packages include
the app bundle, `/usr/bin/upeg`, a desktop launcher, a 512px icon, and AppStream
metadata. Local-host dependencies:

| Recipe | Required tools |
|---|---|
| `flutter-build-linux`, `flutter run -d linux` | `libayatana-appindicator3-dev`, `libkeybinder-3.0-dev` (hotkey_manager X11 backend; runtime: `libkeybinder-3.0-0`) + Flutter's standard Linux dependencies (`libgtk-3-dev`, `pkg-config`, etc.) |
| `package-linux-deb` | `fpm` (`gem install fpm`) |
| `package-linux-appimage` | `appimagetool` ([AppImageKit releases](https://github.com/AppImage/AppImageKit/releases)) + `appstreamcli` (`appstream`); recipe uses `APPIMAGE_EXTRACT_AND_RUN=1` so devcontainers do not need FUSE |
| `package-macos-dmg` | `create-dmg` (`brew install create-dmg`) — macOS host only |
| `package-windows-msix` | the `msix` dev dependency and `msix_config` in `pubspec.yaml` — Windows host only |
| `package-web` | `gzip` |

Signing and notarization are intentionally out of scope; all artifacts are unsigned.
Host HTTP transport uses loopback TCP, so it has the same model on Windows, macOS,
and Linux.

## Testing

Verification has two tiers.

| Tier | Command | When | Duration |
| --- | --- | --- | --- |
| pre-commit | `just check` | **before every commit** | ~2–3 minutes with a warm cache |
| full closure | `just verify` | before pushing / opening a PR | equivalent to one CI run |

`just check` runs fmt-check, clippy-native, file-size-budget (both Rust and handwritten
Dart must be ≤1000 lines; generated FRB code and `*.freezed.dart`/`*.g.dart` are
excluded), lexicon-check, and `cargo test --workspace --lib` (unit tests only). It
includes no Flutter, drift-fixture, or integration tests: it is the smallest gate that
is affordable before every commit.

## Regenerating generated artifacts

Generated artifacts such as Tool output, interface metadata, schemas, FRB bindings,
and UI-parity baselines are checked for drift by `*-check` recipes. After changing a
tool, Tool, or UI, run the appropriate regeneration recipe, update the artifacts, and
commit them.

| Drift gate | Check recipe | Regeneration recipe |
|---|---|---|
| Tool execution baseline | `just test-baseline-check` | `just test-baseline` |
| Interface metadata | `just interface-inventory-check` | `just interface-inventory` |
| Toolkit schema | `just toolkit-schema-check` | `just toolkit-schema` |
| FRB bindings (Rust/Dart) | `just frb-codegen-check` | `just frb-codegen` |
| UI parity (golden regression) | `just ui-parity-check` | `just ui-parity` |

## Adding user Tools

| Method | Location | Notes |
| --- | --- | --- |
| Rust built-in | a `#[upeg::tool]` function in `upeg-tools/src/lib.rs` | Compile-time registration |
| TOML | `~/.upeg/toolkits/{toolkit_id}.toml` | See [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md) and `examples/tools/` |
| WASM plugin | `~/.upeg/wasm/*.wasm` | See [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md) and `examples/plugins/greet/` |
| MCP import | `~/.upeg/mcp-imports/*.toml` | See [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md) and `examples/mcp-imports/` |
| Project Manifest | `upeg.toml` at the project root | Auto-detected (cwd → ancestors through `$HOME`). See [`docs/architecture/project-manifest.md`](docs/architecture/project-manifest.md) |

Each directory can be overridden with `$UPEG_TOOLKITS_DIR`, `$UPEG_WASM_DIR`, or
`$UPEG_MCP_IMPORTS_DIR`. Disable Project Manifest auto-detection with
`$UPEG_PROJECT_MANIFEST_PATH=off`, or pin it to an absolute path. A Project Manifest
can also declare project-specific boards through top-level `[[boards]]`. The external
manifest authoring guide is [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md), and the
editor/CI validation schema is [`fixtures/toolkit.schema.json`](fixtures/toolkit.schema.json).
Both are generated from `upeg-loader` metadata. Validate an authored file with
`upeg tool validate <path>` and check generated-artifact drift with
`just toolkit-schema-check`. An `External` invoker can force child-process color output
even in a non-TTY pipe with `color = "force"`.

File input policy is converted into the same core contract by the Rust macro and TOML.

```rust
required images: File(
    extensions = ["png", "jpg", "jpeg"],
    max_count = 100,
    max_file_bytes = 52_428_800,
    max_total_bytes = 52_428_800,
)
```

```toml
[[tools.inputs]]
name = "images"
type = "file"
required = true
extensions = ["png", "jpg", "jpeg"]
max_count = 100
max_file_bytes = 52428800
max_total_bytes = 52428800
```

`max_count` counts actual files in a recursive `FileValue` and defaults to 1. Declare
extensions without a dot; matching is case-insensitive. Size limits apply to each file
and to the total file bytes respectively.

<a id="file-input-wire"></a>

### File input/output JSON wire contract

File input and output use the same canonical `FileValue` JSON on every surface. Its
top-level fields, in order, are `name`, `is_dir`, optional `mime`, and `content`.
Omit `mime` when absent. `is_dir` is derived from `content.kind`, not independent state:
when reading JSON it must be `false` for `bytes` and `true` for `directory`; a mismatch
is rejected.

For a regular file, `content` is `{"kind":"bytes","bytes":"..."}`. `bytes` must be a
standard padded RFC 4648 Base64 string. For example, if `hello.txt` contains UTF-8
`hello`, its exact JSON is:

```json
{
  "name": "hello.txt",
  "is_dir": false,
  "mime": "text/plain",
  "content": {
    "kind": "bytes",
    "bytes": "aGVsbG8="
  }
}
```

A directory's `content` is `{"kind":"directory","entries":[...]}`. Every `entries`
item has the same `FileValue` format, so directories can be recursive. Send multiple
files as one `Directory`, not as a separate top-level array. For example, the exact
JSON for two files containing `A` and bytes `00 ff` is:

```json
{
  "name": "files",
  "is_dir": true,
  "content": {
    "kind": "directory",
    "entries": [
      {
        "name": "a.txt",
        "is_dir": false,
        "mime": "text/plain",
        "content": {
          "kind": "bytes",
          "bytes": "QQ=="
        }
      },
      {
        "name": "data.bin",
        "is_dir": false,
        "content": {
          "kind": "bytes",
          "bytes": "AP8="
        }
      }
    ]
  }
}
```

URL-safe Base64 `-`/`_`, spaces or line breaks, missing or non-canonical padding, and
the legacy numeric-array byte representation are all rejected. Only an empty file uses
the empty string `""`.

> **Breaking migration (beta):** The former `"bytes":[0,255]` numeric array is no
> longer supported for either input or output. Code that creates or consumes File values
> must use a padded standard RFC 4648 Base64 string such as `"bytes":"AP8="`.

For HTTP, put a File value directly in the Tool arguments and send it to
`POST /v1/tools/{id}`. The following request sends a canonical Directory containing
one PNG to `media.images_convert` and limits the final output to 8 MiB.

```bash
curl -X POST http://127.0.0.1:7174/v1/tools/media.images_convert \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data-binary @- <<'JSON'
{
  "images": {
    "name": "images",
    "is_dir": true,
    "content": {
      "kind": "directory",
      "entries": [{
        "name": "pixel.png",
        "is_dir": false,
        "mime": "image/png",
        "content": {
          "kind": "bytes",
          "bytes": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        }
      }]
    }
  },
  "output_format": "png",
  "max_output_bytes": 8388608
}
JSON
```

MCP stdio `tools/call` also puts the same Directory/Base64 value in `arguments`.
MCP framing is one JSON line per request, so an actual call sends one line without
line breaks as follows.

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"media.images_convert","arguments":{"images":{"name":"images","is_dir":true,"content":{"kind":"directory","entries":[{"name":"pixel.png","is_dir":false,"mime":"image/png","content":{"kind":"bytes","bytes":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="}}]}},"output_format":"png","max_output_bytes":8388608}}}' \
  | ./target/release/upeg mcp
```

The File property in JSON Schema adds `x-upeg-file-wire` to both the input
`inputSchema` and output `outputSchema`. This closed extension object declares, in a
machine-readable form, current version 1, `base64-rfc4648-padded`, no numeric arrays,
recursive Directory support, and this section's public path `README.md#file-input-wire`.
An external legacy schema may omit the extension, but when it is present every key and
value must exactly match the current contract.

The Tool's `max_file_bytes`/`max_total_bytes` policy and the surface safety limits below
both apply; the stricter limit takes precedence.

- Canonical File output is limited per root to 64 MiB (67,108,864 bytes) of decoded raw
  bytes, 128 total nodes including the root, 16 KiB (16,384 bytes) of combined name and
  MIME metadata, and recursive depth 64 with the root counted as 1. Core/CLI output
  processing and Flutter's File-output codec apply the same root budget.
- Core/CLI/Flutter File input is limited to 100 actual files, 128 total `FileValue`
  nodes, 16 KiB (16,384 bytes) of combined name and MIME metadata, recursive depth 64
  with the root counted as 1, and 50 MiB (52,428,800 bytes) of decoded raw bytes. This
  input budget is separate from the output root budget above.
- Each HTTP request body and MCP request line is limited to 1,000,000 bytes. This
  framing limit includes Base64, filenames, MIME, and the JSON/JSON-RPC envelope.
- The Chrome extension (Ext) first limits File-input decoded raw bytes to 640 KiB
  (655,360 bytes), and also limits the final HTTP request to under 1,000,000 bytes. It
  reads File output up to 64 MiB (67,108,864 bytes) of decoded raw bytes.
- `media.images_convert` is limited to a 512 MiB estimated working set, 64 MiB encoded
  result per image, and 64 MiB for the final ZIP. This batch Tool is exposed only on
  CLI/Desktop/MCP/HTTP/PWA/Ext, not TUI.
- `media.image_convert` accepts input files up to 50 MiB (52,428,800 bytes).
- `media.pdf_extract_images` is limited to 100 unique image XObjects, PDF Form XObject
  recursion depth 64, 64 MiB combined extracted encoded images, and a 64 MiB final ZIP
  including headers and notes.
- `media.pdf_inspect`/`media.pdf_to_markdown` are limited to 32 MiB input, 500 pages,
  and 8 MiB returned Markdown.

`x-upeg-file-policy` remains input-only; the fixed File-output root budget above is not
output-policy metadata. Instead, `media.image_convert`, `media.images_convert`,
`media.image_to_pdf`, and `media.pdf_to_images` accept the ordinary optional input
`max_output_bytes`. Its inclusive range is 1..=67,108,864 bytes and its default is
64 MiB. It limits the final returned File `content.bytes` size and fails on overflow;
quality or DPI is not automatically reduced to fit. `media.pdf_inspect` and
`media.pdf_to_markdown` do not accept this argument.

## Media tools

Media Tools exchange **bytes**, not paths. Read and pass file input with
`-a <key>=@<path>`, and receive File output with `--out`. This contract lets the same
Tool work in browsers (PWA/Chrome extension) as well as the CLI.

```bash
# Images → PDF — accepts one image, a flat directory, or a zip.
# A directory and zip become pages in name order.
upeg call media.image_to_pdf -a input=@scan.png --out out.pdf
upeg call media.image_to_pdf -a input=@images --out out.pdf
# Output is at most 8 MiB
upeg call media.image_to_pdf \
  -a input=@images.zip -a max_output_bytes=8388608 --out out.pdf

# PDF → PNG pages; output ZIP is at most 8 MiB
upeg call media.pdf_to_images \
  -a input=@out.pdf -a dpi=144 -a max_output_bytes=8388608 --out pages.zip

# Convert a directory's images to JPEG in one batch; output ZIP is at most 8 MiB
upeg call media.images_convert \
  -a images=@images -a output_format=jpeg -a max_output_bytes=8388608 \
  --out converted.zip

# Extract original images embedded in a document (separate from page rasterization)
upeg call media.pdf_extract_images -a input=@doc.pdf --out images.zip
upeg call media.pptx_extract_images -a input=@deck.pptx --out images.zip

# Inspect a PDF and convert native text to Markdown (also returns pages needing OCR)
upeg call media.pdf_inspect -a input=@doc.pdf --json
upeg call media.pdf_to_markdown -a input=@doc.pdf --json
```

For extraction status, limits, and distribution of the PDF text Tools, see the
[PDF Tools guide](docs/pdf-tools.md).

The zip returned by `media.pdf_to_images` names files `{pdf_stem}-p1.png`,
`{pdf_stem}-p2.png`, and so on. `dpi` defaults to 144 and has a valid range of 36–600.
The CLI directory input for `media.images_convert` reads only regular files immediately
inside it and processes them by name; subdirectories and symlinks are rejected. Flutter
uses the same File policy for multi-file selection and drag and drop. Supported image
input and output formats are PNG, JPEG, WebP, GIF, BMP, TIFF, ICO, and QOI; SVG input is
also supported. SVG is converted to a pixel image and can specify its output width.

```bash
# For one image, save the converted file directly instead of a ZIP
upeg call media.image_convert -a input=@photo.png -a output_format=webp --out photo.webp
# Render SVG to PNG while preserving aspect ratio
upeg call media.image_convert -a input=@icon.svg -a output_format=png -a svg_width=512 --out icon.png
# Composite transparent areas onto white and set JPEG quality
upeg call media.image_convert -a input=@logo.png -a output_format=jpeg \
  -a jpeg_quality=85 -a 'background=#FFFFFF' --out logo.jpeg
```

WebP output is lossless; GIF and multi-page TIFF convert only the first frame/page.
SVG files are limited to 4 MiB and ICO output to 256×256. The UI shows options for the
chosen format and provides supported image previews and file saving. The
development plan and format contract describe the
detailed behavior.

The CLI **refuses rather than overwrites** an existing file. Pass `--force` to overwrite.
If `--out` is omitted, the Tool writes to the current directory using its chosen name.

All PDF engines are pure Rust (`hayro` rendering / `lopdf` extraction / `krilla`
generation), so no native dynamic library or separate environment-variable setup is
needed. When `pdf_extract_images` encounters an unsupported filter or color space
(JPX/JBIG2, CMYK/Indexed, etc.), it skips it rather than emitting a corrupt file and
records the reason in `EXTRACTION-NOTES.txt` in the zip. OCR, page ranges, PDF passwords,
and compression/quality controls are out of scope.

## Documentation

`docs/` is an OKF v0.2 knowledge bundle. Start at [`docs/index.md`](docs/index.md).

- [`docs/LEXICON.md`](docs/LEXICON.md) — term definitions (start here before renaming)
- [`docs/product/identity-and-boundaries.md`](docs/product/identity-and-boundaries.md) — product identity and boundaries
- [`docs/architecture/`](docs/architecture/index.md) — crate boundaries, domain/protocol/host contracts, and technical stack
- [`docs/guides/tool-author.md`](docs/guides/tool-author.md) — Tool-author guide

## Public-mirror verification and licenses

`just verify` in the public mirror runs Rust, WASM, and Flutter validation; Linux and web
builds; and license checks. Tool versions and commands are specified in
`scripts/verify_public.sh` and public CI. Public contribution follows the
[contribution guide](CONTRIBUTING.md).

UPeg's own code is [Apache-2.0](LICENSE). Plugin authors can choose the MIT or
Apache-2.0 license in each directory for `upeg-plugin-api` and `upeg-plugin-macros`.
Third-party components such as vendored code and fonts retain their own licenses;
[NOTICE](NOTICE) records their sources and notices.
