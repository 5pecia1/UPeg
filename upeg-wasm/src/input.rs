use upeg_core::{
    ChoiceOption, ChoiceSpec, FileInputPolicy, FileInputPolicyParams, InputFieldSpec, InputKind,
    InputName, InputSpec, InputSpecError,
};
use upeg_plugin_api::{
    PluginChoiceOption, PluginFileInputPolicy, PluginInputField, PluginInputKind, PluginInputSpec,
};

use crate::{LoadError, WasmManifestError};

/// Convert plugin API DTO input metadata into the Core runtime input spec.
pub fn plugin_input_spec_to_core(spec: &PluginInputSpec) -> Result<InputSpec, WasmManifestError> {
    let fields = spec
        .fields
        .iter()
        .map(plugin_input_field_to_core)
        .collect::<Result<Vec<_>, _>>()?;
    InputSpec::new(fields).map_err(input_spec_error)
}

fn plugin_input_field_to_core(field: &PluginInputField) -> Result<InputFieldSpec, LoadError> {
    InputFieldSpec::new(
        InputName::new(field.name.clone()).map_err(input_spec_error)?,
        field.label.clone(),
        field.description.clone(),
        field.required,
        plugin_input_kind_to_core(&field.kind, field.file_policy.as_ref())?,
    )
    .map_err(input_spec_error)
}

fn plugin_input_kind_to_core(
    kind: &PluginInputKind,
    file_policy: Option<&PluginFileInputPolicy>,
) -> Result<InputKind, LoadError> {
    if !matches!(kind, PluginInputKind::File) && file_policy.is_some() {
        return Err(LoadError::InvalidInputSpec {
            detail: "`file_policy` is only valid for File inputs".to_string(),
        });
    }

    Ok(match kind {
        PluginInputKind::String => InputKind::String,
        PluginInputKind::Number => InputKind::Number,
        PluginInputKind::Integer => InputKind::Integer,
        PluginInputKind::Boolean => InputKind::Boolean,
        PluginInputKind::Options(options) => InputKind::Options(plugin_choices_to_core(options)?),
        PluginInputKind::MultiOptions(options) => {
            InputKind::MultiOptions(plugin_choices_to_core(options)?)
        }
        PluginInputKind::Markdown => InputKind::Markdown,
        PluginInputKind::Json => InputKind::Json,
        PluginInputKind::DateTime => InputKind::DateTime,
        PluginInputKind::FilePath => InputKind::FilePath,
        PluginInputKind::Url => InputKind::Url,
        PluginInputKind::File => InputKind::File(plugin_file_policy_to_core(file_policy)?),
    })
}

fn plugin_file_policy_to_core(
    policy: Option<&PluginFileInputPolicy>,
) -> Result<FileInputPolicy, LoadError> {
    let Some(policy) = policy else {
        return Ok(FileInputPolicy::default());
    };
    FileInputPolicy::try_from(FileInputPolicyParams {
        max_count: policy.max_count,
        extensions: policy.extensions.clone(),
        max_file_bytes: policy.max_file_bytes,
        max_total_bytes: policy.max_total_bytes,
    })
    .map_err(|error| LoadError::InvalidInputSpec {
        detail: error.to_string(),
    })
}

fn plugin_choices_to_core(options: &[PluginChoiceOption]) -> Result<ChoiceSpec, LoadError> {
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
        .map_err(input_spec_error)?;
    ChoiceSpec::new(options).map_err(input_spec_error)
}

fn input_spec_error(error: InputSpecError) -> LoadError {
    LoadError::InvalidInputSpec {
        detail: error.to_string(),
    }
}
