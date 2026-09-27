//! TOML shapes for a tool result's structured presentation metadata.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;

/// Presentation metadata that tells interactive surfaces how to render a
/// structured tool result and which follow-up actions it exposes.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 15)
)]
pub struct PresentationToml {
    /// Version of the presentation contract. Currently `1`.
    pub version: u16,
    /// JSON output field containing the collection to render. Required with
    /// `rows`, `row_key`, and `columns`; omit all four for action-only results.
    #[serde(default)]
    pub output: Option<String>,
    /// JSON Pointer from `output` to the rows array.
    #[serde(default)]
    pub rows: Option<String>,
    /// JSON Pointer from each row to its stable key.
    #[serde(default)]
    pub row_key: Option<String>,
    /// Ordered columns rendered for each row in the collection.
    #[serde(default)]
    pub columns: Vec<PresentationColumnToml>,
    /// Follow-up actions available for the result or its rows.
    #[serde(default)]
    pub actions: Vec<PresentationActionToml>,
    /// String pointers into `output` for a prominent title and context.
    #[serde(default)]
    pub title_pointer: Option<String>,
    /// Secondary context string pointer into `output`.
    #[serde(default)]
    pub subtitle_pointer: Option<String>,
    /// Text and closed tone for the result state.
    #[serde(default)]
    pub status: Option<PresentationStatusToml>,
    /// Scalar facts shown before the collection.
    #[serde(default)]
    pub summary: Vec<PresentationColumnToml>,
    /// Array of visible notices with text and severity.
    #[serde(default)]
    pub notices: Option<PresentationNoticesToml>,
    /// Detail fields and optional Markdown or unified diff for the result.
    #[serde(default)]
    pub detail: Option<PresentationDetailToml>,
    /// Detail fields for the selected collection row.
    #[serde(default)]
    pub row_detail: Option<PresentationDetailToml>,
    /// Message for an empty source array, distinct from a search with no matches.
    #[serde(default)]
    pub empty_message_pointer: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields, extend("x-doc-reference" = true, "x-doc-order" = 19))]
pub struct PresentationStatusToml {
    /// String pointer into the selected JSON output.
    pub label_pointer: String,
    /// Pointer to `info`, `success`, `warning`, or `error`.
    pub tone_pointer: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields, extend("x-doc-reference" = true, "x-doc-order" = 20))]
pub struct PresentationNoticesToml {
    /// Pointer to an array within the selected JSON output.
    pub rows_pointer: String,
    /// String pointer relative to one notice row.
    pub text_pointer: String,
    /// Tone pointer relative to one notice row.
    pub severity_pointer: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields, extend("x-doc-reference" = true, "x-doc-order" = 21))]
pub struct PresentationDetailToml {
    /// Ordered scalar fields in the selected result or row.
    #[serde(default)]
    pub fields: Vec<PresentationColumnToml>,
    /// Optional Markdown string pointer.
    #[serde(default)]
    pub markdown_pointer: Option<String>,
    /// Optional unified diff string pointer.
    #[serde(default)]
    pub diff_pointer: Option<String>,
}

/// One visible column in a collection presentation.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 16)
)]
pub struct PresentationColumnToml {
    /// Human-readable column heading.
    pub label: String,
    /// JSON Pointer from the current row to the displayed value.
    pub pointer: String,
    /// Optional pointer to a closed tone for this row cell.
    #[serde(default)]
    pub tone_pointer: Option<String>,
    /// Whether the UI may offer an equality filter on this column.
    #[serde(default)]
    pub filterable: bool,
}

/// A follow-up tool action that a presentation can offer.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 17)
)]
pub struct PresentationActionToml {
    /// Unique action id within this presentation.
    pub id: String,
    /// Action visibility: `result` or `row` for collection presentations.
    pub scope: String,
    /// Human-readable label shown to the user.
    pub label: String,
    /// Fully qualified id of the tool invoked by this action.
    pub target_tool: String,
    /// Optional post-success behavior; currently `refresh_origin`.
    #[serde(default)]
    pub on_success: Option<String>,
    /// Map of target-tool input names to values resolved from this result.
    #[serde(default)]
    pub bindings: BTreeMap<String, PresentationBindingToml>,
    /// Optional boolean pointer into the selected JSON output; missing is disabled.
    #[serde(default)]
    pub enabled_pointer: Option<String>,
    /// String pointer displayed when the action is disabled.
    #[serde(default)]
    pub disabled_reason_pointer: Option<String>,
}

/// One source declaration for a follow-up action input.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 18)
)]
pub struct PresentationBindingToml {
    /// Value source: `input`, `output`, `row`, or `constant`.
    pub from: String,
    /// JSON Pointer used by non-constant sources.
    #[serde(default)]
    pub pointer: Option<String>,
    /// Literal JSON value used by a `constant` source.
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}
