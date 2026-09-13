//! Guidance edits respect the source that owns a board's instructions.

use std::path::{Path, PathBuf};

use super::{
    BoardGuidance, BoardVisibility, PegboardState, PegboardStateError, state_path_from_env,
};

/// Why an edit could not be applied to the board's guidance.
#[derive(Debug, thiserror::Error)]
pub enum BoardGuidanceEditError {
    #[error("unknown board: {board}")]
    UnknownBoard { board: String },
    #[error("board {board} guidance is owned by the project manifest; edit {}", path.display())]
    ProjectManifest { board: String, path: PathBuf },
    #[error(transparent)]
    Persistence(#[from] PegboardStateError),
}

/// Guidance for an existing board in a loaded state.
#[must_use]
pub fn board_guidance_in<'a>(state: &'a PegboardState, key: &str) -> Option<&'a BoardGuidance> {
    state
        .boards
        .iter()
        .find(|board| board.key == key)
        .map(|board| &board.guidance)
}

/// Replace personal board guidance without changing its title or layout.
///
/// # Errors
/// Returns the manifest path for a project board, or `UnknownBoard` when
/// `key` does not identify a board in this state. Neither changes state.
pub fn set_board_guidance_in(
    state: &mut PegboardState,
    key: &str,
    guidance: BoardGuidance,
    visibility: &BoardVisibility,
) -> Result<(), BoardGuidanceEditError> {
    ensure_personal_guidance(key, visibility)?;
    let board = state
        .boards
        .iter_mut()
        .find(|board| board.key == key)
        .ok_or_else(|| BoardGuidanceEditError::UnknownBoard {
            board: key.to_string(),
        })?;
    board.guidance = guidance;
    Ok(())
}

fn ensure_personal_guidance(
    key: &str,
    visibility: &BoardVisibility,
) -> Result<(), BoardGuidanceEditError> {
    if let Some(scope) = visibility
        .project()
        .filter(|scope| scope.declaration(key).is_some())
    {
        return Err(BoardGuidanceEditError::ProjectManifest {
            board: key.to_string(),
            path: scope.manifest_path().to_path_buf(),
        });
    }
    Ok(())
}

/// Load, edit, and persist guidance using an explicit store and project scope.
///
/// # Errors
/// Returns ownership, missing-board, and storage errors without persisting
/// a rejected edit.
pub fn set_board_guidance_to_path_in(
    path: &Path,
    key: &str,
    guidance: BoardGuidance,
    visibility: &BoardVisibility,
) -> Result<(), BoardGuidanceEditError> {
    ensure_personal_guidance(key, visibility)?;
    let mut store = crate::store::Store::open_at(path).map_err(PegboardStateError::from)?;
    if !store
        .update_board_guidance(key, &guidance, visibility)
        .map_err(PegboardStateError::from)?
    {
        return Err(BoardGuidanceEditError::UnknownBoard {
            board: key.to_string(),
        });
    }
    Ok(())
}

/// Persist personal guidance for the board visible in the current process.
///
/// # Errors
/// Returns ownership, missing-board, configuration, and storage errors.
pub fn set_board_guidance(
    key: &str,
    guidance: BoardGuidance,
) -> Result<(), BoardGuidanceEditError> {
    let path = state_path_from_env().ok_or(PegboardStateError::ConfigRootUnavailable)?;
    set_board_guidance_to_path_in(&path, key, guidance, &BoardVisibility::from_process())
}
