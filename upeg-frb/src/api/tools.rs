//! Tool discovery + dispatch for the Flutter widget tree.
//!
//! `list_toolkits` / `list_tools` feed the palette and board, and
//! `dispatch_tool` runs a tool by id with JSON args — together the
//! tap→dispatch→render loop the Dart UI drives.
//!
//! All return types are `#[frb(non_opaque)]` DTOs with owned `String`
//! fields. Source `ToolMeta`/`ToolkitMeta` have `&'static str` fields
//! that can't cross the FFI boundary as borrows — converting to owned
//! strings is cheap because the underlying static slices never move.

use upeg_core::{
    ActionBinding, ActionScope, ActionSuccess, FileContent, FileValue, Invoker, OutputEntry,
    OutputKind, OutputValue, PegboardUnits, PinKind, Source, ToolEffect, ToolError, ToolId,
    ToolMeta, ToolResult, ToolSuccess, ToolkitMeta,
};
use upeg_runtime::ToolMetaRuntimeExt;

pub mod file_input_policy;
mod gui_context;
pub mod input_field;

pub use file_input_policy::FileInputPolicyDto;
use gui_context::{apply_gui_context, gui_execution_context};
pub use input_field::{
    ChoiceOptionDto, FieldConstraintsDto, InputFieldDto, InputFieldType, NumberConstraintsDto,
    StringConstraintsDto,
};

const INVALID_TOOL_ID_ERROR_CODE: &str = "invalid_tool_id";
const INVALID_ARGS_JSON_ERROR_CODE: &str = "invalid_args_json";
const TOOL_NOT_FOUND_ERROR_CODE: &str = "tool_not_found";
const DISPATCH_UNIMPLEMENTED_ERROR_CODE: &str = "dispatch_unimplemented";

/// Dart-mirrored view of [`upeg_core::ToolkitMeta`].
///
/// ToolkitMeta in upeg-core only has `id`, `tags`, `description` —
/// no display_label (toolkits are not directly callable; their label
/// is derived from id at the UI layer).
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ToolkitDto {
    pub id: String,
    pub description: String,
    pub tags: Vec<String>,
}

impl From<&'static ToolkitMeta> for ToolkitDto {
    fn from(meta: &'static ToolkitMeta) -> Self {
        Self {
            id: meta.id.to_string(),
            description: meta.description.to_string(),
            tags: meta.tags.iter().map(|t| (*t).to_string()).collect(),
        }
    }
}

/// Dart-mirrored view of [`upeg_core::ToolMeta`].
///
/// `input_fields` is a flattened, owned view of the tool's `input_spec`
/// so the Dart `GenericForm` widget can pattern-match on a closed sealed
/// enum without crossing the FFI boundary again; `output_fields` mirrors
/// the `output_spec` the same way.
///
/// Batch 1 added `pin_kind` / `invoker` / `pegboard_units` so the Dart
/// Pin widget can drive its kind-coloured accent dot, kind badge,
/// invoker label, and multi-unit cell size without crossing the FFI
/// boundary again per pin.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ToolDto {
    pub id: String,
    pub toolkit: String,
    pub label: String,
    pub description: String,
    pub tags: Vec<String>,
    pub input_fields: Vec<InputFieldDto>,
    /// Ordered typed output fields sourced from `output_spec.fields`.
    /// This is the bridge-level schema Flutter needs for compact Pin
    /// summaries and typed result renderers.
    pub output_fields: Vec<OutputFieldDto>,
    /// Mirror of [`upeg_core::PinKind`] — drives the kind-coloured
    /// header dot + footer KindBadge on the Dart Pin widget.
    pub pin_kind: PinKindDto,
    /// Mirror of [`upeg_core::Invoker`] — short footer label on the
    /// Dart Pin widget ("function", "external", …).
    pub invoker: InvokerDto,
    /// Mirror of [`upeg_core::PegboardUnits`] — drives the Dart
    /// `BoardCanvas` cell footprint via `placement.w` / `placement.h`.
    pub pegboard_units: PegboardUnitsDto,
    /// Mirror of [`upeg_core::Source`] — Dart uses this to decide which
    /// pins auto-poll (`Timer { interval_ms }`).
    pub source: SourceDto,
    /// First manifest-declared credential name (TOML `credentials`
    /// list, registered by the loader), so the "provider not
    /// configured" pin body can render the exact
    /// `upeg credential add <name>` command. `None` when the tool
    /// declares no credentials.
    pub credential_name: Option<String>,
    /// This tool stops at a human-approval barrier: at least one Chain
    /// step declares `requires_approval`.
    ///
    /// Dart reads this *before* dispatching so the confirmation can sit
    /// in front of the run. Dispatching without it is not unsafe — the
    /// gated step refuses with `approval_required` and nothing runs — but
    /// it is a dead end for the person, who has no way to answer from a
    /// failed result. See `docs/architecture/chain.md`.
    pub requires_approval: bool,
    /// Surface labels (`cli`, `tui`, `desktop`, …) whose approval this
    /// tool honors — the manifest's `approval_surfaces` or the default.
    /// Empty when [`ToolDto::requires_approval`] is false.
    ///
    /// Labels rather than a mirrored enum: `_upeg.surface` is already a
    /// label on the wire, so Dart compares its own surface label against
    /// this list and needs no second vocabulary to keep in sync.
    pub approval_surfaces: Vec<String>,
    pub effect: ToolEffectDto,
    pub presentation: Option<ToolPresentationDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum ToolEffectDto {
    Read,
    Write,
    Unknown,
}

impl From<ToolEffect> for ToolEffectDto {
    fn from(value: ToolEffect) -> Self {
        match value {
            ToolEffect::Read => Self::Read,
            ToolEffect::Write => Self::Write,
            ToolEffect::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ToolPresentationDto {
    pub version: u16,
    pub output: Option<String>,
    pub rows: Option<String>,
    pub row_key: Option<String>,
    pub columns: Vec<PresentationColumnDto>,
    pub actions: Vec<PresentationActionDto>,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationColumnDto {
    pub label: String,
    pub pointer: String,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationActionDto {
    pub id: String,
    pub scope: ActionScopeDto,
    pub label: String,
    pub target_tool: String,
    pub on_success: Option<ActionSuccessDto>,
    pub bindings: Vec<PresentationBindingDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum ActionScopeDto {
    Row,
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum ActionSuccessDto {
    RefreshOrigin,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationBindingDto {
    pub target: String,
    pub source: BindingSourceDto,
    pub pointer: Option<String>,
    pub value_json: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum BindingSourceDto {
    Input,
    Row,
    Output,
    Constant,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationRowsDto {
    pub rows: Vec<PresentationRowDto>,
    pub diagnostics: Vec<String>,
    pub row_actions_enabled: bool,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationRowDto {
    pub key: String,
    pub value_json: String,
    pub cells_json: Vec<String>,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ActionBindingResolutionDto {
    pub values_json: String,
    pub diagnostics: Vec<String>,
    pub unbound_required_inputs: Vec<String>,
}

impl From<&upeg_core::ToolPresentation> for ToolPresentationDto {
    fn from(value: &upeg_core::ToolPresentation) -> Self {
        let columns = value
            .columns
            .iter()
            .map(|column| PresentationColumnDto {
                label: column.label.clone(),
                pointer: column.pointer.clone(),
            })
            .collect();
        let actions = value
            .actions
            .iter()
            .map(|action| PresentationActionDto {
                id: action.id.clone(),
                scope: match action.scope {
                    ActionScope::Row => ActionScopeDto::Row,
                    ActionScope::Result => ActionScopeDto::Result,
                },
                label: action.label.clone(),
                target_tool: action.target_tool.clone(),
                on_success: action.on_success.map(|value| match value {
                    ActionSuccess::RefreshOrigin => ActionSuccessDto::RefreshOrigin,
                }),
                bindings: action
                    .bindings
                    .iter()
                    .map(|(target, binding)| {
                        let (source, pointer, value_json) = match binding {
                            ActionBinding::Input { pointer } => {
                                (BindingSourceDto::Input, Some(pointer.clone()), None)
                            }
                            ActionBinding::Row { pointer } => {
                                (BindingSourceDto::Row, Some(pointer.clone()), None)
                            }
                            ActionBinding::Output { pointer } => {
                                (BindingSourceDto::Output, Some(pointer.clone()), None)
                            }
                            ActionBinding::Constant { value } => {
                                (BindingSourceDto::Constant, None, Some(value.to_string()))
                            }
                        };
                        PresentationBindingDto {
                            target: target.clone(),
                            source,
                            pointer,
                            value_json,
                        }
                    })
                    .collect(),
            })
            .collect();
        Self {
            version: value.version,
            output: value.output.clone(),
            rows: value.rows.clone(),
            row_key: value.row_key.clone(),
            columns,
            actions,
        }
    }
}

/// Sealed enum mirror of [`upeg_core::Source`]. Drives the Dart-side
/// auto-polling decision for `PinKind::Live` pins.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum SourceDto {
    UserInput,
    Timer { interval_ms: u64 },
    Shortcut { keys: String },
    Manual,
    Static,
}

impl From<&Source> for SourceDto {
    fn from(source: &Source) -> Self {
        match source {
            Source::UserInput => Self::UserInput,
            Source::Timer { interval } => Self::Timer {
                interval_ms: u64::try_from(interval.as_millis()).unwrap_or(u64::MAX),
            },
            Source::Shortcut { keys } => Self::Shortcut { keys: keys.clone() },
            Source::Manual => Self::Manual,
            Source::Static => Self::Static,
        }
    }
}

/// Sealed enum mirror of [`upeg_core::PinKind`].
///
/// Lives in the FRB layer (not core) so flutter_rust_bridge emits a
/// proper Dart sealed class instead of a `String` discriminator the
/// pin widget would have to re-parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum PinKindDto {
    Inline,
    Launcher,
    Live,
    Action,
    Embed,
    ControlledEmbed,
    Chain,
    Llm,
}

impl From<PinKind> for PinKindDto {
    fn from(kind: PinKind) -> Self {
        match kind {
            PinKind::Inline => Self::Inline,
            PinKind::Launcher => Self::Launcher,
            PinKind::Live => Self::Live,
            PinKind::Action => Self::Action,
            PinKind::Embed => Self::Embed,
            PinKind::ControlledEmbed => Self::ControlledEmbed,
            PinKind::Chain => Self::Chain,
            PinKind::Llm => Self::Llm,
        }
    }
}

/// Sealed enum mirror of [`upeg_core::Invoker`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum InvokerDto {
    Function,
    External,
    Http,
    Static,
    Embed,
    Chain,
    Llm,
    Wasm,
}

impl From<Invoker> for InvokerDto {
    fn from(invoker: Invoker) -> Self {
        match invoker {
            Invoker::Function => Self::Function,
            Invoker::External => Self::External,
            Invoker::Http => Self::Http,
            Invoker::Static => Self::Static,
            Invoker::Embed => Self::Embed,
            Invoker::Chain => Self::Chain,
            Invoker::Llm => Self::Llm,
            Invoker::Wasm => Self::Wasm,
        }
    }
}

/// Sealed enum mirror of [`upeg_core::PegboardUnits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum PegboardUnitsDto {
    U1,
    U2,
    U2T,
}

impl From<PegboardUnits> for PegboardUnitsDto {
    fn from(units: PegboardUnits) -> Self {
        match units {
            PegboardUnits::U1 => Self::U1,
            PegboardUnits::U2 => Self::U2,
            PegboardUnits::U2T => Self::U2T,
        }
    }
}

/// One typed output field as exposed to Dart.
///
/// `key` is the canonical output name used inside structured content.
/// `label` falls back to `key` when the source output field has no label.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct OutputFieldDto {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub field_type: OutputFieldType,
}

/// Sealed enum of output field kinds Flutter renderers can pattern-match.
/// Mirrors [`upeg_core::OutputKind`] one-to-one, with JSON represented as
/// a multiline text surface until a dedicated JSON viewer lands.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum OutputFieldType {
    Text,
    Number,
    Integer,
    Boolean,
    File,
    Select { options: Vec<String> },
    Multiline,
    MultiOptions { options: Vec<String> },
    Json,
    DateTime,
    Markdown,
    FilePath,
    Url,
    EmbeddedView { url: String },
}

impl From<&OutputKind> for OutputFieldType {
    fn from(kind: &OutputKind) -> Self {
        match kind {
            OutputKind::String => Self::Text,
            OutputKind::Markdown => Self::Markdown,
            OutputKind::DateTime => Self::DateTime,
            OutputKind::FilePath => Self::FilePath,
            OutputKind::Url => Self::Url,
            OutputKind::Number => Self::Number,
            OutputKind::Integer => Self::Integer,
            OutputKind::Boolean => Self::Boolean,
            OutputKind::Json => Self::Json,
            OutputKind::File => Self::File,
            OutputKind::Options(choices) => Self::Select {
                options: choices
                    .options
                    .iter()
                    .map(|opt| opt.value.clone())
                    .collect(),
            },
            OutputKind::MultiOptions(choices) => Self::MultiOptions {
                options: choices
                    .options
                    .iter()
                    .map(|opt| opt.value.clone())
                    .collect(),
            },
            OutputKind::EmbeddedView { url } => Self::EmbeddedView { url: url.clone() },
        }
    }
}

impl From<&'static ToolMeta> for ToolDto {
    fn from(meta: &'static ToolMeta) -> Self {
        let policy = upeg_runtime::tool_approval_policy(meta.id);
        let input_fields = meta
            .input_spec
            .fields
            .iter()
            .map(InputFieldDto::from)
            .collect();
        let output_fields: Vec<OutputFieldDto> = meta
            .output_spec
            .fields
            .iter()
            .map(|field| {
                let key = field.name.clone();
                let label = field.label.clone().unwrap_or_else(|| field.name.clone());
                OutputFieldDto {
                    key,
                    label,
                    description: field.description.clone(),
                    field_type: OutputFieldType::from(&field.kind),
                }
            })
            .collect();
        Self {
            id: meta.id.to_string(),
            toolkit: meta.toolkit.to_string(),
            label: meta.display_label.to_string(),
            description: meta.description.to_string(),
            tags: meta.tag_labels(),
            input_fields,
            output_fields,
            pin_kind: PinKindDto::from(meta.pin),
            invoker: InvokerDto::from(meta.invoker),
            pegboard_units: PegboardUnitsDto::from(meta.pegboard_units),
            source: SourceDto::from(&meta.source),
            credential_name: upeg_runtime::tool_credential_names(meta.id)
                .into_iter()
                .next(),
            requires_approval: policy.requires_approval(),
            approval_surfaces: policy
                .surfaces()
                .iter()
                .map(|surface| surface.label().to_string())
                .collect(),
            effect: ToolEffectDto::from(meta.effect),
            presentation: meta.presentation.as_ref().map(ToolPresentationDto::from),
        }
    }
}

#[flutter_rust_bridge::frb(sync)]
pub fn resolve_tool_presentation_rows(
    tool_id: String,
    outputs_json: String,
) -> PresentationRowsDto {
    let Some(tool) = upeg_runtime::toolbox_tools().find(|tool| tool.id == tool_id) else {
        return PresentationRowsDto {
            rows: Vec::new(),
            diagnostics: vec![format!("tool `{tool_id}` is not installed")],
            row_actions_enabled: false,
        };
    };
    let Some(presentation) = tool.presentation.as_ref() else {
        return PresentationRowsDto {
            rows: Vec::new(),
            diagnostics: vec![format!("tool `{tool_id}` has no presentation")],
            row_actions_enabled: false,
        };
    };
    let outputs = match serde_json::from_str(&outputs_json) {
        Ok(value) => value,
        Err(error) => {
            return PresentationRowsDto {
                rows: Vec::new(),
                diagnostics: vec![format!("outputs JSON is invalid: {error}")],
                row_actions_enabled: false,
            };
        }
    };
    let resolved = upeg_core::resolve_rows(presentation, &outputs);
    PresentationRowsDto {
        rows: resolved
            .rows
            .into_iter()
            .map(|row| PresentationRowDto {
                key: row.key,
                value_json: row.value.to_string(),
                cells_json: row.cells.into_iter().map(|cell| cell.to_string()).collect(),
            })
            .collect(),
        diagnostics: resolved.diagnostics,
        row_actions_enabled: resolved.row_actions_enabled,
    }
}

#[flutter_rust_bridge::frb(sync)]
pub fn resolve_tool_action_bindings(
    tool_id: String,
    action_id: String,
    current_inputs_json: String,
    selected_row_json: Option<String>,
    outputs_json: String,
) -> ActionBindingResolutionDto {
    let diagnostic = |message: String| ActionBindingResolutionDto {
        values_json: "{}".to_string(),
        diagnostics: vec![message],
        unbound_required_inputs: Vec::new(),
    };
    let Some(source) = upeg_runtime::toolbox_tools().find(|tool| tool.id == tool_id) else {
        return diagnostic(format!("tool `{tool_id}` is not installed"));
    };
    let Some(presentation) = source.presentation.as_ref() else {
        return diagnostic(format!("tool `{tool_id}` has no presentation"));
    };
    let Some(action) = presentation
        .actions
        .iter()
        .find(|action| action.id == action_id)
    else {
        return diagnostic(format!(
            "action `{action_id}` is not declared by `{tool_id}`"
        ));
    };
    let Some(target) = upeg_runtime::toolbox_tools().find(|tool| tool.id == action.target_tool)
    else {
        return diagnostic(format!(
            "target tool `{}` is not installed",
            action.target_tool
        ));
    };
    let current_inputs = match serde_json::from_str(&current_inputs_json) {
        Ok(value) => value,
        Err(error) => return diagnostic(format!("current inputs JSON is invalid: {error}")),
    };
    let outputs = match serde_json::from_str(&outputs_json) {
        Ok(value) => value,
        Err(error) => return diagnostic(format!("outputs JSON is invalid: {error}")),
    };
    let selected_row = match selected_row_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
    {
        Ok(value) => value,
        Err(error) => return diagnostic(format!("selected row JSON is invalid: {error}")),
    };
    let resolved = upeg_core::resolve_bindings(
        action,
        &current_inputs,
        selected_row.as_ref(),
        &outputs,
        &target.input_spec,
    );
    ActionBindingResolutionDto {
        values_json: serde_json::Value::Object(resolved.values.into_iter().collect()).to_string(),
        diagnostics: resolved.diagnostics,
        unbound_required_inputs: resolved.unbound_required_inputs,
    }
}

/// All registered toolkits, sorted by `id`. The UI uses this to drive a
/// toolkit selector / nav.
#[flutter_rust_bridge::frb(sync)]
pub fn list_toolkits() -> Vec<ToolkitDto> {
    let mut dtos: Vec<ToolkitDto> = upeg_runtime::toolbox_toolkits()
        .map(ToolkitDto::from)
        .collect();
    dtos.sort_by(|a, b| a.id.cmp(&b.id));
    dtos
}

/// All registered tools, sorted by `id`. Optional filter by toolkit:
/// `Some("convert")` returns only `convert.*` tools. `None` returns
/// everything.
#[flutter_rust_bridge::frb(sync)]
pub fn list_tools(toolkit: Option<String>) -> Vec<ToolDto> {
    let mut dtos: Vec<ToolDto> = upeg_runtime::toolbox_tools()
        .filter(|t| match toolkit.as_deref() {
            Some(tk) => t.toolkit == tk,
            None => true,
        })
        .map(ToolDto::from)
        .collect();
    dtos.sort_by(|a, b| a.id.cmp(&b.id));
    dtos
}

/// Canonical result of a single tool dispatch crossing the FFI boundary.
///
/// Success carries the runtime [`ToolSuccess`] shape directly: `outputs`
/// are canonical [`OutputValue`] mirrors and `primary_output_id` points to
/// one of their `id`s. Failure carries a structured [`ToolError`]. No field
/// is reconstructed from legacy stdout text.
#[derive(Debug, Clone, PartialEq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct CanonicalToolResult {
    pub ok: bool,
    pub primary_output_id: Option<String>,
    pub outputs: Vec<CanonicalOutputEntry>,
    pub error: Option<CanonicalToolError>,
}

impl CanonicalToolResult {
    fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            primary_output_id: None,
            outputs: Vec::new(),
            error: Some(CanonicalToolError {
                code: code.into(),
                message: message.into(),
                details: None,
            }),
        }
    }
}

impl From<ToolResult> for CanonicalToolResult {
    fn from(result: ToolResult) -> Self {
        match result {
            ToolResult::Success(success) => Self::from(success),
            ToolResult::Failure(failure) => Self {
                ok: false,
                primary_output_id: None,
                outputs: Vec::new(),
                error: Some(CanonicalToolError::from(failure.error)),
            },
        }
    }
}

impl From<ToolSuccess> for CanonicalToolResult {
    fn from(success: ToolSuccess) -> Self {
        Self {
            ok: true,
            primary_output_id: success.primary_output_id,
            outputs: success
                .outputs
                .into_iter()
                .map(CanonicalOutputEntry::from)
                .collect(),
            error: None,
        }
    }
}

/// One canonical output entry emitted by a tool dispatch.
///
/// Serde derives back the persisted last-outcome cache
/// (`api::last_outcomes`): entries round-trip through the store's
/// `outputs_json` column as a JSON array of this exact shape.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct CanonicalOutputEntry {
    pub id: String,
    pub label: Option<String>,
    pub kind: String,
    pub value: CanonicalOutputValue,
}

impl From<OutputEntry> for CanonicalOutputEntry {
    fn from(entry: OutputEntry) -> Self {
        Self {
            id: entry.id,
            label: entry.label,
            kind: entry.kind.label().to_string(),
            value: CanonicalOutputValue::from(entry.value),
        }
    }
}

/// Sealed enum mirror of [`OutputValue`] for Flutter renderers.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum CanonicalOutputValue {
    String { value: String },
    Number { value: f64 },
    Integer { value: i64 },
    Boolean { value: bool },
    Options { value: String },
    MultiOptions { value: Vec<String> },
    Markdown { value: String },
    Json { value: String },
    DateTime { value: String },
    FilePath { value: String },
    Url { value: String },
    File { value: CanonicalFileValue },
    EmbeddedView { value: String },
}

impl From<OutputValue> for CanonicalOutputValue {
    fn from(value: OutputValue) -> Self {
        match value {
            OutputValue::String(value) => Self::String { value },
            OutputValue::Number(value) => Self::Number {
                value: value.as_f64().unwrap_or_default(),
            },
            OutputValue::Integer(value) => Self::Integer { value },
            OutputValue::Boolean(value) => Self::Boolean { value },
            OutputValue::Options(value) => Self::Options { value },
            OutputValue::MultiOptions(value) => Self::MultiOptions { value },
            OutputValue::Markdown(value) => Self::Markdown { value },
            OutputValue::Json(value) => Self::Json {
                value: value.to_string(),
            },
            OutputValue::DateTime(value) => Self::DateTime { value },
            OutputValue::FilePath(value) => Self::FilePath { value },
            OutputValue::Url(value) => Self::Url { value },
            OutputValue::File(value) => Self::File {
                value: CanonicalFileValue::from(value),
            },
            OutputValue::EmbeddedView(value) => Self::EmbeddedView { value },
        }
    }
}

/// Runtime carrier for canonical file outputs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct CanonicalFileValue {
    pub name: String,
    pub is_dir: bool,
    pub content: CanonicalFileContent,
    pub mime: Option<String>,
}

impl From<FileValue> for CanonicalFileValue {
    fn from(file: FileValue) -> Self {
        // `FileValue` derives the attribute from its body, so read it before
        // the body moves into the mirror.
        let is_dir = file.is_dir();
        Self {
            name: file.name,
            is_dir,
            content: CanonicalFileContent::from(file.content),
            mime: file.mime,
        }
    }
}

/// File content mirror used by [`CanonicalOutputValue::File`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum CanonicalFileContent {
    Bytes { bytes: Vec<u8> },
    Directory { entries: Vec<CanonicalFileValue> },
}

impl From<FileContent> for CanonicalFileContent {
    fn from(content: FileContent) -> Self {
        match content {
            FileContent::Bytes(bytes) => Self::Bytes { bytes },
            FileContent::Directory(entries) => Self::Directory {
                entries: entries.into_iter().map(CanonicalFileValue::from).collect(),
            },
        }
    }
}

/// Structured failure details emitted by a tool dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct CanonicalToolError {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

impl From<ToolError> for CanonicalToolError {
    fn from(error: ToolError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            details: error.details.map(|details| details.to_string()),
        }
    }
}

/// Run the named tool with JSON-encoded args.
///
/// `args_json` is parsed via `serde_json::from_str`. A parse failure
/// surfaces as `ok = false` with a structured `error`, never as a panic.
///
/// `board_key` scopes the dispatch to a board: the pin's saved args
/// preset merges as defaults and the board execution context is
/// annotated (shared `upeg_runtime` synthesis layer — same contract as
/// CLI/HTTP/MCP/TUI). `None` dispatches globally with only the
/// `desktop` surface label.
///
/// `approve` lifts a Chain's approval barrier — see
/// [`APPROVE_RESERVED_ARG`] for why it is a typed parameter and not a
/// key the caller writes into `args_json`.
#[flutter_rust_bridge::frb(sync)]
pub fn dispatch_tool(
    tool_id: String,
    args_json: String,
    board_key: Option<String>,
    approve: bool,
) -> CanonicalToolResult {
    dispatch_tool_impl(&tool_id, &args_json, board_key.as_deref(), approve)
}

/// Run the named tool with JSON-encoded args on FRB's async dispatch path.
#[flutter_rust_bridge::frb]
pub async fn dispatch_tool_async(
    tool_id: String,
    args_json: String,
    board_key: Option<String>,
    approve: bool,
) -> CanonicalToolResult {
    dispatch_tool_impl(&tool_id, &args_json, board_key.as_deref(), approve)
}

/// Reserved call argument that approves every gated step of a Chain in
/// one call (`docs/architecture/chain.md`).
///
/// A GUI surface never lets it arrive as data. Dart passes a typed
/// `approve` flag and Rust is the only writer of the key — see
/// [`shape_approval_arg`] — so an args map hand-built anywhere in the
/// widget tree cannot self-approve a gated run.
const APPROVE_RESERVED_ARG: &str = "approve";

/// Make the two approval levers say exactly what the typed `approve`
/// flag says, whatever the caller put in `args`.
///
/// There are two, not one. `approve = true` approves the whole call, and
/// `_upeg.approvedSteps` names individual steps
/// (`upeg-loader/src/dispatcher/chain/approval.rs`); the second is
/// caller-preserved by design, so unlike every other `_upeg` key it
/// survives the execution-context merge
/// (`upeg-runtime/src/execution.rs`). Stripping only the first left the
/// second as a data path into an approval: a `upeg://open` deep link
/// carrying `{"_upeg":{"approvedSteps":["gate"]}}` arrived at the
/// dispatch stamped `desktop` — a default approval surface — and lifted
/// a barrier no person had answered.
///
/// So both are removed first, which makes a `false` flag a real denial
/// rather than a missing overwrite, and only `approve` is re-inserted for
/// a `true` flag: the typed flag is the sole writer, and it says "the
/// whole call", which is the only thing a GUI confirmation ever means.
/// Non-object args carry no reserved key by construction (there is
/// nowhere to put one), so they pass through untouched.
fn shape_approval_arg(mut args: serde_json::Value, approve: bool) -> serde_json::Value {
    let Some(object) = args.as_object_mut() else {
        return args;
    };
    object.remove(APPROVE_RESERVED_ARG);
    strip_approved_steps(object);
    if approve {
        object.insert(
            APPROVE_RESERVED_ARG.to_string(),
            serde_json::Value::Bool(true),
        );
    }
    args
}

/// Drop `_upeg.approvedSteps` from a call's execution-context block,
/// leaving the rest of the block (and the block itself) alone.
///
/// Removing the whole `_upeg` block instead would throw away the
/// surface, board and principal annotations
/// [`apply_gui_context`] just stamped — this runs *after* that merge on
/// purpose, so that a board pin's saved args preset cannot smuggle the
/// key in as a default either.
fn strip_approved_steps(args: &mut serde_json::Map<String, serde_json::Value>) {
    if let Some(context) = args
        .get_mut(upeg_core::EXECUTION_CONTEXT_ARG)
        .and_then(serde_json::Value::as_object_mut)
    {
        context.remove(upeg_core::EXECUTION_CONTEXT_APPROVED_STEPS);
    }
}

/// Shared dispatch body: id validation, args parsing, desktop execution
/// context, approval shaping, registry dispatch.
///
/// `pub(crate)` so the streaming entry point (`api::dispatch_stream`)
/// runs the *same* path with a progress sink and a cancellation token
/// installed around it, rather than forking the arg-shaping logic.
pub(crate) fn dispatch_tool_impl(
    tool_id: &str,
    args_json: &str,
    board_key: Option<&str>,
    approve: bool,
) -> CanonicalToolResult {
    if let Err(err) = ToolId::parse_canonical(tool_id) {
        return CanonicalToolResult::error(
            INVALID_TOOL_ID_ERROR_CODE,
            format!("tool_id validation error: {err}"),
        );
    }

    let args: serde_json::Value = match serde_json::from_str(args_json) {
        Ok(v) => v,
        Err(err) => {
            return CanonicalToolResult::error(
                INVALID_ARGS_JSON_ERROR_CODE,
                format!("args_json parse error: {err}"),
            );
        }
    };

    let context = match gui_execution_context(tool_id, board_key) {
        Ok(context) => context,
        Err(err) => return *err,
    };
    // Approval shaping runs *after* the context merge so it is the last
    // writer: a board pin's saved args preset is caller-supplied data
    // too, and it must not be able to smuggle the reserved key in as a
    // default.
    let args = shape_approval_arg(apply_gui_context(args, &context), approve);

    match upeg_tools::dispatch_registered(tool_id, &args) {
        upeg_tools::RegisteredDispatch::Ran(result) => CanonicalToolResult::from(result),
        upeg_tools::RegisteredDispatch::NotFound => CanonicalToolResult::error(
            TOOL_NOT_FOUND_ERROR_CODE,
            format!("tool `{tool_id}` is not registered"),
        ),
        upeg_tools::RegisteredDispatch::Unimplemented => CanonicalToolResult::error(
            DISPATCH_UNIMPLEMENTED_ERROR_CODE,
            format!("dispatch not implemented for `{tool_id}`"),
        ),
    }
}

#[cfg(test)]
mod tests;

/// Native only: the Chain approval barrier it exercises is built by
/// `upeg-loader`, a dev dependency this crate takes on native targets
/// alone.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod chain_approval_tests;

#[cfg(test)]
mod file_input_policy_tests;
#[cfg(test)]
mod input_field_tests;
