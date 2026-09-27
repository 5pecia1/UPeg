//! Pure, source-neutral projection of optional presentation fields.

use serde_json::Value;

use super::{
    PresentationAction, PresentationColumn, PresentationDetail, RowsResolution, ToolPresentation,
    resolve_rows,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationTone {
    Info,
    Success,
    Warning,
    Error,
}

impl PresentationTone {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "info" => Some(Self::Info),
            "success" => Some(Self::Success),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedField {
    pub label: String,
    pub value: Value,
    pub tone: Option<PresentationTone>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedStatus {
    pub label: String,
    pub tone: PresentationTone,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedNotice {
    pub text: String,
    pub severity: PresentationTone,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedDetail {
    pub fields: Vec<ResolvedField>,
    pub markdown: Option<String>,
    pub diff: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedAction {
    pub id: String,
    pub enabled: bool,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedView {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub status: Option<ResolvedStatus>,
    pub summary: Vec<ResolvedField>,
    pub notices: Vec<ResolvedNotice>,
    pub detail: Option<ResolvedDetail>,
    pub rows: RowsResolution,
    pub row_details: Vec<ResolvedDetail>,
    pub empty_message: Option<String>,
    pub actions: Vec<ResolvedAction>,
    pub diagnostics: Vec<String>,
}

fn string_at(
    root: &Value,
    pointer: &str,
    field: &str,
    diagnostics: &mut Vec<String>,
) -> Option<String> {
    if let Some(Value::String(value)) = root.pointer(pointer) {
        Some(value.clone())
    } else {
        diagnostics.push(format!("{field} at `{pointer}` must be a string"));
        None
    }
}

fn tone_at(
    root: &Value,
    pointer: &str,
    field: &str,
    diagnostics: &mut Vec<String>,
) -> Option<PresentationTone> {
    let value = string_at(root, pointer, field, diagnostics)?;
    if let Some(tone) = PresentationTone::parse(&value) {
        Some(tone)
    } else {
        diagnostics.push(format!("{field} at `{pointer}` has an unknown tone"));
        None
    }
}

fn fields(
    root: &Value,
    columns: &[PresentationColumn],
    diagnostics: &mut Vec<String>,
) -> Vec<ResolvedField> {
    columns
        .iter()
        .map(|column| {
            let value = if let Some(
                value @ (Value::Null | Value::String(_) | Value::Number(_) | Value::Bool(_)),
            ) = root.pointer(&column.pointer)
            {
                value.clone()
            } else {
                diagnostics.push(format!(
                    "field `{}` at `{}` must be scalar",
                    column.label, column.pointer
                ));
                Value::Null
            };
            let tone = column
                .tone_pointer
                .as_deref()
                .and_then(|pointer| tone_at(root, pointer, &column.label, diagnostics));
            ResolvedField {
                label: column.label.clone(),
                value,
                tone,
            }
        })
        .collect()
}

fn detail(
    root: &Value,
    spec: &PresentationDetail,
    diagnostics: &mut Vec<String>,
) -> ResolvedDetail {
    ResolvedDetail {
        fields: fields(root, &spec.fields, diagnostics),
        markdown: spec
            .markdown_pointer
            .as_deref()
            .and_then(|pointer| string_at(root, pointer, "markdown", diagnostics)),
        diff: spec
            .diff_pointer
            .as_deref()
            .and_then(|pointer| string_at(root, pointer, "diff", diagnostics)),
    }
}

/// A declared result-action condition is fail-closed. It is a presentation
/// hint; dispatch and the target tool still enforce their own policy.
pub fn resolve_action_availability(
    presentation: &ToolPresentation,
    action: &PresentationAction,
    outputs: &Value,
) -> ResolvedAction {
    let mut result = ResolvedAction {
        id: action.id.clone(),
        enabled: true,
        reason: None,
    };
    let Some(pointer) = action.enabled_pointer.as_deref() else {
        return result;
    };
    let Some(output) = presentation
        .output
        .as_deref()
        .and_then(|id| outputs.get(id))
    else {
        result.enabled = false;
        result.reason = Some("실행 가능 여부를 확인할 수 없습니다".to_string());
        return result;
    };
    match output.pointer(pointer) {
        Some(Value::Bool(true)) => {}
        Some(Value::Bool(false)) => {
            result.enabled = false;
            result.reason = action
                .disabled_reason_pointer
                .as_deref()
                .and_then(|path| output.pointer(path))
                .and_then(Value::as_str)
                .filter(|reason| !reason.is_empty())
                .map(str::to_string)
                .or_else(|| Some("현재 실행할 수 없습니다".to_string()));
        }
        _ => {
            result.enabled = false;
            result.reason = Some("실행 가능 여부를 확인할 수 없습니다".to_string());
        }
    }
    result
}

pub fn resolve_view(presentation: &ToolPresentation, outputs: &Value) -> ResolvedView {
    let mut diagnostics = Vec::new();
    let rows = resolve_rows(presentation, outputs);
    for (index, row) in rows.rows.iter().enumerate() {
        for (column, tone) in presentation.columns.iter().zip(&row.cell_tones) {
            if let Some(pointer) = &column.tone_pointer
                && tone.is_none()
            {
                diagnostics.push(format!(
                    "row {index} column `{}` has no valid tone at `{pointer}`",
                    column.label
                ));
            }
        }
    }
    let Some(output) = presentation
        .output
        .as_deref()
        .and_then(|id| outputs.get(id))
    else {
        return ResolvedView {
            title: None,
            subtitle: None,
            status: None,
            summary: Vec::new(),
            notices: Vec::new(),
            detail: None,
            rows,
            row_details: Vec::new(),
            empty_message: None,
            actions: presentation
                .actions
                .iter()
                .map(|action| resolve_action_availability(presentation, action, outputs))
                .collect(),
            diagnostics,
        };
    };
    let title = presentation
        .title_pointer
        .as_deref()
        .and_then(|pointer| string_at(output, pointer, "title", &mut diagnostics));
    let subtitle = presentation
        .subtitle_pointer
        .as_deref()
        .and_then(|pointer| string_at(output, pointer, "subtitle", &mut diagnostics));
    let status = presentation.status.as_ref().and_then(|spec| {
        let label = string_at(
            output,
            &spec.label_pointer,
            "status label",
            &mut diagnostics,
        );
        let tone = tone_at(output, &spec.tone_pointer, "status tone", &mut diagnostics);
        label
            .zip(tone)
            .map(|(label, tone)| ResolvedStatus { label, tone })
    });
    let summary = fields(output, &presentation.summary, &mut diagnostics);
    let mut notices = Vec::new();
    if let Some(spec) = &presentation.notices {
        if let Some(values) = output.pointer(&spec.rows_pointer).and_then(Value::as_array) {
            for (index, value) in values.iter().enumerate() {
                let text = string_at(
                    value,
                    &spec.text_pointer,
                    &format!("notice {index} text"),
                    &mut diagnostics,
                );
                let severity = tone_at(
                    value,
                    &spec.severity_pointer,
                    &format!("notice {index} severity"),
                    &mut diagnostics,
                );
                if let Some((text, severity)) = text.zip(severity) {
                    notices.push(ResolvedNotice { text, severity });
                }
            }
        } else {
            diagnostics.push(format!(
                "notices at `{}` must be an array",
                spec.rows_pointer
            ));
        }
    }
    let result_detail = presentation
        .detail
        .as_ref()
        .map(|spec| detail(output, spec, &mut diagnostics));
    let row_details = presentation
        .row_detail
        .as_ref()
        .map_or_else(Vec::new, |spec| {
            rows.rows
                .iter()
                .map(|row| detail(&row.value, spec, &mut diagnostics))
                .collect()
        });
    let empty_message = if rows.rows.is_empty() && rows.diagnostics.is_empty() {
        presentation
            .empty_message_pointer
            .as_deref()
            .and_then(|pointer| string_at(output, pointer, "empty message", &mut diagnostics))
    } else {
        None
    };
    let actions = presentation
        .actions
        .iter()
        .map(|action| {
            let resolved = resolve_action_availability(presentation, action, outputs);
            if action.enabled_pointer.is_some()
                && resolved.reason.as_deref() == Some("실행 가능 여부를 확인할 수 없습니다")
            {
                diagnostics.push(format!(
                    "action `{}` has no boolean availability",
                    action.id
                ));
            }
            if let (Some(reason_pointer), true) =
                (&action.disabled_reason_pointer, resolved.enabled)
                && output
                    .pointer(reason_pointer)
                    .and_then(Value::as_str)
                    .is_some_and(|text| !text.is_empty())
            {
                diagnostics.push(format!(
                    "action `{}` is enabled but has a disabled reason",
                    action.id
                ));
            }
            resolved
        })
        .collect();
    ResolvedView {
        title,
        subtitle,
        status,
        summary,
        notices,
        detail: result_detail,
        rows,
        row_details,
        empty_message,
        actions,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn presentation() -> ToolPresentation {
        ToolPresentation {
            version: 1,
            output: Some("result".into()),
            rows: None,
            row_key: None,
            columns: Vec::new(),
            actions: Vec::new(),
            title_pointer: Some("/view/title".into()),
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
    fn output_only_projection_keeps_scalar_and_diagnoses_bad_tone() {
        let mut spec = presentation();
        spec.status = Some(super::super::PresentationStatus {
            label_pointer: "/view/state".into(),
            tone_pointer: "/view/tone".into(),
        });
        let view = resolve_view(
            &spec,
            &json!({"result":{"view":{
            "title":"Changes", "state":"Ready", "tone":"purple"}}}),
        );
        assert_eq!(view.title.as_deref(), Some("Changes"));
        assert!(view.status.is_none());
        assert!(
            view.diagnostics
                .iter()
                .any(|item| item.contains("unknown tone"))
        );
    }

    #[test]
    fn declared_action_condition_fails_closed_on_missing_or_wrong_type() {
        let mut spec = presentation();
        let action = PresentationAction {
            id: "apply".into(),
            scope: super::super::ActionScope::Result,
            label: "Apply".into(),
            target_tool: "demo.apply".into(),
            on_success: None,
            bindings: std::collections::BTreeMap::new(),
            enabled_pointer: Some("/view/apply/enabled".into()),
            disabled_reason_pointer: Some("/view/apply/reason".into()),
        };
        spec.actions.push(action.clone());
        for output in [
            json!({"result":{"view":{}}}),
            json!({"result":{"view":{"apply":{"enabled":"yes"}}}}),
        ] {
            assert!(!resolve_action_availability(&spec, &action, &output).enabled);
        }
        let disabled = resolve_action_availability(
            &spec,
            &action,
            &json!({"result":{"view":{"apply":{"enabled":false,"reason":"No changes"}}}}),
        );
        assert_eq!(disabled.reason.as_deref(), Some("No changes"));
        assert!(
            resolve_action_availability(
                &spec,
                &action,
                &json!({"result":{"view":{"apply":{"enabled":true}}}})
            )
            .enabled
        );
    }
}
