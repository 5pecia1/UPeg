use crate::manifest_examples::{
    CHAIN_EXAMPLES, CREDENTIAL_EXAMPLES, INPUT_EXAMPLES, INVOKER_EXAMPLES, QUICK_START_EXAMPLES,
    VALIDATION_EXAMPLES,
};

use super::render::join_code;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SectionKind {
    WhoThisIsFor,
    QuickStart,
    WhereUPegLoads,
    ToolkitStructure,
    FieldReference,
    OutputContracts,
    InvokerFields,
    InputRules,
    CredentialsAndSecrets,
    JsonSchemaValidation,
    WasmPlugins,
    McpUpstream,
    ValidationCommands,
    Troubleshooting,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ManifestDocSection {
    pub(crate) kind: SectionKind,
    pub(crate) title: &'static str,
    pub(crate) body: &'static str,
    pub(crate) examples: &'static [ManifestExample],
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ManifestExample {
    pub(crate) title: &'static str,
    pub(crate) language: &'static str,
    pub(crate) body: &'static str,
}

pub(crate) const GUIDE_SECTIONS: &[ManifestDocSection] = &[
    ManifestDocSection {
        kind: SectionKind::WhoThisIsFor,
        title: "Who this is for",
        body: "Use this guide when you want to expose local commands, HTTP calls, chains, LLM prompts, WASM modules, embedded web views, or upstream MCP tools through upeg surfaces without adding built-in Rust tools.",
        examples: &[],
    },
    ManifestDocSection {
        kind: SectionKind::QuickStart,
        title: "Quick start: one External tool in TOML",
        body: "Create one Toolkit TOML file, give it a stable root `id`, and add at least one `[[tools]]` entry. This minimal example is based on `examples/tools/echo-bracketed.toml`; copy it to `~/.upeg/toolkits/demo.toml`, then validate it before calling the tool.",
        examples: QUICK_START_EXAMPLES,
    },
    ManifestDocSection {
        kind: SectionKind::WhereUPegLoads,
        title: "Where upeg loads external manifests from",
        body: "Toolkit TOML files load from `~/.upeg/toolkits/{toolkit_id}.toml` unless `UPEG_TOOLKITS_DIR` points elsewhere. WASM plugin binaries load from `~/.upeg/wasm/*.wasm` unless `UPEG_WASM_DIR` is set. MCP upstream configs load from `~/.upeg/mcp-imports/*.toml` unless `UPEG_MCP_IMPORTS_DIR` is set. The `examples/` tree contains copyable fixtures for each source type, including `examples/tools/http-mock-echo.toml`, `examples/tools/llm-echo.toml`, `examples/tools/embed-mdn.toml`, and `examples/tools/chain-md5-then-uppercase.toml`.",
        examples: &[],
    },
    ManifestDocSection {
        kind: SectionKind::ToolkitStructure,
        title: "Toolkit TOML structure",
        body: "The root object describes a non-callable Toolkit namespace. Each `[[tools]]` entry declares one callable tool id local to that Toolkit. The loader registers the tool as `<toolkit>.<tool>` and rejects duplicated toolkit prefixes such as `id = \\\"demo.echo\\\"` inside `[[tools]]`.",
        examples: CHAIN_EXAMPLES,
    },
    ManifestDocSection {
        kind: SectionKind::FieldReference,
        title: "Field reference",
        body: "The table below documents the user-authored Toolkit TOML shape. It intentionally describes Toolkit TOML input and source-specific adapter fields, not the runtime-only internal manifest structs.",
        examples: &[],
    },
    ManifestDocSection {
        kind: SectionKind::OutputContracts,
        title: "Tool output contracts",
        body: "Tool outputs are canonical structured `ToolResult` values, not raw process stdout. A successful result is a JSON envelope with `ok=true`, `primary_output_id`, and ordered `outputs[]` entries. Each output entry carries `id`, optional `label`, `kind`, and typed `value`; `primary_output_id` must reference one declared `outputs[].name` whenever outputs are non-empty and must be omitted for action-only tools.\n\nCLI rendering is a presentation policy over the same canonical result: the default `upeg call` mode prints only the primary output value, `--json` prints the canonical success/error envelope, `--field <id>` prints one output value, and `--pretty` prints labeled rows. Diagnostics, progress, and logs belong on stderr so stdout remains reserved for the selected result representation.\n\nHTTP, daemon, and FRB transports return canonical JSON envelopes. MCP `tools/call` uses the same canonical success envelope as `structuredContent`; the MCP text `content` is only a display fallback containing the primary output text. TUI, Desktop, PWA, Chrome extension, and Controlled Embed output surfaces render labels and values from canonical output entries, with richer modal/card treatment allowed only as presentation.\n\nA canonical failure is `ok=false` plus an `error` object with `code`, `message`, and an optional structured `details` value. `External` fills `details` with `{ \"exit_code\": <int|null>, \"stdout\": \"…\", \"stderr\": \"…\" }`, adding `signal` when the child was killed and `timed_out: true` when `timeout_ms` elapsed. Both streams are captured and capped, so a failing `cargo`, `clippy`, or `flutter analyze` hands back the diagnostics it wrote to stdout instead of only a summary line. Non-JSON surfaces print `message`, then the labeled `stderr` and `stdout` blocks; `--json`, the HTTP body, MCP `structuredContent`, and the FRB bridge carry the whole `details` object.",
        examples: &[],
    },
    ManifestDocSection {
        kind: SectionKind::InvokerFields,
        title: "Invoker-specific fields",
        body: "The loader preserves the TOML shape, infers `Chain` only when `steps` is present, then validates each runtime invoker's required fields before registering dispatchers.\n\nThe `External` invoker also controls how the child process runs. `cwd` sets its working directory; a relative value resolves against the directory holding the manifest. With no declared `cwd`, a Project Manifest (`upeg.toml`) tool runs in the manifest's own directory, and a caller-supplied working directory is honored only when it sits inside that directory — a toolkit-directory tool has no project to belong to, so there the caller's directory wins outright. `env` lists plain, non-secret environment variables and is applied before `credentials`, so a credential wins on a name collision. `timeout_ms` is a wall-clock budget after which upeg terminates the child's whole process group; there is no default, because a long build or test run must stay legal. stdin is always `/dev/null`, so an interactive prompt fails fast instead of hanging the calling surface. `color` decides what the child is told about color support: the default (`\"inherit\"`) leaves it looking at a captured pipe, so it turns color off by itself, while `color = \"force\"` sets `CLICOLOR_FORCE` and `FORCE_COLOR`, unsets an inherited `NO_COLOR` (which the same convention ranks above both), and sets `TERM` only when upeg's own environment has none. That is pure environment — no pty, identical on Unix and Windows — and it applies before `env`, so an explicit variable overrides it. It is the convention rather than a terminal: a program that decides on `isatty(3)` alone, like `git` or `ls`, ignores all of it and needs its own flag (`git -c color.ui=always`). An unknown value is rejected at load time rather than silently ignored at dispatch time.\n\n`pty = true` is the other answer to that same question: it opens a real pseudoterminal on Unix and connects the child's stdout and stderr to it, so `isatty(3)` is true and no flag is needed, and it implies `color = \"force\"` as well unless the tool declares `color` itself, in which case the declaration wins. A terminal has one buffer, so both streams come back merged in `stdout` and `stderr` is empty; stdin stays `/dev/null` because upeg is never the human on the other side. Timeouts and escaped-descendant cleanup are unchanged. A host with no pty (Windows, wasm) skips that one tool at load time — recorded with its reason, never silently downgraded to pipes — and every other tool in the same manifest still loads.\n\nA run can also be cancelled while it is still going. Three surfaces install a cancellation scope today: the HTTP streaming route cancels when its client hangs up, the TUI cancels on `Esc` while a run is on screen, and the FRB bridge cancels on `cancel_dispatch(run_id)` from Dart. Inside any of them `External` reads the token on every tick of its wait loop, terminates the child's process group, and answers with `error.code = \"cancelled\"` and `details.cancelled = true` alongside whatever the child had already written. Cancellation is a request, not a guarantee — a tool that never yields runs to completion and still returns one envelope.\n\nA long-running command does not have to be silent until it exits. While the child runs, `External` also forwards whole lines of stdout and stderr to whichever surface asked for them: `upeg call` mirrors both to the terminal's stderr (stdout stays reserved for the final result, and `--json`/`--field` stay silent), the HTTP surface streams them as NDJSON from `POST /v1/tools/{id}/stream`, and MCP `tools/call` sends them as `notifications/message` log frames. The final envelope, the capture limits, and the timeout behaviour are unchanged; a surface that cannot consume progress pays nothing and sees exactly what it saw before.\n\n`args_template` tokens are small templates: `{key}` substitutes anywhere inside a token (`--manifest-path={path}`, `-p{crate}`) and `{{` and `}}` are literal braces. Every `key` must name a declared `inputs` field — the only exception is `input`, which the `Chain` invoker hands to every step — so a typo is a load error rather than a silently missing argument. A token that is nothing but `{key}` for an optional input with neither a value nor a `default` is dropped from the arg list; every other token keeps its position and renders the absent placeholder as an empty string, because dropping `{dir}/build` would shift every argument after it. A successful command's stdout is the primary output; when the tool declares no `outputs` of its own and the command also wrote to stderr, that text is surfaced as a secondary `stderr` output.",
        examples: INVOKER_EXAMPLES,
    },
    ManifestDocSection {
        kind: SectionKind::InputRules,
        title: "Input rules",
        body: "`inputs` is an optional TOML array of typed fields. Omit it or set `inputs = []` for tools with no inputs. Each field uses the closed upeg input type vocabulary: `string`, `number`, `integer`, `boolean`, `options`, `multi_options`, `markdown`, `json`, `datetime`, `file_path`, `url`, and `file`. Choice fields (`options` and `multi_options`) must declare non-empty `options` entries.",
        examples: INPUT_EXAMPLES,
    },
    ManifestDocSection {
        kind: SectionKind::CredentialsAndSecrets,
        title: "Credentials and secret references",
        body: "Credential entries name secret values resolved from environment variables or OS-backed references at execution time. Manifests store references and metadata only; inline secret fields such as `credentials[].value`, `credentials[].secret_value`, and `credentials[].literal_secret` are rejected.",
        examples: CREDENTIAL_EXAMPLES,
    },
    ManifestDocSection {
        kind: SectionKind::JsonSchemaValidation,
        title: "JSON Schema and editor validation",
        body: "`fixtures/toolkit.schema.json` is generated from the same Rust Toolkit TOML structs that serde deserializes. Use it for editor validation of the TOML-to-JSON shape. `upeg tool validate` is authoritative for semantic validation such as required runtime fields, invoker compatibility, chain acyclicity, and typed input rules. The JSON Schema does not prove absence of chain cycles, credential existence, executable availability, or URL reachability.",
        examples: &[],
    },
    ManifestDocSection {
        kind: SectionKind::WasmPlugins,
        title: "WASM plugin manifests",
        body: "Declarative Toolkit TOML can reference a WASM module with `invoker = \\\"Wasm\\\"` and `wasm_path`, but normal WASM plugins declare their manifest from code. Author tools with `upeg_plugin_macros`: annotate a plain typed Rust fn with `#[tool(id = ..., toolkit = ..., pegboard_units = ..., inputs = [...])]`, then list the annotated fns in a crate-level `upeg_plugin! { toolkit: \\\"...\\\", tools: [...] }` call, which generates the `manifest` and per-tool export wrappers. See `examples/plugins/greet/`, especially `examples/plugins/greet/Cargo.toml` and `examples/plugins/greet/src/lib.rs`. Under the hood the macros still build the same typed `PluginManifest` and `PluginToolDecl` DTOs (including optional `input_spec` and `output_spec`) that `upeg-plugin-api` exposes for manual authoring, and upeg reads the emitted manifest data instead of asking authors to hand-write a Toolkit TOML file for the plugin.",
        examples: &[],
    },
    ManifestDocSection {
        kind: SectionKind::McpUpstream,
        title: "MCP upstream configs",
        body: "MCP upstream server_configs live outside Toolkit TOML under the MCP config directory. See `examples/mcp-imports/local.toml` for a self-hosted upeg MCP server and `examples/mcp-imports/github.toml` for a GitHub upstream. The filename stem becomes the namespace: `local.toml` registers upstream tools as `local.<original_id>`, while `github.toml` registers them as `github.<original_id>`.",
        examples: &[],
    },
    ManifestDocSection {
        kind: SectionKind::ValidationCommands,
        title: "Validation commands",
        body: "Validate authored Toolkit TOML with the CLI before relying on editor schema feedback. Contributors should regenerate the committed schema and generated guide after changing typed metadata or manifest structs.",
        examples: VALIDATION_EXAMPLES,
    },
    ManifestDocSection {
        kind: SectionKind::Troubleshooting,
        title: "Troubleshooting common errors",
        body: "Most manifest errors are deterministic validation failures. Fix the first reported error, rerun `upeg tool validate`, then reload or recopy the manifest.",
        examples: &[],
    },
];

#[derive(Debug, Clone, Copy)]
pub(super) struct InvokerDoc {
    pub(super) name: &'static str,
    pub(super) purpose: &'static str,
    pub(super) required_fields: &'static [&'static str],
    pub(super) optional_fields: &'static [&'static str],
}

impl InvokerDoc {
    pub(crate) fn from_metadata(spec: &crate::invoker_metadata::RuntimeInvokerSpec) -> Self {
        Self {
            name: spec.name,
            purpose: spec.purpose,
            required_fields: spec.required_fields,
            optional_fields: spec.optional_fields,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TroubleshootingEntry {
    pub(super) problem: &'static str,
    pub(super) fix: &'static str,
}

impl TroubleshootingEntry {
    pub(crate) fn from_metadata(entry: &crate::error_metadata::TroubleshootingEntry) -> Self {
        Self {
            problem: entry.problem,
            fix: entry.fix,
        }
    }
}

pub(crate) fn render_invoker_table(markdown: &mut String) {
    markdown.push_str("| Invoker | Purpose | Required fields | Common optional fields |\n");
    markdown.push_str("| --- | --- | --- | --- |\n");
    for spec in crate::invoker_metadata::RUNTIME_INVOKERS {
        let doc = InvokerDoc::from_metadata(spec);
        markdown.push_str("| `");
        markdown.push_str(doc.name);
        markdown.push_str("` | ");
        markdown.push_str(doc.purpose);
        markdown.push_str(" | ");
        markdown.push_str(&join_code(doc.required_fields));
        markdown.push_str(" | ");
        markdown.push_str(&join_code(doc.optional_fields));
        markdown.push_str(" |\n");
    }
    markdown.push('\n');
}

pub(crate) fn render_troubleshooting_table(markdown: &mut String) {
    markdown.push_str("| Problem | Fix |\n");
    markdown.push_str("| --- | --- |\n");
    for entry in crate::error_metadata::TOOLKIT_TROUBLESHOOTING {
        let doc = TroubleshootingEntry::from_metadata(entry);
        markdown.push_str("| ");
        markdown.push_str(doc.problem);
        markdown.push_str(" | ");
        markdown.push_str(doc.fix);
        markdown.push_str(" |\n");
    }
    markdown.push('\n');
}
