pub use boards::BoardEntryToml;
pub use presentation::PresentationToml;

use schemars::JsonSchema;
use serde::Deserialize;
use upeg_core::{
    ChoiceOption, ChoiceSpec, FieldConstraints, IO_TYPE_LIST, InputFieldSpec, InputName,
    OutputFieldSpec, OutputKind,
};

mod boards;
mod input;
mod presentation;
pub(crate) fn chain_step_key(position: usize, id: Option<&str>) -> String {
    id.map(str::trim)
        .filter(|s| !s.is_empty())
        .map_or_else(|| format!("step{}", position + 1), str::to_string)
}
/// Root of the Toolkit TOML manifest loaded from `~/.upeg/toolkits/*.toml`.
///
/// This is the top-level shape that TOML authors write. The `id`, `tools`,
/// and toolkit-level metadata (`tags`, `display_label`, `description`)
/// propagate to every tool entry registered under this namespace.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 1)
)]
pub struct ToolkitToml {
    /// Stable toolkit namespace. Tool entries are registered as `<toolkit>.<tool>`.
    pub id: String,
    /// Toolkit-level tags inherited by every tool unless already present.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Human-readable toolkit label for UI surfaces.
    #[serde(default)]
    pub display_label: Option<String>,
    /// Human-readable toolkit description.
    #[serde(default)]
    pub description: Option<String>,
    /// Tools declared inside this toolkit manifest.
    #[serde(default)]
    #[schemars(length(min = 1), required)]
    pub tools: Vec<ToolEntryToml>,
    /// Boards this manifest declares. Project Manifests (`upeg.toml`)
    /// only — the loader rejects the field on a toolkits-directory
    /// manifest, which has no project to scope a board to.
    #[serde(default)]
    pub boards: Vec<BoardEntryToml>,
}
/// One tool entry inside a toolkit manifest's `[[tools]]` array.
///
/// This shape contains every user-authored field for a single tool: base
/// metadata (`id`, `tags`, `display_label`, `description`), I/O schema,
/// runtime invoker parameters, chain step declarations, and adapter-specific
/// references such as credentials and selector bindings.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 2)
)]
pub struct ToolEntryToml {
    /// Local tool id. The loader prefixes it with the toolkit id.
    pub id: String,
    /// Tool-specific tags appended to toolkit-level tags.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Human-readable tool label for UI surfaces.
    #[serde(default)]
    pub display_label: Option<String>,
    /// Human-readable tool description.
    #[serde(default)]
    pub description: Option<String>,
    /// Typed input fields accepted by this tool. Omit for tools with no inputs.
    #[serde(default)]
    pub inputs: Vec<InputFieldToml>,
    /// Typed output fields produced by this tool. Omit for action-only tools.
    #[serde(default)]
    pub outputs: Vec<OutputFieldToml>,
    /// Canonical output field id used for the default CLI result, MCP text
    /// fallback, and UI primary emphasis. Required when `outputs` is non-empty,
    /// and omitted when `outputs` is empty.
    #[serde(default)]
    pub primary_output_id: Option<String>,
    /// Optional author-declared side-effect classification.
    #[serde(default)]
    pub effect: Option<String>,
    /// Optional structured result presentation metadata.
    #[serde(default)]
    pub presentation: Option<PresentationToml>,
    /// UI rendering hint only, such as `Inline`, `Modal`, or `Embed`; runtime
    /// adapter selection is controlled by `invoker`.
    #[serde(default)]
    pub pin: Option<String>,
    /// Pegboard size class. Loader validation requires `U1`, `U2`, or `U2T`.
    #[serde(default)]
    pub pegboard_units: Option<String>,
    /// Runtime adapter selector. `Embed` is the canonical runtime declaration
    /// for embedded sidecars; `steps` can infer `Chain`, while other runtime
    /// tools must set this explicitly.
    #[serde(default)]
    pub invoker: Option<String>,
    /// Surface ids where this tool should appear, such as `cli`, `tui`, or `http`.
    #[serde(default)]
    pub surfaces: Option<Vec<String>>,
    /// Board ids where UI surfaces should group this tool.
    #[serde(default)]
    pub boards: Option<Vec<String>>,
    /// External-invoker support: program to spawn (e.g. `"git"`).
    /// Honored only when `invoker = "External"`.
    #[serde(default)]
    pub command: Option<String>,
    /// Arg list template for the spawned command. `{key}` placeholders
    /// are substituted anywhere inside a token (`--manifest-path={path}`);
    /// `{{` / `}}` escape a literal brace. Every `key` must name a
    /// declared `inputs` field (or `input`, which the `Chain` invoker
    /// supplies to every step) — a placeholder naming nothing else is a
    /// load error rather than a silently empty argument. A token that is
    /// nothing but `{key}` for an optional input with neither a value
    /// nor a `default` is dropped from the arg list; every other token
    /// keeps its position and renders the absent placeholder as `""`.
    #[serde(default)]
    pub args_template: Option<Vec<String>>,
    /// Working directory for the spawned command. A relative path
    /// resolves against the directory holding this manifest file.
    /// Honored only when `invoker = "External"`. When omitted, a
    /// Project Manifest (`upeg.toml`) tool runs in the manifest's own
    /// directory and a caller-supplied working directory is honored
    /// only if it sits inside that directory.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Plain (non-secret) environment variables handed to the spawned
    /// command. Secrets belong in `credentials`, which is applied after
    /// this list and therefore wins on a name collision.
    #[serde(default)]
    pub env: Option<Vec<KeyValueToml>>,
    /// Wall-clock budget for the spawned command, in milliseconds. When
    /// it elapses upeg terminates the child's whole process group and
    /// fails with `details.timed_out = true`. Omit for no limit.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Whether the spawned command is told color is supported.
    /// `"inherit"` (the default) leaves it seeing a captured pipe and
    /// turning color off; `"force"` sets `CLICOLOR_FORCE`/`FORCE_COLOR`,
    /// unsets `NO_COLOR` (and sets `TERM` when the host has none). It is
    /// the environment convention, not a pty: a program that decides on
    /// `isatty(3)` alone needs its own flag. Honored only when
    /// `invoker = "External"`.
    #[serde(default)]
    pub color: Option<String>,
    /// Whether the spawned command gets a real terminal instead of two
    /// pipes. `true` opens a pseudoterminal on Unix and connects the
    /// child's stdout and stderr to it, so `isatty(3)` is true and a
    /// program that decides on it alone — `git`, `ls`, `grep` — emits
    /// its terminal output. The two streams arrive **merged** in one
    /// captured `stdout`, since a terminal has only one buffer, and
    /// `pty = true` implies `color = "force"` unless `color` is declared
    /// explicitly. stdin stays `/dev/null`. Rejected at load time on a
    /// host with no pty (Windows, wasm). Honored only when
    /// `invoker = "External"`.
    #[serde(default)]
    pub pty: Option<bool>,
    /// Chain steps. This is the single PRD v2.1 manifest shape for
    /// expression flow, branching, approvals, and connected node execution.
    #[serde(default)]
    pub steps: Option<Vec<ChainStepToml>>,
    /// Directed edges between chain steps. Steps without incoming connections
    /// receive the chain input; connected steps receive upstream outputs.
    #[serde(default)]
    pub connections: Option<Vec<ChainConnectionToml>>,
    /// Surface labels allowed to satisfy this chain's `requires_approval`
    /// steps. An approval (`approve = true` / `_upeg.approvedSteps`) is
    /// honored only when the call's `_upeg.surface` is in this list;
    /// every other surface is refused with `approval_denied_for_surface`.
    /// Defaults to the three surfaces a person is sitting at — `cli`,
    /// `tui`, `desktop` — each of which ships a real approval gesture
    /// (`upeg call <chain> -a approve=true`, the TUI's confirm dialog,
    /// the desktop's confirm dialog). Every other surface must be named
    /// explicitly. Must overlap this tool's `surfaces`, or the gated
    /// step could never be approved by anyone who can reach it.
    #[serde(default)]
    pub approval_surfaces: Option<Vec<String>>,
    /// Optional final output expression for Chain tools.
    #[serde(default)]
    pub output: Option<String>,
    /// HTTP invoker URL template.
    #[serde(default)]
    pub url: Option<String>,
    /// HTTP method (`GET`/`POST`/...). Defaults to POST when a body is present,
    /// GET otherwise.
    #[serde(default)]
    pub method: Option<String>,
    /// HTTP headers as name/value templates.
    #[serde(default)]
    pub headers: Option<Vec<KeyValueToml>>,
    /// HTTP request body template.
    #[serde(default)]
    pub body: Option<String>,
    /// LLM prompt template. `{{input}}`, `{{input.key}}`, and
    /// `{{credential.name}}` expressions are resolved at execution time.
    #[serde(default)]
    pub prompt: Option<String>,
    /// LLM provider. `echo` is deterministic/local; `tool:<id>` delegates the
    /// rendered prompt into another Tool; `openai` calls an OpenAI-compatible
    /// `/chat/completions` HTTP endpoint (see `base_url`).
    #[serde(default)]
    pub provider: Option<String>,
    /// Provider model name. Required when `provider = "openai"`.
    #[serde(default)]
    pub model: Option<String>,
    /// OpenAI-compatible provider base URL, e.g. `https://api.openai.com/v1`.
    /// Only meaningful when `provider = "openai"`. Defaults to the loader's
    /// built-in OpenAI endpoint when omitted; set this to point at a
    /// self-hosted OpenAI-compatible server instead.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Primary credential name/key for HTTP/LLM templates. Reuses the matching
    /// declared entry from `credentials` when present; otherwise resolves the
    /// default env fallback `UPEG_CREDENTIAL_<NAME>`. Never inline a secret.
    /// For `provider = "openai"`, this is the credential resolved as the
    /// `Authorization: Bearer` API key.
    #[serde(default)]
    pub credential: Option<String>,
    /// Named credential references from env, keychain, or adapter sources; no
    /// secret values are persisted in toolkit manifests.
    #[serde(default)]
    pub credentials: Option<Vec<CredentialRefToml>>,
    /// WASM module path for declarative WASM adapter diagnostics/host loading.
    #[serde(default)]
    pub wasm_path: Option<String>,
    /// Trigger declarations. Runtime adapters normalize all sources into the
    /// same Tool-dispatch event contract.
    #[serde(default)]
    pub triggers: Option<Vec<TriggerToml>>,
    /// GUI sidecar URL registered when present and non-empty. Recommended together with
    /// `invoker = "Embed"` and `pin = "Embed"`, but it is not gated
    /// only by `pin`.
    #[serde(default)]
    pub embed_url: Option<String>,
    /// Controlled Embed browser settings for User-Agent and viewport overrides.
    #[serde(default)]
    pub controlled_embed: Option<ControlledEmbedToml>,
}

/// Fully-qualified tool TOML shape after toolkit-level defaults are applied.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct ToolToml {
    /// Fully-qualified tool id in `<toolkit>.<tool>` form.
    pub id: String,
    /// Toolkit namespace that owns this tool.
    pub toolkit: String,
    /// Effective tags after merging toolkit-level and tool-level tags.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Human-readable tool label for UI surfaces.
    #[serde(default)]
    pub display_label: Option<String>,
    /// Human-readable tool description.
    #[serde(default)]
    pub description: Option<String>,
    /// Typed input fields accepted by this tool. Omit for tools with no inputs.
    #[serde(default)]
    pub inputs: Vec<InputFieldToml>,
    /// Typed output fields produced by this tool. Omit for action-only tools.
    #[serde(default)]
    pub outputs: Vec<OutputFieldToml>,
    /// Canonical output field id used for the default CLI result, MCP text
    /// fallback, and UI primary emphasis. Required when `outputs` is non-empty,
    /// and omitted when `outputs` is empty.
    #[serde(default)]
    pub primary_output_id: Option<String>,
    #[serde(default)]
    pub effect: Option<String>,
    #[serde(default)]
    pub presentation: Option<PresentationToml>,
    /// Pin kind hint for UI surfaces, such as `Inline`, `Modal`, or `Embed`.
    #[serde(default)]
    pub pin: Option<String>,
    /// Pegboard size class. Loader validation requires `U1`, `U2`, or `U2T`.
    #[serde(default)]
    pub pegboard_units: Option<String>,
    /// Runtime invoker. `steps` can infer `Chain`; other runtime tools must set it.
    #[serde(default)]
    pub invoker: Option<String>,
    /// Surface ids where this tool should appear, such as `cli`, `tui`, or `http`.
    #[serde(default)]
    pub surfaces: Option<Vec<String>>,
    /// Board ids where UI surfaces should group this tool.
    #[serde(default)]
    pub boards: Option<Vec<String>>,
    /// External-invoker support: program to spawn (e.g. `"git"`).
    /// Honored only when `invoker = "External"`.
    #[serde(default)]
    pub command: Option<String>,
    /// Arg list template for the spawned command. `{key}` placeholders
    /// are substituted anywhere inside a token (`--manifest-path={path}`);
    /// `{{` / `}}` escape a literal brace. Every `key` must name a
    /// declared `inputs` field (or `input`, which the `Chain` invoker
    /// supplies to every step) — a placeholder naming nothing else is a
    /// load error rather than a silently empty argument. A token that is
    /// nothing but `{key}` for an optional input with neither a value
    /// nor a `default` is dropped from the arg list; every other token
    /// keeps its position and renders the absent placeholder as `""`.
    #[serde(default)]
    pub args_template: Option<Vec<String>>,
    /// Working directory for the spawned command. A relative path
    /// resolves against the directory holding this manifest file.
    /// Honored only when `invoker = "External"`. When omitted, a
    /// Project Manifest (`upeg.toml`) tool runs in the manifest's own
    /// directory and a caller-supplied working directory is honored
    /// only if it sits inside that directory.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Plain (non-secret) environment variables handed to the spawned
    /// command. Secrets belong in `credentials`, which is applied after
    /// this list and therefore wins on a name collision.
    #[serde(default)]
    pub env: Option<Vec<KeyValueToml>>,
    /// Wall-clock budget for the spawned command, in milliseconds. When
    /// it elapses upeg terminates the child's whole process group and
    /// fails with `details.timed_out = true`. Omit for no limit.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Whether the spawned command is told color is supported.
    /// `"inherit"` (the default) leaves it seeing a captured pipe and
    /// turning color off; `"force"` sets `CLICOLOR_FORCE`/`FORCE_COLOR`,
    /// unsets `NO_COLOR` (and sets `TERM` when the host has none). It is
    /// the environment convention, not a pty: a program that decides on
    /// `isatty(3)` alone needs its own flag. Honored only when
    /// `invoker = "External"`.
    #[serde(default)]
    pub color: Option<String>,
    /// Whether the spawned command gets a real terminal instead of two
    /// pipes. `true` opens a pseudoterminal on Unix and connects the
    /// child's stdout and stderr to it, so `isatty(3)` is true and a
    /// program that decides on it alone — `git`, `ls`, `grep` — emits
    /// its terminal output. The two streams arrive **merged** in one
    /// captured `stdout`, since a terminal has only one buffer, and
    /// `pty = true` implies `color = "force"` unless `color` is declared
    /// explicitly. stdin stays `/dev/null`. Rejected at load time on a
    /// host with no pty (Windows, wasm). Honored only when
    /// `invoker = "External"`.
    #[serde(default)]
    pub pty: Option<bool>,
    /// Chain steps. This is the single PRD v2.1 manifest shape for
    /// expression flow, branching, approvals, and connected node execution.
    #[serde(default)]
    pub steps: Option<Vec<ChainStepToml>>,
    /// Directed edges between chain steps. Steps without incoming connections
    /// receive the chain input; connected steps receive upstream outputs.
    #[serde(default)]
    pub connections: Option<Vec<ChainConnectionToml>>,
    /// Surface labels allowed to satisfy this chain's `requires_approval`
    /// steps. An approval (`approve = true` / `_upeg.approvedSteps`) is
    /// honored only when the call's `_upeg.surface` is in this list;
    /// every other surface is refused with `approval_denied_for_surface`.
    /// Defaults to the three surfaces a person is sitting at — `cli`,
    /// `tui`, `desktop` — each of which ships a real approval gesture
    /// (`upeg call <chain> -a approve=true`, the TUI's confirm dialog,
    /// the desktop's confirm dialog). Every other surface must be named
    /// explicitly. Must overlap this tool's `surfaces`, or the gated
    /// step could never be approved by anyone who can reach it.
    #[serde(default)]
    pub approval_surfaces: Option<Vec<String>>,
    /// Optional final output expression for Chain tools.
    #[serde(default)]
    pub output: Option<String>,
    /// HTTP invoker URL template.
    #[serde(default)]
    pub url: Option<String>,
    /// HTTP method (`GET`/`POST`/...). Defaults to POST when a body is present,
    /// GET otherwise.
    #[serde(default)]
    pub method: Option<String>,
    /// HTTP headers as name/value templates.
    #[serde(default)]
    pub headers: Option<Vec<KeyValueToml>>,
    /// HTTP request body template.
    #[serde(default)]
    pub body: Option<String>,
    /// LLM prompt template. `{{input}}`, `{{input.key}}`, and
    /// `{{credential.name}}` expressions are resolved at execution time.
    #[serde(default)]
    pub prompt: Option<String>,
    /// LLM provider. `echo` is deterministic/local; `tool:<id>` delegates the
    /// rendered prompt into another Tool; `openai` calls an OpenAI-compatible
    /// `/chat/completions` HTTP endpoint (see `base_url`).
    #[serde(default)]
    pub provider: Option<String>,
    /// Provider model name. Required when `provider = "openai"`.
    #[serde(default)]
    pub model: Option<String>,
    /// OpenAI-compatible provider base URL. Only meaningful when
    /// `provider = "openai"`. Defaults to the loader's built-in OpenAI
    /// endpoint when omitted.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Primary credential reference for HTTP/LLM adapters. For
    /// `provider = "openai"`, this is the credential resolved as the
    /// `Authorization: Bearer` API key.
    #[serde(default)]
    pub credential: Option<String>,
    /// Named credential references used by adapters. Values resolve from
    /// environment/OS-reference adapters and are never persisted by upeg.
    #[serde(default)]
    pub credentials: Option<Vec<CredentialRefToml>>,
    /// WASM module path for declarative WASM adapter diagnostics/host loading.
    #[serde(default)]
    pub wasm_path: Option<String>,
    /// Trigger declarations. Runtime adapters normalize all sources into the
    /// same Tool-dispatch event contract.
    #[serde(default)]
    pub triggers: Option<Vec<TriggerToml>>,
    /// Embed Tool (PRD §5.5): the URL the desktop UI's sandboxed
    /// `<iframe>` loads when this tool is opened in the expanded modal.
    /// Honored only when `pin = "Embed"`. Registered into the
    /// `upeg_runtime::embed_url_for(id)` sidecar at load time.
    #[serde(default)]
    pub embed_url: Option<String>,
    /// Controlled Embed browser settings for User-Agent and viewport overrides.
    /// Omit to preserve the call-site default behavior; set explicit values
    /// when a tool requires a specific browser identity or viewport.
    #[serde(default)]
    pub controlled_embed: Option<ControlledEmbedToml>,
}

/// One typed input field declared in Toolkit TOML.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 3)
)]
pub struct InputFieldToml {
    /// Canonical argument key, such as `input`, `path`, or `mode`.
    pub name: String,
    /// Closed input kind: `string`, `number`, `integer`, `boolean`, `options`,
    /// `multi_options`, `markdown`, `json`, `datetime`, `file_path`, `url`, or `file`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Optional human-readable label for form surfaces.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional help text for form surfaces.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether the caller must provide this input. Defaults to optional.
    #[serde(default)]
    pub required: bool,
    /// Value used when the caller omits this input. Lowered into the
    /// field's [`upeg_core::FieldConstraints`], so it also reaches the
    /// generated JSON Schema and every form surface, and the External
    /// invoker substitutes it into `args_template`.
    /// `number` / `integer` inputs take a number; `string`, `options`,
    /// `markdown`, `json`, `datetime`, `file_path`, and `url` take a
    /// string. Other types (including `boolean`) have no default slot
    /// and are rejected at load time.
    #[serde(default)]
    pub default: Option<InputDefaultToml>,
    /// Selectable choices for `type = "options"` or `type = "multi_options"`.
    #[serde(default)]
    pub options: Option<Vec<InputChoiceToml>>,
    /// Allowed file suffixes. Values are normalized to lowercase without a
    /// leading dot. An empty or omitted list allows every extension.
    #[serde(default)]
    pub extensions: Option<Vec<String>>,
    /// Maximum recursive file leaf count for `type = "file"`.
    #[serde(default)]
    #[schemars(range(min = 1, max = 100))]
    pub max_count: Option<u32>,
    /// Optional maximum byte length of each file leaf.
    #[serde(default)]
    pub max_file_bytes: Option<u64>,
    /// Optional maximum aggregate byte length across all file leaves.
    #[serde(default)]
    pub max_total_bytes: Option<u64>,
}

/// Default value declared on an input field.
///
/// The core's `FieldConstraints` has a numeric slot and a string slot
/// and nothing else, so this mirrors exactly what can be lowered.
/// `Boolean` is accepted by the parser only to reject it with a field-
/// specific error instead of an opaque "matched no variant" TOML error.
// `inline`: a scalar union has no field table to render, so keeping it
// out of `$defs` makes the generated `default` row read
// `boolean or number or string` instead of pointing at a named type
// with no documentation page behind it.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(untagged)]
#[schemars(inline)]
pub enum InputDefaultToml {
    /// TOML boolean. Rejected at load time — booleans have no
    /// constraint slot in the core input spec.
    Boolean(bool),
    /// TOML integer or float, for `type = "number"` / `"integer"`.
    Number(f64),
    /// TOML string, for the text-shaped input kinds.
    Text(String),
}

/// Largest magnitude an `f64` represents as an exact whole number.
/// Beyond it, "looks like an integer" stops being meaningful and the
/// plain `f64` rendering (`1e30`) is the honest one.
const MAX_EXACT_WHOLE_F64: f64 = 9_007_199_254_740_992.0;

/// Render a numeric default the way a command-line argument should look:
/// whole values lose the `.0` tail that `f64`'s `Display` would add, so
/// `default = 10` substitutes as `10` and not `10.0`.
pub(crate) fn format_number_default(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() <= MAX_EXACT_WHOLE_F64 {
        return format!("{value:.0}");
    }
    value.to_string()
}

/// One selectable choice for `options` and `multi_options` input fields.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 4)
)]
pub struct InputChoiceToml {
    /// Stable value sent in tool arguments when this choice is selected.
    pub value: String,
    /// Optional human-readable label for form surfaces.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional help text for this choice.
    #[serde(default)]
    pub description: Option<String>,
}

/// One typed output field declared in Toolkit TOML.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 5)
)]
pub struct OutputFieldToml {
    /// Canonical output key, such as `result`, `count`, or `view`.
    pub name: String,
    /// Closed output kind: `string`, `number`, `integer`, `boolean`, `options`,
    /// `multi_options`, `markdown`, `json`, `datetime`, `file_path`, `url`,
    /// `file`, or `embedded_view`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Optional human-readable label for rendering surfaces.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional help text for rendering surfaces.
    #[serde(default)]
    pub description: Option<String>,
    /// Selectable choices for `type = "options"` or `type = "multi_options"`.
    #[serde(default)]
    pub options: Option<Vec<InputChoiceToml>>,
    /// URL for `type = "embedded_view"` outputs.
    #[serde(default)]
    pub url: Option<String>,
}

impl TryFrom<InputFieldToml> for InputFieldSpec {
    type Error = crate::LoadError;

    fn try_from(field: InputFieldToml) -> Result<Self, Self::Error> {
        let InputFieldToml {
            name,
            kind,
            label,
            description,
            required,
            default,
            options,
            extensions,
            max_count,
            max_file_bytes,
            max_total_bytes,
        } = field;
        let input_kind = input::InputKindToml {
            kind,
            options,
            file_policy: input::FileInputPolicyToml {
                extensions,
                max_count,
                max_file_bytes,
                max_total_bytes,
            },
        }
        .into_core(&name)?;
        let constraints = input::field_constraints_from_default(&name, &input_kind, default)?;

        Ok(Self::with_constraints(
            InputName::new(name)?,
            label,
            description,
            required,
            input_kind,
            constraints,
        )?)
    }
}

impl TryFrom<InputChoiceToml> for ChoiceOption {
    type Error = crate::LoadError;

    fn try_from(choice: InputChoiceToml) -> Result<Self, Self::Error> {
        Ok(Self::new(choice.value, choice.label, choice.description)?)
    }
}

impl TryFrom<OutputFieldToml> for OutputFieldSpec {
    type Error = crate::LoadError;

    fn try_from(field: OutputFieldToml) -> Result<Self, Self::Error> {
        let OutputFieldToml {
            name,
            kind,
            label,
            description,
            options,
            url,
        } = field;
        let output_kind = output_kind_from_toml(&name, &kind, options, url)?;

        Ok(Self {
            name,
            label,
            description,
            kind: output_kind,
            constraints: FieldConstraints::default(),
        })
    }
}

fn output_kind_from_toml(
    name: &str,
    kind: &str,
    options: Option<Vec<InputChoiceToml>>,
    url: Option<String>,
) -> Result<OutputKind, crate::LoadError> {
    Ok(match kind {
        "string" => scalar_output_kind(name, kind, options, url, OutputKind::String)?,
        "number" => scalar_output_kind(name, kind, options, url, OutputKind::Number)?,
        "integer" => scalar_output_kind(name, kind, options, url, OutputKind::Integer)?,
        "boolean" => scalar_output_kind(name, kind, options, url, OutputKind::Boolean)?,
        "options" => output_choice_kind(name, kind, options, url, OutputKind::Options)?,
        "multi_options" => output_choice_kind(name, kind, options, url, OutputKind::MultiOptions)?,
        "markdown" => scalar_output_kind(name, kind, options, url, OutputKind::Markdown)?,
        "json" => scalar_output_kind(name, kind, options, url, OutputKind::Json)?,
        "datetime" => scalar_output_kind(name, kind, options, url, OutputKind::DateTime)?,
        "file_path" => scalar_output_kind(name, kind, options, url, OutputKind::FilePath)?,
        "url" => scalar_output_kind(name, kind, options, url, OutputKind::Url)?,
        "file" => scalar_output_kind(name, kind, options, url, OutputKind::File)?,
        "embedded_view" => embedded_view_output_kind(name, options, url)?,
        other => {
            return Err(crate::LoadError::UnknownOutputType {
                name: name.to_string(),
                kind: other.to_string(),
                expected: IO_TYPE_LIST,
            });
        }
    })
}

fn scalar_output_kind(
    name: &str,
    kind: &str,
    options: Option<Vec<InputChoiceToml>>,
    url: Option<String>,
    output_kind: OutputKind,
) -> Result<OutputKind, crate::LoadError> {
    if options.is_some() {
        return Err(crate::LoadError::UnexpectedOutputOptions {
            name: name.to_string(),
            kind: kind.to_string(),
        });
    }
    if url.is_some() {
        return Err(crate::LoadError::UnexpectedOutputUrl {
            name: name.to_string(),
            kind: kind.to_string(),
        });
    }
    Ok(output_kind)
}

fn output_choice_kind(
    name: &str,
    kind: &str,
    options: Option<Vec<InputChoiceToml>>,
    url: Option<String>,
    constructor: fn(ChoiceSpec) -> OutputKind,
) -> Result<OutputKind, crate::LoadError> {
    if url.is_some() {
        return Err(crate::LoadError::UnexpectedOutputUrl {
            name: name.to_string(),
            kind: kind.to_string(),
        });
    }
    let choices =
        input::choice_spec_from_toml(options).map_err(upeg_core::OutputSpecError::from)?;
    Ok(constructor(choices))
}

fn embedded_view_output_kind(
    name: &str,
    options: Option<Vec<InputChoiceToml>>,
    url: Option<String>,
) -> Result<OutputKind, crate::LoadError> {
    if options.is_some() {
        return Err(crate::LoadError::UnexpectedOutputOptions {
            name: name.to_string(),
            kind: "embedded_view".to_string(),
        });
    }
    let Some(url) = url else {
        return Err(crate::LoadError::MissingEmbeddedViewUrl {
            name: name.to_string(),
        });
    };
    if url.trim().is_empty() {
        return Err(crate::LoadError::EmptyEmbeddedViewUrl {
            name: name.to_string(),
        });
    }
    if url != url.trim() {
        return Err(crate::LoadError::NonCanonicalEmbeddedViewUrl {
            name: name.to_string(),
            url,
        });
    }
    Ok(OutputKind::EmbeddedView { url })
}

/// TOML shape for a single selector binding. Mirrors
/// `upeg_runtime::SelectorBinding` but lives here so the loader doesn't
/// expose `SelectorBinding`'s `'static`-string semantics to TOML
/// authors. The loader leaks the strings on registration.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 10)
)]
pub struct SelectorBindingToml {
    /// Role this binding plays in the Controlled Embed pipeline:
    /// `"input"` (write field → DOM), `"trigger"` (click DOM element),
    /// `"output"` (read DOM → field). Defaults to `"input"` when
    /// omitted so simple manifests stay terse.
    #[serde(default = "default_binding_role")]
    pub role: String,
    /// Tool input or output field name to bind. Unused for `trigger`.
    #[serde(default)]
    pub field: String,
    /// CSS selector inside the embedded page.
    pub selector: String,
    /// Trigger action for `role = "trigger"`: `"click"` (default) or
    /// `"enter"`. Non-trigger bindings may omit this or use `"click"` only.
    #[serde(default)]
    pub action: Option<String>,
    /// Optional wait settings before this binding is applied or read. Supported
    /// only for Controlled Embed selector bindings; network-idle,
    /// DOM-stability, and page lifecycle waits are out of scope.
    #[serde(default)]
    pub wait: Option<BindingWaitToml>,
}

fn default_binding_role() -> String {
    "input".to_string()
}

/// Optional Controlled Embed wait settings for a single selector binding.
/// Waits are selector readiness checks only; network-idle, DOM-stability,
/// and page lifecycle waits are out of scope.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 13)
)]
pub struct BindingWaitToml {
    /// CSS selector to wait for. Defaults to the binding's own selector.
    #[serde(default)]
    pub for_selector: Option<String>,
    /// Wait condition. Supported values are `"exists"` and `"visible"`.
    /// Defaults to `"exists"`.
    #[serde(default)]
    pub condition: Option<String>,
    /// Maximum wait duration in milliseconds. Defaults to 5000.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Fixed delay in milliseconds after the condition matches. Defaults to 0.
    /// This is not a network-idle or DOM-stability detector.
    #[serde(default)]
    pub settle_ms: Option<u64>,
    /// Timeout behavior. Supported values are `"fail"` and `"continue"`.
    /// Defaults to `"fail"`.
    #[serde(default)]
    pub on_timeout: Option<String>,
}

/// A single step in a Chain tool's execution flow.
///
/// Chain steps declare which tool to invoke, templated arguments, conditional
/// guards, and optional approval barriers. Directed edges in `connections`
/// order step execution.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 6)
)]
pub struct ChainStepToml {
    /// Stable local node id used by expressions and `connections`.
    #[serde(default)]
    pub id: Option<String>,
    /// Canonical Tool id to invoke.
    pub tool: String,
    /// JSON object template for step args. When omitted, unconnected source
    /// steps receive the chain input. A step with one upstream receives
    /// `{ "input": upstream_output }`; multiple upstreams receive
    /// `{ "input": last_upstream_output, "inputs": { "<upstream>": "<output>" } }`.
    #[serde(default)]
    pub args: Option<String>,
    /// Boolean expression. False skips the step.
    #[serde(default)]
    pub when: Option<String>,
    /// Approval barrier. The caller must include this step id in
    /// `_upeg.approvedSteps` or pass `approve = true`.
    #[serde(default)]
    pub requires_approval: Option<bool>,
}

/// Directed edge between two chain steps.
///
/// The `from` step id produces an output that flows into the `to` step as
/// its input. Steps without incoming connections receive the chain input.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 7)
)]
pub struct ChainConnectionToml {
    /// Upstream chain step id.
    pub from: String,
    /// Downstream chain step id.
    pub to: String,
}

/// TOML name/value template pair used for HTTP headers and similar maps.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 8)
)]
pub struct KeyValueToml {
    /// Key or header name.
    pub name: String,
    /// Value template.
    pub value: String,
}

/// Reference to a credential value resolved outside the manifest.
///
/// Manifests declare metadata and reference names only; the actual secret
/// value is resolved at execution time from env, keychain, or adapter.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 9)
)]
pub struct CredentialRefToml {
    /// Logical credential name used by prompt/header/body templates.
    pub name: String,
    /// Secret value type for adapter validation (`api_key`, `bearer`, etc.).
    /// This is schema metadata only; the value itself is never stored.
    #[serde(default, rename = "type")]
    pub value_type: Option<String>,
    /// Reference backend. `env` reads an environment variable; `keychain`
    /// reads an OS keychain item by service/account when the host supports it.
    #[serde(default)]
    pub store: Option<String>,
    /// Environment variable to read. Defaults to `UPEG_CREDENTIAL_<NAME>`.
    #[serde(default)]
    pub env: Option<String>,
    /// OS keychain service name when `store = "keychain"`.
    #[serde(default)]
    pub keychain_service: Option<String>,
    /// OS keychain account/user when `store = "keychain"`.
    #[serde(default)]
    pub keychain_account: Option<String>,
    /// Adapter-specific target name (for example external process env name or
    /// HTTP header expression name). Defaults to `name`.
    #[serde(default)]
    pub target: Option<String>,
    /// Missing credentials fail by default. Set false only for optional
    /// provider features.
    #[serde(default)]
    pub required: Option<bool>,
}

/// Declarative event trigger that can dispatch a tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 11)
)]
pub struct TriggerToml {
    /// `webhook`, `schedule`, `file`, `directory`, `clipboard`, or `hotkey`.
    pub source: String,
    /// Source-specific predicate. Every source's contract is checked at load
    /// time, so a missing or superfluous condition fails `upeg tool validate`
    /// rather than at watch time.
    ///
    /// `schedule` takes `now` (fire once when the watch starts) or
    /// `every:<duration>` such as `every:30s`, and defaults to `now` when
    /// omitted; the watch polls once per second, so a shorter interval is
    /// rejected and an interval that is not a whole multiple of the poll fires
    /// on the first poll at or after each due moment. `file` and `directory`
    /// require the watched path and fire on creation and on modification.
    /// `hotkey` requires an accelerator such as `ctrl+shift+u`. `clipboard` and
    /// `webhook` take no condition and reject one.
    #[serde(default)]
    pub condition: Option<String>,
}

/// Controlled Embed browser settings declared under `controlled_embed.browser`.
///
/// `user_agent` accepts `"default"`, `"mobile_safari"`, or `"custom"`.
/// `custom_user_agent` is required only for `user_agent = "custom"`.
/// `viewport` accepts `"mobile"`, `"tablet"`, `"desktop"`, or `"custom"`.
/// `viewport_width` and `viewport_height` are required only for
/// `viewport = "custom"`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 12)
)]
pub struct ControlledEmbedBrowserToml {
    /// User-Agent mode: `default`, `mobile_safari`, or `custom`.
    #[serde(default)]
    pub user_agent: Option<String>,
    /// Custom User-Agent string. Required only when `user_agent = "custom"`.
    #[serde(default)]
    pub custom_user_agent: Option<String>,
    /// Viewport mode: `mobile`, `tablet`, `desktop`, or `custom`.
    #[serde(default)]
    pub viewport: Option<String>,
    /// Custom viewport width. Required only when `viewport = "custom"`.
    #[serde(default)]
    pub viewport_width: Option<u16>,
    /// Custom viewport height. Required only when `viewport = "custom"`.
    #[serde(default)]
    pub viewport_height: Option<u16>,
}

static EMPTY_CONTROLLED_EMBED_BROWSER_TOML: ControlledEmbedBrowserToml =
    ControlledEmbedBrowserToml {
        user_agent: None,
        custom_user_agent: None,
        viewport: None,
        viewport_width: None,
        viewport_height: None,
    };

/// Controlled Embed settings declared at the tool level.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 12)
)]
pub struct ControlledEmbedToml {
    /// Browser identity and viewport overrides.
    #[serde(default)]
    pub browser: Option<ControlledEmbedBrowserToml>,
    /// Selector bindings nested under Controlled Embed.
    #[serde(default)]
    pub bindings: Option<Vec<SelectorBindingToml>>,
}

impl std::ops::Deref for ControlledEmbedToml {
    type Target = ControlledEmbedBrowserToml;

    fn deref(&self) -> &Self::Target {
        self.browser
            .as_ref()
            .unwrap_or(&EMPTY_CONTROLLED_EMBED_BROWSER_TOML)
    }
}
