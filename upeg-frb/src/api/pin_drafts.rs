//! Persist the current form values of each pin without changing its saved
//! dispatch preset or the input attached to its last result.

use upeg_core::ArgsPreset;

use super::boot::FrbError;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PinDraftDto {
    pub pin_id: String,
    pub tool_id: String,
    pub args_json: String,
}

#[flutter_rust_bridge::frb(sync)]
pub fn save_pin_draft(
    board_key: String,
    pin_id: String,
    tool_id: String,
    args_json: String,
) -> Result<(), FrbError> {
    let args_json = ArgsPreset::parse(&args_json)
        .map_err(|err| FrbError::Validation {
            field: "args_json".to_string(),
            reason: err.to_string(),
        })?
        .as_str()
        .to_string();
    save_pin_draft_inner(
        &board_key,
        PinDraftDto {
            pin_id,
            tool_id,
            args_json,
        },
    )
}

/// Returns valid drafts for one board, sorted by pin id. Browser storage or
/// SQLite failures surface as errors so the UI can offer retry.
#[flutter_rust_bridge::frb(sync)]
pub fn load_pin_drafts(board_key: String) -> Result<Vec<PinDraftDto>, FrbError> {
    Ok(load_pin_drafts_inner(&board_key)?
        .into_iter()
        .filter(|draft| ArgsPreset::parse(&draft.args_json).is_ok())
        .collect())
}

#[cfg(not(target_arch = "wasm32"))]
fn save_pin_draft_inner(board_key: &str, draft: PinDraftDto) -> Result<(), FrbError> {
    let _catalog_read = upeg_runtime::project_scope::catalog_read_guard();
    let store_board_key =
        upeg_sources::pegboard::BoardVisibility::from_process().store_key(board_key);
    let mut store = upeg_sources::store::Store::open().map_err(store_error)?;
    store
        .save_pin_draft(
            &store_board_key,
            &upeg_sources::store::PinDraftRecord {
                pin_id: draft.pin_id,
                tool_id: draft.tool_id,
                args_json: draft.args_json,
            },
        )
        .map_err(store_error)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_pin_drafts_inner(board_key: &str) -> Result<Vec<PinDraftDto>, FrbError> {
    let _catalog_read = upeg_runtime::project_scope::catalog_read_guard();
    let store_board_key =
        upeg_sources::pegboard::BoardVisibility::from_process().store_key(board_key);
    let store = upeg_sources::store::Store::open().map_err(store_error)?;
    Ok(store
        .load_pin_drafts(&store_board_key)
        .map_err(store_error)?
        .into_iter()
        .map(|draft| PinDraftDto {
            pin_id: draft.pin_id,
            tool_id: draft.tool_id,
            args_json: draft.args_json,
        })
        .collect())
}

#[cfg(not(target_arch = "wasm32"))]
fn store_error(err: upeg_sources::store::StoreError) -> FrbError {
    FrbError::Io {
        message: format!("pin drafts: {err}"),
    }
}

#[cfg(target_arch = "wasm32")]
fn legacy_storage_key(board_key: &str) -> String {
    format!("upeg.pin_drafts.v1.{board_key}")
}

#[cfg(target_arch = "wasm32")]
fn save_pin_draft_inner(board_key: &str, draft: PinDraftDto) -> Result<(), FrbError> {
    let key = format!(
        "{}{}",
        super::pin_web_storage::prefix("pin_drafts", board_key)?,
        draft.pin_id
    );
    let json = serde_json::to_string(&draft).map_err(|err| FrbError::Internal {
        message: format!("serialize pin draft: {err}"),
    })?;
    upeg_pegboard_ui::platform::storage::set_item(&key, &json).map_err(|()| FrbError::Io {
        message: "save pin draft to browser storage failed".to_string(),
    })
}

#[cfg(target_arch = "wasm32")]
fn load_pin_drafts_inner(board_key: &str) -> Result<Vec<PinDraftDto>, FrbError> {
    Ok(load_web_rows(board_key)?
        .into_iter()
        .filter_map(|(pin_id, value)| {
            serde_json::from_value::<PinDraftDto>(value)
                .ok()
                .filter(|draft| draft.pin_id == pin_id)
        })
        .collect())
}

#[cfg(target_arch = "wasm32")]
fn load_web_rows(
    board_key: &str,
) -> Result<std::collections::BTreeMap<String, serde_json::Value>, FrbError> {
    use upeg_pegboard_ui::platform::storage;
    let mut rows = if super::pin_web_storage::epoch(board_key)? == 0
        && let Some(json) =
            storage::try_get_item(&legacy_storage_key(board_key)).map_err(|()| FrbError::Io {
                message: "read legacy pin drafts from browser storage failed".to_string(),
            })? {
        serde_json::from_str(&json).map_err(|err| FrbError::Io {
            message: format!("read legacy pin drafts: {err}"),
        })?
    } else {
        std::collections::BTreeMap::new()
    };
    let prefix = super::pin_web_storage::prefix("pin_drafts", board_key)?;
    let keys = storage::list_keys(&prefix).map_err(|()| FrbError::Io {
        message: "list pin drafts from browser storage failed".to_string(),
    })?;
    for key in keys {
        let Some(pin_id) = key.strip_prefix(&prefix) else {
            continue;
        };
        let Some(raw) = storage::try_get_item(&key).map_err(|()| FrbError::Io {
            message: "read pin draft from browser storage failed".to_string(),
        })?
        else {
            continue;
        };
        match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(serde_json::Value::Null) => {
                rows.remove(pin_id);
            }
            Ok(value) => {
                rows.insert(pin_id.to_string(), value);
            }
            Err(_) => {
                rows.remove(pin_id);
            }
        }
    }
    Ok(rows)
}

/// Called after a web placement is removed. Native tombstones clear drafts
/// in the same SQLite transaction as the placement change.
#[cfg(target_arch = "wasm32")]
pub(crate) fn clear_web_pin_draft(board_key: &str, pin_id: &str) -> Result<(), FrbError> {
    super::pin_web_storage::clear_pin_work_kind("pin_drafts", board_key, pin_id)
}
