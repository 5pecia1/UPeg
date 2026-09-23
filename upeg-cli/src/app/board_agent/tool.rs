//! Effective inputs and prerequisite inspection, without executing any tool.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use upeg_core::{ArgsPreset, Invoker, ToolMeta};

use super::{BoardAgentTool, BoardToolReadiness, BoardToolReadinessStatus};
use upeg_runtime::{
    execution_requirements::tool_execution_requirements,
    readiness::{ToolReadinessContext, ToolReadinessStatus, inspect_tool_readiness},
};

pub(super) fn effective_tool_schema(meta: &ToolMeta, preset: Option<&ArgsPreset>) -> Value {
    let mut schema = meta.input_spec.to_json_schema_value();
    let mut supplied = Vec::new();
    if let Some(preset) = preset {
        let defaults = preset.to_object();
        if let Some(properties) = schema["properties"].as_object_mut() {
            for (name, value) in defaults {
                if let Some(property) = properties.get_mut(&name)
                    && valid_preset_field(meta, &name, &value)
                {
                    property["default"] = value;
                    supplied.push(name);
                }
            }
        }
    }
    if let Some(required) = schema["required"].as_array_mut() {
        required.retain(|name| {
            !name
                .as_str()
                .is_some_and(|name| supplied.iter().any(|key| key == name))
        });
    }
    schema
}

fn valid_preset_field(meta: &ToolMeta, name: &str, value: &Value) -> bool {
    let Some(field) = meta
        .input_spec
        .fields
        .iter()
        .find(|field| field.name.as_str() == name)
    else {
        return false;
    };
    let Ok(spec) = upeg_core::InputSpec::new(vec![field.clone()]) else {
        return false;
    };
    spec.validate_json_args(&Map::from_iter([(name.into(), value.clone())]))
        .is_ok()
}

pub(super) fn inspect(
    meta: &ToolMeta,
    preset: Option<&ArgsPreset>,
    directory: &Path,
    board_path: Option<&str>,
) -> BoardAgentTool {
    let input_schema = effective_tool_schema(meta, preset);
    let defaults = input_schema["properties"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, property)| {
            if input_schema["required"]
                .as_array()
                .is_some_and(|required| required.contains(&Value::String(name.clone())))
            {
                return None;
            }
            property
                .get("default")
                .map(|value| (name.clone(), value.clone()))
        })
        .collect::<Map<String, Value>>();
    let (working_directory, mut readiness) = prerequisites(meta, directory, board_path);
    if let Some(preset) = preset {
        let mut partial = meta.input_spec.clone();
        for field in &mut partial.fields {
            field.required = false;
        }
        if let Err(error) = super::super::args::validate_call_args(
            &partial,
            super::super::args::ReservedInputs::for_invoker(meta.invoker),
            &Value::Object(preset.to_object()),
        ) {
            readiness.status = BoardToolReadinessStatus::Unavailable;
            readiness
                .reasons
                .push(format!("Invalid saved inputs: {}", error.message()));
        }
    }
    let credentials = upeg_runtime::tool_credential_names(meta.id);
    if !credentials.is_empty() {
        if readiness.status == BoardToolReadinessStatus::Ready {
            readiness.status = BoardToolReadinessStatus::Unchecked;
        }
        readiness.reasons.push(format!(
            "Credentials are resolved during execution: {}",
            credentials.join(", ")
        ));
    }
    BoardAgentTool {
        id: meta.id.into(),
        description: meta.description.into(),
        input_schema,
        defaults: Value::Object(defaults),
        working_directory,
        readiness,
    }
}

fn prerequisites(
    meta: &ToolMeta,
    directory: &Path,
    board_path: Option<&str>,
) -> (Option<PathBuf>, BoardToolReadiness) {
    if tool_execution_requirements(meta.id).is_some()
        && let Some(readiness) = inspect_tool_readiness(
            meta.id,
            &ToolReadinessContext {
                working_directory: directory.to_path_buf(),
                search_path: board_path.map(Into::into),
            },
        )
    {
        let reason = match readiness.status {
            ToolReadinessStatus::Ready => readiness.command.map_or_else(
                || "External prerequisites checked; no command is declared.".into(),
                |command| format!("Executable checked: {command}. The command has not been run."),
            ),
            ToolReadinessStatus::MissingExecutable => readiness.command.map_or_else(
                || "Executable not found.".into(),
                |command| format!("Executable not found: {command}"),
            ),
            ToolReadinessStatus::MissingWorkingDirectory => {
                readiness.working_directory.as_ref().map_or_else(
                    || "Working directory does not exist.".into(),
                    |path| format!("Working directory does not exist: {}", path.display()),
                )
            }
            ToolReadinessStatus::UncheckedCredentialPath => {
                "Command lookup uses a credential-provided PATH, checked only at execution.".into()
            }
        };
        let status = match readiness.status {
            ToolReadinessStatus::Ready => BoardToolReadinessStatus::Ready,
            ToolReadinessStatus::MissingExecutable
            | ToolReadinessStatus::MissingWorkingDirectory => BoardToolReadinessStatus::Unavailable,
            ToolReadinessStatus::UncheckedCredentialPath => BoardToolReadinessStatus::Unchecked,
        };
        return (
            readiness.working_directory,
            BoardToolReadiness {
                status,
                reasons: vec![reason],
            },
        );
    }
    let readiness = match meta.invoker {
        Invoker::Function | Invoker::Static => BoardToolReadiness {
            status: BoardToolReadinessStatus::Ready,
            reasons: vec![
                "Tool is registered in this process; execution has not been performed.".into(),
            ],
        },
        _ => BoardToolReadiness {
            status: BoardToolReadinessStatus::Unchecked,
            reasons: vec![
                "Tool is registered; external runtime requirements have not been checked.".into(),
            ],
        },
    };
    (None, readiness)
}

#[cfg(test)]
mod tests;
