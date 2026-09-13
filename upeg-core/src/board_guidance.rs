//! Descriptive context shared by personal and project boards.

/// Optional board context. Empty strings mean no guidance was supplied.
///
/// Instructions contain Markdown and are preserved verbatim, including
/// indentation and trailing line breaks that can affect its rendering.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct BoardGuidance {
    /// A short description of the board's purpose.
    pub description: String,
    /// Markdown instructions for people and agents using this board.
    pub instructions: String,
}
