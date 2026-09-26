---
type: Guide
title: Tool author guide
description: "The keys and variants available when defining a new Tool with the `#[tool(...)]` macro, and how to validate."
tags: [guide, tools, macros, authoring]
status: stable
sources:
  - id: tool-macro
    resource: ../../upeg-macros/src/lib.rs
    title: "`#[upeg::tool]` macro implementation"
  - id: greet-plugin
    resource: ../../examples/plugins/greet
    title: WASM guest plugin example
---

Declare `inputs` and `outputs` for automatic rendering on declared surfaces
that support the Tool's capabilities; bespoke UI code is usually unnecessary.

```rust
#[tool(
    id = "demo.upper", toolkit = "demo",
    description = "Convert an ASCII string to uppercase",
    inputs  = [ required input: String = "Source text" ],
    outputs = [ result: String = "Uppercase result" ],
    pin = Inline, pegboard_units = U1, invoker = Function,
)]
pub fn upper(input: &str) -> String {
    input.to_uppercase()
}
```

# Source — how a tool starts

The `source` key declares **how the tool starts** in the GUI; non-GUI
surfaces call the function directly. Duration suffixes: `ms`, `s`, `m`, `h`.

| Variant | Meaning | Example |
|---|---|---|
| `UserInput` (default) | user fills the form, then runs | can be omitted |
| `Manual` | a button click triggers it | a UUID generator that works in one shot |
| `Timer("30s")` | periodic automatic run | live clock, network status |
| `Shortcut("⌘⇧N")` | keyboard shortcut | action tools like note creation |
| `Static` | never starts; static output | View Embed |

# Inputs and outputs

The closed type set and inline constraints are defined by the
[I/O type system](../architecture.md#io-types).

```rust
inputs = [
    required hex:     String                                    = "Hex value",
    required base:    Options(["hex", "dec", "bin"])            = "Output base",
    required port:    Number(min=1, max=65535, default=8080)    = "Port number",
    optional pattern: String(regex="^[a-z]+$", placeholder="a") = "Pattern",
    optional flags:   MultiOptions(["i","m","s","x"])           = "Regex flags",
],
outputs = [
    result: Number = "Decimal value",
    log:    String = "Debug log",
],
```

The `outputs` syntax mirrors `inputs` minus the `required`/`optional`
keywords. One output-only variant exists: `EmbeddedView("https://...")`.

# The two embed modes

## View Embed (passive)

Displays an external website on the pegboard as-is. upeg is not involved in
its input or output.

```rust
#[tool(
    id = "embed.mdn", toolkit = "embed", source = Static,
    outputs = [ view: EmbeddedView("https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference") ],
    pin = Embed, pegboard_units = U2, invoker = Static,
    surfaces = [desktop, pwa, ext],
)]
pub fn mdn() {}
```

## Controlled Embed (driven)

Uses an external website as the tool engine: upeg manipulates its DOM
through CSS selectors while the user sees an ordinary form and result.

The macro can describe this Tool's interface, but it cannot supply the
`embed_url` and `controlled_embed.bindings` needed to run the selector
pipeline. For a complete, offline TOML example with input, trigger, and
output bindings, see the [Controlled Embed example](https://github.com/5pecia1/UPeg/blob/main/examples/tools/embed-mdn.toml).

In the GUI, `webview_flutter` (desktop) or the browser itself (PWA/ext)
runs the page; non-GUI surfaces use a system Chrome/Chromium/Edge in
headless mode. `Invoker::Embed` pairs only with `PinKind::ControlledEmbed`
and `Invoker::Static` only with `PinKind::Embed` — see the confusable pair
in the [Lexicon](../LEXICON.md).

Selector robustness: prefer `#id`, then `[data-testid]`; class/structural
selectors break on the first redesign. Test on the desktop webview and
headless Chrome.

# Declarative TOML tools

The TOML manifest loader accepts the same `inputs` and `outputs`; runtime
behavior is declared through `invoker` / `embed_url` /
`controlled_embed.bindings`. Full field reference: the
[generated manifest guide](../TOOL_MANIFEST.md).

## Where tools come from

| Method | Location | Notes |
|---|---|---|
| TOML Toolkit | `~/.upeg/toolkits/{toolkit_id}.toml` | Wrap commands, HTTP calls, scripts — see `examples/tools/` |
| Project manifest | `upeg.toml` at a project root | Cwd is always checked; ancestors are checked only inside `$HOME`. Can declare project boards. |
| WASM plugin | `~/.upeg/wasm/*.wasm` | `upeg plugin new` scaffolds a guest crate — see `examples/plugins/greet/` |
| MCP import | `~/.upeg/mcp-imports/*.toml` | Re-expose an upstream MCP server's tools — see `examples/mcp-imports/` |
| Rust built-in | `#[upeg::tool]` in `upeg-tools/` | Compile-time registration |

Directories override via `$UPEG_TOOLKITS_DIR`, `$UPEG_WASM_DIR`,
`$UPEG_MCP_IMPORTS_DIR`; `$UPEG_PROJECT_MANIFEST_PATH=off` disables
project-manifest detection, while an absolute path selects one manifest
directly. Validate with `upeg tool validate <path>`.

## External — wrapping a local command

`invoker = "External"` declares not just a `command` but **how the child
process runs** — cwd, environment, timeout, terminal. A complete copyable
example lives in `examples/tools/dev-external-demo.toml`.

```toml
[[tools]]
id = "cargo_check"
pegboard_units = "U2"
invoker = "External"
command = "cargo"
args_template = ["check", "--manifest-path={manifest_path}", "--quiet"]
cwd = "."             # relative to the manifest's directory
timeout_ms = 600000   # unlimited when omitted
env = [{ name = "RUST_LOG", value = "warn" }]

[[tools.inputs]]
name = "manifest_path"
type = "file_path"
default = "Cargo.toml"
```

| Field | Meaning |
|---|---|
| `cwd` | The directory the child runs in; relative paths resolve against the manifest's directory. |
| `env` | Plaintext env vars only — secrets go in `credentials`, which apply later and win a name collision. |
| `timeout_ms` | Wall-clock budget; on expiry the whole process group dies with `details.timed_out = true`. No default. |
| `color` | `"inherit"` (default) or `"force"` — see below. |
| `pty` | Unix-only real terminal — see below. |
| `setup` | Display-only install/usage guidance — never executed. |

**Working directory.** A declared `cwd` always wins. Without one, toolkit
tools (`~/.upeg/toolkits/*.toml`) run in the caller-supplied `_upeg.cwd`
(else upeg's own cwd); project-manifest tools run in **the manifest's
directory** — a caller `_upeg.cwd` outside it is ignored after
canonicalizing both sides, so `..` or a symlink cannot cross. `_upeg.cwd`
must be an absolute path to a real directory.

**stdin is always `/dev/null`** — a child that waits for input finishes
immediately instead of hanging; feed input through `args_template`.

**`args_template` tokens.** One token becomes one argument. `{key}`
substitutes **anywhere** in a token — `--manifest-path={path}`,
`-p{crate}`; `{{`/`}}` are literal braces (a shell `${VAR:-x}` needs
`${{VAR:-x}}`). The key must be a declared `inputs` name — anything else
is rejected at load time (the `{input}` a chain step receives is the one
exception). A missing value falls back to the input's `default`. A bare
`{key}` token for an optional input with no value drops out entirely —
write `"-n{count}"` as one token, not `"-n", "{count}"`. Mixed tokens
(`{dir}/build`) substitute an empty string and **keep their position**.

**`color = "force"`.** upeg captures both streams over pipes, so the child
sees a non-TTY and well-behaved CLIs turn color off — that is the default
and it is deliberate. `force` opts out via environment variables only:
`CLICOLOR_FORCE=1`, `FORCE_COLOR=1`, `NO_COLOR` removed, and
`TERM=xterm-256color` only when upeg has no `TERM` — all overridable by
declared `env`. It only reaches CLIs that read the convention (`cargo`,
`gh`, chalk-based tools); `isatty`-only programs (`git`, `ls`, `grep`)
need `pty` or their own flags (`git -c color.ui=always`). An unknown value
is rejected at load time.

**`pty = true`** connects the child's stdout and stderr to a real
pseudoterminal so `isatty`-deciding tools emit terminal output. The two
streams merge into `stdout` (`stderr` becomes empty, in-progress chunks
included); it implies `color = "force"` unless `color` is written
directly; stdin is still `/dev/null`; `OPOST` is off so `\n` stays `\n`;
`timeout_ms` and the process-group kill are unchanged. Limitations: **Unix
only** — where no pty exists (Windows, wasm) that one tool is skipped at
load time with its reason recorded; no controlling terminal (`/dev/tty`
openers like `ssh`/`sudo` still fail); no window size. When only color is
needed, `color = "force"` is cheaper.

**Failure envelope.** Failures come out in the canonical `ToolResult`
envelope; `External` fills `error.details` with `exit_code`, `stdout`,
`stderr` — decisive for `cargo`/clippy/`flutter analyze`, which write
diagnostics to stdout. Signal death → `exit_code: null` + `signal`;
`timeout_ms` expiry → `timed_out: true`. Both streams are truncated under
a diagnostics budget. On success stdout is the primary output; undeclared
stderr text remains as a secondary `stderr` output. `upeg call <id>` shows
message + streams; `--json` gives the envelope.

**Cancellation.** When the caller installed cancellation (the HTTP stream
consumer disconnecting, TUI `Esc`, Flutter `cancel_dispatch`), the invoker
kills the process group and answers `error.code = "cancelled"` with
`details.cancelled = true`, `exit_code = null`, and whatever was written.
Cancellation is a request, not a guarantee.

**In-progress output.** While running, `External` emits captured output as
an optional channel **added** to the final envelope — a surface that cannot
consume it receives the same single envelope. Chunks split on `\n` (or `\r`
for progress bars); `seq` increases without gaps across both streams within
one call (a `pty` child sends everything on `stdout`). CLI mirrors chunks
to the terminal's stderr; TUI/Desktop show a live tail; HTTP exposes
`POST /v1/tools/{id}/stream` as NDJSON; MCP sends `notifications/message`.
A TUI on an older host without the route falls back to buffered output.

Field-level detail: [TOOL_MANIFEST.md](../TOOL_MANIFEST.md).

## Project boards

A project manifest declares its own boards in a top-level `[[boards]]`
array. A Tool's `boards = ["..."]` refers to them; a board exists only
while the manifest is detected.

```toml
[[boards]]
id = "upeg-dev"        # the name surfaces use as-is (`upeg board upeg-dev list`)
label = "upeg dev"     # GUI tab title; defaults to the id when omitted

[[tools]]
id = "git_status"
boards = ["upeg-dev"]
```

Authoring rules: `id` must be canonical and non-empty, cannot contain `:`
(store-key separator), cannot shadow a built-in board, and cannot repeat
within one manifest. `~/.upeg/toolkits/*.toml` cannot declare boards — the
loader rejects the file's registration. Pins live under
`project:<manifest-path-digest>:<board-id>`, so two projects sharing a
board id do not share pins and moving the project leaves them behind;
inside the project a project board shadows a global board of the same id
and cannot be deleted.

## WASM plugins and MCP imports

`upeg plugin new <name>` scaffolds a WASM guest crate —
`examples/plugins/greet/` is a complete example; built plugins land in
`~/.upeg/wasm/*.wasm` (the `wasm-plugin` cargo feature is on by default;
`upeg doctor` lists enabled features).

`~/.upeg/mcp-imports/<server>.toml` (see `examples/mcp-imports/`) declares
one upstream MCP server; the file stem becomes the `<server>.<tool>`
namespace, and `reexport = true` opts it into re-exposure on upeg's own
`mcp` surface. Imports load only inside long-lived host processes —
reloading means restarting the host.

# Surface exposure control

```rust
surfaces = [desktop, pwa, ext],   // GUI only
surfaces = [cli, mcp, http],      // headless only; omit for all seven
```

A tool whose outputs only make sense in a GUI (e.g. `EmbeddedView`) should
be limited to GUI surfaces. Whether a declared surface can actually run it
is decided by `upeg_core::capability` (see [Architecture](../architecture.md#surfaces)).

# Bespoke renderers — the last resort

Visualizations that automatic rendering cannot express (audio waveforms,
3D viewers) can be registered in the bespoke renderer map at
`pin_renderers/registry.dart` in `flutter_app`. Bespoke renderers erode
surface parity — confirm automatic rendering really is insufficient first.

Expanded-modal bespoke form eligibility is a separate rule: **only tools
whose output updates live as you type** qualify — a distinct run step means
the generic form. Run and copy actions are already provided around the
generic form and are **not** reasons for a bespoke one. Details:
`flutter_app/lib/src/widgets/expanded_modal/bespoke_forms/README.md`.

# Validation checklist

- [ ] `cargo test --workspace` passes
- [ ] `just verify` passes (see [Development gates](development.md#gates))
- [ ] The new tool shows up in `upeg tool list`
- [ ] `upeg call <id> -a key=value` works
- [ ] Automatic rendering confirmed on the desktop pegboard
- [ ] (Controlled Embed only) works headless too
