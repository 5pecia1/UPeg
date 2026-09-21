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
