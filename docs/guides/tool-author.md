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

# Quick start

Declare `inputs` and `outputs` and the Tool works on every surface — it is
rendered on the pegboard automatically, with no UI code to write.

```rust
use upeg_core::tool;

#[tool(
    id = "demo.upper",
    toolkit = "demo",
    description = "Convert an ASCII string to uppercase",
    inputs = [
        required input: String = "Source text",
    ],
    outputs = [
        result: String = "Uppercase result",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn upper(input: &str) -> String {
    input.to_uppercase()
}
```

# Source — how a tool starts

The `source` key declares **how the tool starts** in the GUI. Non-GUI
surfaces ignore it and call the function directly.

| Variant | Meaning | Example |
|---|---|---|
| `UserInput` (default) | user fills the form, then runs | can be omitted |
| `Manual` | a button click triggers it | a UUID generator that works in one shot |
| `Timer("30s")` | periodic automatic run | live clock, network status |
| `Shortcut("⌘⇧N")` | keyboard shortcut | action tools like note creation |
| `Static` | never starts; static output | View Embed |

Duration suffixes: `ms`, `s`, `m`, `h`.

# Inputs and outputs

The closed type set and inline constraints are defined by the
[I/O type system](../architecture/io-types.md).

```rust
inputs = [
    required hex:     String                                     = "Hex value",
    required base:    Options(["hex", "dec", "bin"])             = "Output base",
    required port:    Number(min=1, max=65535, default=8080)     = "Port number",
    optional pattern: String(regex="^[a-z]+$", placeholder="abc") = "Pattern",
    optional flags:   MultiOptions(["i","m","s","x"])            = "Regex flags",
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
    id = "embed.mdn",
    toolkit = "embed",
    source = Static,
    outputs = [
        view: EmbeddedView("https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference"),
    ],
    pin = Embed,
    pegboard_units = U2,
    invoker = Static,
    surfaces = [desktop, pwa, ext],
)]
pub fn mdn() {}
```

## Controlled Embed (driven)

Uses an external website as the tool engine: upeg manipulates its DOM
through CSS selectors while the user sees an ordinary form and result.

```rust
#[tool(
    id = "embed.json_to_ts",
    toolkit = "embed",
    description = "transform.tools backend JSON→TS converter",
    inputs  = [ required json: String = "JSON input" ],
    outputs = [ ts: String = "TypeScript output" ],
    pin = ControlledEmbed,
    pegboard_units = U2,
    invoker = Embed,
    surfaces = [desktop, pwa, ext],
)]
pub fn json_to_ts() {}
```

In the GUI, `webview_flutter` (desktop) or the browser itself (PWA/ext)
runs the page. Non-GUI surfaces use a system-installed
Chrome/Chromium/Edge in headless mode.

`Invoker::Embed` pairs only with `PinKind::ControlledEmbed`, and
`Invoker::Static` only with `PinKind::Embed` — see the confusable pair in
the [Lexicon](../LEXICON.md).

## Selector robustness

- **id first**: `#input`, `#output`-style id selectors are the most stable.
- **data attributes next**: `[data-testid="input"]` survives SPAs well.
- **classes/structure last**: `.col-md-6 > textarea` breaks on the first
  redesign.
- Test on both the desktop webview and headless Chrome.

# Declarative TOML tools

The TOML manifest loader accepts the same `inputs` and `outputs`. The
macro's `source` key and inline-constraint syntax are still separate; in
TOML, runtime behavior is declared through `invoker` / `embed_url` /
`controlled_embed.bindings`. The full field reference is the
[manifest contract](../architecture/manifest.md) and the
[generated guide](../TOOL_MANIFEST.md).

## Wrapping a local command as a tool

`invoker = "External"` declares **where, with what environment, and for how
long** the child process runs. A complete copyable example lives in
`examples/tools/dev-external-demo.toml`.

```toml
[[tools]]
id = "git_log"
pegboard_units = "U1"
invoker = "External"
command = "git"
args_template = ["log", "--oneline", "-n", "{count}"]
cwd = "."             # relative to the manifest directory. Without it,
                      # Project Manifest tools run in the manifest's directory
timeout_ms = 10000    # omit for no limit
env = [{ name = "GIT_PAGER", value = "cat" }]

[[tools.inputs]]
name = "count"
type = "integer"
default = 10          # substituted when the caller omits it
```

Common traps when authoring:

- **Give optional inputs a `default`.** If an *optional* input has neither a
  value nor a default and a token is the bare `{count}` placeholder, that
  token drops out of the arg list entirely — `-n {count}` collapses to a
  broken lone `-n`. Write it as one token (`["-n{count}"]`) so the whole
  thing disappears together. Tokens with literal text mixed in
  (`{dir}/build`) and required-input tokens substitute an empty string and
  **keep their position**.
- **`{key}` can go anywhere inside a token** — `--manifest-path={path}`,
  `-p{crate}`. Literal braces escape as `{{` and `}}`.
- **`{key}` must be a declared `inputs` name** — anything else is rejected
  at load time (the `{input}` a chain step receives is the one exception).
  Shell one-liners that use `${VAR:-default}` need `${{VAR:-default}}` to
  reach the shell intact.
- **stdin is always `/dev/null`.** Commands that wait for input (`cat`, a
  `git` that opens an auth prompt) finish immediately instead of hanging.
  Feed input through `args_template`.
- **You don't have to set `timeout_ms` on long jobs.** There is no default,
  so a build or test run finishes on its own. Set it and expiry kills the
  whole process group.
- **Secrets go in `credentials`, not `env`.** `env` is plaintext-only, and
  credentials apply afterwards — a credential wins a name collision.

When a command exits non-zero, the failure envelope's `error.details`
carries `exit_code`, `stdout`, and `stderr` verbatim — decisive for
`cargo`/clippy/`flutter analyze`, which write diagnostics to stdout. The
envelope shape is in the [manifest contract](../architecture/manifest.md).

```bash
upeg call dev.cargo_check              # human: message + stderr + stdout
upeg call dev.cargo_check --json       # machine: canonical envelope with details
```

# Surface exposure control

```rust
surfaces = [desktop, pwa, ext],   // GUI only
surfaces = [cli, mcp, http],      // headless only
// omit for all: cli, tui, desktop, pwa, ext, mcp, http
```

A tool whose outputs only make sense in a GUI (e.g. `EmbeddedView`) should
be explicitly limited to GUI surfaces. Whether a declared surface can
actually run the tool is audited by the capability contract — see the
[surface contract](../ui-ux-surface-contract.md).

# Bespoke renderers — the last resort

Visualizations that automatic rendering cannot express (audio waveforms, 3D
viewers) can be registered in the bespoke renderer map at
`flutter_app/lib/src/widgets/pin_renderers/registry.dart`. Bespoke renderers
erode surface parity, so confirm automatic rendering really is insufficient
first.

Expanded-modal bespoke form eligibility is a separate rule: **only tools
whose output updates live as you type** qualify. If there is a distinct run
step, it is not a live preview. `num.hex_to_decimal` is currently the only
eligible tool; everything else uses the generic form.

Things that are **not** reasons for a bespoke form — the host already
provides them around the generic form:

- one-click re-run: `F1` run and `F2` copy are bound for every tool.
- a copy button on results: every result block already has one.
- a run button for input-less tools: the modal's primary button and the
  inline pin's Run button supply the affordance instead of a "no inputs"
  placeholder.

The detailed rules live in the [surface contract](../ui-ux-surface-contract.md).

# Validation checklist

- [ ] `cargo test --workspace` passes
- [ ] `just verify` passes (fmt, file-size, clippy, lexicon, baseline)
- [ ] The new tool shows up in `upeg tool list`
- [ ] `upeg call <id> -a key=value` works
- [ ] Automatic rendering confirmed on the desktop pegboard
- [ ] (Controlled Embed only) works headless too
