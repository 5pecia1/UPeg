//! Optional presentation metadata and pure JSON binding resolution.

use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use crate::InputSpec;

pub const PRESENTATION_VERSION_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolEffect {
    Read,
    Write,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolPresentation {
    pub version: u16,
    pub output: Option<String>,
    pub rows: Option<String>,
    pub row_key: Option<String>,
    pub columns: Vec<PresentationColumn>,
    pub actions: Vec<PresentationAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationColumn {
    pub label: String,
    pub pointer: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionScope {
    Row,
    Result,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionSuccess {
    RefreshOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationAction {
    pub id: String,
    pub scope: ActionScope,
    pub label: String,
    pub target_tool: String,
    pub on_success: Option<ActionSuccess>,
    pub bindings: BTreeMap<String, ActionBinding>,
}

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
        rows.push(PresentationRow {
            key,
            value: value.clone(),
            cells,
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
            }],
            actions: Vec::new(),
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
            bindings: BTreeMap::from([(
                "_upeg".to_string(),
                ActionBinding::Constant { value: json!(true) },
            )]),
        };
        let resolved = resolve_bindings(&action, &json!({}), None, &json!({}), &InputSpec::empty());
        assert!(resolved.diagnostics[0].contains("reserved"));
    }
}
