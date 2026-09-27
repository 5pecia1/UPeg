//! Resolved fixed-slot result display DTOs and pure resolver adapter.

use super::{PresentationRowDto, PresentationRowsDto};

/// Resolved optional display fields. No JSON pointers cross into the UI.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationViewDto {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub status: Option<PresentationStatusDto>,
    pub summary: Vec<PresentationFieldDto>,
    pub notices: Vec<PresentationNoticeDto>,
    pub detail: Option<PresentationDetailDto>,
    pub row_details: Vec<PresentationRowDetailDto>,
    pub empty_message: Option<String>,
    pub actions: Vec<PresentationActionAvailabilityDto>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationFieldDto {
    pub label: String,
    pub value: String,
    pub tone: Option<String>,
}
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationStatusDto {
    pub label: String,
    pub tone: String,
}
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationNoticeDto {
    pub text: String,
    pub severity: String,
}
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationDetailDto {
    pub fields: Vec<PresentationFieldDto>,
    pub markdown: Option<String>,
    pub diff: Option<String>,
}
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationRowDetailDto {
    pub key: String,
    pub detail: PresentationDetailDto,
}
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PresentationActionAvailabilityDto {
    pub id: String,
    pub enabled: bool,
    pub reason: Option<String>,
}

pub(super) fn resolve_rows(tool_id: String, outputs_json: String) -> PresentationRowsDto {
    let Some(tool) = upeg_runtime::toolbox_tools().find(|tool| tool.id == tool_id) else {
        return PresentationRowsDto {
            rows: Vec::new(),
            diagnostics: vec![format!("tool `{tool_id}` is not installed")],
            row_actions_enabled: false,
        };
    };
    let Some(presentation) = tool.presentation.as_ref() else {
        return PresentationRowsDto {
            rows: Vec::new(),
            diagnostics: vec![format!("tool `{tool_id}` has no presentation")],
            row_actions_enabled: false,
        };
    };
    let outputs = match serde_json::from_str(&outputs_json) {
        Ok(value) => value,
        Err(error) => {
            return PresentationRowsDto {
                rows: Vec::new(),
                diagnostics: vec![format!("outputs JSON is invalid: {error}")],
                row_actions_enabled: false,
            };
        }
    };
    let resolved = upeg_core::resolve_rows(presentation, &outputs);
    PresentationRowsDto {
        rows: resolved
            .rows
            .into_iter()
            .map(|row| PresentationRowDto {
                key: row.key,
                value_json: row.value.to_string(),
                cells_json: row.cells.into_iter().map(|cell| cell.to_string()).collect(),
                cell_tones: row
                    .cell_tones
                    .into_iter()
                    .map(|tone| tone.map(presentation_tone_text))
                    .collect(),
            })
            .collect(),
        diagnostics: resolved.diagnostics,
        row_actions_enabled: resolved.row_actions_enabled,
    }
}

pub(super) fn presentation_tone_text(tone: upeg_core::PresentationTone) -> String {
    match tone {
        upeg_core::PresentationTone::Info => "info",
        upeg_core::PresentationTone::Success => "success",
        upeg_core::PresentationTone::Warning => "warning",
        upeg_core::PresentationTone::Error => "error",
    }
    .to_string()
}

fn presentation_field_dto(field: upeg_core::ResolvedField) -> PresentationFieldDto {
    let value = match field.value {
        serde_json::Value::String(value) => value,
        other => other.to_string(),
    };
    PresentationFieldDto {
        label: field.label,
        value,
        tone: field.tone.map(presentation_tone_text),
    }
}

fn presentation_detail_dto(detail: upeg_core::ResolvedDetail) -> PresentationDetailDto {
    PresentationDetailDto {
        fields: detail
            .fields
            .into_iter()
            .map(presentation_field_dto)
            .collect(),
        markdown: detail.markdown,
        diff: detail.diff,
    }
}

pub(super) fn resolve_view(tool_id: String, outputs_json: String) -> PresentationViewDto {
    let diagnostic = |message: String| PresentationViewDto {
        title: None,
        subtitle: None,
        status: None,
        summary: Vec::new(),
        notices: Vec::new(),
        detail: None,
        row_details: Vec::new(),
        empty_message: None,
        actions: Vec::new(),
        diagnostics: vec![message],
    };
    let Some(tool) = upeg_runtime::toolbox_tools().find(|tool| tool.id == tool_id) else {
        return diagnostic(format!("tool `{tool_id}` is not installed"));
    };
    let Some(presentation) = tool.presentation.as_ref() else {
        return diagnostic(format!("tool `{tool_id}` has no presentation"));
    };
    let outputs = match serde_json::from_str(&outputs_json) {
        Ok(value) => value,
        Err(error) => return diagnostic(format!("outputs JSON is invalid: {error}")),
    };
    let resolved = upeg_core::resolve_view(presentation, &outputs);
    let mut diagnostics = resolved.rows.diagnostics;
    diagnostics.extend(resolved.diagnostics);
    let row_details = resolved
        .row_details
        .into_iter()
        .zip(resolved.rows.rows)
        .map(|(detail, row)| PresentationRowDetailDto {
            key: row.key,
            detail: presentation_detail_dto(detail),
        })
        .collect();
    PresentationViewDto {
        title: resolved.title,
        subtitle: resolved.subtitle,
        status: resolved.status.map(|status| PresentationStatusDto {
            label: status.label,
            tone: presentation_tone_text(status.tone),
        }),
        summary: resolved
            .summary
            .into_iter()
            .map(presentation_field_dto)
            .collect(),
        notices: resolved
            .notices
            .into_iter()
            .map(|notice| PresentationNoticeDto {
                text: notice.text,
                severity: presentation_tone_text(notice.severity),
            })
            .collect(),
        detail: resolved.detail.map(presentation_detail_dto),
        row_details,
        empty_message: resolved.empty_message,
        actions: resolved
            .actions
            .into_iter()
            .map(|action| PresentationActionAvailabilityDto {
                id: action.id,
                enabled: action.enabled,
                reason: action.reason,
            })
            .collect(),
        diagnostics,
    }
}
