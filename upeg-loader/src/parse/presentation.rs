//! Validation and lowering of tool result presentation metadata.

use std::collections::{BTreeMap, HashSet};

use upeg_core::{
    ActionBinding, ActionScope, ActionSuccess, OutputSpec, PRESENTATION_VERSION_V1,
    PresentationAction, PresentationColumn, ToolPresentation, validate_json_pointer,
};

use crate::{LoadError, model::PresentationToml};

/// Validate the manifest's presentation declaration and lower it into the
/// surface-neutral runtime form.
pub(crate) fn resolve_presentation(
    value: &PresentationToml,
    output_spec: &OutputSpec,
) -> Result<ToolPresentation, LoadError> {
    if value.version != PRESENTATION_VERSION_V1 {
        return Err(LoadError::InvalidPresentation(format!(
            "unsupported presentation version `{}`",
            value.version
        )));
    }
    let collection_fields = [
        value.output.is_some(),
        value.rows.is_some(),
        value.row_key.is_some(),
        !value.columns.is_empty(),
    ];
    if collection_fields.iter().any(|present| *present)
        && !collection_fields.iter().all(|present| *present)
    {
        return Err(LoadError::InvalidPresentation(
            "output, rows, row_key, and columns must be declared together".to_string(),
        ));
    }
    let check_pointer = |pointer: &str| {
        if validate_json_pointer(pointer) {
            Ok(())
        } else {
            Err(LoadError::InvalidPresentation(format!(
                "`{pointer}` is not a valid JSON Pointer"
            )))
        }
    };
    if let Some(pointer) = &value.rows {
        check_pointer(pointer)?;
    }
    if let Some(pointer) = &value.row_key {
        check_pointer(pointer)?;
    }
    if let Some(output_id) = &value.output {
        let Some(output) = output_spec
            .fields
            .iter()
            .find(|output| output.name == *output_id)
        else {
            return Err(LoadError::InvalidPresentation(format!(
                "presentation output `{output_id}` is not declared"
            )));
        };
        if output.kind != upeg_core::OutputKind::Json {
            return Err(LoadError::InvalidPresentation(format!(
                "presentation output `{output_id}` must have type `json`"
            )));
        }
    }
    let columns = value
        .columns
        .iter()
        .map(|column| {
            check_pointer(&column.pointer)?;
            Ok(PresentationColumn {
                label: column.label.clone(),
                pointer: column.pointer.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoadError>>()?;
    let mut action_ids = HashSet::new();
    let actions = value
        .actions
        .iter()
        .map(|action| {
            if action.id.is_empty() || !action_ids.insert(action.id.as_str()) {
                return Err(LoadError::InvalidPresentation(format!(
                    "presentation action id `{}` must be non-empty and unique",
                    action.id
                )));
            }
            let scope = match action.scope.as_str() {
                "row" if value.rows.is_some() => ActionScope::Row,
                "row" => {
                    return Err(LoadError::InvalidPresentation(
                        "row action requires a collection".to_string(),
                    ));
                }
                "result" => ActionScope::Result,
                other => {
                    return Err(LoadError::InvalidPresentation(format!(
                        "unknown action scope `{other}`"
                    )));
                }
            };
            let on_success = match action.on_success.as_deref() {
                None => None,
                Some("refresh_origin") => Some(ActionSuccess::RefreshOrigin),
                Some(other) => {
                    return Err(LoadError::InvalidPresentation(format!(
                        "unknown on_success `{other}`"
                    )));
                }
            };
            let bindings = action
                .bindings
                .iter()
                .map(|(target, binding)| {
                    if target == "_upeg" {
                        return Err(LoadError::InvalidPresentation(
                            "reserved input `_upeg` cannot be bound".to_string(),
                        ));
                    }
                    let resolved = match binding.from.as_str() {
                        "input" | "row" | "output" => {
                            let pointer = binding.pointer.clone().ok_or_else(|| {
                                LoadError::InvalidPresentation(format!(
                                    "binding `{target}` requires pointer"
                                ))
                            })?;
                            check_pointer(&pointer)?;
                            match binding.from.as_str() {
                                "input" => ActionBinding::Input { pointer },
                                "row" => ActionBinding::Row { pointer },
                                _ => ActionBinding::Output { pointer },
                            }
                        }
                        "constant" => ActionBinding::Constant {
                            value: binding.value.clone().ok_or_else(|| {
                                LoadError::InvalidPresentation(format!(
                                    "constant binding `{target}` requires value"
                                ))
                            })?,
                        },
                        other => {
                            return Err(LoadError::InvalidPresentation(format!(
                                "unknown binding source `{other}`"
                            )));
                        }
                    };
                    Ok((target.clone(), resolved))
                })
                .collect::<Result<BTreeMap<_, _>, LoadError>>()?;
            Ok(PresentationAction {
                id: action.id.clone(),
                scope,
                label: action.label.clone(),
                target_tool: action.target_tool.clone(),
                on_success,
                bindings,
            })
        })
        .collect::<Result<Vec<_>, LoadError>>()?;
    Ok(ToolPresentation {
        version: value.version,
        output: value.output.clone(),
        rows: value.rows.clone(),
        row_key: value.row_key.clone(),
        columns,
        actions,
    })
}
