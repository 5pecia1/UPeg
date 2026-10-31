//! Board deletion storage commit across native and browser backends.

#[cfg(not(target_arch = "wasm32"))]
use super::load_boards_and_layouts;
use super::{Board, BoardLayouts, FrbError};
#[cfg(target_arch = "wasm32")]
use super::{default_boards, default_layouts, storage, try_save_boards, try_save_layouts};

#[cfg(target_arch = "wasm32")]
pub(super) struct MetadataSnapshot {
    boards: Option<String>,
    layouts: Option<String>,
}

#[cfg(target_arch = "wasm32")]
impl MetadataSnapshot {
    pub(super) fn needs_quota_reclaim(&self) -> bool {
        self.boards.is_none() || self.layouts.is_none()
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) fn snapshot_metadata() -> Result<MetadataSnapshot, FrbError> {
    Ok(MetadataSnapshot {
        boards: storage::try_get_item(storage::BOARDS_KEY).map_err(|()| FrbError::Io {
            message: "read boards before deletion failed".to_string(),
        })?,
        layouts: storage::try_get_item(storage::LAYOUTS_KEY).map_err(|()| FrbError::Io {
            message: "read layouts before deletion failed".to_string(),
        })?,
    })
}

#[cfg(target_arch = "wasm32")]
pub(super) fn restore_metadata(snapshot: &MetadataSnapshot) -> Result<(), FrbError> {
    let entries = [
        (storage::BOARDS_KEY, &snapshot.boards),
        (storage::LAYOUTS_KEY, &snapshot.layouts),
    ];
    let mut failed = false;
    for (key, value) in entries {
        if value.is_none() && storage::remove_item(key).is_err() {
            failed = true;
        }
    }
    for (key, value) in entries {
        if let Some(value) = value
            && storage::set_item(key, value).is_err()
        {
            failed = true;
        }
    }
    if failed {
        Err(FrbError::Io {
            message: "restore board metadata after failed deletion failed".to_string(),
        })
    } else {
        Ok(())
    }
}

/// Restore the old browser state after a board delete could not commit.
/// Restore the epoch and metadata first so large work values have room.
#[cfg(target_arch = "wasm32")]
pub(super) fn rollback_failed_delete(
    metadata: &MetadataSnapshot,
    reservation: Option<&crate::api::pin_web_storage::EpochReservation>,
    work: Option<&crate::api::pin_web_storage::BoardWorkSnapshot>,
    cause: FrbError,
) -> FrbError {
    let mut failures = Vec::new();
    if let Some(reservation) = reservation
        && let Err(err) = crate::api::pin_web_storage::restore_epoch(reservation)
    {
        failures.push(err.to_string());
    }
    if let Err(err) = restore_metadata(metadata) {
        failures.push(err.to_string());
    }
    if let Some(work) = work
        && let Err(err) = crate::api::pin_web_storage::restore_snapshot(work)
    {
        failures.push(err.to_string());
    }
    if failures.is_empty() {
        cause
    } else {
        FrbError::Io {
            message: format!("{cause}; rollback incomplete: {}", failures.join("; ")),
        }
    }
}

/// Browser board deletion must cope with quota-full storage, including a
/// built-in board whose board/layout keys have not been created yet.
#[cfg(target_arch = "wasm32")]
pub(super) fn delete_board_web(
    board_key: &str,
    boards: &[Board],
    mut layouts: BoardLayouts,
) -> Result<(), FrbError> {
    use crate::api::pin_web_storage;

    let metadata = snapshot_metadata()?;
    let old_epoch = pin_web_storage::epoch(board_key)?;
    let work = if metadata.needs_quota_reclaim() {
        let snapshot = pin_web_storage::snapshot_board_work(board_key, old_epoch)?;
        if let Err(err) = pin_web_storage::remove_snapshot(&snapshot) {
            let restore = pin_web_storage::restore_snapshot(&snapshot);
            return Err(match restore {
                Ok(()) => err,
                Err(restore_err) => FrbError::Io {
                    message: format!("{err}; work rollback failed: {restore_err}"),
                },
            });
        }
        Some(snapshot)
    } else {
        None
    };

    layouts.retain(|key, _| *key != board_key);
    if let Err(err) = save_boards_and_layouts(boards, &layouts) {
        return Err(rollback_failed_delete(&metadata, None, work.as_ref(), err));
    }
    let reservation = match pin_web_storage::reserve_board_epoch(board_key) {
        Ok(reservation) => reservation,
        Err(err) => return Err(rollback_failed_delete(&metadata, None, work.as_ref(), err)),
    };
    if reservation.old_epoch() != old_epoch {
        return Err(rollback_failed_delete(
            &metadata,
            Some(&reservation),
            work.as_ref(),
            FrbError::Io {
                message: "pin work epoch changed during board deletion".to_string(),
            },
        ));
    }
    if let Err(err) = pin_web_storage::advance_board_epoch(board_key, old_epoch) {
        return Err(rollback_failed_delete(
            &metadata,
            Some(&reservation),
            work.as_ref(),
            err,
        ));
    }
    if let Err(err) = pin_web_storage::cleanup_deleted_board_keys(board_key, old_epoch) {
        tracing::warn!(board_key = %board_key, error = %err, "board deleted but old work cleanup failed");
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn save_boards_and_layouts(
    boards: &[Board],
    layouts: &BoardLayouts,
) -> Result<(), FrbError> {
    let state = upeg_sources::pegboard::PegboardState {
        boards: boards
            .iter()
            .map(|board| upeg_sources::pegboard::BoardData {
                key: board.key.to_string(),
                title: board.title.to_string(),
                guidance: board.guidance.clone(),
            })
            .collect(),
        layouts: layouts
            .iter()
            .map(|(key, placements)| ((*key).to_string(), placements.clone()))
            .collect(),
        selection: upeg_sources::pegboard::selection_value(),
    };
    upeg_sources::pegboard::save_state(&state).map_err(|err| FrbError::Io {
        message: format!("save boards and layouts: {err}"),
    })
}

#[cfg(target_arch = "wasm32")]
pub(super) fn save_boards_and_layouts(
    boards: &[Board],
    layouts: &BoardLayouts,
) -> Result<(), FrbError> {
    let previous = storage::try_get_item(storage::LAYOUTS_KEY).map_err(|()| FrbError::Io {
        message: "read layouts before board deletion failed".to_string(),
    })?;
    try_save_layouts(layouts).map_err(|_| FrbError::Io {
        message: "save layouts before board deletion failed".to_string(),
    })?;
    if try_save_boards(boards).is_err() {
        let rollback = match previous {
            Some(value) => storage::set_item(storage::LAYOUTS_KEY, &value),
            None => storage::remove_item(storage::LAYOUTS_KEY),
        };
        return Err(FrbError::Io {
            message: if rollback.is_ok() {
                "save boards failed; layouts restored".to_string()
            } else {
                "save boards failed; layout rollback also failed".to_string()
            },
        });
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn load_boards_and_layouts_for_mutation() -> Result<(Vec<Board>, BoardLayouts), FrbError>
{
    Ok(load_boards_and_layouts())
}

#[cfg(target_arch = "wasm32")]
pub(super) fn load_boards_and_layouts_for_mutation() -> Result<(Vec<Board>, BoardLayouts), FrbError>
{
    let boards =
        match storage::try_get_item(storage::BOARDS_KEY).map_err(|()| FrbError::Io {
            message: "read boards before mutation failed".to_string(),
        })? {
            Some(json) => upeg_pegboard_ui::features::boards::deserialize_boards(&json)
                .ok_or_else(|| FrbError::Io {
                    message: "stored boards are invalid".to_string(),
                })?,
            None => default_boards(),
        };
    let layouts = match storage::try_get_item(storage::LAYOUTS_KEY).map_err(|()| FrbError::Io {
        message: "read layouts before mutation failed".to_string(),
    })? {
        Some(json) => upeg_pegboard_ui::features::layouts::deserialize_layouts(&json, &boards)
            .ok_or_else(|| FrbError::Io {
                message: "stored layouts are invalid".to_string(),
            })?,
        None => default_layouts(&boards),
    };
    Ok((boards, layouts))
}
