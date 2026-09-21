//! WASM Toolkit plugin host for upeg.
//!
//! Loads `.wasm` files via [`extism`] and registers each declared Tool
//! through the `upeg_runtime` dispatcher primitive (iter 39). The
//! plugin contract is intentionally narrow:
//!
//! - The plugin **must** export a function `manifest` that takes no
//!   meaningful input and returns a JSON document of shape
//!   `{"id":"toolkit_id","tools": [PluginToolDecl, …]}`. See [`PluginManifest`].
//!   Tool declarations may carry typed `input_spec` and `output_spec`
//!   metadata, which the host lowers into the same core contracts used by
//!   declarative Toolkit TOML.
//! - For each declared tool id, the plugin **must** export a function
//!   with that exact name. The host calls it with the JSON-stringified
//!   args object and expects a UTF-8 string result; non-string returns
//!   should be encoded as JSON by the plugin.
//!
//! Threading: `extism::Plugin` is `!Sync`, so each loaded plugin is
//! wrapped in `Arc<Mutex<Plugin>>`. Dispatches grab the lock for the
//! duration of one wasm call; concurrent dispatches against the same
//! plugin serialize. This matches Extism's own thread-safety model.
//!
//! Wired in by `upeg-cli` behind the `wasm-plugin` feature so default
//! builds don't pay the wasmtime dep tax (~30s extra clean compile).

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::get_unwrap,
        clippy::tests_outside_test_module,
        clippy::print_stdout,
        clippy::unreachable,
        clippy::string_add,
        clippy::manual_let_else,
        reason = "tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
    )
)]

use extism::{Manifest, Plugin, Wasm};
use std::path::Path;
use std::sync::{Arc, Mutex};
use upeg_core::{
    ChoiceOption, ChoiceSpec, FieldConstraints, InputSpec, InputSpecError, OutputFieldSpec,
    OutputKind, OutputSpec, OutputSpecError, ToolIdError, ToolMeta, ToolkitMeta,
    validate_primary_output_id,
};
use upeg_plugin_api::{
    PluginChoiceOption, PluginManifest, PluginOutputKind, PluginOutputSpec, PluginToolDecl,
};
use upeg_runtime::{
    manifest::{CollisionError, ExternalToolManifest, ManifestError, lower_manifest_to_tool_meta},
    toolbox_add_single_text_tool_with_dispatcher, toolbox_add_toolkit,
};

mod input;

pub use input::plugin_input_spec_to_core;

/// Errors surfaced by [`load_and_register`].
#[derive(Debug)]
pub enum LoadError {
    /// `extism::Plugin::new` rejected the .wasm bytes (parse, link, or
    /// validation failure).
    Extism(extism::Error),
    /// I/O error reading the .wasm path.
    Io(std::io::Error),
    /// Calling the plugin's `manifest` export failed.
    Manifest(extism::Error),
    /// The manifest export's output isn't valid JSON or doesn't match
    /// [`PluginManifest`].
    ManifestShape(serde_json::Error),
    /// A tool declares a `pin` value we don't recognize.
    UnknownPinKind(String),
    /// A tool declares a `surfaces` entry we don't recognize.
    UnknownSurface(String),
    /// A plugin tool omits its pegboard footprint.
    MissingPegboardUnits,
    /// A plugin tool declares an empty pegboard footprint.
    EmptyPegboardUnits,
    /// A tool declares a pegboard footprint we don't recognize.
    UnknownPegboardUnits(String),
    /// `id = ""` (or whitespace-only): a plugin tool with no id is
    /// unidentifiable downstream — every CLI/HTTP/MCP dispatch path
    /// keys on the id. Iter 198 (parallel to upeg-loader iter 196).
    EmptyId,
    /// `toolkit = ""` (or whitespace-only): every Tool must belong to a
    /// Toolkit, and full ids must use `{toolkit}.{tool}`.
    EmptyToolkit,
    /// Full id prefix and explicit Toolkit id disagree.
    ToolkitMismatch { id: String, toolkit: String },
    /// Tool ids are registry keys; surrounding whitespace must be rejected
    /// rather than silently normalized.
    NonCanonicalId(String),
    /// Toolkit ids are namespace keys; surrounding whitespace must be rejected
    /// rather than silently normalized.
    NonCanonicalToolkit(String),
    /// `tags = [""]` (or whitespace-only entry): tags are discovery keys.
    EmptyTag { position: usize },
    /// Tags are discovery keys stored and compared exactly.
    NonCanonicalTag { position: usize, tag: String },
    /// `pin = ""` or whitespace-only: parallel to upeg-loader's
    /// iter-205 `EmptyPinKind`. Distinct from omission (which defaults
    /// to `Inline`); a present-but-empty value signals user intent
    /// gone wrong. Iter 206.
    EmptyPinKind,
    /// `surfaces = ["", ...]`: parallel to upeg-loader iter-207
    /// `EmptyInSurfaces`. Iter 207.
    EmptyInSurfaces { position: usize },
    /// `input_spec` is present but cannot be lowered into upeg's closed
    /// typed input model.
    InvalidInputSpec { detail: String },
    /// `output_spec` is present but cannot be lowered into upeg's closed
    /// typed output model.
    InvalidOutputSpec { detail: String },
    /// `id` shadows a link-time `#[upeg::tool]` built-in. Parallel to
    /// upeg-loader iter-249. Pre-iter-249 such a plugin loaded
    /// cleanly but `toolbox_tool(id)` returned the built-in's
    /// meta while `try_runtime_dispatch` ran the plugin's exported
    /// function — confusingly mixing sources. Iter 249.
    IdShadowsBuiltIn(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Extism(e) => write!(f, "extism: {e}"),
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Manifest(e) => write!(f, "calling `manifest`: {e}"),
            Self::ManifestShape(e) => write!(f, "manifest JSON shape: {e}"),
            // Iter 136: include the valid set in the message, matching
            // upeg-loader's parity. Previously bare "unknown pin
            // `Mauve`" left plugin authors guessing the valid options.
            Self::UnknownPinKind(k) => write!(
                f,
                "unknown pin `{k}` (Inline/Launcher/Live/Action/Embed/Chain/Llm)"
            ),
            Self::UnknownSurface(s) => write!(
                f,
                "unknown surface `{s}` (cli/tui/desktop/pwa/ext/mcp/http)"
            ),
            Self::MissingPegboardUnits => {
                write!(f, "`pegboard_units` is required (U1/U2/U2T)")
            }
            Self::EmptyPegboardUnits => {
                write!(f, "`pegboard_units` is empty — set one of U1, U2, or U2T")
            }
            Self::UnknownPegboardUnits(units) => {
                write!(f, "unknown pegboard_units `{units}` (U1/U2/U2T)")
            }
            Self::EmptyId => write!(f, "`id` must be a non-empty tool identifier"),
            Self::EmptyToolkit => write!(f, "`toolkit` must be a non-empty Toolkit id"),
            Self::ToolkitMismatch { id, toolkit } => write!(
                f,
                "`id = \"{id}\"` must start with its owning Toolkit prefix `{toolkit}.`"
            ),
            Self::NonCanonicalId(id) => {
                write!(f, "`id = \"{id}\"` must be canonical and unpadded")
            }
            Self::NonCanonicalToolkit(toolkit) => write!(
                f,
                "`toolkit = \"{toolkit}\"` must be canonical and unpadded"
            ),
            Self::EmptyTag { position } => write!(
                f,
                "`tags[{position}]` is empty — every tag must be a non-empty discovery key"
            ),
            Self::NonCanonicalTag { position, tag } => write!(
                f,
                "`tags[{position}] = \"{tag}\"` must be canonical and unpadded"
            ),
            Self::EmptyPinKind => write!(
                f,
                "`pin` is empty — omit the field entirely to default to `Inline`, or set a non-empty value"
            ),
            Self::EmptyInSurfaces { position } => write!(
                f,
                "`surfaces[{position}]` is empty — every surface must be one of cli/tui/desktop/pwa/ext/mcp/http"
            ),
            Self::InvalidInputSpec { detail } => {
                write!(f, "`input_spec` is invalid: {detail}")
            }
            Self::InvalidOutputSpec { detail } => {
                write!(f, "`output_spec` is invalid: {detail}")
            }
            Self::IdShadowsBuiltIn(id) => write!(
                f,
                "`id = \"{id}\"` shadows a built-in tool (link-time `#[upeg::tool]` inventory entry). \
                 Rename to a non-colliding id — pre-iter-249 the runtime override silently broke \
                 `tool show` (showed built-in meta) and `dispatch_tool` (ran your wasm export)"
            ),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<ManifestError> for LoadError {
    fn from(error: ManifestError) -> Self {
        match error {
            ManifestError::ToolId {
                source,
                id,
                toolkit,
            } => load_error_for_tool_id(source, &id, &toolkit),
            ManifestError::Collision(CollisionError::ShadowsBuiltIn { id, .. }) => {
                Self::IdShadowsBuiltIn(id)
            }
            ManifestError::Collision(CollisionError::DuplicateTool { id, .. }) => {
                Self::IdShadowsBuiltIn(id)
            }
            ManifestError::EmptyTag { position } => Self::EmptyTag { position },
            ManifestError::NonCanonicalTag { position, tag } => {
                Self::NonCanonicalTag { position, tag }
            }
            ManifestError::EmptyDisplayLabel => Self::EmptyPinKind,
            ManifestError::EmptyPinKind => Self::EmptyPinKind,
            ManifestError::UnknownPinKind(kind) => Self::UnknownPinKind(kind),
            ManifestError::MissingPegboardUnits => Self::MissingPegboardUnits,
            ManifestError::EmptyPegboardUnits => Self::EmptyPegboardUnits,
            ManifestError::UnknownPegboardUnits(units) => Self::UnknownPegboardUnits(units),
            ManifestError::EmptyInvoker | ManifestError::MissingInvoker => {
                Self::UnknownPinKind(String::new())
            }
            ManifestError::UnknownInvoker(invoker) => Self::UnknownPinKind(invoker),
            ManifestError::EmptyInSurfaces { position } => Self::EmptyInSurfaces { position },
            ManifestError::UnknownSurface(surface) => Self::UnknownSurface(surface),
            ManifestError::EmptyBoard { .. } | ManifestError::NonCanonicalBoard { .. } => {
                Self::UnknownSurface(String::new())
            }
            ManifestError::PrimaryOutputId(error) => Self::InvalidOutputSpec {
                detail: error.to_string(),
            },
        }
    }
}

/// Error type raised while lowering a WASM plugin manifest.
pub type WasmManifestError = LoadError;

/// Convert plugin API DTO output metadata into the core runtime output spec.
pub fn plugin_output_spec_to_core(
    spec: &PluginOutputSpec,
) -> Result<OutputSpec, WasmManifestError> {
    let fields = spec
        .fields
        .iter()
        .map(plugin_output_field_to_core)
        .collect::<Result<Vec<_>, _>>()?;
    let output_spec = OutputSpec::new(fields).map_err(output_spec_error)?;
    validate_primary_output_id(&output_spec.fields, spec.primary_output_id.as_deref())
        .map_err(output_spec_error_from_primary)?;
    Ok(output_spec)
}

fn plugin_output_field_to_core(
    field: &upeg_plugin_api::PluginOutputField,
) -> Result<OutputFieldSpec, LoadError> {
    Ok(OutputFieldSpec {
        name: field.name.clone(),
        label: field.label.clone(),
        description: field.description.clone(),
        kind: plugin_output_kind_to_core(&field.kind)?,
        constraints: FieldConstraints::default(),
    })
}

fn plugin_output_kind_to_core(kind: &PluginOutputKind) -> Result<OutputKind, LoadError> {
    Ok(match kind {
        PluginOutputKind::String => OutputKind::String,
        PluginOutputKind::Number => OutputKind::Number,
        PluginOutputKind::Integer => OutputKind::Integer,
        PluginOutputKind::Boolean => OutputKind::Boolean,
        PluginOutputKind::Options(options) => {
            OutputKind::Options(plugin_output_choices_to_core(options)?)
        }
        PluginOutputKind::MultiOptions(options) => {
            OutputKind::MultiOptions(plugin_output_choices_to_core(options)?)
        }
        PluginOutputKind::Markdown => OutputKind::Markdown,
        PluginOutputKind::Json => OutputKind::Json,
        PluginOutputKind::DateTime => OutputKind::DateTime,
        PluginOutputKind::FilePath => OutputKind::FilePath,
        PluginOutputKind::Url => OutputKind::Url,
        PluginOutputKind::File => OutputKind::File,
        PluginOutputKind::EmbeddedView { url } => OutputKind::EmbeddedView { url: url.clone() },
    })
}

fn plugin_output_choices_to_core(options: &[PluginChoiceOption]) -> Result<ChoiceSpec, LoadError> {
    let options = options
        .iter()
        .map(|option| {
            ChoiceOption::new(
                option.value.clone(),
                option.label.clone(),
                option.description.clone(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(output_spec_error_from_input)?;
    ChoiceSpec::new(options).map_err(output_spec_error_from_input)
}

fn output_spec_error(error: OutputSpecError) -> LoadError {
    LoadError::InvalidOutputSpec {
        detail: error.to_string(),
    }
}

fn output_spec_error_from_input(error: InputSpecError) -> LoadError {
    output_spec_error(OutputSpecError::Input(error))
}

fn output_spec_error_from_primary(error: upeg_core::PrimaryOutputIdError) -> LoadError {
    LoadError::InvalidOutputSpec {
        detail: error.to_string(),
    }
}

/// Convert a [`PluginToolDecl`] into a [`ToolMeta`]. Strings are
/// `Box::leak`ed (same pattern as the TOML loader) so the registry
/// can hold `&'static str`s.
pub(crate) fn decl_to_meta(decl: PluginToolDecl) -> Result<ToolMeta, LoadError> {
    let input_spec = decl
        .input_spec
        .as_ref()
        .map(plugin_input_spec_to_core)
        .transpose()?
        .unwrap_or_else(InputSpec::empty);
    let output_spec = decl
        .output_spec
        .as_ref()
        .map(plugin_output_spec_to_core)
        .transpose()?
        .unwrap_or_else(OutputSpec::empty);
    lower_manifest_to_tool_meta(ExternalToolManifest {
        id: decl.id.into_string(),
        toolkit: decl.toolkit.into_string(),
        tags: decl.tags.unwrap_or_default(),
        display_label: decl.display_label,
        description: decl.description,
        input_spec,
        output_spec,
        primary_output_id: decl
            .output_spec
            .as_ref()
            .and_then(|spec| spec.primary_output_id.clone()),
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        pin: decl.pin,
        pegboard_units: Some(decl.pegboard_units),
        invoker: Some("Wasm".to_string()),
        surfaces: decl.surfaces,
        boards: Vec::new(),
    })
    .map_err(LoadError::from)
}

fn load_error_for_tool_id(error: ToolIdError, id: &str, toolkit: &str) -> LoadError {
    match error {
        ToolIdError::EmptyId | ToolIdError::EmptyLocal => LoadError::EmptyId,
        ToolIdError::EmptyToolkit => LoadError::EmptyToolkit,
        ToolIdError::NonCanonicalId { id } => LoadError::NonCanonicalId(id),
        ToolIdError::NonCanonicalToolkit { toolkit } => LoadError::NonCanonicalToolkit(toolkit),
        ToolIdError::MissingSeparator => LoadError::ToolkitMismatch {
            id: id.to_string(),
            toolkit: toolkit.to_string(),
        },
        ToolIdError::ToolkitMismatch { id, toolkit } => LoadError::ToolkitMismatch { id, toolkit },
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn leak_slice(v: Vec<String>) -> &'static [&'static str] {
    let leaked: Vec<&'static str> = v.into_iter().map(leak).collect();
    Box::leak(leaked.into_boxed_slice())
}

/// Read-only summary of a validated plugin manifest, returned by
/// [`inspect_bytes`]. Carries just enough to drive `upeg plugin
/// install`/`list` (toolkit id + declared tool ids) without exposing
/// the internal [`upeg_core::ToolMeta`] registration shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInspection {
    /// Owning Toolkit id (the manifest's top-level `id`).
    pub toolkit: String,
    /// Every tool id the plugin declares, in manifest order.
    pub tool_ids: Vec<String>,
}

/// One manifest-declared tool that has passed every check
/// [`register_from_bytes`] would apply, but is not yet in the live
/// toolbox registry.
struct ValidatedTool {
    meta: ToolMeta,
    export_name: &'static str,
}

/// The result of parsing + validating a plugin's `manifest` export,
/// short of touching the live toolbox registry. Both [`inspect_bytes`]
/// (read-only) and [`register_from_bytes`] (registers) are built on top
/// of [`validate_manifest`] so there is exactly one place that encodes
/// the plugin manifest contract.
struct ValidatedManifest {
    toolkit_id: &'static str,
    toolkit_tags: Vec<String>,
    toolkit_description: &'static str,
    tools: Vec<ValidatedTool>,
}

impl From<&ValidatedManifest> for PluginInspection {
    fn from(validated: &ValidatedManifest) -> Self {
        Self {
            toolkit: validated.toolkit_id.to_string(),
            tool_ids: validated
                .tools
                .iter()
                .map(|tool| tool.meta.id.to_string())
                .collect(),
        }
    }
}

/// Load the .wasm file at `path`, ask its `manifest` export for the
/// tool list, register each tool's `ToolMeta` and a runtime dispatcher
/// that re-enters the plugin for execution. Returns the registered ids
/// on success.
pub fn load_and_register(path: &Path) -> Result<Vec<&'static str>, LoadError> {
    let bytes = std::fs::read(path).map_err(LoadError::Io)?;
    register_from_bytes(&bytes)
}

/// Instantiate the plugin and call its `manifest` export, parse the
/// result, and run every per-declaration validation check —
/// WITHOUT registering anything into the live toolbox. Shared by
/// [`inspect_bytes`] and [`register_from_bytes`].
fn inspect_manifest(bytes: &[u8]) -> Result<(Arc<Mutex<Plugin>>, ValidatedManifest), LoadError> {
    let manifest = Manifest::new([Wasm::data(bytes.to_vec())]);
    let plugin = Plugin::new(&manifest, [], false).map_err(LoadError::Extism)?;
    let plugin = Arc::new(Mutex::new(plugin));

    let manifest_json: String = {
        #[allow(
            clippy::expect_used,
            reason = "Mutex poisoning only happens after another thread panicked while holding the plugin lock; the host is already in a fatal state."
        )]
        let mut guard = plugin.lock().expect("plugin mutex poisoned");
        guard
            .call::<&str, &str>("manifest", "")
            .map_err(LoadError::Manifest)?
            .to_string()
    };
    let validated = validate_manifest(&manifest_json)?;
    Ok((plugin, validated))
}

/// Parse + validate a plugin's `manifest` JSON export. Pure function:
/// no extism calls, no registry access.
fn validate_manifest(manifest_json: &str) -> Result<ValidatedManifest, LoadError> {
    let parsed: PluginManifest =
        serde_json::from_str(manifest_json).map_err(LoadError::ManifestShape)?;
    let toolkit = parsed.id.as_str().trim();
    if toolkit.is_empty() {
        return Err(LoadError::EmptyToolkit);
    }
    if parsed.id.as_str() != toolkit {
        return Err(LoadError::NonCanonicalToolkit(
            parsed.id.as_str().to_string(),
        ));
    }
    if let Some(tags) = &parsed.tags
        && let Some(position) = tags.iter().position(|tag| tag.trim().is_empty())
    {
        return Err(LoadError::EmptyTag { position });
    }
    let toolkit_tags: Vec<String> = parsed
        .tags
        .unwrap_or_default()
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .collect();
    let toolkit_id: &'static str = leak(toolkit.to_string());
    let toolkit_description: &'static str = parsed
        .description
        .map_or("", |description| leak(description.trim().to_string()));

    let mut tools = Vec::with_capacity(parsed.tools.len());
    for mut decl in parsed.tools {
        if decl.toolkit.as_str().trim().is_empty() {
            return Err(LoadError::EmptyToolkit);
        }
        if decl.toolkit.as_str() != decl.toolkit.as_str().trim() {
            return Err(LoadError::NonCanonicalToolkit(
                decl.toolkit.as_str().to_string(),
            ));
        }
        if decl.toolkit.as_str() != toolkit {
            return Err(LoadError::ToolkitMismatch {
                id: decl.id.as_str().to_string(),
                toolkit: toolkit.to_string(),
            });
        }
        let mut effective_tags = toolkit_tags.clone();
        for tag in decl.tags.clone().unwrap_or_default() {
            let tag = tag.trim().to_string();
            if !effective_tags.iter().any(|existing| existing == &tag) {
                effective_tags.push(tag);
            }
        }
        decl.tags = Some(effective_tags);
        // `export` defaults to id, leaked alongside other static strings
        // so a later registration's dispatcher closure can reference it
        // cheaply.
        let export_name: &'static str = leak(decl.export.clone().map_or_else(
            || decl.id.as_str().to_string(),
            upeg_plugin_api::ExportName::into_string,
        ));
        let meta = decl_to_meta(decl)?;
        tools.push(ValidatedTool { meta, export_name });
    }

    Ok(ValidatedManifest {
        toolkit_id,
        toolkit_tags,
        toolkit_description,
        tools,
    })
}

/// Validate a plugin's bytes and summarize its declared tools, WITHOUT
/// registering anything into the live toolbox. Used by `upeg plugin
/// install`/`list` to check a plugin before copying it into
/// `~/.upeg/wasm/` (or to inspect one already there).
pub fn inspect_bytes(bytes: &[u8]) -> Result<PluginInspection, LoadError> {
    let (_plugin, validated) = inspect_manifest(bytes)?;
    Ok(PluginInspection::from(&validated))
}

/// Same as [`load_and_register`] but takes raw bytes — the form tests
/// reach for via `include_bytes!`.
pub fn register_from_bytes(bytes: &[u8]) -> Result<Vec<&'static str>, LoadError> {
    let (plugin, validated) = inspect_manifest(bytes)?;
    register_validated(plugin, validated)
}

/// Register an already-[`validate_manifest`]ed plugin into the live
/// toolbox: one [`ToolkitMeta`] plus one dispatcher-backed tool per
/// declaration.
fn register_validated(
    plugin: Arc<Mutex<Plugin>>,
    validated: ValidatedManifest,
) -> Result<Vec<&'static str>, LoadError> {
    toolbox_add_toolkit(ToolkitMeta {
        id: validated.toolkit_id,
        tags: leak_slice(validated.toolkit_tags),
        description: validated.toolkit_description,
    });

    let mut ids = Vec::with_capacity(validated.tools.len());
    for tool in validated.tools {
        let id_static: &'static str = tool.meta.id;
        let export_static = tool.export_name;
        let plugin = Arc::clone(&plugin);
        toolbox_add_single_text_tool_with_dispatcher(tool.meta, move |args| {
            let args_str = args.to_string();
            let mut guard = plugin
                .lock()
                .map_err(|_| "plugin mutex poisoned".to_string())?;
            guard
                .call::<&str, String>(export_static, args_str.as_str())
                .map_err(|e| format!("{e}"))
        });
        ids.push(id_static);
    }
    Ok(ids)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod round_trip_tests;
