---
type: Contract
title: Project Manifest
description: "`upeg.toml` auto-detection, the `$HOME` boundary, the `UPEG_PROJECT_MANIFEST_PATH` override, toolbox merge precedence, project-board declaration and persistence, and Board execution-context injection."
tags: [architecture, manifest, project, security]
status: stable
---

# Contract

`upeg.toml` is a Toolkit manifest detected by walking upward from the current
directory (cwd), never crossing `$HOME`. The nearest manifest is loaded into
the toolbox before CLI dispatch, and board-aware CLI/HTTP calls have the Board
execution context injected.

# Detection scope — the `$HOME` boundary (security)

A project manifest can run arbitrary commands through `invoker = "External"`
Tools, and it loads automatically with no consent step. So the upward scan
**never leaves `$HOME`** (see
[security absolutes](../product/security-absolutes.md)):

1. cwd itself is always checked, inside or outside `$HOME`.
2. When cwd is inside `$HOME`, the walk climbs only through ancestors that
   stay inside `$HOME`, `$HOME` itself included.
3. When cwd is outside `$HOME` (or `$HOME` does not exist at all), no
   ancestors are walked — cwd itself is checked, and then, when `$HOME`
   exists, that single directory is checked as a fallback.
4. When `$HOME` is absent (the `HOME` variable unset), only cwd is checked.

Example: with `$HOME` at `/home/user`, a `upeg.toml` planted at `/tmp/x` is
never auto-loaded by `cd /tmp/x/anything` — this blocks pulling arbitrary
commands out of a manifest in a world-writable parent directory.

# `UPEG_PROJECT_MANIFEST_PATH` — detection override

One input environment variable changes detection behavior.

| Value | Behavior |
|---|---|
| Unset or empty | `Detect` — the detection rules above, unchanged |
| `off` (case-insensitive) | `Disabled` — never loads a manifest |
| Absolute path | `Explicit` — skips detection and uses only that path. A missing file means "no manifest"; it does not fall back to detection |
| Relative path | Treated as invalid: a one-line warning goes to stderr and detection falls back to `Detect` |

> **`UPEG_PROJECT_MANIFEST` (no `_PATH`) is not this variable.** That one is an
> **output** variable upeg injects into `External` Tool child processes (see
> [Call envelope](call-envelope.md)); the `UPEG_PROJECT_MANIFEST_PATH`
> described here is an **input** variable the upeg process itself reads. They
> are different variables — never interchangeable.

# Consent notice

"Which file is upeg trusting right now" (the consent notice) and "how did
loading that file go" (the tally) are about **the same file**, so they are
merged into **one line** instead of two, printed to stderr once per process
(`project_manifest_summary_line` in `upeg-cli/src/main.rs`).

| origin | Declared Tools | Output |
|---|---|---|
| `Detected` | some | `upeg: loaded project manifest <path> (N tool(s), M failed)` |
| `Detected` | none | `upeg: loaded project manifest <path>` |
| `EnvOverride` (`UPEG_PROJECT_MANIFEST_PATH`) | some | `upeg: loaded N project tool(s) from <path> (M failed)` |
| `EnvOverride` (`UPEG_PROJECT_MANIFEST_PATH`) | none | *(nothing is printed)* |

The consent-notice half attaches only to `Detected` on purpose: a path the
user pointed at through `UPEG_PROJECT_MANIFEST_PATH` was already named
knowingly, so there is nothing to notify — and when it holds no Tools there is
no tally to report either, so it stays silent entirely.

`--quiet`/`-q` suppresses the whole startup report, this line included.

# Precedence

1. Built-in static Tools are immutable.
2. Runtime Toolkits from `~/.upeg/toolkits` load first.
3. The detected project `upeg.toml` loads next and **may replace** earlier
   runtime metadata/dispatchers under the same id.
4. Shadowing a built-in id is rejected by the loader.

# Project boards — `[[boards]]`

A project manifest may **declare its own boards** in a top-level `[[boards]]`
array. A Tool's `boards = ["..."]` refers to those boards, and a board exists
only while the manifest is detected.

```toml
[[boards]]
id = "upeg-dev"        # the name surfaces use as-is (`upeg board upeg-dev list`)
label = "upeg dev"     # GUI tab title. Defaults to the id when omitted

[[tools]]
id = "git_status"
boards = ["upeg-dev"]
# ...
```

**Project manifests only.** `~/.upeg/toolkits/*.toml` files are global — there
is no project to scope a board to — so a `[[boards]]` there makes the loader
reject that whole file's registration with `BoardsOutsideProjectManifest`.

## Declaration rules

| Rule | Reason |
|---|---|
| `id` must be canonical (no surrounding whitespace) and non-empty | It is the key surfaces print and take as input |
| `id` cannot contain `:` | Reserved as the store-key namespace separator (below) |
| `id` cannot shadow a built-in board (`dev`/`trading`/`personal`) | "Which board is meant" would differ per surface |
| No duplicate `id` inside one manifest | Which of two declarations won would be unknowable |

The built-in board list itself is now data, not code
(`upeg_core::BUILTIN_BOARDS`). Project boards **extend** that table at
runtime.

## Visibility — only while detected

A project board is enumerated **on every surface** only while the manifest is
detected, because `upeg board list`, `upeg board <b> list`, HTTP
`/v1/boards`·`/v1/boards/{b}`, board-scope MCP `tools/list`, and the Desktop
tabs all go through the same `upeg_sources::pegboard` load path. Calling that
board outside the repository answers "unknown board"; over HTTP it is a 404.

When the manifest removes a declaration, the board disappears from the next
load. Its rows stay in the store but invisible, and if the declaration comes
back the pins come back with it.

## Persistence rule — namespaced by manifest path

A project board's rows are stored under this key:

```
project:<manifest-path-digest>:<board-id>
```

`<manifest-path-digest>` is a stable 64-bit digest of the manifest's
**absolute path** (`upeg_core::ProjectBoardNamespace`). Therefore:

- Two different projects declaring the same board id (`dev-board`, say) do
  not share pins.
- Two checkouts of the same repository are also different projects — the
  paths differ.
- **Moving the project directory does not bring the project board's pins
  along.** The path is the only identity upeg can read before parsing the
  manifest — the accepted cost. Using the manifest-declared id as identity
  would collide across unrelated repositories.

Reads and writes pass through the same `BoardVisibility`. One pass sweeps
**only the rows it could write** (tombstones); another project's namespace is
neither read nor erased. That is why editing boards outside a project leaves
the project board's pins untouched.

When a user-made global board and a project board share an id, **inside the
project the project board wins.** The shadowed global board's rows are not
touched, so leaving the project restores the original pins. A
manifest-declared board cannot be deleted (`D` of `n`/`R`/`D` is refused) —
a deletion that would merge right back on the next load is not honest.

# Injected context

```json
{
  "_upeg": {
    "board": "dev",
    "boardEnv": {},
    "projectManifest": "/path/to/upeg.toml"
  }
}
```

See [Call envelope](call-envelope.md) for what the keys mean.

# Provenance and dispatch location

A Tool registered by a project manifest is stamped with
`source = "project-manifest:<path>"` provenance (the `source` field of `upeg
tool list --json`, HTTP `/v1/tools`, and MCP `tools/list`).

That provenance feeds straight into the auto-attach decision on CLI/TUI/
MCP-proxy. A Project Manifest is resolved from **the caller's working
directory** — a host started elsewhere parsed its own `upeg.toml` (or none),
so it does not know these Tools. Therefore:

- A Tool whose provenance is `project-manifest:*` **always runs in-process**,
  even while a host is up.
- The same reason applies **one level up at the board**: `upeg board <b> call`
  does not auto-attach when `<b>` is a project-declared board; it dispatches
  locally. A project board exists only while this process detects the
  manifest, and the host — which parsed its own `upeg.toml`, or none — **does
  not have that board at all**; attaching would return a 404 from
  `/v1/boards/<b>`. Instead of a remote 404 for a board that plainly exists,
  the call goes local — the only path that can succeed. Global boards
  auto-attach as before.
- For other Tools handed to a host, the caller's absolute cwd rides in
  `_upeg.cwd`. The `External` invoker applies it as the child process's
  working directory, so the run happens where the call was made, not at the
  host's cwd (see [Call envelope](call-envelope.md)).

`upeg mcp` in proxy mode follows the same rule. A `tools/call` arriving over
stdio splits on the provenance of `params.name` — in-process or host — and a
frame going out to the host has `params.arguments._upeg.cwd` stamped. The
host's `tools/list` response merges this process's project-manifest Tools,
deduped by name — otherwise an agent would face the contradiction of being
able to call a Tool absent from the listing (see "Proxy mode" in
[MCP](mcp.md)).

# Diagnostics

`upeg doctor` and `upeg host status` each report the detected manifest path
(`none` when absent) and the override state (`detect` / `off` / `explicit`),
in both text and JSON. Both reuse this document's `UPEG_PROJECT_MANIFEST_PATH`
verdict logic verbatim, so what they report cannot diverge from actual
detection behavior.
