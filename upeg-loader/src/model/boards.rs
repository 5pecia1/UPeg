use schemars::JsonSchema;
use serde::Deserialize;

/// One entry in a Project Manifest's top-level `[[boards]]` array.
///
/// Project-declared boards are the only way a manifest can add a board
/// tab; they exist while the manifest is detected and disappear when it
/// is not (`upeg_sources::project` module docs). Toolkit manifests
/// under `~/.upeg/toolkits` are global and have no project to scope a
/// board to, so the loader rejects `[[boards]]` there.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
// Appended at the end of the generated reference (`x-doc-order = 14`)
// rather than renumbering the existing sections: board declarations are
// a top-level concern but a rare one, and the `boards` row already sits
// in `ToolkitToml`'s own field table where an author meets it first.
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 14)
)]
pub struct BoardEntryToml {
    /// Board id, as typed on every surface (`upeg board <id> list`) and
    /// as named by a tool's `boards = [...]` array. Must be canonical
    /// (unpadded), must not contain `:`, and must not shadow a built-in
    /// board.
    pub id: String,
    /// Tab title for GUI surfaces. Defaults to `id` when omitted.
    #[serde(default)]
    pub label: Option<String>,
    /// Short description of the board's purpose. Empty when omitted.
    #[serde(default)]
    pub description: String,
    /// Markdown guidance for people and agents using this board.
    #[serde(default)]
    pub instructions: String,
}
