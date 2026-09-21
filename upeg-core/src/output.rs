//! Tool output specification — mirror of [`crate::input`].
//!
//! Outputs are typed result fields the tool's function produces. Surfaces
//! auto-render each field based on its [`OutputKind`]. See LEXICON v2.3 §2
//! (IoType variants) and the Pin rename + auto-render design doc for the
//! presentation rules.

use std::collections::HashSet;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::input::{
    ChoiceOption, ChoiceSpec, FieldConstraints, FileValue, InputSpecError, StaticChoiceOption,
    StaticFieldConstraints, file_value_to_json,
};

mod json_schema;
mod process_details;

pub use process_details::{ProcessErrorDetails, ProcessErrorStreams, ProcessTermination};

/// Validated specification of a Tool's output fields.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OutputSpec {
    pub fields: Vec<OutputFieldSpec>,
}

/// One typed output field.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OutputFieldSpec {
    pub name: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub kind: OutputKind,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "FieldConstraints::is_empty")
    )]
    pub constraints: FieldConstraints,
}

/// Closed output kind vocabulary. Mirrors [`crate::input::InputKind`] plus
/// `EmbeddedView`, which is meaningful only as an output (a Pin showing
/// an external page).
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum OutputKind {
    String,
    Number,
    Integer,
    Boolean,
    Options(ChoiceSpec),
    MultiOptions(ChoiceSpec),
    Markdown,
    Json,
    DateTime,
    FilePath,
    Url,
    File,
    EmbeddedView { url: String },
}

impl OutputKind {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::Options(_) => "options",
            Self::MultiOptions(_) => "multi_options",
            Self::Markdown => "markdown",
            Self::Json => "json",
            Self::DateTime => "datetime",
            Self::FilePath => "file_path",
            Self::Url => "url",
            Self::File => "file",
            Self::EmbeddedView { .. } => "embedded_view",
        }
    }

    fn validate_choice_constraints(&self) -> Result<(), OutputSpecError> {
        match self {
            Self::Options(choices) | Self::MultiOptions(choices) => {
                if choices.options.is_empty() {
                    Err(OutputSpecError::Input(InputSpecError::EmptyChoices))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }
}

/// Runtime output value emitted by a tool function for a single field.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum OutputValue {
    String(String),
    Number(serde_json::Number),
    Integer(i64),
    Boolean(bool),
    Options(String),
    MultiOptions(Vec<String>),
    Markdown(String),
    Json(serde_json::Value),
    DateTime(String),
    FilePath(String),
    Url(String),
    File(FileValue),
    EmbeddedView(String),
}

/// Canonical tool execution result.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ToolResult {
    Success(ToolSuccess),
    Failure(ToolFailure),
}

/// Successful tool execution with canonical outputs.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ToolSuccess {
    pub primary_output_id: Option<String>,
    pub outputs: Vec<OutputEntry>,
}

/// Failed tool execution with structured error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ToolFailure {
    pub error: ToolError,
}

/// Structured error for tool execution failure.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ToolError {
    pub code: String,
    pub message: String,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub details: Option<serde_json::Value>,
}

/// One output entry in canonical result.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OutputEntry {
    pub id: String,
    pub label: Option<String>,
    pub kind: OutputKind,
    pub value: OutputValue,
}

/// Errors that make an output specification invalid.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OutputSpecError {
    #[error("output name must not be empty")]
    EmptyName,
    #[error("output name `{name}` must not have leading or trailing whitespace")]
    PaddedName { name: String },
    #[error("output name `{name}` appears more than once")]
    DuplicateName { name: String },
    #[error(transparent)]
    Input(#[from] InputSpecError),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ToolResultError {
    #[error("duplicate output id: {id}")]
    DuplicateOutputId { id: String },
    #[error("primary_output_id required when outputs are non-empty")]
    MissingPrimaryOutputId,
    #[error("primary_output_id '{id}' does not reference any output")]
    InvalidPrimaryOutputId { id: String },
    #[error("primary_output_id must be None when outputs are empty")]
    UnexpectedPrimaryOutputId,
}

impl ToolResult {
    #[must_use]
    pub fn to_canonical_json(&self) -> serde_json::Value {
        match self {
            Self::Success(success) => success.to_canonical_json(),
            Self::Failure(failure) => failure.to_canonical_json(),
        }
    }
}

impl ToolSuccess {
    pub fn new(
        primary_output_id: Option<String>,
        outputs: Vec<OutputEntry>,
    ) -> Result<Self, ToolResultError> {
        let mut seen = HashSet::with_capacity(outputs.len());
        for entry in &outputs {
            if !seen.insert(entry.id.as_str()) {
                return Err(ToolResultError::DuplicateOutputId {
                    id: entry.id.clone(),
                });
            }
        }

        if outputs.is_empty() {
            if primary_output_id.is_some() {
                return Err(ToolResultError::UnexpectedPrimaryOutputId);
            }
        } else {
            match primary_output_id.as_deref() {
                None => return Err(ToolResultError::MissingPrimaryOutputId),
                Some(id) if !seen.contains(id) => {
                    return Err(ToolResultError::InvalidPrimaryOutputId { id: id.to_string() });
                }
                Some(_) => {}
            }
        }

        Ok(Self {
            primary_output_id,
            outputs,
        })
    }

    #[must_use]
    pub fn to_canonical_json(&self) -> serde_json::Value {
        let outputs = self
            .outputs
            .iter()
            .map(output_entry_to_json_value)
            .collect::<Vec<_>>();

        serde_json::json!({
            "ok": true,
            "primary_output_id": self.primary_output_id,
            "outputs": outputs,
        })
    }
}

impl ToolFailure {
    #[must_use]
    pub fn to_canonical_json(&self) -> serde_json::Value {
        let mut error = Map::from_iter([
            ("code".to_string(), Value::String(self.error.code.clone())),
            (
                "message".to_string(),
                Value::String(self.error.message.clone()),
            ),
        ]);
        if let Some(details) = &self.error.details {
            error.insert("details".to_string(), details.clone());
        }

        serde_json::json!({
            "ok": false,
            "error": Value::Object(error),
        })
    }
}

fn output_entry_to_json_value(entry: &OutputEntry) -> serde_json::Value {
    serde_json::json!({
        "id": entry.id,
        "label": entry.label,
        "kind": entry.kind.label(),
        "value": output_value_to_json_value(&entry.value),
    })
}

fn output_value_to_json_value(value: &OutputValue) -> serde_json::Value {
    match value {
        OutputValue::String(value)
        | OutputValue::Options(value)
        | OutputValue::Markdown(value)
        | OutputValue::DateTime(value)
        | OutputValue::FilePath(value)
        | OutputValue::Url(value)
        | OutputValue::EmbeddedView(value) => Value::String(value.clone()),
        OutputValue::Number(value) => Value::Number(value.clone()),
        OutputValue::Integer(value) => Value::Number((*value).into()),
        OutputValue::Boolean(value) => Value::Bool(*value),
        OutputValue::MultiOptions(values) => {
            Value::Array(values.iter().cloned().map(Value::String).collect())
        }
        OutputValue::Json(value) => value.clone(),
        // `file_value_to_json` is the single encoder for this object shape.
        // Its owned API requires the clone and keeps encoding drift in one place.
        OutputValue::File(file) => file_value_to_json(file.clone()),
    }
}

impl OutputValue {
    /// Native JSON representation used by presentation bindings. Unlike
    /// display text, this preserves number, boolean, array, object and file shapes.
    #[must_use]
    pub fn to_json_value(&self) -> serde_json::Value {
        output_value_to_json_value(self)
    }
}

impl OutputSpec {
    #[must_use]
    pub fn structured_content_from_text(&self, text: &str) -> Option<Value> {
        match self.fields.as_slice() {
            [] => None,
            [field] => {
                let value = output_value_from_text(&field.kind, text)?;
                Some(Value::Object(Map::from_iter([(field.name.clone(), value)])))
            }
            _ => match serde_json::from_str::<Value>(text).ok()? {
                Value::Object(map) => Some(Value::Object(map)),
                _ => None,
            },
        }
    }

    pub fn new(fields: Vec<OutputFieldSpec>) -> Result<Self, OutputSpecError> {
        let mut seen = HashSet::with_capacity(fields.len());
        for field in &fields {
            if field.name.is_empty() {
                return Err(OutputSpecError::EmptyName);
            }
            if field.name.trim() != field.name {
                return Err(OutputSpecError::PaddedName {
                    name: field.name.clone(),
                });
            }
            if !seen.insert(field.name.as_str()) {
                return Err(OutputSpecError::DuplicateName {
                    name: field.name.clone(),
                });
            }
            field.kind.validate_choice_constraints()?;
        }
        Ok(Self { fields })
    }

    pub const fn empty() -> Self {
        Self { fields: Vec::new() }
    }
}

fn output_value_from_text(kind: &OutputKind, text: &str) -> Option<Value> {
    let trimmed = text.trim();
    match kind {
        OutputKind::String
        | OutputKind::Options(_)
        | OutputKind::Markdown
        | OutputKind::DateTime
        | OutputKind::FilePath
        | OutputKind::Url
        | OutputKind::EmbeddedView { .. } => Some(Value::String(text.to_string())),
        OutputKind::Number => match serde_json::from_str::<Value>(trimmed).ok()? {
            value @ Value::Number(_) => Some(value),
            _ => None,
        },
        OutputKind::Integer => trimmed
            .parse::<i64>()
            .ok()
            .map(|value| Value::Number(value.into())),
        OutputKind::Boolean => trimmed.parse::<bool>().ok().map(Value::Bool),
        OutputKind::MultiOptions(_) => match serde_json::from_str::<Value>(trimmed).ok()? {
            Value::Array(values) if values.iter().all(Value::is_string) => {
                Some(Value::Array(values))
            }
            _ => None,
        },
        OutputKind::Json => serde_json::from_str::<Value>(text).ok(),
        OutputKind::File => match serde_json::from_str::<Value>(text).ok()? {
            value @ Value::Object(_) => Some(value),
            _ => None,
        },
    }
}

/// Inventory-safe static form emitted by compile-time registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticOutputSpec {
    pub fields: &'static [StaticOutputFieldSpec],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticOutputFieldSpec {
    pub name: &'static str,
    pub label: Option<&'static str>,
    pub description: Option<&'static str>,
    pub kind: StaticOutputKind,
    pub constraints: StaticFieldConstraints,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticOutputKind {
    String,
    Number,
    Integer,
    Boolean,
    Options(&'static [StaticChoiceOption]),
    MultiOptions(&'static [StaticChoiceOption]),
    Markdown,
    Json,
    DateTime,
    FilePath,
    Url,
    File,
    EmbeddedView { url: &'static str },
}

impl StaticOutputSpec {
    pub const fn empty() -> Self {
        Self { fields: &[] }
    }

    pub fn to_output_spec(self) -> Result<OutputSpec, OutputSpecError> {
        let mut fields = Vec::with_capacity(self.fields.len());
        for field in self.fields {
            fields.push(OutputFieldSpec {
                name: field.name.to_string(),
                label: field.label.map(str::to_string),
                description: field.description.map(str::to_string),
                kind: field.kind.to_output_kind()?,
                constraints: field.constraints.to_owned(),
            });
        }
        OutputSpec::new(fields)
    }
}

impl StaticOutputKind {
    fn to_output_kind(self) -> Result<OutputKind, InputSpecError> {
        Ok(match self {
            Self::String => OutputKind::String,
            Self::Number => OutputKind::Number,
            Self::Integer => OutputKind::Integer,
            Self::Boolean => OutputKind::Boolean,
            Self::Options(options) => OutputKind::Options(choice_spec_from_static(options)?),
            Self::MultiOptions(options) => {
                OutputKind::MultiOptions(choice_spec_from_static(options)?)
            }
            Self::Markdown => OutputKind::Markdown,
            Self::Json => OutputKind::Json,
            Self::DateTime => OutputKind::DateTime,
            Self::FilePath => OutputKind::FilePath,
            Self::Url => OutputKind::Url,
            Self::File => OutputKind::File,
            Self::EmbeddedView { url } => OutputKind::EmbeddedView {
                url: url.to_string(),
            },
        })
    }
}

fn choice_spec_from_static(
    options: &'static [StaticChoiceOption],
) -> Result<ChoiceSpec, InputSpecError> {
    let options = options
        .iter()
        .map(|option| {
            ChoiceOption::new(
                option.value,
                option.label.map(str::to_string),
                option.description.map(str::to_string),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    ChoiceSpec::new(options)
}

#[cfg(test)]
mod tests {
    include!("output_tests.inc.rs");
}
