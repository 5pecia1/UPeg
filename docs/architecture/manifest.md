---
type: Manifest Contract
title: Manifest Contract
description: The structure of Toolkit TOML, per-invoker required fields, and credential reference rules.
tags: [architecture, manifest, toml]
status: stable
sources:
  - id: loader-model
    resource: ../../upeg-loader/src/model.rs
    title: Toolkit TOML type definitions
---

# Shape

One manifest defines one non-callable Toolkit and one or more callable Tools.

```toml
id = "num"
tags = ["pure", "numeric"]
description = "Numeric base conversions"

[[tools]]
id = "hex_to_decimal"      # local id; the full id is num.hex_to_decimal
tags = ["hex", "number"]
invoker = "External"
pin = "Inline"
pegboard_units = "U1"
surfaces = ["cli", "tui", "desktop", "pwa", "ext", "mcp", "http"]
boards = ["dev"]

[[tools.inputs]]
name = "input"
type = "string"
required = true

command = "hex-to-decimal"
```

# Rules

- `[[tools]].id` is a local id inside the Toolkit. Repeating the `{toolkit}.`
  prefix is rejected.
- `pegboard_units` is required and shared by every pegboard surface: `U1`, `U2`,
  `U2T`.
- Inputs/outputs use only the closed type set — see the
  [I/O type system](io-types.md).
- `category` is rejected. Use tags instead.
- Static Rust `#[tool]` supports `Function`. Runtime TOML/WASM/MCP manifests use
  the `External`, `Http`, `Embed`, `Chain`, `Llm`, and `Wasm` runtime adapters.

# Required fields per invoker

| Invoker | Required fields | Secret rules |
|---|---|---|
| `External` | `command`; optional `args_template`/`cwd`/`env`/`timeout_ms`/`color`/`setup` | Credential values reach the process as environment only at spawn time. |
| `Http` | `url`; optional `method`/`headers`/`body` | Header/body templates reference credential *names* only. The built-in lightweight adapter supports `http://` and `mock://echo` for tests; when TLS is needed, use an `External` wrapper. |
| `Embed` | `embed_url`, `controlled_embed.bindings` | Selector mappings are user-verified values. |
| `Chain` | `steps` | Step args may use `{{steps.<id>.output}}` expressions. |
| `Llm` | `prompt`; optional `provider`/`model`/`credential` | API keys resolve by credential name only. `provider = "echo"` is the offline default and `provider = "tool:<id>"` delegates to a configured provider Tool. An unknown provider fails explicitly rather than silently mocking. |
| `Wasm` | `wasm_path` or a loaded WASM Toolkit declaration | The host validates the exported manifest before registering. |

# External execution contract

`External` declares not just a `command` but **how the child process runs**.

```toml
[[tools]]
id = "cargo_check"
pegboard_units = "U2"
invoker = "External"
command = "cargo"
args_template = ["check", "--manifest-path={manifest_path}", "--quiet"]
cwd = "."             # relative to the manifest's directory
timeout_ms = 600000   # unlimited when omitted

[[tools.env]]
name = "RUST_LOG"
value = "warn"

[[tools.inputs]]
name = "manifest_path"
type = "file_path"
default = "Cargo.toml"
```

| Field | Meaning |
|---|---|
| `cwd` | The directory the child runs in. Relative paths resolve against **the directory containing this manifest file**. |
| `env` | Plaintext (non-secret) environment variables. `credentials` apply later, so a colliding name is won by the credential. |
| `timeout_ms` | Wall-clock budget. On expiry the child's **entire process group** is terminated and the call fails with `details.timed_out = true`. **No default** — a 20-minute `just verify` must stay legal. |
| `color` | `"inherit"` (default) or `"force"`. Tells the child whether color is supported — see below. |
| `setup` | Optional display-only guide URL, instructions, and OS-specific install commands. It is never executed; [External readiness](external-readiness.md) selects it for the host. |

The working directory is decided differently depending on where the manifest
came from.

A declared `cwd` always wins. When there is none:

| Source | Rule |
|---|---|
| Toolkit directory (`~/.upeg/toolkits/*.toml`) | The caller-supplied `_upeg.cwd`, else the upeg process's own cwd. There is no owning project, so the caller may point anywhere. |
| Project manifest (`upeg.toml`) | **That manifest's directory** is both the default and the boundary. A caller `_upeg.cwd` inside the manifest directory (itself included) is used; one outside it is **ignored** and the manifest directory is used. |

`_upeg.cwd` must be an **absolute path** and a real directory — anything else is
rejected with `invalid_args`, because upeg cannot reconstruct a caller's
"current location" from a relative path.

A value pointing outside the project is ignored, not rejected. Surfaces send the
user's shell directory along as ambient context, and treating it as a rejection
would turn "happened to be in `/tmp`" into a hard failure. Containment is
decided after canonicalizing both sides, so `..` or a symlink cannot cross the
boundary.

stdin is always `/dev/null`. An inherited stdin would leave `cat`, `git`
credential prompts, or `bash -l` stuck forever, and dispatch arrives from
daemons, MCP, and GUI pins — not just shells — so there is no terminal to hand
over anyway.

## `color` — letting the child use color

upeg captures both streams over pipes, so the child sees a **non-TTY**.
Well-behaved CLIs turn color off there, and tools like `gh` shorten their
human formatting itself. That is the default and it is deliberate — captured
output stays deterministic and free of escape sequences.

`color = "force"` is a per-tool opt-out.

```toml
[[tools]]
id = "gh_pr_list"
invoker = "External"
command = "gh"
args_template = ["pr", "list"]
color = "force"
```

It works purely through **environment variables**. No pty is opened, so it
behaves identically on Unix and Windows, and not a single line of the capture,
timeout, or process-group-kill machinery changes.

| Variable | Value | Condition |
|---|---|---|
| `CLICOLOR_FORCE` | `1` | Always |
| `FORCE_COLOR` | `1` | Always |
| `NO_COLOR` | **removed** | Always. The [no-color.org](https://no-color.org) convention treats any value (including the empty string) as "turn color off" and outranks the two above — left inherited, it would make `force` meaningless |
| `TERM` | `xterm-256color` | **Only when upeg's own environment has no `TERM`.** The real terminal's `TERM` is more accurate than any guess |

The four apply as *defaults* — declared `env` entries apply later, so
`env = [{ name = "FORCE_COLOR", value = "0" }]` beats the policy and
`env = [{ name = "NO_COLOR", value = "1" }]` reinstates the removal.

`force` is a **convention, not a terminal.** It only reaches CLIs that read
`CLICOLOR_FORCE`/`FORCE_COLOR` (`cargo`, `gh`, every chalk-based tool).
Programs that decide on `isatty(3)` alone — `git`, `ls`, `grep` — never read
those variables and need their own flags: `git -c color.ui=always`,
`ls --color=always`. An example using both is `dev.git_log` in
`examples/tools/dev-external-demo.toml`.

An unknown value (`color = "always"`) is not silently ignored at dispatch — it
is rejected **at load time** with the allowed values listed.

## `pty` — giving the child a real terminal

`color = "force"` only reaches programs that read the convention. The ones that
decide on `isatty(3)` alone need **the terminal itself**. That is `pty = true`.

```toml
[[tools]]
id = "git_log"
invoker = "External"
command = "git"
args_template = ["log", "--oneline", "-n", "10"]
pty = true
```

A pseudoterminal is opened and the child's **stdout and stderr** are connected
to it. `isatty(1)` and `isatty(2)` are true for the child, and tools that
withhold terminal output without a flag — `git`, `ls`, `grep` — emit it on
their own judgment.

Contract:

| Item | Detail |
|---|---|
| **The two streams merge** | A terminal has one buffer. Output interleaves in the order the child wrote it and lands in `stdout`; `stderr` becomes the **empty string**. In-progress output too ([HTTP streaming](http-api.md), MCP notifications) all goes out on `stdout` |
| **It includes `color = "force"`** | A manifest that asked for a real terminal wants color from convention-reading programs too. `CLICOLOR_FORCE`/`FORCE_COLOR` are raised alongside. **Implication, not overwrite** — a `color` written directly wins (`color = "inherit"` grants the terminal but leaves the variables alone), and declared `env` still applies on top |
| **stdin is still `/dev/null`** | There is no person on the other side of the terminal. Granting tty stdin would let a single credential prompt stall a dispatch forever. `isatty(0)` stays false |
| **Newlines pass through** | The terminal's output post-processing (`OPOST`) is off, so `\n` does not become `\r\n`. The captured bytes are the bytes the program wrote |
| **Timeout and containment are unchanged** | `timeout_ms` still kills the process group, and descendants that escaped the group are still reaped — on Linux the escapees are found through the **`/dev/pts/N` device** instead of a pipe inode |

Limitations:

- **Unix only.** On a host without a pty (Windows, wasm), **that one tool** is
  skipped at load time — with its reason recorded (`pty = true` needs a host
  that can open a pseudoterminal…) — rather than silently sinking to pipes. The
  same manifest must not mean different things on different machines, and that
  difference (`isatty` returning false) is exactly why the field exists.
  Skipping applies to that tool alone: the rest of the file's tools have
  nothing to do with pty and load normally. Windows needs ConPTY, which is not
  implemented.
- **Not a controlling terminal.** The child sits in its own process group and
  does not acquire this pts as its controlling terminal. Programs that open
  `/dev/tty` directly (`ssh` password prompts, `sudo`) still fail.
- **No window size.** `TIOCGWINSZ` returns the kernel default (0×0 or 24×80).
  upeg has no real window to relay.
- **Do not use it for tools whose stderr must stay separate.** A merged stream
  cannot be un-merged. When only color is needed, `color = "force"` is cheaper;
  when the tool has its own flag, the flag is cheaper still.

## Cancellation

A running child **can be cancelled.** When the caller dispatches with
cancellation installed (`upeg_runtime::with_cancellation`), the External invoker
reads it on every tick of its wait loop and, once it is set, kills the process
group and answers with the cancellation envelope.

| Field | Value |
|---|---|
| `error.code` | `cancelled` — the tool did not fail, so this is distinct from `tool_error` |
| `error.details.cancelled` | `true` |
| `error.details.exit_code` | `null` — the child never had a chance to report one |
| `error.details.stdout` / `stderr` | Whatever was written before it died |

Three surfaces install cancellation today.

| Surface | The act that cancels |
|---|---|
| [HTTP streaming route](http-api.md) | The consumer disconnects; the response body drops, and that drop is the cancellation |
| TUI | `Esc` while running |
| FRB (Flutter) | `cancel_dispatch(run_id)` on the Dart side |

The canonical source for per-surface in-progress display and cancel gestures is
the "live output" cancellation table in the
[UI/UX surface contract](../ui-ux-surface-contract.md). **Cancellation is a
request, not a guarantee**: a tool that never reads the token runs to the end,
and the call still settles as one final envelope. Chain runs its steps on the
dispatch thread, so it inherits the installed cancellation as-is.

## `args_template` tokens

One token becomes one argument. Each token is a small template.

- `{key}` is substituted **anywhere** in the token — `--manifest-path={path}`,
  `-p{crate}`.
- `{{` and `}}` are literal braces.
- The `key` in `{key}` must be **a declared `inputs` field name**. Anything else
  is rejected at load time — a typo must never render as a silently empty
  argument. The single exception is `input`: the `Chain` invoker hands every
  step `{"input": <upstream output>}`, so a tool used as a chain node can read
  `{input}` without declaring it.
- A one-line shell command's `${VAR:-default}` uses braces too. To pass it to
  the shell untouched, escape it as `${{VAR:-default}}` — unescaped it parses
  as a placeholder and the load is rejected.
- A missing value falls back to the input's `default`.
- A token disappears in exactly **one** case: the token is a bare `{key}` with
  no literal around it and the input is `required = false`. Only then is it
  dropped from the argument list entirely.
- Anything else substitutes the empty string and the token **keeps its place**.
  `["rm", "-rf", "{dir}/build"]` never collapses into `["rm", "-rf"]` and shifts
  the arguments that follow.

## Failure envelope

Failures come out in the canonical `ToolResult` failure envelope. `External`
fills in `error.details`.

```json
{
  "ok": false,
  "error": {
    "code": "tool_error",
    "message": "`cargo` exited with code 1",
    "details": { "exit_code": 1, "stdout": "…", "stderr": "…" }
  }
}
```

- On a signal death, `exit_code` is `null` and `signal` is attached.
- On `timeout_ms` expiry, `exit_code` is `null` and `timed_out: true` is
  attached.
- `cargo fmt --check`, clippy, `cargo test`, `flutter analyze`, and
  `gh pr checks` write their diagnostics to **stdout**. That is why both streams
  are preserved.
- Both streams are truncated under a diagnostics budget. However large the
  envelope grows, it stays within what CLI `--json`, an HTTP body, MCP
  `structuredContent`, and the FRB bridge can carry.

On success, stdout is the primary output. If the tool declared no `outputs` and
the command also wrote to stderr, that text remains as the secondary output
`stderr` — the progress log of a successful run is not silently dropped.

## In-progress output (streaming)

So the screen is not empty while a ten-minute command runs, `External` also
emits the output it reads **while running**. This is an optional channel
**added** to the final envelope — a surface that cannot consume it receives the
same single final envelope as before, and the capture limit, timeout, and
process-group kill do not change at all.

- With no consumer, the invoker skips the delivery work entirely (zero cost).
- The chunk boundary is the **line**: split on `\n`, and when there is no `\n`
  at all, on the `\r` a progress bar writes. An unterminated 64 KiB is emitted
  as-is.
- An unfinished tail left at the end is always flushed when the stream ends —
  including what a timed-out child had written by then.
- `seq` increases without gaps starting at 0 **across both stdout and stderr**,
  so the consumer can restore the total order. The sequence belongs to **the
  span where a consumer installed a sink** — that is, to one call.
- `Chain` steps run on the same thread, so each step's in-progress output flows
  out as-is, and the sequence continues as one across multiple steps — it does
  not restart at 0 per step.
- A tool with `pty = true` sends every chunk on `stdout`. A terminal has one
  buffer; there was never a stderr to split.

Per-surface consumption:

| Surface | In-progress output |
|---|---|
| CLI (`upeg call`, dynamic route, `board <b> call`, `trigger fire`) | Mirrors the child's stdout·stderr verbatim to **the terminal's stderr**. stdout stays reserved for the final result. `--json`/`--field` emit nothing |
| TUI | An 8-line live tail in the result pane. Dispatch runs on a worker thread, so the UI never freezes |
| Desktop (Flutter) | A live tail — last 8 lines in the expanded modal, last 3 in an inline pin — delivered by the FRB `dispatch_stream` API |
| HTTP | [`POST /v1/tools/{id}/stream`](http-api.md) — `application/x-ndjson` |
| MCP | `notifications/message` log frames during `tools/call` (see the [MCP contract](mcp.md)) |
| TUI attached to a host | Same live tail — chunks arrive over the host's `POST /v1/tools/{id}/stream`, and dropping the response body is the cancel. A host that predates the route answers `404`; the TUI then falls back to the buffered route and says so in the tail |
| `upeg call` attached to a host | None — the call uses the buffered route and receives the host's final envelope as-is |
| Chrome extension | None — the final envelope only |

# Credential references

```toml
credentials = [
  { name = "openai", type = "api_key", store = "keychain", keychain_service = "upeg", keychain_account = "openai" },
  { name = "etherscan", type = "api_key", store = "env", env = "ETHERSCAN_API_KEY" },
]
```

An entry only specifies **where the secret can be resolved** at run time.
`credentials[].value`, `credentials[].secret_value`, and inline literal secrets
are all invalid. Secret bytes appear nowhere: not in the Toolkit manifest, not
in the project manifest, not in execution logs, and not in HTTP/MCP listings.

# Field reference and validation

The full field table is not written by hand — the
[generated manifest guide](../TOOL_MANIFEST.md) is canonical, derived from the
Rust types. The JSON Schema for editors and CI is
`fixtures/toolkit.schema.json`, regenerated by the same command.

```bash
upeg tool validate ~/.upeg/toolkits/demo.toml   # semantic validation of one file
upeg toolkit validate                           # the whole directory
just toolkit-schema                             # regenerate schema + guide
just toolkit-schema-check                       # check generated artifacts for drift
```

The JSON Schema proves only the TOML→JSON shape. `upeg tool validate` validates
declarations such as invoker fields, typed inputs, chain structure, and HTTP(S)
guide URLs; it does not prove runtime credentials, network reachability, or
executable availability. Executable availability is a non-executing, host- and
context-dependent readiness inspection, not a manifest validity condition.
