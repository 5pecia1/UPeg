---
type: Generated Reference
title: External Tool Manifest Guide
description: Full Toolkit TOML field reference generated from the Rust manifest types.
tags: [manifest, toml, reference, generated]
status: stable
generated: { by: process:upeg-interface-toolkit-schema }
---

# External Tool Manifest Guide

Generated from upeg-loader manifest documentation metadata. Do not edit by hand.

## Who this is for

Use this guide when you want to expose local commands, HTTP calls, chains, LLM prompts, WASM modules, embedded web views, or upstream MCP tools through upeg surfaces without adding built-in Rust tools.

## Quick start: one External tool in TOML

Create one Toolkit TOML file, give it a stable root `id`, and add at least one `[[tools]]` entry. This minimal example is based on `examples/tools/echo-bracketed.toml`; copy it to `~/.upeg/toolkits/demo.toml`, then validate it before calling the tool.

### ~/.upeg/toolkits/demo.toml

```toml
# Minimal External invoker example showing token-substitution.
#
#   upeg call demo.echo_bracketed -a tag=hi
#   → [hi]
#
# `{key}` placeholders are substituted with `args[key]` from the JSON
# args, anywhere inside a token (`--out={path}` works). Write `{{` or
# `}}` for a literal brace.

id = "demo"
tags = ["example"]

[[tools]]
id = "echo_bracketed"
description = "Wrap an input tag in brackets via printf."
pegboard_units = "U1"
invoker = "External"
command = "printf"
args_template = ["[%s]", "{tag}"]

[[tools.inputs]]
name = "tag"
type = "string"
required = true

```

### Validate and call it

```bash
upeg tool validate ~/.upeg/toolkits/demo.toml
upeg call demo.echo_bracketed -a tag=hi
```

## Where upeg loads external manifests from

Toolkit TOML files load from `~/.upeg/toolkits/{toolkit_id}.toml` unless `UPEG_TOOLKITS_DIR` points elsewhere. WASM plugin binaries load from `~/.upeg/wasm/*.wasm` unless `UPEG_WASM_DIR` is set. MCP upstream configs load from `~/.upeg/mcp-imports/*.toml` unless `UPEG_MCP_IMPORTS_DIR` is set. The `examples/` tree contains copyable fixtures for each source type, including `examples/tools/http-mock-echo.toml`, `examples/tools/llm-echo.toml`, `examples/tools/embed-mdn.toml`, and `examples/tools/chain-md5-then-uppercase.toml`.

## Toolkit TOML structure

The root object describes a non-callable Toolkit namespace. Each `[[tools]]` entry declares one callable tool id local to that Toolkit. The loader registers the tool as `<toolkit>.<tool>` and rejects duplicated toolkit prefixes such as `id = \"demo.echo\"` inside `[[tools]]`.

### Toolkit with a chain tool

```toml
# Chain Tool composing two built-ins: md5 hex digest, then uppercase.
#
#   upeg call demo.loud_md5 -a input=hello
#   → 5D41402ABC4B2A76B9719D911017C592
#
# `hash` gets the chain's args. The connection from `hash` to
# `uppercase` makes `uppercase` receive the MD5 stdout as
# `{ "input": "<text>" }`.

id = "demo"
tags = ["chain", "example"]

[[tools]]
id = "loud_md5"
description = "MD5 of input, then uppercased."
invoker = "Chain"
pin = "Chain"
pegboard_units = "U1"
connections = [{ from = "hash", to = "uppercase" }]

[[tools.inputs]]
name = "input"
type = "string"
required = true

[[tools.steps]]
id = "hash"
tool = "hash.md5"

[[tools.steps]]
id = "uppercase"
tool = "text.uppercase"

```

## Field reference

The table below documents the user-authored Toolkit TOML shape. It intentionally describes Toolkit TOML input and source-specific adapter fields, not the runtime-only internal manifest structs.

### ToolkitToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Stable toolkit namespace. Tool entries are registered as `<toolkit>.<tool>`. |
| `tags` | no | array<string> or null | Toolkit-level tags inherited by every tool unless already present. |
| `display_label` | no | string or null | Human-readable toolkit label for UI surfaces. |
| `description` | no | string or null | Human-readable toolkit description. |
| `tools` | yes | array<ToolEntryToml> | Tools declared inside this toolkit manifest. |
| `boards` | no | array<BoardEntryToml> | Boards this manifest declares. Project Manifests (`upeg.toml`)<br>only — the loader rejects the field on a toolkits-directory<br>manifest, which has no project to scope a board to. |

### ToolEntryToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Local tool id. The loader prefixes it with the toolkit id. |
| `tags` | no | array<string> or null | Tool-specific tags appended to toolkit-level tags. |
| `display_label` | no | string or null | Human-readable tool label for UI surfaces. |
| `description` | no | string or null | Human-readable tool description. |
| `inputs` | no | array<InputFieldToml> | Typed input fields accepted by this tool. Omit for tools with no inputs. |
| `outputs` | no | array<OutputFieldToml> | Typed output fields produced by this tool. Omit for action-only tools. |
| `primary_output_id` | no | string or null | Canonical output field id used for the default CLI result, MCP text<br>fallback, and UI primary emphasis. Required when `outputs` is non-empty,<br>and omitted when `outputs` is empty. |
| `effect` | no | string or null | Optional author-declared side-effect classification. |
| `presentation` | no | PresentationToml or null | Optional structured result presentation metadata. |
| `pin` | no | string or null | UI rendering hint only, such as `Inline`, `Modal`, or `Embed`; runtime<br>adapter selection is controlled by `invoker`. |
| `pegboard_units` | no | string or null | Pegboard size class. Loader validation requires `U1`, `U2`, or `U2T`. |
| `invoker` | no | string or null | Runtime adapter selector. `Embed` is the canonical runtime declaration<br>for embedded sidecars; `steps` can infer `Chain`, while other runtime<br>tools must set this explicitly. |
| `surfaces` | no | array<string> or null | Surface ids where this tool should appear, such as `cli`, `tui`, or `http`. |
| `boards` | no | array<string> or null | Board ids where UI surfaces should group this tool. |
| `command` | no | string or null | External-invoker support: program to spawn (e.g. `"git"`).<br>Honored only when `invoker = "External"`. |
| `setup` | no | ToolSetupToml or null | Optional display-only setup guidance. Honored only by `External`. |
| `args_template` | no | array<string> or null | Arg list template for the spawned command. `{key}` placeholders<br>are substituted anywhere inside a token (`--manifest-path={path}`);<br>`{{` / `}}` escape a literal brace. Every `key` must name a<br>declared `inputs` field (or `input`, which the `Chain` invoker<br>supplies to every step) — a placeholder naming nothing else is a<br>load error rather than a silently empty argument. A token that is<br>nothing but `{key}` for an optional input with neither a value<br>nor a `default` is dropped from the arg list; every other token<br>keeps its position and renders the absent placeholder as `""`. |
| `cwd` | no | string or null | Working directory for the spawned command. A relative path<br>resolves against the directory holding this manifest file.<br>Honored only when `invoker = "External"`. When omitted, a<br>Project Manifest (`upeg.toml`) tool runs in the manifest's own<br>directory and a caller-supplied working directory is honored<br>only if it sits inside that directory. |
| `env` | no | array<KeyValueToml> or null | Plain (non-secret) environment variables handed to the spawned<br>command. Secrets belong in `credentials`, which is applied after<br>this list and therefore wins on a name collision. |
| `timeout_ms` | no | integer or null | Wall-clock budget for the spawned command, in milliseconds. When<br>it elapses upeg terminates the child's whole process group and<br>fails with `details.timed_out = true`. Omit for no limit. |
| `color` | no | string or null | Whether the spawned command is told color is supported.<br>`"inherit"` (the default) leaves it seeing a captured pipe and<br>turning color off; `"force"` sets `CLICOLOR_FORCE`/`FORCE_COLOR`,<br>unsets `NO_COLOR` (and sets `TERM` when the host has none). It is<br>the environment convention, not a pty: a program that decides on<br>`isatty(3)` alone needs its own flag. Honored only when<br>`invoker = "External"`. |
| `pty` | no | boolean or null | Whether the spawned command gets a real terminal instead of two<br>pipes. `true` opens a pseudoterminal on Unix and connects the<br>child's stdout and stderr to it, so `isatty(3)` is true and a<br>program that decides on it alone — `git`, `ls`, `grep` — emits<br>its terminal output. The two streams arrive **merged** in one<br>captured `stdout`, since a terminal has only one buffer, and<br>`pty = true` implies `color = "force"` unless `color` is declared<br>explicitly. stdin stays `/dev/null`. Rejected at load time on a<br>host with no pty (Windows, wasm). Honored only when<br>`invoker = "External"`. |
| `steps` | no | array<ChainStepToml> or null | Chain steps. This is the single PRD v2.1 manifest shape for<br>expression flow, branching, approvals, and connected node execution. |
| `connections` | no | array<ChainConnectionToml> or null | Directed edges between chain steps. Steps without incoming connections<br>receive the chain input; connected steps receive upstream outputs. |
| `approval_surfaces` | no | array<string> or null | Surface labels allowed to satisfy this chain's `requires_approval`<br>steps. An approval (`approve = true` / `_upeg.approvedSteps`) is<br>honored only when the call's `_upeg.surface` is in this list;<br>every other surface is refused with `approval_denied_for_surface`.<br>Defaults to the three surfaces a person is sitting at — `cli`,<br>`tui`, `desktop` — each of which ships a real approval gesture<br>(`upeg call <chain> -a approve=true`, the TUI's confirm dialog,<br>the desktop's confirm dialog). Every other surface must be named<br>explicitly. Must overlap this tool's `surfaces`, or the gated<br>step could never be approved by anyone who can reach it. |
| `output` | no | string or null | Optional final output expression for Chain tools. |
| `url` | no | string or null | HTTP invoker URL template. |
| `method` | no | string or null | HTTP method (`GET`/`POST`/...). Defaults to POST when a body is present,<br>GET otherwise. |
| `headers` | no | array<KeyValueToml> or null | HTTP headers as name/value templates. |
| `body` | no | string or null | HTTP request body template. |
| `prompt` | no | string or null | LLM prompt template. `{{input}}`, `{{input.key}}`, and<br>`{{credential.name}}` expressions are resolved at execution time. |
| `provider` | no | string or null | LLM provider. `echo` is deterministic/local; `tool:<id>` delegates the<br>rendered prompt into another Tool; `openai` calls an OpenAI-compatible<br>`/chat/completions` HTTP endpoint (see `base_url`). |
| `model` | no | string or null | Provider model name. Required when `provider = "openai"`. |
| `base_url` | no | string or null | OpenAI-compatible provider base URL, e.g. `https://api.openai.com/v1`.<br>Only meaningful when `provider = "openai"`. Defaults to the loader's<br>built-in OpenAI endpoint when omitted; set this to point at a<br>self-hosted OpenAI-compatible server instead. |
| `credential` | no | string or null | Primary credential name/key for HTTP/LLM templates. Reuses the matching<br>declared entry from `credentials` when present; otherwise resolves the<br>default env fallback `UPEG_CREDENTIAL_<NAME>`. Never inline a secret.<br>For `provider = "openai"`, this is the credential resolved as the<br>`Authorization: Bearer` API key. |
| `credentials` | no | array<CredentialRefToml> or null | Named credential references from env, keychain, or adapter sources; no<br>secret values are persisted in toolkit manifests. |
| `wasm_path` | no | string or null | WASM module path for declarative WASM adapter diagnostics/host loading. |
| `triggers` | no | array<TriggerToml> or null | Trigger declarations. Runtime adapters normalize all sources into the<br>same Tool-dispatch event contract. |
| `embed_url` | no | string or null | GUI sidecar URL registered when present and non-empty. Recommended together with<br>`invoker = "Embed"` and `pin = "Embed"`, but it is not gated<br>only by `pin`. |
| `controlled_embed` | no | ControlledEmbedToml or null | Controlled Embed browser settings for User-Agent and viewport overrides. |

### ToolSetupToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `guide_url` | no | string or null | HTTP(S) guide for obtaining or configuring the required command. |
| `instructions` | no | string or null | Plain-language operator guidance. UPeg never executes this text. |
| `install` | no | ToolSetupInstallToml or null | Display-only commands, selected by the host operating system. |

### ToolSetupInstallToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `linux` | no | array<string> | — |
| `macos` | no | array<string> | — |
| `windows` | no | array<string> | — |

### InputFieldToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `name` | yes | string | Canonical argument key, such as `input`, `path`, or `mode`. |
| `type` | yes | string | Closed input kind: `string`, `number`, `integer`, `boolean`, `options`,<br>`multi_options`, `markdown`, `json`, `datetime`, `file_path`, `url`, or `file`. |
| `label` | no | string or null | Optional human-readable label for form surfaces. |
| `description` | no | string or null | Optional help text for form surfaces. |
| `required` | no | boolean | Whether the caller must provide this input. Defaults to optional. |
| `default` | no | boolean or number or string or null | Value used when the caller omits this input. Lowered into the<br>field's [`upeg_core::FieldConstraints`], so it also reaches the<br>generated JSON Schema and every form surface, and the External<br>invoker substitutes it into `args_template`.<br>`number` / `integer` inputs take a number; `string`, `options`,<br>`markdown`, `json`, `datetime`, `file_path`, and `url` take a<br>string. Other types (including `boolean`) have no default slot<br>and are rejected at load time. |
| `options` | no | array<InputChoiceToml> or null | Selectable choices for `type = "options"` or `type = "multi_options"`. |
| `extensions` | no | array<string> or null | Allowed file suffixes. Values are normalized to lowercase without a<br>leading dot. An empty or omitted list allows every extension. |
| `max_count` | no | integer or null | Maximum recursive file leaf count for `type = "file"`. |
| `max_file_bytes` | no | integer or null | Optional maximum byte length of each file leaf. |
| `max_total_bytes` | no | integer or null | Optional maximum aggregate byte length across all file leaves. |

### InputChoiceToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `value` | yes | string | Stable value sent in tool arguments when this choice is selected. |
| `label` | no | string or null | Optional human-readable label for form surfaces. |
| `description` | no | string or null | Optional help text for this choice. |

### OutputFieldToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `name` | yes | string | Canonical output key, such as `result`, `count`, or `view`. |
| `type` | yes | string | Closed output kind: `string`, `number`, `integer`, `boolean`, `options`,<br>`multi_options`, `markdown`, `json`, `datetime`, `file_path`, `url`,<br>`file`, or `embedded_view`. |
| `label` | no | string or null | Optional human-readable label for rendering surfaces. |
| `description` | no | string or null | Optional help text for rendering surfaces. |
| `options` | no | array<InputChoiceToml> or null | Selectable choices for `type = "options"` or `type = "multi_options"`. |
| `url` | no | string or null | URL for `type = "embedded_view"` outputs. |

### ChainStepToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `id` | no | string or null | Stable local node id used by expressions and `connections`. |
| `tool` | yes | string | Canonical Tool id to invoke. |
| `args` | no | string or null | JSON object template for step args. When omitted, unconnected source<br>steps receive the chain input. A step with one upstream receives<br>`{ "input": upstream_output }`; multiple upstreams receive<br>`{ "input": last_upstream_output, "inputs": { "<upstream>": "<output>" } }`. |
| `when` | no | string or null | Boolean expression. False skips the step. |
| `requires_approval` | no | boolean or null | Approval barrier. The caller must include this step id in<br>`_upeg.approvedSteps` or pass `approve = true`. |

### ChainConnectionToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `from` | yes | string | Upstream chain step id. |
| `to` | yes | string | Downstream chain step id. |

### KeyValueToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `name` | yes | string | Key or header name. |
| `value` | yes | string | Value template. |

### CredentialRefToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `name` | yes | string | Logical credential name used by prompt/header/body templates. |
| `type` | no | string or null | Secret value type for adapter validation (`api_key`, `bearer`, etc.).<br>This is schema metadata only; the value itself is never stored. |
| `store` | no | string or null | Reference backend. `env` reads an environment variable; `keychain`<br>reads an OS keychain item by service/account when the host supports it. |
| `env` | no | string or null | Environment variable to read. Defaults to `UPEG_CREDENTIAL_<NAME>`. |
| `keychain_service` | no | string or null | OS keychain service name when `store = "keychain"`. |
| `keychain_account` | no | string or null | OS keychain account/user when `store = "keychain"`. |
| `target` | no | string or null | Adapter-specific target name (for example external process env name or<br>HTTP header expression name). Defaults to `name`. |
| `required` | no | boolean or null | Missing credentials fail by default. Set false only for optional<br>provider features. |

### SelectorBindingToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `role` | no | string | Role this binding plays in the Controlled Embed pipeline:<br>`"input"` (write field → DOM), `"trigger"` (click DOM element),<br>`"output"` (read DOM → field). Defaults to `"input"` when<br>omitted so simple manifests stay terse. |
| `field` | no | string | Tool input or output field name to bind. Unused for `trigger`. |
| `selector` | yes | string | CSS selector inside the embedded page. |
| `action` | no | string or null | Trigger action for `role = "trigger"`: `"click"` (default) or<br>`"enter"`. Non-trigger bindings may omit this or use `"click"` only. |
| `wait` | no | BindingWaitToml or null | Optional wait settings before this binding is applied or read. Supported<br>only for Controlled Embed selector bindings; network-idle,<br>DOM-stability, and page lifecycle waits are out of scope. |

### TriggerToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `source` | yes | string | `webhook`, `schedule`, `file`, `directory`, `clipboard`, or `hotkey`. |
| `condition` | no | string or null | Source-specific predicate. Every source's contract is checked at load<br>time, so a missing or superfluous condition fails `upeg tool validate`<br>rather than at watch time.<br>`schedule` takes `now` (fire once when the watch starts) or<br>`every:<duration>` such as `every:30s`, and defaults to `now` when<br>omitted; the watch polls once per second, so a shorter interval is<br>rejected and an interval that is not a whole multiple of the poll fires<br>on the first poll at or after each due moment. `file` and `directory`<br>require the watched path and fire on creation and on modification.<br>`hotkey` requires an accelerator such as `ctrl+shift+u`. `clipboard` and<br>`webhook` take no condition and reject one. |

### ControlledEmbedToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `browser` | no | ControlledEmbedBrowserToml or null | Browser identity and viewport overrides. |
| `bindings` | no | array<SelectorBindingToml> or null | Selector bindings nested under Controlled Embed. |

### ControlledEmbedBrowserToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `user_agent` | no | string or null | User-Agent mode: `default`, `mobile_safari`, or `custom`. |
| `custom_user_agent` | no | string or null | Custom User-Agent string. Required only when `user_agent = "custom"`. |
| `viewport` | no | string or null | Viewport mode: `mobile`, `tablet`, `desktop`, or `custom`. |
| `viewport_width` | no | integer or null | Custom viewport width. Required only when `viewport = "custom"`. |
| `viewport_height` | no | integer or null | Custom viewport height. Required only when `viewport = "custom"`. |

### BindingWaitToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `for_selector` | no | string or null | CSS selector to wait for. Defaults to the binding's own selector. |
| `condition` | no | string or null | Wait condition. Supported values are `"exists"` and `"visible"`.<br>Defaults to `"exists"`. |
| `timeout_ms` | no | integer or null | Maximum wait duration in milliseconds. Defaults to 5000. |
| `settle_ms` | no | integer or null | Fixed delay in milliseconds after the condition matches. Defaults to 0.<br>This is not a network-idle or DOM-stability detector. |
| `on_timeout` | no | string or null | Timeout behavior. Supported values are `"fail"` and `"continue"`.<br>Defaults to `"fail"`. |

### BoardEntryToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Board id, as typed on every surface (`upeg board <id> list`) and<br>as named by a tool's `boards = [...]` array. Must be canonical<br>(unpadded), must not contain `:`, and must not shadow a built-in<br>board. |
| `label` | no | string or null | Tab title for GUI surfaces. Defaults to `id` when omitted. |
| `description` | no | string | Short description of the board's purpose. Empty when omitted. |
| `instructions` | no | string | Markdown guidance for people and agents using this board. |

### PresentationToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `version` | yes | integer | Version of the presentation contract. Currently `1`. |
| `output` | no | string or null | JSON output field containing the collection to render. Required with<br>`rows`, `row_key`, and `columns`; omit all four for action-only results. |
| `rows` | no | string or null | JSON Pointer from `output` to the rows array. |
| `row_key` | no | string or null | JSON Pointer from each row to its stable key. |
| `columns` | no | array<PresentationColumnToml> | Ordered columns rendered for each row in the collection. |
| `actions` | no | array<PresentationActionToml> | Follow-up actions available for the result or its rows. |

### PresentationColumnToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `label` | yes | string | Human-readable column heading. |
| `pointer` | yes | string | JSON Pointer from the current row to the displayed value. |

### PresentationActionToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Unique action id within this presentation. |
| `scope` | yes | string | Action visibility: `result` or `row` for collection presentations. |
| `label` | yes | string | Human-readable label shown to the user. |
| `target_tool` | yes | string | Fully qualified id of the tool invoked by this action. |
| `on_success` | no | string or null | Optional post-success behavior; currently `refresh_origin`. |
| `bindings` | no | object | Map of target-tool input names to values resolved from this result. |

### PresentationBindingToml

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `from` | yes | string | Value source: `input`, `output`, `row`, or `constant`. |
| `pointer` | no | string or null | JSON Pointer used by non-constant sources. |
| `value` | no | unknown | Literal JSON value used by a `constant` source. |

## Tool output contracts

Tool outputs are canonical structured `ToolResult` values, not raw process stdout. A successful result is a JSON envelope with `ok=true`, `primary_output_id`, and ordered `outputs[]` entries. Each output entry carries `id`, optional `label`, `kind`, and typed `value`; `primary_output_id` must reference one declared `outputs[].name` whenever outputs are non-empty and must be omitted for action-only tools.

CLI rendering is a presentation policy over the same canonical result: the default `upeg call` mode prints only the primary output value, `--json` prints the canonical success/error envelope, `--field <id>` prints one output value, and `--pretty` prints labeled rows. Diagnostics, progress, and logs belong on stderr so stdout remains reserved for the selected result representation.

HTTP, daemon, and FRB transports return canonical JSON envelopes. MCP `tools/call` uses the same canonical success envelope as `structuredContent`; the MCP text `content` is only a display fallback containing the primary output text. TUI, Desktop, PWA, Chrome extension, and Controlled Embed output surfaces render labels and values from canonical output entries, with richer modal/card treatment allowed only as presentation.

A canonical failure is `ok=false` plus an `error` object with `code`, `message`, and an optional structured `details` value. `External` fills `details` with `{ "exit_code": <int|null>, "stdout": "…", "stderr": "…" }`, adding `signal` when the child was killed and `timed_out: true` when `timeout_ms` elapsed. Both streams are captured and capped, so a failing `cargo`, `clippy`, or `flutter analyze` hands back the diagnostics it wrote to stdout instead of only a summary line. Non-JSON surfaces print `message`, then the labeled `stderr` and `stdout` blocks; `--json`, the HTTP body, MCP `structuredContent`, and the FRB bridge carry the whole `details` object.

## Invoker-specific fields

The loader preserves the TOML shape, infers `Chain` only when `steps` is present, then validates each runtime invoker's required fields before registering dispatchers.

The `External` invoker also controls how the child process runs. `cwd` sets its working directory; a relative value resolves against the directory holding the manifest. With no declared `cwd`, a Project Manifest (`upeg.toml`) tool runs in the manifest's own directory, and a caller-supplied working directory is honored only when it sits inside that directory — a toolkit-directory tool has no project to belong to, so there the caller's directory wins outright. `env` lists plain, non-secret environment variables and is applied before `credentials`, so a credential wins on a name collision. `timeout_ms` is a wall-clock budget after which upeg terminates the child's whole process group; there is no default, because a long build or test run must stay legal. stdin is always `/dev/null`, so an interactive prompt fails fast instead of hanging the calling surface. `color` decides what the child is told about color support: the default (`"inherit"`) leaves it looking at a captured pipe, so it turns color off by itself, while `color = "force"` sets `CLICOLOR_FORCE` and `FORCE_COLOR`, unsets an inherited `NO_COLOR` (which the same convention ranks above both), and sets `TERM` only when upeg's own environment has none. That is pure environment — no pty, identical on Unix and Windows — and it applies before `env`, so an explicit variable overrides it. It is the convention rather than a terminal: a program that decides on `isatty(3)` alone, like `git` or `ls`, ignores all of it and needs its own flag (`git -c color.ui=always`). An unknown value is rejected at load time rather than silently ignored at dispatch time.

`pty = true` is the other answer to that same question: it opens a real pseudoterminal on Unix and connects the child's stdout and stderr to it, so `isatty(3)` is true and no flag is needed, and it implies `color = "force"` as well unless the tool declares `color` itself, in which case the declaration wins. A terminal has one buffer, so both streams come back merged in `stdout` and `stderr` is empty; stdin stays `/dev/null` because upeg is never the human on the other side. Timeouts and escaped-descendant cleanup are unchanged. A host with no pty (Windows, wasm) skips that one tool at load time — recorded with its reason, never silently downgraded to pipes — and every other tool in the same manifest still loads.

A run can also be cancelled while it is still going. Three surfaces install a cancellation scope today: the HTTP streaming route cancels when its client hangs up, the TUI cancels on `Esc` while a run is on screen, and the FRB bridge cancels on `cancel_dispatch(run_id)` from Dart. Inside any of them `External` reads the token on every tick of its wait loop, terminates the child's process group, and answers with `error.code = "cancelled"` and `details.cancelled = true` alongside whatever the child had already written. Cancellation is a request, not a guarantee — a tool that never yields runs to completion and still returns one envelope.

A long-running command does not have to be silent until it exits. While the child runs, `External` also forwards whole lines of stdout and stderr to whichever surface asked for them: `upeg call` mirrors both to the terminal's stderr (stdout stays reserved for the final result, and `--json`/`--field` stay silent), the HTTP surface streams them as NDJSON from `POST /v1/tools/{id}/stream`, and MCP `tools/call` sends them as `notifications/message` log frames. The final envelope, the capture limits, and the timeout behaviour are unchanged; a surface that cannot consume progress pays nothing and sees exactly what it saw before.

`args_template` tokens are small templates: `{key}` substitutes anywhere inside a token (`--manifest-path={path}`, `-p{crate}`) and `{{` and `}}` are literal braces. Every `key` must name a declared `inputs` field — the only exception is `input`, which the `Chain` invoker hands to every step — so a typo is a load error rather than a silently missing argument. A token that is nothing but `{key}` for an optional input with neither a value nor a `default` is dropped from the arg list; every other token keeps its position and renders the absent placeholder as an empty string, because dropping `{dir}/build` would shift every argument after it. A successful command's stdout is the primary output; when the tool declares no `outputs` of its own and the command also wrote to stderr, that text is surfaced as a secondary `stderr` output.

| Invoker | Purpose | Required fields | Common optional fields |
| --- | --- | --- | --- |
| `External` | spawn a local process | `command` | `args_template`, `cwd`, `env`, `timeout_ms`, `color`, `credentials` |
| `Http` | call an HTTP endpoint | `url` | `method`, `headers`, `body`, `credential`, `credentials` |
| `Embed` | open a GUI sidecar runtime adapter in a WebView or iframe | — | `embed_url`, `controlled_embed.browser`, `controlled_embed.bindings` |
| `Chain` | compose other tools with ordered steps and connections | `steps` | `connections`, `output` |
| `Llm` | render a prompt into an LLM/provider adapter | `prompt` | `provider`, `model`, `credential`, `credentials` |
| `Wasm` | load a WASM module adapter | `wasm_path` | — |

### External execution knobs (cwd, env, timeout_ms, input defaults)

```toml
# Every External-invoker execution knob in one Toolkit.
#
#   upeg call dev.git_log                   # uses default count = 10
#   upeg call dev.git_log -a count=3
#   upeg call dev.cargo_check
#
# What this file demonstrates:
#
#   cwd        the child runs here instead of inheriting upeg's own
#              working directory. A relative path resolves against the
#              directory holding this manifest, so a Project Manifest
#              tool works from any subdirectory and from the host
#              daemon. A caller may also pass an absolute path as
#              `_upeg.cwd`; a declared `cwd` wins over it.
#   env        plain (non-secret) environment variables. Secrets stay
#              in `credentials`, which is applied afterwards and so
#              wins on a name collision.
#   timeout_ms wall-clock budget. On expiry upeg terminates the child's
#              whole process group and fails with
#              `details.timed_out = true`. Omit it for no limit — a
#              twenty-minute `just verify` must stay legal.
#   color      whether the child is told color is supported. upeg pipes
#              both streams, so a child sees a non-TTY and turns color
#              off by itself — `color = "force"` sets CLICOLOR_FORCE /
#              FORCE_COLOR, unsets NO_COLOR (and sets TERM when the host
#              has none). Pure env: no pty, same behaviour on Unix and
#              Windows. It is the *convention*, not a terminal: a program
#              that decides on isatty(3) alone — git, ls, grep — ignores
#              all three and needs its own flag, as `git_log` below shows.
#   pty        the other answer to the same question: `pty = true` opens
#              a real pseudoterminal (Unix only) and connects the child's
#              stdout and stderr to it, so isatty(3) is true and no flag
#              is needed. The price is that a terminal has one buffer:
#              both streams come back merged in `stdout` and `stderr` is
#              empty. It implies `color = "force"` unless you declare
#              `color` yourself. `git_log_pty` below is `git_log`
#              without the flag. On a host that cannot open a
#              pseudoterminal that one tool is skipped at load time, with
#              its reason recorded; this file's other three tools load
#              there exactly as they do on Unix.
#   default    value substituted when the caller omits the input, so
#              `git log -n {count}` never degrades into a bare
#              `git log -n`.
#
# When a command exits non-zero, the canonical failure envelope carries
# `error.details = { exit_code, stdout, stderr }` — `cargo`, `clippy`,
# and `flutter analyze` write their diagnostics to stdout, so the
# envelope keeps both streams rather than only the summary line.

id = "dev"
tags = ["dev", "example"]

[[tools]]
id = "git_log"
description = "Recent commits, one line each."
pegboard_units = "U1"
invoker = "External"
command = "git"
# `-c color.ui=always` is what actually turns the colored ref decoration
# back on: git decides on isatty(3) and never reads CLICOLOR_FORCE or
# FORCE_COLOR, so the flag is the only thing it listens to here.
args_template = ["-c", "color.ui=always", "log", "--oneline", "-n", "{count}"]
timeout_ms = 10000
# Declared alongside the flag because that is the half of the recipe
# every convention-honoring CLI (`cargo`, `gh`, `eza`, anything using
# chalk) reads instead. Both halves together are the honest way to ask a
# captured child for color.
color = "force"

[tools.setup]
guide_url = "https://git-scm.com/downloads"
instructions = "Install Git, then recheck this tool."
[tools.setup.install]
linux = ["sudo apt install git"]
macos = ["brew install git"]
windows = ["winget install Git.Git"]

[[tools.inputs]]
name = "count"
type = "integer"
default = 10

[[tools]]
id = "cargo_check"
description = "Type-check one crate of the manifest's own project."
pegboard_units = "U2"
invoker = "External"
command = "cargo"
args_template = ["check", "--manifest-path={manifest_path}", "--quiet"]
cwd = "."
timeout_ms = 600000

[[tools.env]]
name = "RUST_LOG"
value = "warn"

[[tools.inputs]]
name = "manifest_path"
type = "file_path"
default = "Cargo.toml"

# A slow command is where the live-output contract earns its keep. Run
# `upeg call dev.slow_progress` and the three steps appear one per
# second on stderr instead of all at once at the end; stdout still
# carries only the final result. The same lines reach
# `POST /v1/tools/dev.slow_progress/stream` as NDJSON and MCP clients as
# `notifications/message` frames. Add `--json` and nothing is streamed —
# that mode exists to be parsed.
[[tools]]
id = "slow_progress"
description = "Writes one line per second so live output is visible."
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "for i in 1 2 3; do echo \"step $i\"; sleep 1; done"]
timeout_ms = 10000

# The same recent-commits list, asked for the other way. `git` decides on
# isatty(3), so with a real terminal it colors its ref decoration with no
# `-c color.ui=always` at all — the flag `git_log` needs exists precisely
# because a pipe is not a terminal.
#
# What you give up is the split: a pty has one buffer, so anything git
# writes to stderr (a progress line, a warning) lands inside the same
# `stdout` text, interleaved where it happened, and `stderr` comes back
# empty. That is the trade — pick `git_log` when the two streams must
# stay apart, `git_log_pty` when the tool only cooperates with a terminal.
#
# Unix only. On a host with no pseudoterminal (Windows, wasm) this one
# tool is skipped at load time and the rest of this file still loads —
# which is why it can live here next to `git_log` instead of in a
# Unix-only manifest of its own. It is skipped rather than downgraded on
# purpose: silently falling back to pipes would make the same manifest
# mean two different things on two machines, and that difference is the
# whole reason to declare `pty`.
[[tools]]
id = "git_log_pty"
description = "Recent commits, colored because the child sees a real terminal."
pegboard_units = "U1"
invoker = "External"
command = "git"
args_template = ["log", "--oneline", "-n", "{count}"]
timeout_ms = 10000
# Implies `color = "force"` as well: a manifest that asked for a terminal
# wants color out of the convention-reading programs too. Declaring
# `color` yourself wins over the implication.
pty = true

[[tools.inputs]]
name = "count"
type = "integer"
default = 10

```

## Input rules

`inputs` is an optional TOML array of typed fields. Omit it or set `inputs = []` for tools with no inputs. Each field uses the closed upeg input type vocabulary: `string`, `number`, `integer`, `boolean`, `options`, `multi_options`, `markdown`, `json`, `datetime`, `file_path`, `url`, and `file`. Choice fields (`options` and `multi_options`) must declare non-empty `options` entries.

### Llm tool with inputs

```toml
# Offline Llm invoker example using the deterministic local echo provider.
#
# `provider = "echo"` renders the prompt locally, so this example makes
# no network request and needs no configured account.

id = "llm_demo"
tags = ["example", "llm"]

[[tools]]
id = "echo_prompt"
description = "Render a prompt with the deterministic local echo provider."
pin = "Llm"
pegboard_units = "U1"
invoker = "Llm"
provider = "echo"
prompt = "Summarize: {{input}}"

[[tools.inputs]]
name = "input"
type = "string"
required = true

```

## Credentials and secret references

Credential entries name secret values resolved from environment variables or OS-backed references at execution time. Manifests store references and metadata only; inline secret fields such as `credentials[].value`, `credentials[].secret_value`, and `credentials[].literal_secret` are rejected.

### Environment credential reference

```toml
# Http invoker example with an environment-based credential reference.
#
# This example shows how to reference a credential stored in an
# environment variable without embedding the secret value in the TOML.
# `upeg` resolves `credentials[].env` at execution time.
#
# IMPORTANT: No secret values are included — only the reference metadata.
# Set `UPEG_DEMO_API_KEY` in your environment before running.

id = "http_cred_demo"
tags = ["example", "http", "credential"]

[[tools]]
id = "authenticated_echo"
description = "Echo request carrying a bearer token from the environment."
pegboard_units = "U1"
invoker = "Http"
url = "mock://echo"
method = "GET"
credential = "api_key"
credentials = [
  { name = "api_key", type = "bearer", store = "env", env = "UPEG_DEMO_API_KEY", required = true }
]

```

## JSON Schema and editor validation

`fixtures/toolkit.schema.json` is generated from the same Rust Toolkit TOML structs that serde deserializes. Use it for editor validation of the TOML-to-JSON shape. `upeg tool validate` is authoritative for semantic validation such as required runtime fields, invoker compatibility, chain acyclicity, and typed input rules. The JSON Schema does not prove absence of chain cycles, credential existence, executable availability, or URL reachability.

Generated schema title: `ToolkitToml`. Top-level schema fields: `id`, `tags`, `display_label`, `description`, `tools`, `boards`.

## WASM plugin manifests

Declarative Toolkit TOML can reference a WASM module with `invoker = \"Wasm\"` and `wasm_path`, but normal WASM plugins declare their manifest from code. Author tools with `upeg_plugin_macros`: annotate a plain typed Rust fn with `#[tool(id = ..., toolkit = ..., pegboard_units = ..., inputs = [...])]`, then list the annotated fns in a crate-level `upeg_plugin! { toolkit: \"...\", tools: [...] }` call, which generates the `manifest` and per-tool export wrappers. See `examples/plugins/greet/`, especially `examples/plugins/greet/Cargo.toml` and `examples/plugins/greet/src/lib.rs`. Under the hood the macros still build the same typed `PluginManifest` and `PluginToolDecl` DTOs (including optional `input_spec` and `output_spec`) that `upeg-plugin-api` exposes for manual authoring, and upeg reads the emitted manifest data instead of asking authors to hand-write a Toolkit TOML file for the plugin.

## MCP upstream configs

MCP upstream server_configs live outside Toolkit TOML under the MCP config directory. See `examples/mcp-imports/local.toml` for a self-hosted upeg MCP server and `examples/mcp-imports/github.toml` for a GitHub upstream. The filename stem becomes the namespace: `local.toml` registers upstream tools as `local.<original_id>`, while `github.toml` registers them as `github.<original_id>`.

## Validation commands

Validate authored Toolkit TOML with the CLI before relying on editor schema feedback. Contributors should regenerate the committed schema and generated guide after changing typed metadata or manifest structs.

### Validate one Toolkit TOML file

```bash
upeg tool validate ~/.upeg/toolkits/demo.toml
```

### Validate every Toolkit TOML file in a directory

```bash
upeg toolkit validate               # defaults to ~/.upeg/toolkits (or $UPEG_TOOLKITS_DIR)
upeg toolkit validate ~/.upeg/toolkits
```

### Regenerate committed schema and guide artifacts

```bash
just toolkit-schema
```

## Troubleshooting common errors

Most manifest errors are deterministic validation failures. Fix the first reported error, rerun `upeg tool validate`, then reload or recopy the manifest.

| Problem | Fix |
| --- | --- |
| retired `category` or `cat` field | Replace the field with `tags` on the Toolkit or tool entry. |
| `pegboard_units` is required | Set `pegboard_units = "U1"`, `"U2"`, or `"U2T"` on each runtime tool. |
| `invoker` is required | Set an invoker explicitly, or add `steps` to infer `Chain`. |
| missing invoker-specific field | Add the required field for the selected invoker, such as `command` for `External`, `url` for `Http`, `prompt` for `Llm`, or `wasm_path` for `Wasm`. `cwd`, `env`, and `timeout_ms` belong to `External` only, `timeout_ms` must be greater than zero, and every `env` entry needs a non-empty `name`. |
| inline secret field | Remove `credentials[].value`, `credentials[].secret_value`, or `credentials[].literal_secret`; store the secret in env or keychain and keep only the reference in TOML. |
| unknown surface or invoker | Use supported surfaces `cli`, `tui`, `desktop`, `pwa`, `ext`, `mcp`, or `http`, and supported runtime invokers `External`, `Http`, `Embed`, `Chain`, `Llm`, or `Wasm`. |
| chain cycle | Keep `connections` acyclic. Model loops with an explicit loop-capable tool instead of cyclic Chain edges. |
| invalid `inputs` | Declare each input with a canonical `name`, closed `type`, and non-empty `options` choices when using `options` or `multi_options`. A `default` must match the input type: a number for `number` / `integer`, a string for the text-shaped types. |
| invalid `outputs` | Declare each output with a canonical `name`, closed `type`, non-empty choices for `options` / `multi_options`, and a canonical `url` for `embedded_view`. |
