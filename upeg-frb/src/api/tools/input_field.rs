//! Typed input-field DTOs crossing the FFI boundary.
//!
//! One field of a tool's `input_spec` as the Dart `GenericForm` sees it.
//! The rule this module exists to hold: **everything a form needs to
//! render a field generically travels here**, so a tool only earns a
//! bespoke Dart form when it needs an interaction model the generic form
//! does not have (live preview) — never because its metadata got lost in
//! transit.
//!
//! Concretely that means the DTO mirrors [`upeg_core::InputFieldSpec`]
//! without lossy folding:
//! - `description` rides along (it is the field's helper text),
//! - [`InputKind::Integer`] stays distinct from [`InputKind::Number`] so
//!   Dart can use an integer keyboard and reject `1.5`,
//! - choice options carry their `label` / `description`, not just values,
//! - per-field [`FieldConstraints`] (min/max/default, regex/placeholder/
//!   default) ride along so the form can seed defaults, show placeholders
//!   and pre-validate. Rust remains the enforcer; the Dart mirror only
//!   saves an obvious round-trip.

use upeg_core::{
    ChoiceOption, ChoiceSpec, FieldConstraints, InputFieldSpec, InputKind, NumberConstraints,
    StringConstraints,
};

use super::FileInputPolicyDto;

/// One typed input field as exposed to Dart.
///
/// `key` is the canonical input name used as the JSON map key when the
/// Dart form serialises its state for `dispatchTool`. `label` falls back
/// to `key` when the source `InputFieldSpec.label` is `None` so the Dart
/// side never has to deal with `Option<String>` for display strings.
/// `description` stays optional — it is helper text, not a display name,
/// and there is nothing to fall back to.
#[derive(Debug, Clone, PartialEq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct InputFieldDto {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub field_type: InputFieldType,
    pub required: bool,
    /// `None` when the field declares no constraints at all — the same
    /// distinction [`FieldConstraints::is_empty`] draws in core, kept
    /// rather than shipping an all-`null` struct for every plain field.
    pub constraints: Option<FieldConstraintsDto>,
}

/// Mirror of [`upeg_core::FieldConstraints`].
#[derive(Debug, Clone, PartialEq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct FieldConstraintsDto {
    pub number: Option<NumberConstraintsDto>,
    pub string: Option<StringConstraintsDto>,
}

/// Mirror of [`upeg_core::NumberConstraints`] — range and seed value for
/// `Number` / `Integer` fields.
#[derive(Debug, Clone, PartialEq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct NumberConstraintsDto {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub default: Option<f64>,
}

/// Mirror of [`upeg_core::StringConstraints`] — pattern, placeholder and
/// seed value for `String` fields.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct StringConstraintsDto {
    pub regex: Option<String>,
    pub placeholder: Option<String>,
    pub default: Option<String>,
}

/// Mirror of [`upeg_core::ChoiceOption`].
///
/// `label` falls back to `value` (same convention as
/// [`InputFieldDto::label`]) so a Dart dropdown/chip renders
/// `option.label` unconditionally; `description` stays optional.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ChoiceOptionDto {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

/// Sealed enum of input field kinds the Dart `GenericForm` knows how to
/// render. Mirrors [`upeg_core::InputKind`] one-to-one — every kind gets
/// its own typed widget on the Dart side:
/// - `Markdown` → multiline text (Markdown render is a separate row).
/// - `DateTime` → text + `showDatePicker` icon.
/// - `FilePath` → text + `file_picker` button.
/// - `Url` → text with `Uri.parse` validation.
/// - `Options(_)` → single-select Dropdown.
/// - `MultiOptions(_)` → `Wrap` of `FilterChip`s.
/// - `Json` → multiline (so users can paste structured payloads).
/// - `File` → real `file_picker` integration.
///
/// `Integer` is its own variant rather than folded into `Number`: the two
/// differ in keyboard, parsing and validation on the Dart side.
#[derive(Debug, Clone, PartialEq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum InputFieldType {
    Text,
    Number,
    Integer,
    Boolean,
    File { policy: FileInputPolicyDto },
    Select { options: Vec<ChoiceOptionDto> },
    Multiline,
    MultiOptions { options: Vec<ChoiceOptionDto> },
    DateTime,
    Markdown,
    FilePath,
    Url,
}

impl From<&InputFieldSpec> for InputFieldDto {
    fn from(field: &InputFieldSpec) -> Self {
        Self {
            key: field.name.as_str().to_string(),
            label: field
                .label
                .clone()
                .unwrap_or_else(|| field.name.as_str().to_string()),
            description: field.description.clone(),
            field_type: InputFieldType::from(&field.kind),
            required: field.required,
            constraints: FieldConstraintsDto::from_core(&field.constraints),
        }
    }
}

impl FieldConstraintsDto {
    /// `None` for an empty [`FieldConstraints`] so Dart can treat
    /// "no constraints declared" as one absent value.
    fn from_core(constraints: &FieldConstraints) -> Option<Self> {
        if constraints.is_empty() {
            return None;
        }
        Some(Self {
            number: constraints.number.as_ref().map(NumberConstraintsDto::from),
            string: constraints.string.as_ref().map(StringConstraintsDto::from),
        })
    }
}

impl From<&NumberConstraints> for NumberConstraintsDto {
    fn from(constraints: &NumberConstraints) -> Self {
        Self {
            min: constraints.min,
            max: constraints.max,
            default: constraints.default,
        }
    }
}

impl From<&StringConstraints> for StringConstraintsDto {
    fn from(constraints: &StringConstraints) -> Self {
        Self {
            regex: constraints.regex.clone(),
            placeholder: constraints.placeholder.clone(),
            default: constraints.default.clone(),
        }
    }
}

impl From<&ChoiceOption> for ChoiceOptionDto {
    fn from(option: &ChoiceOption) -> Self {
        Self {
            value: option.value.clone(),
            label: option.label.clone().unwrap_or_else(|| option.value.clone()),
            description: option.description.clone(),
        }
    }
}

fn choice_options(choices: &ChoiceSpec) -> Vec<ChoiceOptionDto> {
    choices.options.iter().map(ChoiceOptionDto::from).collect()
}

impl From<&InputKind> for InputFieldType {
    fn from(kind: &InputKind) -> Self {
        match kind {
            InputKind::String => Self::Text,
            InputKind::Markdown => Self::Markdown,
            InputKind::DateTime => Self::DateTime,
            InputKind::FilePath => Self::FilePath,
            InputKind::Url => Self::Url,
            InputKind::Number => Self::Number,
            InputKind::Integer => Self::Integer,
            InputKind::Boolean => Self::Boolean,
            InputKind::Json => Self::Multiline,
            InputKind::File(policy) => Self::File {
                policy: FileInputPolicyDto::from(policy),
            },
            InputKind::Options(choices) => Self::Select {
                options: choice_options(choices),
            },
            InputKind::MultiOptions(choices) => Self::MultiOptions {
                options: choice_options(choices),
            },
        }
    }
}
