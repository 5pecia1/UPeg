//! Validation and lowering of tool result presentation metadata.

use std::collections::{BTreeMap, HashSet};

use upeg_core::{
    ActionBinding, ActionScope, ActionSuccess, OutputSpec, PRESENTATION_VERSION_V1,
    PresentationAction, PresentationColumn, PresentationDetail, PresentationNotices,
    PresentationStatus, ToolPresentation, validate_json_pointer,
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
    let has_rich = value.title_pointer.is_some()
        || value.subtitle_pointer.is_some()
        || value.status.is_some()
        || !value.summary.is_empty()
        || value.notices.is_some()
        || value.detail.is_some()
        || value.row_detail.is_some()
        || value.empty_message_pointer.is_some()
        || value
            .actions
            .iter()
            .any(|action| action.enabled_pointer.is_some());
    let has_collection =
        value.rows.is_some() && value.row_key.is_some() && !value.columns.is_empty();
    if (value.rows.is_some() || value.row_key.is_some() || !value.columns.is_empty())
        && !has_collection
        || value.output.is_some() && !has_collection && !has_rich
        || (has_collection || has_rich) && value.output.is_none()
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
            if let Some(pointer) = &column.tone_pointer {
                check_pointer(pointer)?;
            }
            Ok(PresentationColumn {
                label: column.label.clone(),
                pointer: column.pointer.clone(),
                tone_pointer: column.tone_pointer.clone(),
                filterable: column.filterable,
            })
        })
        .collect::<Result<Vec<_>, LoadError>>()?;
    let lower_columns = |items: &[crate::model::presentation::PresentationColumnToml]| {
        items
            .iter()
            .map(|item| {
                check_pointer(&item.pointer)?;
                if let Some(pointer) = &item.tone_pointer {
                    check_pointer(pointer)?;
                }
                Ok(PresentationColumn {
                    label: item.label.clone(),
                    pointer: item.pointer.clone(),
                    tone_pointer: item.tone_pointer.clone(),
                    filterable: item.filterable,
                })
            })
            .collect::<Result<Vec<_>, LoadError>>()
    };
    let lower_detail = |detail: &crate::model::presentation::PresentationDetailToml| -> Result<PresentationDetail, LoadError> {
        for pointer in [&detail.markdown_pointer, &detail.diff_pointer].into_iter().flatten() {
            check_pointer(pointer)?;
        }
        Ok(PresentationDetail { fields: lower_columns(&detail.fields)?,
            markdown_pointer: detail.markdown_pointer.clone(), diff_pointer: detail.diff_pointer.clone() })
    };
    for pointer in [
        &value.title_pointer,
        &value.subtitle_pointer,
        &value.empty_message_pointer,
    ]
    .into_iter()
    .flatten()
    {
        check_pointer(pointer)?;
    }
    if let Some(status) = &value.status {
        check_pointer(&status.label_pointer)?;
        check_pointer(&status.tone_pointer)?;
    }
    if let Some(notices) = &value.notices {
        check_pointer(&notices.rows_pointer)?;
        check_pointer(&notices.text_pointer)?;
        check_pointer(&notices.severity_pointer)?;
    }
    if value.row_detail.is_some() && !has_collection {
        return Err(LoadError::InvalidPresentation(
            "row_detail requires a collection".to_string(),
        ));
    }
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
            if let Some(pointer) = &action.enabled_pointer {
                if scope != ActionScope::Result {
                    return Err(LoadError::InvalidPresentation(
                        "enabled_pointer requires a result action".to_string(),
                    ));
                }
                check_pointer(pointer)?;
            }
            if let Some(pointer) = &action.disabled_reason_pointer {
                if action.enabled_pointer.is_none() {
                    return Err(LoadError::InvalidPresentation(
                        "disabled_reason_pointer requires enabled_pointer".to_string(),
                    ));
                }
                check_pointer(pointer)?;
            }
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
                enabled_pointer: action.enabled_pointer.clone(),
                disabled_reason_pointer: action.disabled_reason_pointer.clone(),
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
        title_pointer: value.title_pointer.clone(),
        subtitle_pointer: value.subtitle_pointer.clone(),
        status: value.status.as_ref().map(|status| PresentationStatus {
            label_pointer: status.label_pointer.clone(),
            tone_pointer: status.tone_pointer.clone(),
        }),
        summary: lower_columns(&value.summary)?,
        notices: value.notices.as_ref().map(|notices| PresentationNotices {
            rows_pointer: notices.rows_pointer.clone(),
            text_pointer: notices.text_pointer.clone(),
            severity_pointer: notices.severity_pointer.clone(),
        }),
        detail: value.detail.as_ref().map(lower_detail).transpose()?,
        row_detail: value.row_detail.as_ref().map(lower_detail).transpose()?,
        empty_message_pointer: value.empty_message_pointer.clone(),
    })
}
