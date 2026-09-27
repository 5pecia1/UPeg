//! Tool input specification — and the closed I/O type set every surface
//! shares. [`crate::output`] mirrors it.
//!
//! # Closed I/O type set
//!
//! Every Tool's inputs and outputs are declared from a closed set —
//! [`crate::types::IoType`] / [`InputKind`] / [`crate::output::OutputKind`]
//! — identical on all seven surfaces and serializable to text on CLI
//! stdout. Rendering is the extensible layer: adding a type means
//! updating renderers, never the contract.
//!
//! | Type | CLI representation |
//! |---|---|
//! | `String` / `Markdown` / `Url` | as-is |
//! | `Number` / `Integer` | as-is |
//! | `Boolean` | `true` / `false` |
//! | `Options` / `MultiOptions` | the selected value / comma-separated |
//! | `Json` | pretty-printed JSON |
//! | `Datetime` | ISO 8601 |
//! | `FilePath` | absolute path |
//! | `File` | canonical `FileValue` JSON (see `file_value`); written via `--out` |
//! | `EmbeddedView` | output only — the URL on non-GUI surfaces |
//!
//! Inline constraints attach at the declaration site and serve both
//! form rendering and validation: `min=`/`max=`/`default=` on
//! `Number`/`Integer`; `regex=`/`placeholder=`/`default=` on `String`;
//! a choice list in first-argument position on `Options`/`MultiOptions`.
//! Types outside the closed set are unsupported — such Tools fall back
//! to an iframe/launcher instead of an Inline pin, and GUI-only outputs
//! like `EmbeddedView` must restrict themselves via `surfaces`.

use std::collections::HashSet;
use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

mod adapter_error;
mod constraints;
mod file_base64;
#[cfg(test)]
mod file_base64_tests;
mod file_budget;
mod file_output_preflight;
mod file_policy;
mod file_policy_schema;
mod file_resource_limits;
mod file_structure;
mod file_validation;
mod file_value;
mod file_wire_schema;
pub mod json_schema;
mod numeric_constraint_schema;
mod string_constraint_schema;
mod validators;

pub use adapter_error::InputAdapterError;
pub use constraints::{
    FieldConstraints, NumberConstraints, StaticFieldConstraints, StaticNumberConstraints,
    StaticStringConstraints, StringConstraints,
};
pub use file_output_preflight::{
    FileOutputPreflightError, MAX_FILE_OUTPUT_METADATA_BYTES, MAX_FILE_OUTPUT_NESTING_DEPTH,
    MAX_FILE_OUTPUT_NODES, MAX_FILE_OUTPUT_RAW_BYTES, MAX_UNTRUSTED_OUTPUT_WIRE_BYTES,
    preflight_file_output_json, validate_file_output_tree,
};
pub use file_policy::{
    DEFAULT_FILE_MAX_COUNT, FileInputPolicy, FileInputPolicyError, FileInputPolicyParams,
    StaticFileInputPolicy,
};
pub use file_policy_schema::FilePolicySchemaError;
pub use file_resource_limits::{
    MAX_FILE_INPUT_COUNT, MAX_FILE_INPUT_METADATA_BYTES, MAX_FILE_INPUT_NODES,
    MAX_FILE_INPUT_RAW_BYTES,
};
pub use file_validation::FileInputValueError;
pub(crate) use file_value::file_value_to_json;
pub use file_value::{FileContent, FileValue};
pub use file_wire_schema::FileWireSchemaError;
pub(crate) use file_wire_schema::{FileWireContract, UPEG_FILE_WIRE_SCHEMA_KEY};

/// Canonical typed input specification shared by all upeg surfaces.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InputSpec {
    pub fields: Vec<InputFieldSpec>,
}

/// One typed input field. State lives in [`FormFieldState`], not here.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InputFieldSpec {
    pub name: InputName,
    pub label: Option<String>,
    pub description: Option<String>,
    pub required: bool,
    pub kind: InputKind,
    /// Optional per-field constraints (min/max, regex, default).
    /// Empty `FieldConstraints::default()` means none.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "FieldConstraints::is_empty")
    )]
    pub constraints: FieldConstraints,
}

/// Canonical input field name.
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InputName(String);

/// Closed input kind vocabulary for upeg-native tools.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum InputKind {
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
    /// File or directory body. The runtime value carries name, content
    /// (bytes or recursive entries), and an optional MIME type.
    File(FileInputPolicy),
}

/// Choice metadata for single- and multi-select inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChoiceSpec {
    pub options: Vec<ChoiceOption>,
}

/// One selectable choice.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChoiceOption {
    pub value: String,
    pub label: Option<String>,
    pub description: Option<String>,
}

/// Typed input value after validation/coercion.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum InputValue {
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
}

/// Draft value held by interactive form surfaces before final validation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DraftInputValue {
    Empty,
    Text(String),
    Boolean(bool),
    Options(Option<String>),
    MultiOptions(Vec<String>),
    File(Option<FileValue>),
}

/// Per-field validation status used by UI surfaces.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FieldValidation {
    Unknown,
    Valid,
    Invalid(String),
}

/// Ordered form state for a typed input spec.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FormState {
    pub fields: Vec<FormFieldState>,
}

/// One field's editable state.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FormFieldState {
    pub name: InputName,
    pub draft: DraftInputValue,
    pub validation: FieldValidation,
}

/// Inventory-safe static metadata emitted by compile-time registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticInputSpec {
    pub fields: &'static [StaticInputFieldSpec],
}

/// Static field metadata emitted by compile-time registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticInputFieldSpec {
    pub name: &'static str,
    pub label: Option<&'static str>,
    pub description: Option<&'static str>,
    pub required: bool,
    pub kind: StaticInputKind,
    pub constraints: StaticFieldConstraints,
}

/// Static closed input kind vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticInputKind {
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
    File(StaticFileInputPolicy),
}

/// Static choice metadata emitted by compile-time registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticChoiceOption {
    pub value: &'static str,
    pub label: Option<&'static str>,
    pub description: Option<&'static str>,
}

/// Errors that make an input specification invalid.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InputSpecError {
    #[error("input name must not be empty")]
    EmptyName,
    #[error("input name `{name}` must not have leading or trailing whitespace")]
    PaddedName { name: String },
    #[error("input name `{name}` appears more than once")]
    DuplicateName { name: InputName },
    #[error("choice inputs must declare at least one option")]
    EmptyChoices,
    #[error("choice option value must not be empty")]
    EmptyChoiceValue,
    #[error("choice option value `{value}` appears more than once")]
    DuplicateChoiceValue { value: String },
    #[error("input `{name}` declares a pattern upeg cannot compile: `{pattern}` — {detail}")]
    UncompilablePattern {
        name: InputName,
        pattern: String,
        detail: String,
    },
    #[error(transparent)]
    FilePolicy(#[from] FileInputPolicyError),
}

/// Errors that make supplied JSON arguments invalid for an input spec.
//
// Only `PartialEq` (not `Eq`) because `OutOfRange` carries `f64` actual /
// min / max values for accurate diagnostics. NaN can never reach this
// enum — numeric validators run after `value.as_f64()` succeeded — but
// `Eq` is unsound for `f64`.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum InputValueError {
    #[error("input `{name}` is required")]
    MissingRequired { name: InputName },
    #[error("input `{name}` expected {expected}, got {actual}")]
    TypeMismatch {
        name: InputName,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("input `{name}` must be one of {allowed_values:?}")]
    InvalidChoice {
        name: InputName,
        allowed_values: Vec<String>,
    },
    #[error(
        "input `{name}` contains invalid option `{invalid_value}`; allowed values: {allowed_values:?}"
    )]
    InvalidMultiChoice {
        name: InputName,
        allowed_values: Vec<String>,
        invalid_value: String,
    },
    #[error("input `{name}` value {actual} is out of range (min={min:?}, max={max:?})")]
    OutOfRange {
        name: InputName,
        min: Option<f64>,
        max: Option<f64>,
        actual: f64,
    },
    #[error("input `{name}` does not match required pattern `{pattern}`")]
    PatternMismatch { name: InputName, pattern: String },
    #[error("input `{name}` declares a pattern upeg cannot compile: `{pattern}`")]
    UncompilablePattern { name: InputName, pattern: String },
    #[error(transparent)]
    File(#[from] FileInputValueError),
}

impl InputSpec {
    pub fn new(fields: Vec<InputFieldSpec>) -> Result<Self, InputSpecError> {
        let mut seen = HashSet::with_capacity(fields.len());
        for field in &fields {
            field.kind.validate_choice_constraints()?;
            if !seen.insert(field.name.as_str()) {
                return Err(InputSpecError::DuplicateName {
                    name: field.name.clone(),
                });
            }
        }
        Ok(Self { fields })
    }

    pub const fn empty() -> Self {
        Self { fields: Vec::new() }
    }

    pub fn from_static_fields(
        fields: &'static [StaticInputFieldSpec],
    ) -> Result<Self, InputSpecError> {
        let mut converted = Vec::with_capacity(fields.len());
        for field in fields {
            converted.push(InputFieldSpec::with_constraints(
                InputName::new(field.name)?,
                field.label.map(str::to_string),
                field.description.map(str::to_string),
                field.required,
                field.kind.to_input_kind()?,
                field.constraints.to_owned(),
            )?);
        }
        Self::new(converted)
    }

    pub fn initial_form_state(&self) -> FormState {
        FormState {
            fields: self
                .fields
                .iter()
                .map(InputFieldSpec::initial_form_field_state)
                .collect(),
        }
    }

    pub fn validate_json_args(
        &self,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), InputValueError> {
        for field in &self.fields {
            let Some(value) = args.get(field.name.as_str()) else {
                if field.required {
                    return Err(InputValueError::MissingRequired {
                        name: field.name.clone(),
                    });
                }
                continue;
            };
            if value.is_null() && !matches!(field.kind, InputKind::Json) {
                if field.required {
                    return Err(InputValueError::MissingRequired {
                        name: field.name.clone(),
                    });
                }
                continue;
            }
            validate_json_value(field, value)?;
        }
        Ok(())
    }

    /// Validate only values supplied by a presentation binding. Missing required
    /// fields remain editable in the target form and are reported separately.
    pub fn validate_bound_json_args(
        &self,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), InputValueError> {
        for (name, value) in args {
            let Some(field) = self.fields.iter().find(|field| field.name.as_str() == name) else {
                continue;
            };
            if value.is_null() && !matches!(field.kind, InputKind::Json) {
                return Err(InputValueError::MissingRequired {
                    name: field.name.clone(),
                });
            }
            validate_json_value(field, value)?;
        }
        Ok(())
    }

    pub fn args_from_form_state(
        &self,
        state: &FormState,
    ) -> Result<serde_json::Value, InputAdapterError> {
        self.validate_form_state_shape(state)?;

        let mut args = serde_json::Map::with_capacity(self.fields.len());
        for field in &self.fields {
            let Some(form_field) = state
                .fields
                .iter()
                .find(|form_field| form_field.name == field.name)
            else {
                if field.required {
                    return Err(InputValueError::MissingRequired {
                        name: field.name.clone(),
                    }
                    .into());
                }
                continue;
            };

            let Some(value) = json_value_from_draft(field, &form_field.draft)? else {
                if field.required {
                    return Err(InputValueError::MissingRequired {
                        name: field.name.clone(),
                    }
                    .into());
                }
                continue;
            };
            validate_json_value(field, &value)?;
            args.insert(field.name.as_str().to_string(), value);
        }
        Ok(serde_json::Value::Object(args))
    }

    fn validate_form_state_shape(&self, state: &FormState) -> Result<(), InputAdapterError> {
        let mut seen = HashSet::with_capacity(state.fields.len());
        for form_field in &state.fields {
            if !self
                .fields
                .iter()
                .any(|field| field.name == form_field.name)
            {
                return Err(InputAdapterError::UnknownFormField {
                    name: form_field.name.clone(),
                });
            }
            if !seen.insert(form_field.name.as_str()) {
                return Err(InputAdapterError::DuplicateFormField {
                    name: form_field.name.clone(),
                });
            }
        }
        Ok(())
    }
}

impl InputFieldSpec {
    pub fn new(
        name: InputName,
        label: Option<String>,
        description: Option<String>,
        required: bool,
        kind: InputKind,
    ) -> Result<Self, InputSpecError> {
        Self::with_constraints(
            name,
            label,
            description,
            required,
            kind,
            FieldConstraints::default(),
        )
    }

    pub fn with_constraints(
        name: InputName,
        label: Option<String>,
        description: Option<String>,
        required: bool,
        kind: InputKind,
        constraints: FieldConstraints,
    ) -> Result<Self, InputSpecError> {
        kind.validate_choice_constraints()?;
        reject_uncompilable_pattern(&name, &constraints)?;
        Ok(Self {
            name,
            label,
            description,
            required,
            kind,
            constraints,
        })
    }

    fn initial_form_field_state(&self) -> FormFieldState {
        FormFieldState {
            name: self.name.clone(),
            draft: self.kind.initial_draft_value(),
            validation: FieldValidation::Unknown,
        }
    }
}

impl InputName {
    pub fn new(name: impl Into<String>) -> Result<Self, InputSpecError> {
        let name = name.into();
        if name.is_empty() {
            return Err(InputSpecError::EmptyName);
        }
        if name.trim() != name {
            return Err(InputSpecError::PaddedName { name });
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for InputName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for InputName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<String> for InputName {
    type Error = InputSpecError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for InputName {
    type Error = InputSpecError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<InputName> for String {
    fn from(value: InputName) -> Self {
        value.into_string()
    }
}

#[cfg(feature = "serde")]
impl Serialize for InputName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for InputName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl InputKind {
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
            Self::File(_) => "file",
        }
    }

    fn expected_json_kind(&self) -> &'static str {
        match self {
            Self::String
            | Self::Options(_)
            | Self::Markdown
            | Self::DateTime
            | Self::FilePath
            | Self::Url => "string",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::MultiOptions(_) => "string array",
            Self::Json => "json value",
            Self::File(_) => "object",
        }
    }

    fn initial_draft_value(&self) -> DraftInputValue {
        match self {
            Self::Boolean => DraftInputValue::Boolean(false),
            Self::Options(_) => DraftInputValue::Options(None),
            Self::MultiOptions(_) => DraftInputValue::MultiOptions(Vec::new()),
            Self::File(_) => DraftInputValue::File(None),
            Self::String
            | Self::Number
            | Self::Integer
            | Self::Markdown
            | Self::Json
            | Self::DateTime
            | Self::FilePath
            | Self::Url => DraftInputValue::Empty,
        }
    }

    fn validate_choice_constraints(&self) -> Result<(), InputSpecError> {
        match self {
            Self::Options(choices) | Self::MultiOptions(choices) => choices.validate(),
            Self::String
            | Self::Number
            | Self::Integer
            | Self::Boolean
            | Self::Markdown
            | Self::Json
            | Self::DateTime
            | Self::FilePath
            | Self::Url
            | Self::File(_) => Ok(()),
        }
    }
}

impl ChoiceSpec {
    pub fn new(options: Vec<ChoiceOption>) -> Result<Self, InputSpecError> {
        let spec = Self { options };
        spec.validate()?;
        Ok(spec)
    }

    pub fn contains(&self, value: &str) -> bool {
        self.options.iter().any(|option| option.value == value)
    }

    pub fn allowed_values(&self) -> Vec<String> {
        self.options
            .iter()
            .map(|option| option.value.clone())
            .collect()
    }

    fn validate(&self) -> Result<(), InputSpecError> {
        if self.options.is_empty() {
            return Err(InputSpecError::EmptyChoices);
        }
        let mut seen = HashSet::with_capacity(self.options.len());
        for option in &self.options {
            if option.value.is_empty() {
                return Err(InputSpecError::EmptyChoiceValue);
            }
            if !seen.insert(option.value.as_str()) {
                return Err(InputSpecError::DuplicateChoiceValue {
                    value: option.value.clone(),
                });
            }
        }
        Ok(())
    }
}

impl ChoiceOption {
    pub fn new(
        value: impl Into<String>,
        label: Option<String>,
        description: Option<String>,
    ) -> Result<Self, InputSpecError> {
        let option = Self {
            value: value.into(),
            label,
            description,
        };
        if option.value.is_empty() {
            return Err(InputSpecError::EmptyChoiceValue);
        }
        Ok(option)
    }
}

impl InputValue {
    pub fn into_json_value(self) -> serde_json::Value {
        match self {
            Self::String(value)
            | Self::Options(value)
            | Self::Markdown(value)
            | Self::DateTime(value)
            | Self::FilePath(value)
            | Self::Url(value) => serde_json::Value::String(value),
            Self::Number(value) => serde_json::Value::Number(value),
            Self::Integer(value) => serde_json::Value::Number(value.into()),
            Self::Boolean(value) => serde_json::Value::Bool(value),
            Self::MultiOptions(values) => serde_json::Value::Array(
                values.into_iter().map(serde_json::Value::String).collect(),
            ),
            Self::Json(value) => value,
            Self::File(file) => file_value_to_json(file),
        }
    }
}

impl StaticInputSpec {
    pub const fn empty() -> Self {
        Self { fields: &[] }
    }

    pub fn to_input_spec(self) -> Result<InputSpec, InputAdapterError> {
        Ok(InputSpec::from_static_fields(self.fields)?)
    }
}

impl StaticInputKind {
    fn to_input_kind(self) -> Result<InputKind, InputSpecError> {
        Ok(match self {
            Self::String => InputKind::String,
            Self::Number => InputKind::Number,
            Self::Integer => InputKind::Integer,
            Self::Boolean => InputKind::Boolean,
            Self::Options(options) => InputKind::Options(choice_spec_from_static(options)?),
            Self::MultiOptions(options) => {
                InputKind::MultiOptions(choice_spec_from_static(options)?)
            }
            Self::Markdown => InputKind::Markdown,
            Self::Json => InputKind::Json,
            Self::DateTime => InputKind::DateTime,
            Self::FilePath => InputKind::FilePath,
            Self::Url => InputKind::Url,
            Self::File(policy) => {
                InputKind::File(FileInputPolicy::try_from(FileInputPolicyParams {
                    max_count: policy.max_count,
                    extensions: policy
                        .extensions
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                    max_file_bytes: policy.max_file_bytes,
                    max_total_bytes: policy.max_total_bytes,
                })?)
            }
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

use validators::validate_json_value;

fn json_value_from_draft(
    field: &InputFieldSpec,
    draft: &DraftInputValue,
) -> Result<Option<serde_json::Value>, InputAdapterError> {
    match (&field.kind, draft) {
        (_, DraftInputValue::Empty) => Ok(None),
        (
            InputKind::String
            | InputKind::Markdown
            | InputKind::DateTime
            | InputKind::FilePath
            | InputKind::Url,
            DraftInputValue::Text(value),
        ) => Ok(Some(serde_json::Value::String(value.clone()))),
        (InputKind::Number, DraftInputValue::Text(value)) => number_from_text(&field.name, value),
        (InputKind::Integer, DraftInputValue::Text(value)) => integer_from_text(&field.name, value),
        (InputKind::Json, DraftInputValue::Text(value)) => json_from_text(&field.name, value),
        (InputKind::Boolean, DraftInputValue::Boolean(value)) => {
            Ok(Some(serde_json::Value::Bool(*value)))
        }
        (InputKind::Options(_), DraftInputValue::Options(Some(value))) => {
            Ok(Some(serde_json::Value::String(value.clone())))
        }
        (InputKind::Options(_), DraftInputValue::Options(None)) => Ok(None),
        (InputKind::MultiOptions(_), DraftInputValue::MultiOptions(values)) => {
            if values.is_empty() {
                Ok(None)
            } else {
                Ok(Some(serde_json::Value::Array(
                    values
                        .iter()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect(),
                )))
            }
        }
        (InputKind::File(_), DraftInputValue::File(None)) => Ok(None),
        (InputKind::File(_), DraftInputValue::File(Some(file))) => {
            Ok(Some(file_value_to_json(file.clone())))
        }
        _ => Err(InputAdapterError::DraftKindMismatch {
            name: field.name.clone(),
            expected: expected_draft_kind(&field.kind),
            actual: draft.kind_label(),
        }),
    }
}

fn number_from_text(
    name: &InputName,
    value: &str,
) -> Result<Option<serde_json::Value>, InputAdapterError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let Ok(parsed) = trimmed.parse::<f64>() else {
        return Err(InputAdapterError::InvalidNumber {
            name: name.clone(),
            value: value.to_string(),
        });
    };
    let Some(number) = serde_json::Number::from_f64(parsed) else {
        return Err(InputAdapterError::InvalidNumber {
            name: name.clone(),
            value: value.to_string(),
        });
    };
    Ok(Some(serde_json::Value::Number(number)))
}

fn integer_from_text(
    name: &InputName,
    value: &str,
) -> Result<Option<serde_json::Value>, InputAdapterError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let Ok(parsed) = trimmed.parse::<i64>() else {
        return Err(InputAdapterError::InvalidInteger {
            name: name.clone(),
            value: value.to_string(),
        });
    };
    Ok(Some(serde_json::Value::Number(parsed.into())))
}

fn json_from_text(
    name: &InputName,
    value: &str,
) -> Result<Option<serde_json::Value>, InputAdapterError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .map(Some)
        .map_err(|err| InputAdapterError::InvalidJson {
            name: name.clone(),
            detail: err.to_string(),
        })
}

fn expected_draft_kind(kind: &InputKind) -> &'static str {
    match kind {
        InputKind::String
        | InputKind::Number
        | InputKind::Integer
        | InputKind::Markdown
        | InputKind::Json
        | InputKind::DateTime
        | InputKind::FilePath
        | InputKind::Url => "text",
        InputKind::Boolean => "boolean",
        InputKind::Options(_) => "options",
        InputKind::MultiOptions(_) => "multi_options",
        InputKind::File(_) => "file",
    }
}

impl DraftInputValue {
    fn kind_label(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Text(_) => "text",
            Self::Boolean(_) => "boolean",
            Self::Options(_) => "options",
            Self::MultiOptions(_) => "multi_options",
            Self::File(_) => "file",
        }
    }
}

/// Reject a declared pattern the validator could never apply.
///
/// MCP servers publish ECMA-262 `pattern`s, where look-around and
/// backreferences are legal; upeg validates with the `regex` crate,
/// which supports neither. Compiling once here — at the single choke
/// point every `InputFieldSpec` passes through — turns that mismatch
/// into an honest declaration error, which an MCP import reports as a
/// skipped tool with a reason. Before this, the failure surfaced at
/// validation time as `PatternMismatch`, blaming whatever value the
/// user typed and leaving the field permanently unsatisfiable.
fn reject_uncompilable_pattern(
    name: &InputName,
    constraints: &FieldConstraints,
) -> Result<(), InputSpecError> {
    let Some(pattern) = constraints
        .string
        .as_ref()
        .and_then(|string| string.regex.as_deref())
    else {
        return Ok(());
    };
    regex::Regex::new(pattern)
        .map(|_| ())
        .map_err(|error| InputSpecError::UncompilablePattern {
            name: name.clone(),
            pattern: pattern.to_string(),
            detail: error.to_string(),
        })
}

/// `number` as an `i64`, or `None` when it has a fractional part or
/// falls outside `i64`.
///
/// `NumberConstraints` stores every bound as `f64` because `number` and
/// `integer` share the struct, so the integer side has to ask this
/// question in two places: when writing JSON Schema (emit `20`, not
/// `20.0`) and when validating a supplied value (accept `20.0`, which
/// JSON cannot distinguish from `20`). One helper keeps the two answers
/// identical.
#[allow(
    clippy::float_cmp,
    clippy::float_cmp_const,
    reason = "integrality test, not an approximate equality: f64::fract is exact, so `== 0.0` is the precise question being asked"
)]
fn whole_i64(number: f64) -> Option<i64> {
    // `i64::MAX as f64` rounds *up* to 2^63, so the range admits one
    // value an `i64` cannot hold. The saturating `as` cast maps it to
    // `i64::MAX` — the closest representable answer, and never
    // undefined.
    let in_range = (i64::MIN as f64..=i64::MAX as f64).contains(&number);
    (number.fract() == 0.0 && in_range).then_some(number as i64)
}

fn json_value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod constraint_tests;
