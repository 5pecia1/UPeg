---
type: Protocol Contract
title: Call Envelope and Reserved Context
description: The call envelope every non-UI protocol shares, the `_upeg` reserved context, and the CLI positional-binding and shell-completion rules.
tags: [architecture, protocol, cli, ipc]
status: stable
sources:
  - id: cli-args
    resource: ../../upeg-cli/src/app/args.rs
    title: Positional-binding implementation
  - id: execution-context
    resource: ../../upeg-runtime/src/execution.rs
    title: Reserved-block erasure and caller-preserved keys
  - id: fired-trigger
    resource: ../../upeg-runtime/src/triggers.rs
    title: FiredTrigger label construction
  - id: principal
    resource: ../../upeg-core/src/principal.rs
    title: Principal / PrincipalRole vocabulary
  - id: cli-completion
    resource: ../../upeg-cli/src/surfaces/cli/completion.rs
    title: Completion-script generation and dynamic Tool candidate injection
---

# The shared envelope

Every non-UI protocol reduces to:

```json
{
  "tool": "toolkit.tool",
  "args": { "input": "...", "_upeg": { "board": "dev" } }
}
```

| Surface | Shape |
|---|---|
| CLI plain | `upeg call <id> <json>` or `upeg call <id> -a key=value` |
| CLI dynamic | `upeg <toolkit> <tool> [pos1] [pos2]...` |
| MCP | `tools/call` uses the same `name` + `arguments` shape |
| HTTP | `POST /v1/tools/{id}`, `POST /v1/boards/{board}/tools/{id}` |

# The reserved context `_upeg`

An argument key reserved for context supplied by the surface. It cannot collide
with user input, so the dispatcher API stays stable.

The keys come in two kinds. **Keys the surface stamps** are always erased when
the caller sends them (anti-spoofing). **Keys the caller carries** survive that
erasure as a narrow allow-list — there is no surface-side original to stamp over
them (`CALLER_PRESERVED_CONTEXT_KEYS` in `upeg-runtime/src/execution.rs`).

| Key | Origin | Meaning |
|---|---|---|
| `board` | surface | The active Board key |
| `boardEnv` | surface | The Board-scoped environment-variable map |
| `projectManifest` | surface | The detected `upeg.toml` path |
| `surface` | surface | The calling surface's label (`cli`, `mcp`, `http`, …). The basis for logs, parity diagnostics, and Chain approval authorization. A local client attached to a host declares its surface through `X-Upeg-Origin-Surface`, which the host verifies and stamps ([HTTP API](http-api.md)) — the fact that transport is HTTP does not change the caller's identity |
| `principal` | surface | The caller principal `{ role, surface }`. If `surface` is "which door," this is "what authority." See [Principal](#principal) below |
| `trigger` | surface | The label of the Trigger that started this call — `<source>` or `<source>:<condition>` |
| `cwd` | caller | The absolute working directory the caller supplied. Used to place the child process of an `External` invoker |
| `approvedSteps` | caller | The Chain-step approval allow-list. A caller can fill it freely, so it expresses only **intent** — whether to honor that intent is judged separately by `principal.role` and `surface` ([Chain Tool](chain.md)). GUI surfaces (`desktop`/`pwa`) are the exception: they block this key from **arriving** in args at all (see [The GUI approval lever](#the-gui-approval-lever) below) |

An `External` invoker additionally receives `UPEG_BOARD`,
`UPEG_PROJECT_MANIFEST`, and each `boardEnv` entry as process environment
variables.

## The GUI approval lever

`approvedSteps` is a caller-preserved key for the sake of surfaces where **the
caller writes the envelope itself** — CLI, MCP, HTTP. GUI surfaces (`desktop`,
`pwa`) are the opposite: a person presses a button, and the approval crosses the
FRB boundary as one typed parameter.

So GUI dispatch erases **both** approval levers that args can carry and then
uses the typed flag (`shape_approval_arg` in `upeg-frb/src/api/tools.rs`): the
`approve` key and `_upeg.approvedSteps`. While only `approve` was being erased,
the other one remained a data path — a `upeg://open?...&input=` deep link
carrying `{"_upeg":{"approvedSteps":["gate"]}}` arrived at `desktop`, a default
approval surface, and opened a barrier no person had answered. The deep-link
side applies the same rule once more: input carried by the URL reaches dispatch
only after being filtered down to **the input fields the Tool declared**
(`flutter_app/lib/src/widgets/launch_intent_applier.dart`).

## Principal

`_upeg.principal = { "role": "operator|agent|local", "surface": "cli" }`.

`surface` alone cannot tell two callers who walked in the same door apart. One
HTTP listener has both a person and an agent attached to it, and what separates
them is which bearer token they carried. `principal` puts that distinction into
the envelope.

| role | Proof |
|---|---|
| `operator` | An in-process `cli`/`tui`/`desktop` (the OS user account *is* the caller), or an HTTP request carrying the operator bearer token |
| `local` | An in-process program the OS user launched — the MCP **stdio** lane |
| `agent` | An HTTP request carrying an agent token, and any caller this host could not identify (the floor value for `http`/`pwa`/`ext`) |

The `mcp` surface has two lanes, and the principal differs per lane. stdio
(`upeg mcp`) is a process the OS user launched, so it is `local`; the host's
`POST /mcp` is a request that crossed a listener, so it is **whatever the token
proves** (`operator` or `agent`). The surface is `mcp` on both.

- **The caller cannot stamp it.** Sitting on the same branch as `surface`, a
  caller-sent `principal` is discarded by the reserved-block erasure and the
  runtime writes its own (`upeg_runtime::apply_execution_context`).
- **Every call gets a principal.** The per-surface defaults are total, so no
  dispatch path can drop the principal, and only an authenticating surface
  (HTTP) narrows that default with a token — on **both** `/v1/*` and `/mcp`.

- **It is used in two places.** The principal gate of Chain approval
  ([Chain Tool](chain.md)) and the `principal` column of the execution log
  (`upeg log` shows `principal=<role>` and `--json` carries it too). The log
  holds **only the role label** — token values are stored nowhere.

## Sub-calls inherit `_upeg`

A Chain step's args are a **new object** built by the manifest template, and
`{{input.*}}` expressions pour caller text into it. So a `_upeg` written into
step args is not the surface's stamp — it may be a caller-written value. The
engine discards it and **hands down the call's `_upeg` block as-is**
(`upeg_runtime::inherit_call_context` — same definition as the reserved-block
erasure above).

- A step sees the call's `surface`/`principal`/`board`/`boardEnv`/`cwd`
  verbatim. A nested chain's approval barrier is judged by the outer call's
  real surface and principal too ([Chain Tool](chain.md)).
- There is no way to fabricate `_upeg.surface` or `_upeg.principal` in step
  args to pass authorization.
- If the call itself has no `_upeg`, no such key appears on step args either —
  the dispatcher never sees a key nobody wrote.

## The `_upeg.trigger` label

It stamps **the Trigger that fired**, not the Tool id. A Tool already knows its
own id, and a Tool that declared several Triggers cannot tell which one fired
from the id alone.

- Shape: `<source>`, or `<source>:<condition>` when there is a condition
  (`clipboard`, `webhook`, `file:/tmp/drop.txt`, `schedule:every:30s`,
  `hotkey:ctrl+shift+u`)
- A `condition` may itself contain `:`, so splitting it back apart happens only
  at the **first** delimiter.
- Why a string and not a JSON object: the execution log's `trigger` column and
  the `--trigger` filter read a string
  (`upeg-cli/src/adapters/execution_log.rs`). Switch it to an object and the
  trigger quietly disappears from every log record.
- Stamp points: the CLI trigger poll loop, the `hotkey` adapter, the HTTP
  `POST /v1/trigger/{tool_id}` route, and `upeg trigger fire`.
- `upeg trigger fire <tool_id>` names a **Tool**, not a binding, so there is no
  way to know which of several bindings it means to imitate. The rule is the
  simplest one that stays honest: **the first declared binding wins**. A Tool
  with no bindings stamps nothing — `_upeg.trigger` answers "what started this
  call," and that Tool's answer is "a person, not a Trigger." Synthesizing a
  source here would put a value no Tool could ever declare into the execution
  log's `trigger` column.

# CLI dynamic route — positional binding

`upeg {toolkit} {tool} <pos1> <pos2> ...` is the only dynamic route. There is no
hardcoded per-Toolkit subcommand enum. Positional arguments bind 1:1 to the
fields of `ToolMeta.input_spec` **in declaration order** (`InputSpec` preserves
author declaration order).

1. List `input_spec.fields` in declaration order.
2. Bind positional arguments 1:1 in that order.
3. Coerce each value to the field's `InputKind`.
   - `string` / `markdown` / `file_path` / `url` / `datetime`: as-is.
   - `number` / `integer`: parse. Failure is a clear Tool error.
   - `boolean`: `true` / `false` (case-insensitive).
   - `json`: parse as JSON.
   - `options`: validate against the choices.
   - `multi_options`: split on commas, then validate each choice.
4. If the input spec is empty, fall back to `{ "input": "<joined>" }` — that
   keeps `upeg num hex-to-decimal 0xff` working for a single-input tool. This
   fallback applies **to positional arguments only**: a Tool that declared no
   fields has no place for stdin to land, so the automatic-stdin rule in item 6
   does not apply. (It was why no-input Tools like `upeg time iso-now` and
   `upeg id uuid-v7` used to hang forever on an inherited pipe that never sees
   EOF.)
5. Positional arguments beyond the input-field count are a clear Tool error.
   `upeg text uppercase hello world` does not silently drop `world`.
6. When stdin is a pipe (non-TTY) and there are no positional arguments, stdin
   is read as the value of the first required input field. It does not apply to
   a Tool with no required input field (including no-input Tools).

`-` is the positional placeholder meaning "read this slot from stdin."

Positional binding applies only to this route. `upeg call <id> <json>` and
`upeg call <id> -a key=value` remain schema-agnostic explicit forms.

# Shell completion — a generation-time snapshot

`upeg completions <shell>` writes a script for bash, zsh, fish, elvish, or
powershell to stdout (`completion` is an alias). Installation redirects it into
the shell's completion directory.

```bash
# bash — the per-user path of bash-completion 2.x
mkdir -p ~/.local/share/bash-completion/completions
upeg completions bash > ~/.local/share/bash-completion/completions/upeg
```

```zsh
# zsh — place it as `_upeg` in a directory on fpath, and widen fpath before compinit
mkdir -p ~/.zfunc
upeg completions zsh > ~/.zfunc/_upeg
# ~/.zshrc: put fpath=(~/.zfunc $fpath) before the compinit call
```

```fish
mkdir -p ~/.config/fish/completions
upeg completions fish > ~/.config/fish/completions/upeg.fish
```

The dynamic route has no subcommands clap knows about, so the generator **reads
the toolbox at generation time** and bakes candidates into the script. It bakes
two things.

1. For `upeg call <TAB>` it writes the **canonical ids** of the Tools exposed
   on the CLI surface.
2. For each Toolkit it synthesizes one fake subcommand and puts the kebab-case
   Tool names under it, so `upeg <TAB>` offers Toolkits and `upeg num <TAB>`
   offers Tools. When a name collides with an existing built-in subcommand, it
   is not synthesized — a built-in subcommand is never shadowed.

This synthesis is **generation-only**. Real parsing still goes through
`Cli::parse`, so `external_subcommand` semantics do not change.

## What being a snapshot means

A generated script is an artifact with that moment's Tool list baked in as
strings. So after installing a new Toolkit TOML, adding a WASM plugin, or moving
to a different project directory, **new Tools do not complete until the script
is regenerated.** This is not a defect but the price this approach pays, and it
is why regeneration instructions belong with the install instructions.

## Where there are no candidates — do not mix the two kinds

Two different things sit in the slots that have no candidates: **slots a
generation-time enumeration could fill** and **slots the current fixed
`PossibleValuesParser` injection alone cannot express**. That is a distinction
in the current generator implementation, not a limit of static scripts in
general.

### Enumerable at generation time but not yet generated

The candidate set does not depend on other arguments, so the same
generation-time enumeration that fills Toolkit candidates solves these.

| Slot | Where the candidates come from |
|---|---|
| `--surface` | `Surface` is a closed enum of 7 variants (`upeg-core/src/types.rs`). Independent of Tools and user state |
| `--tag` | `upeg_runtime::tags_for_surface()` (`toolbox.rs:462`). Same family as the `toolkits_for_surface()` the generator already uses |
| `--board` (manifest-declared) | `upeg_runtime::boards_for_surface()` (`toolbox.rs:479`). Boards declared by the manifest enumerate without touching disk |
| `--board` (user-pin based) | Could read pegboard state at generation time and bake a snapshot. Later board changes do not reflect until regeneration |
| `tool show <id>` · `trigger fire <id>` | The canonical Tool ids. The same list already baked for `call`'s positional |

### Not expressible by fixed candidate injection today

The candidate set **depends on an earlier argument**, or a scoped path has no
clap metadata. The current generator only injects a fixed
`PossibleValuesParser` candidate per argument and does not emit such
context-dependent branching. Generating per-shell branching from the
generation-time schema or synthesizing the metadata is possible, so this is not
a claim of impossibility for static scripts.

| Slot | What it depends on |
|---|---|
| The name in `-a <name>=` | The field names are unknowable until the preceding positional (`tool_id`) is known |
| The `options` choices in `-a name=<value>` | Same as above, and the value set differs per field |
| `--field <ID>` | The output-field ids likewise depend on the target Tool |
| After `upeg board <board> ...` | `BoardAction::Scoped` is a second `external_subcommand`, so it is invisible in clap metadata |

Three places accept `-a`: `call`, `board <b> call`, and `trigger fire`. The
dynamic route does not accept `-a`, so it is unrelated to this entry.

## To go the callback route

`clap_complete` already ships the machinery for a shell to ask the binary for
candidates on every keystroke (`CompleteEnv`, `ArgValueCandidates`). That
callback is one way to pick candidates based on earlier arguments or reflect
the latest user state. The first table can be filled with just the
generation-time snapshot.

The cost is that each completion becomes a process launch, so startup cost
becomes response latency. `main` unconditionally pays
`load_local_runtime_sources` before entering `run()`, and that includes project
manifest discovery and loading `~/.upeg/wasm/*.wasm`. The `-a` field names, the
`options` choices, and the `--field` output ids are enumerable from memory once
that load finishes at no added cost, but a user-pin-based board reads pegboard
state, which needs SQLite opened once more.

# Output representations

`call` and the dynamic route share the same flags.

| Flag | Output |
|---|---|
| (none) | The primary output value |
| `--json` | The canonical success/error JSON envelope |
| `--field <ID>` | One output field's value |
| `--pretty` | All labeled output rows |
| `--out <PATH>` | Where a `File` output is written. Without `--force`, an existing file is not overwritten |
| `--local` | Dispatch in-process even when a host is running (skip discovery auto-attach) |

## In-progress output does not mix into stdout

The envelope is untouched — in-progress output adds no field to it. What splits
by output mode is **where it goes**.

| Mode | While running |
|---|---|
| (none) · `--pretty` | Mirrors the child's stdout·stderr verbatim to **the terminal's stderr**, no prefix |
| `--json` · `--field` | Emits nothing |

The criterion is not "verbosity" but **machine versus human**. `--json` and
`--field` are modes that exist to be parsed by something, and that something
has nowhere to put progress text.

So in the human modes, a successful command's stdout appears **twice** — once
on stderr while it runs, once on stdout as the result after it ends. Keeping
stdout the single representation of the result was judged worth more than that
duplication, and `2>/dev/null` makes it go away.

It is not switched on TTY-ness. Piping progress output into a file or a log
collector is a legitimate need, and an isatty check makes this behavior
unpredictable under `tee`.

A call attached to a host is the exception: the host returns one final envelope
over HTTP, so there is nothing to mirror. When live output is needed there, use
the host's [HTTP streaming route](http-api.md) directly.
