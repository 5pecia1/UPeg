//! Optional presentation metadata and pure JSON binding resolution.
//!
//! Contract: a Tool still accepts one input object and returns one
//! canonical `ToolResult`; presentation metadata only describes how a
//! surface displays a JSON output and offers a follow-up Tool form. It
//! introduces no new invoker, I/O kind, workflow engine, or
//! domain-specific widget, and Tools without it behave as before. v1
//! supports a collection of JSON objects with a stable string/integer
//! row key and scalar columns, addressed by an output id plus JSON
//! Pointers for rows/key/columns. Missing or duplicate row keys disable
//! row actions and retain the raw JSON with a diagnostic; partial or
//! restored results are never presented as fresh complete data.
//!
//! Actions have row or result scope and a statically declared target
//! Tool id. Input bindings copy native JSON values from the invocation
//! input, the selected row, the output-id-to-value map, or a literal —
//! a binding can never populate reserved `_upeg` context. Missing
//! declared sources and invalid bound values are diagnosed; required
//! fields left unbound are filled in the target's existing form, and
//! opening a form never executes it. Capability, surface, approval, and
//! input validation stay with the dispatcher; text stays data.
//!
//! Invocation context: the surface retains a transient identity —
//! input, host identity, result generation, selected row key. A
//! persisted result may display but cannot seed actions until the Tool
//! is run again; follow-up forms keep the original context, not global
//! state. The `effect` hint (`read`/`write`/`unknown`, default unknown)
//! is author metadata, not authorization; only a declared `read` origin
//! may auto re-run via `refresh_origin` after a successful action. The
//! origin is valid only while its frame, input, host, and generation
//! are unchanged, and the refresh response re-checks that. A failed
//! refresh does not turn a successful write into a failure; transport
//! loss leaves a write outcome unknown — the surface marks the original
//! result stale and lets the user re-read.

use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use crate::InputSpec;

mod view;
pub use view::{
    PresentationTone, ResolvedAction, ResolvedDetail, ResolvedField, ResolvedNotice,
    ResolvedStatus, ResolvedView, resolve_action_availability, resolve_view,
};

pub const PRESENTATION_VERSION_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolEffect {
    Read,
    Write,
    #[default]
    Unknown,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolPresentation {
    pub version: u16,
    pub output: Option<String>,
    pub rows: Option<String>,
    pub row_key: Option<String>,
    pub columns: Vec<PresentationColumn>,
    pub actions: Vec<PresentationAction>,
    pub title_pointer: Option<String>,
    pub subtitle_pointer: Option<String>,
    pub status: Option<PresentationStatus>,
    pub summary: Vec<PresentationColumn>,
    pub notices: Option<PresentationNotices>,
    pub detail: Option<PresentationDetail>,
    pub row_detail: Option<PresentationDetail>,
    pub empty_message_pointer: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationColumn {
    pub label: String,
    pub pointer: String,
    pub tone_pointer: Option<String>,
    pub filterable: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationStatus {
    pub label_pointer: String,
    pub tone_pointer: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationNotices {
    pub rows_pointer: String,
    pub text_pointer: String,
    pub severity_pointer: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationDetail {
    pub fields: Vec<PresentationColumn>,
    pub markdown_pointer: Option<String>,
    pub diff_pointer: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionScope {
    Row,
    Result,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionSuccess {
    RefreshOrigin,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationAction {
    pub id: String,
    pub scope: ActionScope,
    pub label: String,
    pub target_tool: String,
    pub on_success: Option<ActionSuccess>,
    pub bindings: BTreeMap<String, ActionBinding>,
    pub enabled_pointer: Option<String>,
    pub disabled_reason_pointer: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionBinding {
    Input { pointer: String },
    Row { pointer: String },
    Output { pointer: String },
    Constant { value: Value },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationRow {
    pub key: String,
    pub value: Value,
    pub cells: Vec<Value>,
    pub cell_tones: Vec<Option<PresentationTone>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowsResolution {
    pub rows: Vec<PresentationRow>,
    pub diagnostics: Vec<String>,
    pub row_actions_enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BindingResolution {
    pub values: BTreeMap<String, Value>,
    pub diagnostics: Vec<String>,
    pub unbound_required_inputs: Vec<String>,
}

pub fn resolve_rows(presentation: &ToolPresentation, outputs: &Value) -> RowsResolution {
    let mut diagnostics = Vec::new();
    let Some(output_id) = presentation.output.as_deref() else {
        return RowsResolution {
            rows: Vec::new(),
            diagnostics,
            row_actions_enabled: false,
        };
    };
    let Some(rows_pointer) = presentation.rows.as_deref() else {
        return RowsResolution {
            rows: Vec::new(),
            diagnostics,
            row_actions_enabled: false,
        };
    };
    let Some(row_key_pointer) = presentation.row_key.as_deref() else {
        return RowsResolution {
            rows: Vec::new(),
            diagnostics,
            row_actions_enabled: false,
        };
    };
    let Some(output) = outputs.get(output_id) else {
        diagnostics.push(format!("output `{output_id}` is missing"));
        return RowsResolution {
            rows: Vec::new(),
            diagnostics,
            row_actions_enabled: false,
        };
    };
    let Some(values) = output.pointer(rows_pointer).and_then(Value::as_array) else {
        diagnostics.push(format!(
            "rows pointer `{rows_pointer}` does not resolve to an array"
        ));
        return RowsResolution {
            rows: Vec::new(),
            diagnostics,
            row_actions_enabled: false,
        };
    };
    let mut seen = HashSet::new();
    let mut rows = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let Some(key_value) = value.pointer(row_key_pointer) else {
            diagnostics.push(format!("row {index} has no key at `{row_key_pointer}`"));
            continue;
        };
        let key = match key_value {
            Value::String(value) => value.clone(),
            Value::Number(value) if value.is_i64() || value.is_u64() => value.to_string(),
            _ => {
                diagnostics.push(format!("row {index} key must be a string or integer"));
                continue;
            }
        };
        if !seen.insert(key.clone()) {
            diagnostics.push(format!("row key `{key}` is duplicated"));
        }
        let cells = presentation
            .columns
            .iter()
            .map(|column| {
                value
                    .pointer(&column.pointer)
                    .cloned()
                    .unwrap_or(Value::Null)
            })
            .collect();
        let cell_tones = presentation
            .columns
            .iter()
            .map(|column| {
                column
                    .tone_pointer
                    .as_deref()
                    .and_then(|pointer| value.pointer(pointer))
                    .and_then(Value::as_str)
                    .and_then(PresentationTone::parse)
            })
            .collect();
        rows.push(PresentationRow {
            key,
            value: value.clone(),
            cells,
            cell_tones,
        });
    }
    RowsResolution {
        row_actions_enabled: diagnostics.is_empty(),
        rows,
        diagnostics,
    }
}

pub fn resolve_bindings(
    action: &PresentationAction,
    current_inputs: &Value,
    selected_row: Option<&Value>,
    outputs: &Value,
    target_inputs: &InputSpec,
) -> BindingResolution {
    let mut values = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for (target, binding) in &action.bindings {
        if target == "_upeg" {
            diagnostics.push("reserved input `_upeg` cannot be bound".to_string());
            continue;
        }
        if !target_inputs
            .fields
            .iter()
            .any(|field| field.name.as_str() == target)
        {
            diagnostics.push(format!("target input `{target}` is not declared"));
            continue;
        }
        let resolved = match binding {
            ActionBinding::Input { pointer } => current_inputs.pointer(pointer).cloned(),
            ActionBinding::Row { pointer } => {
                selected_row.and_then(|row| row.pointer(pointer)).cloned()
            }
            ActionBinding::Output { pointer } => outputs.pointer(pointer).cloned(),
            ActionBinding::Constant { value } => Some(value.clone()),
        };
        match resolved {
            Some(Value::Null) => {}
            Some(value) => {
                values.insert(target.clone(), value);
            }
            None => diagnostics.push(format!("binding for `{target}` has no source value")),
        }
    }
    let map = values
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if let Err(error) = target_inputs.validate_bound_json_args(&map) {
        diagnostics.push(error.to_string());
    }
    let unbound_required_inputs = target_inputs
        .fields
        .iter()
        .filter(|field| field.required && !values.contains_key(field.name.as_str()))
        .map(|field| field.name.as_str().to_string())
        .collect();
    BindingResolution {
        values,
        diagnostics,
        unbound_required_inputs,
    }
}

pub fn validate_json_pointer(pointer: &str) -> bool {
    pointer.is_empty()
        || (pointer.starts_with('/')
            && pointer.split('/').skip(1).all(|token| {
                let bytes = token.as_bytes();
                let mut index = 0;
                while index < bytes.len() {
                    if bytes[index] == b'~'
                        && (index + 1 >= bytes.len() || !matches!(bytes[index + 1], b'0' | b'1'))
                    {
                        return false;
                    }
                    index += if bytes[index] == b'~' { 2 } else { 1 };
                }
                true
            }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{InputFieldSpec, InputKind, InputName};

    fn presentation() -> ToolPresentation {
        ToolPresentation {
            version: 1,
            output: Some("result".to_string()),
            rows: Some("/items".to_string()),
            row_key: Some("/id".to_string()),
            columns: vec![PresentationColumn {
                label: "Enabled".to_string(),
                pointer: "/enabled".to_string(),
                tone_pointer: None,
                filterable: false,
            }],
            actions: Vec::new(),
            title_pointer: None,
            subtitle_pointer: None,
            status: None,
            summary: Vec::new(),
            notices: None,
            detail: None,
            row_detail: None,
            empty_message_pointer: None,
        }
    }

    #[test]
    fn rows_preserve_scalar_cells_and_disable_actions_for_duplicate_keys() {
        let resolved = resolve_rows(
            &presentation(),
            &json!({"result":{"items":[{"id":7,"enabled":true},{"id":7,"enabled":false}]}}),
        );
        assert_eq!(resolved.rows[0].cells, vec![json!(true)]);
        assert!(!resolved.row_actions_enabled);
        assert!(resolved.diagnostics[0].contains("duplicated"));
    }

    #[test]
    fn a_null_project_binding_leaves_the_required_folder_for_the_user_to_choose() {
        let target = InputSpec::new(vec![
            InputFieldSpec::new(
                InputName::new("project").unwrap(),
                None,
                None,
                true,
                InputKind::FilePath,
            )
            .unwrap(),
        ])
        .unwrap();
        let action = PresentationAction {
            id: "choose".into(),
            scope: ActionScope::Result,
            label: "Choose project".into(),
            target_tool: "demo.project".into(),
            on_success: None,
            enabled_pointer: None,
            disabled_reason_pointer: None,
            bindings: BTreeMap::from([(
                "project".into(),
                ActionBinding::Output {
                    pointer: "/result/project".into(),
                },
            )]),
        };
        let resolved = resolve_bindings(
            &action,
            &json!({}),
            None,
            &json!({"result": {"project": null}}),
            &target,
        );
        assert!(resolved.diagnostics.is_empty());
        assert!(resolved.values.is_empty());
        assert_eq!(resolved.unbound_required_inputs, vec!["project"]);
    }

    #[test]
    fn bindings_preserve_native_types_and_report_unbound_required_separately() {
        let target = InputSpec::new(vec![
            InputFieldSpec::new(
                InputName::new("count").unwrap(),
                None,
                None,
                true,
                InputKind::Integer,
            )
            .unwrap(),
            InputFieldSpec::new(
                InputName::new("mode").unwrap(),
                None,
                None,
                true,
                InputKind::String,
            )
            .unwrap(),
        ])
        .unwrap();
        let action = PresentationAction {
            id: "open".to_string(),
            scope: ActionScope::Row,
            label: "Open".to_string(),
            target_tool: "demo.target".to_string(),
            on_success: None,
            enabled_pointer: None,
            disabled_reason_pointer: None,
            bindings: BTreeMap::from([(
                "count".to_string(),
                ActionBinding::Row {
                    pointer: "/count".to_string(),
                },
            )]),
        };
        let resolved = resolve_bindings(
            &action,
            &json!({}),
            Some(&json!({"count":3})),
            &json!({}),
            &target,
        );
        assert_eq!(resolved.values["count"], json!(3));
        assert_eq!(resolved.unbound_required_inputs, vec!["mode"]);
        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn reserved_context_binding_is_rejected_by_the_resolver() {
        let action = PresentationAction {
            id: "bad".to_string(),
            scope: ActionScope::Result,
            label: "Bad".to_string(),
            target_tool: "demo.target".to_string(),
            on_success: None,
            enabled_pointer: None,
            disabled_reason_pointer: None,
            bindings: BTreeMap::from([(
                "_upeg".to_string(),
                ActionBinding::Constant { value: json!(true) },
            )]),
        };
        let resolved = resolve_bindings(&action, &json!({}), None, &json!({}), &InputSpec::empty());
        assert!(resolved.diagnostics[0].contains("reserved"));
    }
}
