//! Validation and lowering of a manifest's top-level `[[boards]]`.
//!
//! Kept apart from `parse.rs`'s tool lowering because these entries are
//! not tools: they describe *where* tools can be pinned, and only a
//! Project Manifest may declare them.

use upeg_core::{BoardGuidance, BoardKey, is_builtin_board};
use upeg_runtime::pegboard_project::ProjectBoardDecl;

use crate::LoadError;
use crate::model::BoardEntryToml;

/// Validate and lower every `[[boards]]` entry.
///
/// Rules, in the order they are checked per entry:
///   1. the id parses as a [`BoardKey`] — non-empty, unpadded, and free
///      of the `:` reserved for project board store keys;
///   2. it does not shadow a built-in board (`dev`, `trading`, …),
///      which would make "which board did you mean" ambiguous on every
///      surface;
///   3. it is not declared twice in the same manifest.
///
/// A missing `label` falls back to the id, so a minimal declaration is
/// one line.
pub(crate) fn lower_board_entries(
    entries: &[BoardEntryToml],
) -> Result<Vec<ProjectBoardDecl>, LoadError> {
    let mut out: Vec<ProjectBoardDecl> = Vec::with_capacity(entries.len());
    for (position, entry) in entries.iter().enumerate() {
        let id = BoardKey::parse(&entry.id).map_err(|error| LoadError::InvalidProjectBoardId {
            position,
            board: entry.id.clone(),
            error,
        })?;
        if is_builtin_board(id.as_str()) {
            return Err(LoadError::BuiltinProjectBoardId {
                position,
                board: id.to_string(),
            });
        }
        if out.iter().any(|existing| existing.id == id) {
            return Err(LoadError::DuplicateProjectBoardId {
                position,
                board: id.to_string(),
            });
        }
        let label = entry
            .label
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .unwrap_or(id.as_str())
            .to_string();
        out.push(
            ProjectBoardDecl::new(id, label).with_guidance(BoardGuidance {
                description: entry.description.clone(),
                instructions: entry.instructions.clone(),
            }),
        );
    }
    Ok(out)
}
