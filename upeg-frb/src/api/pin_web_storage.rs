//! Browser key epoch shared by drafts and outcomes. Board deletion advances
//! the epoch, then removes hidden keys to reclaim browser storage.

#[cfg(target_arch = "wasm32")]
use super::boot::FrbError;
#[cfg(target_arch = "wasm32")]
use upeg_pegboard_ui::platform::storage;

#[cfg(any(test, target_arch = "wasm32"))]
fn board_part(board_key: &str) -> String {
    format!("{}:{board_key}", board_key.len())
}

#[cfg(target_arch = "wasm32")]
fn epoch_key(board_key: &str) -> String {
    format!("upeg.pin_work.v2.epoch.{}", board_part(board_key))
}

#[cfg(any(test, target_arch = "wasm32"))]
fn prefix_for_epoch(kind: &str, board_key: &str, epoch: u64) -> String {
    format!("upeg.{kind}.v2.{}.{epoch}.", board_part(board_key))
}

/// Select only old keys of this board. The board length prevents `dev` from
/// matching another board named `dev.ops`.
#[cfg(any(test, target_arch = "wasm32"))]
fn cleanup_keys_for_board(board_key: &str, old_epoch: u64, keys: &[String]) -> Vec<String> {
    let board_part = board_part(board_key);
    let prefixes = [
        format!("upeg.pin_drafts.v2.{board_part}."),
        format!("upeg.last_outcomes.v2.{board_part}."),
    ];
    let legacy = [
        format!("upeg.pin_drafts.v1.{board_key}"),
        format!("upeg.last_outcomes.v1.{board_key}"),
    ];
    keys.iter()
        .filter(|key| {
            legacy
                .iter()
                .any(|legacy_key| key.as_str() == legacy_key.as_str())
                || prefixes.iter().any(|prefix| {
                    key.strip_prefix(prefix)
                        .and_then(|suffix| suffix.split_once('.'))
                        .is_some_and(|(epoch, pin_id)| {
                            !pin_id.is_empty()
                                && epoch.parse::<u64>().is_ok_and(|epoch| epoch <= old_epoch)
                        })
                })
        })
        .cloned()
        .collect()
}

#[cfg(any(test, target_arch = "wasm32"))]
enum LegacyPinRemoval {
    Unchanged,
    Empty,
    Remaining(String),
}

#[cfg(any(test, target_arch = "wasm32"))]
fn remove_legacy_pin_row(json: &str, pin_id: &str) -> Result<LegacyPinRemoval, serde_json::Error> {
    let mut rows: std::collections::BTreeMap<String, serde_json::Value> =
        serde_json::from_str(json)?;
    if rows.remove(pin_id).is_none() {
        return Ok(LegacyPinRemoval::Unchanged);
    }
    if rows.is_empty() {
        Ok(LegacyPinRemoval::Empty)
    } else {
        serde_json::to_string(&rows).map(LegacyPinRemoval::Remaining)
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn epoch(board_key: &str) -> Result<u64, FrbError> {
    let Some(raw) = storage::try_get_item(&epoch_key(board_key)).map_err(|()| FrbError::Io {
        message: "read pin work epoch from browser storage failed".to_string(),
    })?
    else {
        return Ok(0);
    };
    raw.parse().map_err(|err| FrbError::Io {
        message: format!("invalid pin work epoch: {err}"),
    })
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn prefix(kind: &str, board_key: &str) -> Result<String, FrbError> {
    Ok(prefix_for_epoch(kind, board_key, epoch(board_key)?))
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct EpochReservation {
    old_epoch: u64,
    previous: Option<String>,
    board_key: String,
}

#[cfg(target_arch = "wasm32")]
impl EpochReservation {
    pub(crate) fn old_epoch(&self) -> u64 {
        self.old_epoch
    }
}

/// Reserve a fixed-width epoch key after shrinking metadata (or removing a
/// snapshot of the board's work). Its previous value can be restored if the
/// board deletion cannot commit.
#[cfg(target_arch = "wasm32")]
pub(crate) fn reserve_board_epoch(board_key: &str) -> Result<EpochReservation, FrbError> {
    let previous = storage::try_get_item(&epoch_key(board_key)).map_err(|()| FrbError::Io {
        message: "read pin work epoch before board deletion failed".to_string(),
    })?;
    let current = previous
        .as_deref()
        .map(str::parse::<u64>)
        .transpose()
        .map_err(|err| FrbError::Io {
            message: format!("invalid pin work epoch: {err}"),
        })?
        .unwrap_or(0);
    current.checked_add(1).ok_or_else(|| FrbError::Internal {
        message: "pin work epoch exhausted".to_string(),
    })?;
    storage::set_item(&epoch_key(board_key), &format!("{current:020}")).map_err(|()| {
        FrbError::Io {
            message: "reserve pin work epoch in browser storage failed".to_string(),
        }
    })?;
    Ok(EpochReservation {
        old_epoch: current,
        previous,
        board_key: board_key.to_string(),
    })
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn restore_epoch(reservation: &EpochReservation) -> Result<(), FrbError> {
    let key = epoch_key(&reservation.board_key);
    let restore = match &reservation.previous {
        Some(previous) => storage::set_item(&key, previous),
        None => storage::remove_item(&key),
    };
    restore.map_err(|()| FrbError::Io {
        message: "restore pin work epoch after failed board deletion failed".to_string(),
    })
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct BoardWorkSnapshot(Vec<(String, String)>);

/// Hold only this board's old pin records in memory. The caller can remove
/// them to free quota for first-time board/layout metadata writes.
#[cfg(target_arch = "wasm32")]
pub(crate) fn snapshot_board_work(
    board_key: &str,
    old_epoch: u64,
) -> Result<BoardWorkSnapshot, FrbError> {
    let keys = storage::list_keys("upeg.").map_err(|()| FrbError::Io {
        message: "list board work before deletion failed".to_string(),
    })?;
    let mut entries = Vec::new();
    for key in cleanup_keys_for_board(board_key, old_epoch, &keys) {
        if let Some(value) = storage::try_get_item(&key).map_err(|()| FrbError::Io {
            message: "read board work before deletion failed".to_string(),
        })? {
            entries.push((key, value));
        }
    }
    Ok(BoardWorkSnapshot(entries))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn remove_snapshot(snapshot: &BoardWorkSnapshot) -> Result<(), FrbError> {
    for (key, _) in &snapshot.0 {
        storage::remove_item(key).map_err(|()| FrbError::Io {
            message: "remove board work to free browser storage failed".to_string(),
        })?;
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn restore_snapshot(snapshot: &BoardWorkSnapshot) -> Result<(), FrbError> {
    for (key, value) in &snapshot.0 {
        let existing = storage::try_get_item(key).map_err(|()| FrbError::Io {
            message: "read board work during rollback failed".to_string(),
        })?;
        if existing.is_none() {
            storage::set_item(key, value).map_err(|()| FrbError::Io {
                message: "restore board work after failed deletion failed".to_string(),
            })?;
        }
    }
    Ok(())
}

/// Clear one removed pin without allocating a tombstone key. The legacy map
/// entry is pruned first so deleting the v2 key cannot reveal an old value.
#[cfg(target_arch = "wasm32")]
pub(crate) fn clear_pin_work_kind(
    kind: &str,
    board_key: &str,
    pin_id: &str,
) -> Result<(), FrbError> {
    let legacy_key = match kind {
        "pin_drafts" => format!("upeg.pin_drafts.v1.{board_key}"),
        "last_outcomes" => format!("upeg.last_outcomes.v1.{board_key}"),
        _ => {
            return Err(FrbError::Internal {
                message: "unknown pin work kind".to_string(),
            });
        }
    };
    if epoch(board_key)? == 0
        && let Some(json) = storage::try_get_item(&legacy_key).map_err(|()| FrbError::Io {
            message: "read legacy pin work before removal failed".to_string(),
        })?
    {
        match remove_legacy_pin_row(&json, pin_id).map_err(|err| FrbError::Io {
            message: format!("parse legacy pin work before removal: {err}"),
        })? {
            LegacyPinRemoval::Unchanged => {}
            LegacyPinRemoval::Empty => {
                storage::remove_item(&legacy_key).map_err(|()| FrbError::Io {
                    message: "remove empty legacy pin work failed".to_string(),
                })?;
            }
            LegacyPinRemoval::Remaining(remaining) => {
                storage::set_item(&legacy_key, &remaining).map_err(|()| FrbError::Io {
                    message: "save smaller legacy pin work failed".to_string(),
                })?;
            }
        }
    }
    let key = format!("{}{}", prefix(kind, board_key)?, pin_id);
    storage::remove_item(&key).map_err(|()| FrbError::Io {
        message: "remove pin work from browser storage failed".to_string(),
    })
}

/// Commit logical deletion. The caller can still restore board/layout keys
/// if this write fails; it must remove old work only after this succeeds.
#[cfg(target_arch = "wasm32")]
pub(crate) fn advance_board_epoch(board_key: &str, expected_epoch: u64) -> Result<(), FrbError> {
    if epoch(board_key)? != expected_epoch {
        return Err(FrbError::Io {
            message: "pin work epoch changed during board deletion".to_string(),
        });
    }
    let next = expected_epoch
        .checked_add(1)
        .ok_or_else(|| FrbError::Internal {
            message: "pin work epoch exhausted".to_string(),
        })?;
    storage::set_item(&epoch_key(board_key), &format!("{next:020}")).map_err(|()| FrbError::Io {
        message: "advance pin work epoch in browser storage failed".to_string(),
    })
}

/// Reclaim old per-pin keys and v1 maps after the epoch has advanced. Keys
/// written under the new epoch are preserved.
#[cfg(target_arch = "wasm32")]
pub(crate) fn cleanup_deleted_board_keys(board_key: &str, old_epoch: u64) -> Result<(), FrbError> {
    let keys = storage::list_keys("upeg.").map_err(|()| FrbError::Io {
        message: "board deleted, but listing old work for cleanup failed".to_string(),
    })?;
    for key in cleanup_keys_for_board(board_key, old_epoch, &keys) {
        storage::remove_item(&key).map_err(|()| FrbError::Io {
            message: "board deleted, but removing old work from browser storage failed".to_string(),
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn deleted_board_cleanup_reclaims_only_its_old_work() {
        let old_draft = format!("{}pin-a", prefix_for_epoch("pin_drafts", "dev", 0));
        let old_outcome = format!("{}pin-b", prefix_for_epoch("last_outcomes", "dev", 1));
        let new_draft = format!("{}pin-c", prefix_for_epoch("pin_drafts", "dev", 2));
        let other_draft = format!("{}pin-d", prefix_for_epoch("pin_drafts", "dev.ops", 0));
        let mut store = BTreeMap::from([
            ("upeg.pin_drafts.v1.dev".to_string(), "legacy draft"),
            ("upeg.last_outcomes.v1.dev".to_string(), "legacy result"),
            (old_draft.clone(), "old draft"),
            (old_outcome.clone(), "old result"),
            (new_draft.clone(), "new work"),
            (other_draft.clone(), "other board"),
            ("upeg.pin_drafts.v1.dev.ops".to_string(), "other legacy"),
        ]);
        let keys: Vec<_> = store.keys().cloned().collect();
        for key in cleanup_keys_for_board("dev", 1, &keys) {
            store.remove(&key);
        }
        assert_eq!(store.len(), 3);
        assert!(store.contains_key(&new_draft));
        assert!(store.contains_key(&other_draft));
        assert!(store.contains_key("upeg.pin_drafts.v1.dev.ops"));
        assert!(!store.contains_key(&old_draft));
        assert!(!store.contains_key(&old_outcome));
    }

    #[test]
    fn legacy_pin_removal_shrinks_map_and_preserves_other_pin() {
        let source = r#"{"pin-a":{"args_json":"large"},"pin-b":{"args_json":"keep"}}"#;
        let LegacyPinRemoval::Remaining(smaller) =
            remove_legacy_pin_row(source, "pin-a").expect("valid legacy map")
        else {
            panic!("one sibling should remain");
        };
        assert!(smaller.len() < source.len());
        assert_eq!(smaller, r#"{"pin-b":{"args_json":"keep"}}"#);
        assert!(matches!(
            remove_legacy_pin_row(&smaller, "pin-b").expect("last pin"),
            LegacyPinRemoval::Empty
        ));
    }
}
