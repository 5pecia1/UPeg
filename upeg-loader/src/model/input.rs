use upeg_core::{
    ChoiceOption, ChoiceSpec, DEFAULT_FILE_MAX_COUNT, FieldConstraints, FileInputPolicy,
    FileInputPolicyParams, IO_TYPE_LIST, InputKind, NumberConstraints, StringConstraints,
};

use super::{InputChoiceToml, InputDefaultToml};
use crate::LoadError;

pub(super) struct InputKindToml {
    pub(super) kind: String,
    pub(super) options: Option<Vec<InputChoiceToml>>,
    pub(super) file_policy: FileInputPolicyToml,
}

#[derive(Default)]
pub(super) struct FileInputPolicyToml {
    pub(super) extensions: Option<Vec<String>>,
    pub(super) max_count: Option<u32>,
    pub(super) max_file_bytes: Option<u64>,
    pub(super) max_total_bytes: Option<u64>,
}

impl InputKindToml {
    pub(super) fn into_core(self, name: &str) -> Result<InputKind, LoadError> {
        if self.kind != "file"
            && let Some(field) = self.file_policy.first_declared_field()
        {
            return Err(LoadError::UnexpectedInputFilePolicy {
                name: name.to_string(),
                kind: self.kind,
                field,
            });
        }

        let kind = self.kind;
        Ok(match kind.as_str() {
            "string" => scalar_input_kind(name, &kind, self.options, InputKind::String)?,
            "number" => scalar_input_kind(name, &kind, self.options, InputKind::Number)?,
            "integer" => scalar_input_kind(name, &kind, self.options, InputKind::Integer)?,
            "boolean" => scalar_input_kind(name, &kind, self.options, InputKind::Boolean)?,
            "options" => InputKind::Options(choice_spec_from_toml(self.options)?),
            "multi_options" => InputKind::MultiOptions(choice_spec_from_toml(self.options)?),
            "markdown" => scalar_input_kind(name, &kind, self.options, InputKind::Markdown)?,
            "json" => scalar_input_kind(name, &kind, self.options, InputKind::Json)?,
            "datetime" => scalar_input_kind(name, &kind, self.options, InputKind::DateTime)?,
            "file_path" => scalar_input_kind(name, &kind, self.options, InputKind::FilePath)?,
            "url" => scalar_input_kind(name, &kind, self.options, InputKind::Url)?,
            "file" => InputKind::File(self.file_policy.into_core(name, self.options)?),
            other => {
                return Err(LoadError::UnknownInputType {
                    name: name.to_string(),
                    kind: other.to_string(),
                    expected: IO_TYPE_LIST,
                });
            }
        })
    }
}

impl FileInputPolicyToml {
    fn first_declared_field(&self) -> Option<&'static str> {
        [
            (self.extensions.is_some(), "extensions"),
            (self.max_count.is_some(), "max_count"),
            (self.max_file_bytes.is_some(), "max_file_bytes"),
            (self.max_total_bytes.is_some(), "max_total_bytes"),
        ]
        .into_iter()
        .find_map(|(is_declared, field)| is_declared.then_some(field))
    }

    fn into_core(
        self,
        name: &str,
        options: Option<Vec<InputChoiceToml>>,
    ) -> Result<FileInputPolicy, LoadError> {
        if options.is_some() {
            return Err(LoadError::UnexpectedInputOptions {
                name: name.to_string(),
                kind: "file".to_string(),
            });
        }
        FileInputPolicy::try_from(FileInputPolicyParams {
            max_count: self.max_count.unwrap_or(DEFAULT_FILE_MAX_COUNT),
            extensions: self.extensions.unwrap_or_default(),
            max_file_bytes: self.max_file_bytes,
            max_total_bytes: self.max_total_bytes,
        })
        .map_err(|error| LoadError::InvalidInputSpec {
            detail: error.to_string(),
        })
    }
}

fn scalar_input_kind(
    name: &str,
    kind: &str,
    options: Option<Vec<InputChoiceToml>>,
    input_kind: InputKind,
) -> Result<InputKind, LoadError> {
    if options.is_some() {
        return Err(LoadError::UnexpectedInputOptions {
            name: name.to_string(),
            kind: kind.to_string(),
        });
    }
    Ok(input_kind)
}

pub(super) fn choice_spec_from_toml(
    options: Option<Vec<InputChoiceToml>>,
) -> Result<ChoiceSpec, upeg_core::InputSpecError> {
    let choices = options
        .unwrap_or_default()
        .into_iter()
        .map(|choice| ChoiceOption::new(choice.value, choice.label, choice.description))
        .collect::<Result<Vec<_>, _>>()?;
    ChoiceSpec::new(choices)
}

/// Which constraint slot an input kind's `default` lowers into.
///
/// `FieldConstraints` only has a numeric slot and a string slot, so
/// every input kind either maps to one of them or cannot carry a
/// default at all — this enum makes that mapping total instead of
/// leaving it as a scattered `match` with a silent fallthrough.
enum DefaultSlot {
    Numeric,
    Text,
    Unsupported,
}

impl DefaultSlot {
    const fn of(kind: &InputKind) -> Self {
        match kind {
            InputKind::Number | InputKind::Integer => Self::Numeric,
            InputKind::String
            | InputKind::Options(_)
            | InputKind::Markdown
            | InputKind::Json
            | InputKind::DateTime
            | InputKind::FilePath
            | InputKind::Url => Self::Text,
            InputKind::Boolean | InputKind::MultiOptions(_) | InputKind::File(_) => {
                Self::Unsupported
            }
        }
    }
}

/// Lower an input's declared `default` into [`FieldConstraints`].
///
/// A default whose TOML type does not match the input's slot is a load
/// error rather than a silent coercion: `default = "8080"` on a
/// `type = "number"` input is a manifest bug the author wants to hear
/// about at load time, not at the first dispatch.
pub(super) fn field_constraints_from_default(
    name: &str,
    kind: &InputKind,
    default: Option<InputDefaultToml>,
) -> Result<FieldConstraints, LoadError> {
    let Some(default) = default else {
        return Ok(FieldConstraints::default());
    };
    let kind_label = kind.label();
    match (DefaultSlot::of(kind), default) {
        (DefaultSlot::Numeric, InputDefaultToml::Number(value)) => Ok(FieldConstraints {
            number: Some(NumberConstraints {
                min: None,
                max: None,
                default: Some(value),
            }),
            string: None,
        }),
        (DefaultSlot::Text, InputDefaultToml::Text(value)) => Ok(FieldConstraints {
            number: None,
            string: Some(StringConstraints {
                regex: None,
                placeholder: None,
                default: Some(value),
            }),
        }),
        (DefaultSlot::Unsupported, _) => Err(LoadError::UnsupportedInputDefault {
            name: name.to_string(),
            kind: kind_label.to_string(),
        }),
        (DefaultSlot::Numeric | DefaultSlot::Text, _) => Err(LoadError::InvalidInputDefault {
            name: name.to_string(),
            kind: kind_label.to_string(),
        }),
    }
}
