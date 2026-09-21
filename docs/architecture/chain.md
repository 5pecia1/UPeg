---
type: Domain Contract
title: Chain Tool
description: The Chain node/connection model, expression grammar, and execution rules.
tags: [architecture, domain, chain]
status: stable
sources:
  - id: chain-dispatcher
    resource: ../../upeg-loader/src/dispatcher.rs
    title: Chain dispatcher implementation
  - id: chain-approval
    resource: ../../upeg-loader/src/dispatcher/chain/approval.rs
    title: Approval authorization policy
  - id: chain-principal
    resource: ../../upeg-core/src/principal.rs
    title: Caller-principal vocabulary
  - id: chain-approval-policy
    resource: ../../upeg-runtime/src/approval.rs
    title: The approval policy UIs read before dispatch
  - id: chain-step-summary
    resource: ../../upeg-loader/src/dispatcher/chain/summary.rs
    title: Step summary metadata
---

# Contract

A Chain is a Tool with `invoker = "Chain"`. Like every other Tool it is pinned,
listed, filtered, logged, triggered, and dispatched. The complexity stays sealed
inside the Chain engine; the user sees one pin.

# Runtime model

`steps` declares the node instances and `connections = [{ from, to }]` declares
the directed edges. Linear chains, fan-out, and joins all use the same
connection list. A node with no incoming connection receives the chain input.

Each step references a Tool id and may declare:

- `args`: a JSON object holding `{{ }}` expressions.
- `when`: a boolean expression. When false, the step is skipped.
- `requires_approval`: an approval barrier before execution. Who may cross that
  barrier is decided by the chain-level `approval_surfaces` (see
  [Approval authorization](#approval-authorization) below).

Use meaningful node ids instead of positional names.

```toml
connections = [{ from = "hash", to = "uppercase" }]

[[steps]]
id = "hash"
tool = "hash.md5"

[[steps]]
id = "uppercase"
tool = "text.uppercase"
```

# Expression grammar

Deliberately small. Nothing beyond `{{step.field}}` substitution is supported.

- `{{input.key}}` — a top-level call argument.
- `{{steps.step_id.output}}` — an earlier step's text output.
- `{{steps.step_id.ok}}` — whether an earlier step succeeded.
- `{{context.board}}` — the reserved `_upeg` context.

A bad reference is a deterministic Tool error, logged without the value.

# Execution rules

1. Duplicate node ids, unknown connection endpoints, and cycles are rejected at
   load time.
2. Run each ready step whose upstream connections are satisfied.
3. Stop at a failed required step.
4. Each step's outcome rides out in the result envelope (see
   [Step summary](#step-summary) below).
5. An approval step requires explicit approval, and that approval is honored
   only when the **caller (principal)** is not excluded and the **calling
   surface** is authorized (see [Approval authorization](#approval-authorization)).
6. The final output is the output of the last completed step unless an `output`
   expression is set.

# Approval authorization

`approve = true` and `_upeg.approvedSteps` are ordinary values inside the call
envelope. A caller can fill them in freely, so by themselves they express only
**intent**. `_upeg.surface` and `_upeg.principal`, by contrast, are stamped by
the runtime, and caller-sent values are erased before dispatch (see
[Call envelope](call-envelope.md)) — they are the caller identity the
dispatcher can trust. Approval authorization separates intent from identity, and
splits identity again into **two independent gates**.

| Gate | What it asks | Basis | Denial code |
|---|---|---|---|
| Principal | Is this caller qualified to approve? | `_upeg.principal.role` | `approval_denied_for_principal` |
| Surface | Does this chain honor approvals arriving through that door? | `_upeg.surface` + `approval_surfaces` | `approval_denied_for_surface` |

The two gates do not stand in for each other. Passing the principal gate still
leaves the surface gate, and widening the surface list with
`approval_surfaces` does not open the principal gate.

## The principal gate

`role` in `_upeg.principal = { role, surface }` is one of three values (see
"Principal" in [Call envelope](call-envelope.md)).

| role | Who | Approval |
|---|---|---|
| `operator` | The person who started this host — in-process `cli`/`tui`/`desktop`, or an HTTP request carrying the operator bearer token | Allowed |
| `local` | An in-process program the OS user launched — the MCP stdio lane | Allowed, but the surface gate blocks it separately |
| `agent` | A program-authenticated caller — an HTTP request carrying an agent token, and any caller the host cannot identify at all | **Denied** |

`agent` alone is excluded, and the reason is the operator's own configuration:
issuing an agent token is saying "you are not me," so a manifest cannot widen
`approval_surfaces` to reverse that judgment. Blocking `local` with it would
*look* stricter but is wrong — `local` stands on the same OS-user boundary as
`cli`, and the moment it were blocked, `approval_surfaces = ["mcp"]` written
deliberately by a chain author would silently become void.

## The surface gate

A chain declares which surfaces it honors approvals from via
`approval_surfaces`.

```toml
[[tools]]
id = "precommit"
invoker = "Chain"
approval_surfaces = ["cli", "tui", "desktop"]   # same as the default when omitted
```

- **The default is the three surfaces a person sits at (`cli`/`tui`/`desktop`).**
  The three share one property: the caller is the OS user account itself, so the
  principal is `operator` without a token having to prove it.
- **All three surfaces have an approval gesture.** `cli` has
  `upeg call <chain> -a approve=true`; `tui` has the confirm dialog in front of
  the run (`Enter`/`F1`/`y` approve, `Esc`/`n`/`q` cancel); `desktop` has the
  pre-run confirm dialog. All three attach `approve` **only after a person's
  confirmation** — no UI has a path that dispatches while skipping the
  confirmation. The per-surface gestures sit in one table under "Approval and
  live output" in the [UI/UX surface contract](../ui-ux-surface-contract.md).
- **On GUI surfaces `approve` is a typed parameter, not data.** Dart passes
  `approve: bool` across the FRB boundary and only Rust puts a reserved key into
  args. `shape_approval_arg` (`upeg-frb/src/api/tools.rs`) erases **both** levers
  the caller could carry — the `approve` key and `_upeg.approvedSteps`. The
  latter is the only key that survives the reserved-block erasure, so while only
  `approve` was being erased, a `upeg://open` deep link carrying
  `{"_upeg":{"approvedSteps":["gate"]}}` opened the barrier untouched. Deep-link
  input is filtered once more on top of that, down to the input fields the Tool
  declared. No args assembled anywhere in the widget tree, no args preset stored
  on a pin, and no URL can approve itself.
- **The GUI surface is decided by this build's runtime.** A native build is
  `desktop`; a wasm32 (PWA) build is `pwa` (`surface_for` in
  `upeg-frb/src/api/capability.rs`). Freeze the two into one and a browser tab
  dispatches as `desktop` — the default approval surface.
- **A surface that is not honored does not ask.** If a surface is absent from
  `approval_surfaces`, the TUI writes **who can approve** in the result pane and
  desktop explains it in a dialog, and neither dispatches. Squeezing a "yes" out
  of a surface that will not honor it is pretending a person holds a power they
  do not.
- **Machine-timed runs do not approve.** Desktop's machine-timed paths — timer
  polling, pin activation — always dispatch with `approve: false`, and a Tool
  behind an approval barrier never auto-runs from an inline pin (only the
  explicit Run button remains). Approval is a question asked of a person; a
  schedule cannot answer it for them.
- **The UI can ask before dispatch.** `ToolMeta` exposes `requires_approval`
  (should a confirmation be shown?) and `approval_surfaces` (is my surface's
  confirmation honored?), and the FRB `ToolDto` carries the same two values as
  `requiresApproval` / `approvalSurfaces`. The loader fills them at chain
  registration (`upeg-runtime/src/approval.rs`). A Tool with no approval barrier
  reports `requires_approval = false` and an empty `approval_surfaces` — with
  nothing to approve, it names no approver.
- `mcp`/`http` are program callers that fill in their own envelopes, and
  `pwa`/`ext` arrive remotely through the pairing-token HTTP surface. All four
  must be named explicitly to be able to approve.
- **Attaching to a host does not stop a terminal being a terminal.** When a host
  is running, `upeg call` attaches over HTTP, but the host verifies the
  origin-surface header the client sends and stamps it as `_upeg.surface`, so
  the stamped value is still `cli` (see "Origin surface" in
  [HTTP API](http-api.md)). `--local` therefore remains a choice that selects
  in-process execution, not a prerequisite for approval.
- **Conversely, `/mcp` cannot be moved by any header.** An MCP client is a
  program and that lane's surface is always `mcp`. The bearer token decides the
  principal — even on a chain that names `mcp` in `approval_surfaces`, a request
  arriving on an agent token is `approval_denied_for_principal` (see
  "MCP JSON-RPC" in [HTTP API](http-api.md)).
- Validation happens at load time. Unknown surfaces, empty entries, an empty
  list that authorizes nobody, the declaration on a non-Chain Tool, and **a list
  that does not intersect the Tool's `surfaces` at all**
  (`approval_surfaces = ["cli"]` + `surfaces = ["mcp"]` — no caller who could
  approve can reach the Tool) are all load errors.
- The argument shapes are unchanged. `approve` / `_upeg.approvedSteps` work as
  they always did; only **the conditions under which they are honored** changed.

## Denial codes

Denials go out as honest typed codes, because an AI agent must be able to tell
"cannot approve here" apart from "retry".

| Situation | `error.code` |
|---|---|
| Caller principal is `agent` | `approval_denied_for_principal` |
| Surface not authorized (or a call with no `_upeg.surface`) | `approval_denied_for_surface` |
| Authorized surface but no approval sent | `approval_required` |

The verdict order is principal → surface. If an agent reaching an authorized
surface were told "wrong surface," it would go door to door looking for one that
was never going to open.

The `approval_denied_for_surface` message lists which surfaces can approve, and
when that list contains `cli` it writes out **the exact command to run** —
`upeg call <chain-id> -a approve=true`. The `approval_denied_for_principal`
message states that changing the surface or widening `approval_surfaces` gives
the same answer. Either way the answer an agent gets is "ask a person to run
this command," not "send it again."

**Nested chains are judged by the same caller.** A step's args are a new object
built from the manifest template with caller text mixed in through
`{{input.*}}`, so a `_upeg` written there is not identity. The engine discards
the step args' `_upeg` block and **hands down the call's block as-is**
(`upeg_runtime::inherit_call_context`) — so the inner chain's approval barrier
is judged by the outer call's real surface and principal, and there is no way to
fabricate `_upeg.surface` or `_upeg.principal` in step args to open the barrier.

**The limit of the boundary.** A principal is still not a user identity. It
cannot tell two operator sessions apart, and it cannot bind an approval to the
name of the person who granted it — that would require real user accounts,
which upeg does not have. What can be proven today ends at "did this host
authenticate this caller as an operator."

# Step summary

Every Chain result carries per-step metadata. This is where the "skipped-step
record" execution rule 4 promises actually reaches the caller; because it rides
the canonical envelope, CLI `--json`, HTTP, and MCP `structuredContent` all see
the same value with no per-surface wiring.

- On success: an output row named `steps` (kind `json`).
- On failure: `error.details.steps`. Same array shape.

Each row is `{ id, tool, status, duration_ms }`.

| `status` | Meaning |
|---|---|
| `ran` | Dispatched and succeeded |
| `skipped` | `when` was false, so it was never dispatched |
| `failed` | Dispatched and failed (or the Tool was not found) |
| `denied` | Blocked at an approval barrier. Whether the run lacked approval or failed principal/surface authorization is told apart by `error.code` |

Only steps that reached an outcome are included — when a chain stops halfway, a
link it never reached has no row at all. `duration_ms` is the wall-clock time
that step's dispatch took; a step that was never dispatched reports 0.

`steps` is a reserved output id on Chain. A Chain Tool that declares `outputs`
under the same name is a load error, and if the last step emits an output with
the same id at run time the call fails with `chain_step_summary_conflict` — an
engine row never silently overwrites an author's row. A chain that produced no
output of its own reports `steps` as the primary output.

**When the last step is itself a chain**, the engine rows of that inner run come
back in the envelope. Every chain answers for its own steps, so the outer engine
**folds** that row and puts down its own — a nested chain is recorded in the
outer summary as the single step the manifest declared. Only the shape the
engine made (`steps` + `json` + a `{ id, tool, status, duration_ms }` array) is
folded, so a `steps` output from a non-chain tool is still reported as
`chain_step_summary_conflict`.

When a step fails, **that step's `error.code` becomes the chain's code as-is.**
If a nested chain stopped at an approval barrier, the outer call must read
`approval_denied_for_surface` (or `approval_denied_for_principal`) so the agent
knows retrying is not the move.

Related: [Manifest contract](manifest.md), [Call envelope](call-envelope.md)
