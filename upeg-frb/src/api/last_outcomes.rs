//! Persisted per-pin last dispatch outcomes.
//!
//! The Dart `lastOutcomeProvider` cache is write-through: every recorded
//! outcome lands here as well, keyed per placement (`board_key`,
//! `pin_id`) in the shared SQLite store (`upeg-sources::store`,
//! schema v8). On boot / board switch the provider hydrates from
//! [`load_last_outcomes`], so a restart restores the most recent result
//! of every pinned tool as a "restored" (not fresh) state.
//!
//! Outputs cross the store as a complete JSON array of
//! [`CanonicalOutputEntry`] (serde derives on the DTO). Older rows may
//! carry `truncated = true` from the former size cap.
//! `CanonicalToolError::details` is deliberately not persisted — the
//! store carries only `error_code` / `error_message`. The input snapshot
//! attached to a result remains independent of the pin's current draft.
//!
//! Wasm32 stores the same per-pin snapshot in browser localStorage.

use super::boot::FrbError;
use super::tools::CanonicalToolResult;

/// One persisted outcome for a pinned tool, as returned by
/// [`load_last_outcomes`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct LastOutcomeDto {
    /// The placement identity, independent from `tool_id`.
    pub pin_id: String,
    pub tool_id: String,
    /// Input snapshot that produced this result. `None` for pre-v8 rows.
    pub args_json: Option<String>,
    /// The canonical result as recorded (minus `error.details`, which
    /// the store does not persist).
    pub result: CanonicalToolResult,
    /// `true` for an older row whose outputs were truncated.
    pub truncated: bool,
    /// Epoch milliseconds of the recording write (store clock).
    pub updated_at_ms: i64,
}

/// Persist `result` as the last outcome of `pin_id` / `tool_id` on
/// `board_key`.
/// Both ok and error results are recorded — the store keeps whatever
/// the dispatch produced; presentation policy (e.g. render only ok
/// results inline) stays with the consumer.
#[flutter_rust_bridge::frb(sync)]
#[cfg(not(target_arch = "wasm32"))]
pub fn record_last_outcome(
    board_key: String,
    pin_id: String,
    tool_id: String,
    args_json: Option<String>,
    result: CanonicalToolResult,
) -> Result<(), FrbError> {
    let args_json = validated_args_json(args_json)?;
    let _catalog_read = upeg_runtime::project_scope::catalog_read_guard();
    let store_board_key =
        upeg_sources::pegboard::BoardVisibility::from_process().store_key(&board_key);
    let outcome = native::new_last_outcome_from(pin_id, tool_id, args_json, &result)?;
    let mut store = native::open_store()?;
    store
        .record_last_outcome(&store_board_key, &outcome)
        .map_err(|err| FrbError::Io {
            message: format!("record last outcome: {err}"),
        })
}

/// Save the latest result for one pin in browser storage.
#[flutter_rust_bridge::frb(sync)]
#[cfg(target_arch = "wasm32")]
pub fn record_last_outcome(
    board_key: String,
    pin_id: String,
    tool_id: String,
    args_json: Option<String>,
    result: CanonicalToolResult,
) -> Result<(), FrbError> {
    let args_json = validated_args_json(args_json)?;
    let mut result = result;
    result.error.as_mut().map(|error| error.details = None);
    let value = serde_json::to_value(LastOutcomeDto {
        pin_id: pin_id.clone(),
        tool_id,
        args_json,
        result,
        truncated: false,
        updated_at_ms: js_sys::Date::now() as i64,
    })
    .map_err(|err| FrbError::Internal {
        message: format!("serialize last outcome: {err}"),
    })?;
    web::write_pin(&board_key, &pin_id, &value)
}

fn validated_args_json(args_json: Option<String>) -> Result<Option<String>, FrbError> {
    args_json
        .map(|json| {
            upeg_core::ArgsPreset::parse(&json)
                .map(|args| args.as_str().to_string())
                .map_err(|err| FrbError::Validation {
                    field: "args_json".to_string(),
                    reason: err.to_string(),
                })
        })
        .transpose()
}

/// All persisted outcomes for `board_key`, sorted by pin id.
///
/// Storage failures reach the caller for retry. Corrupt result payload rows
/// are skipped so one bad pin does not hide its healthy neighbors.
#[flutter_rust_bridge::frb(sync)]
#[cfg(not(target_arch = "wasm32"))]
pub fn load_last_outcomes(board_key: String) -> Result<Vec<LastOutcomeDto>, FrbError> {
    let _catalog_read = upeg_runtime::project_scope::catalog_read_guard();
    let store_board_key =
        upeg_sources::pegboard::BoardVisibility::from_process().store_key(&board_key);
    let store = native::open_store()?;
    Ok(store
        .load_last_outcomes(&store_board_key)
        .map_err(|err| FrbError::Io {
            message: format!("load last outcomes: {err}"),
        })?
        .into_iter()
        .filter_map(native::dto_from_record)
        .collect())
}

/// Restore the latest results for a board from browser storage.
#[flutter_rust_bridge::frb(sync)]
#[cfg(target_arch = "wasm32")]
pub fn load_last_outcomes(board_key: String) -> Result<Vec<LastOutcomeDto>, FrbError> {
    Ok(web::load_rows(&board_key)?
        .into_iter()
        .filter_map(|(pin_id, value)| {
            serde_json::from_value::<LastOutcomeDto>(value)
                .ok()
                .filter(|outcome| outcome.pin_id == pin_id)
        })
        .collect())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn clear_web_pin_outcome(board_key: &str, pin_id: &str) -> Result<(), FrbError> {
    super::pin_web_storage::clear_pin_work_kind("last_outcomes", board_key, pin_id)
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::collections::BTreeMap;

    use super::FrbError;

    fn legacy_storage_key(board_key: &str) -> String {
        format!("upeg.last_outcomes.v1.{board_key}")
    }

    pub(super) fn load_rows(
        board_key: &str,
    ) -> Result<BTreeMap<String, serde_json::Value>, FrbError> {
        use upeg_pegboard_ui::platform::storage;
        let mut rows = if super::super::pin_web_storage::epoch(board_key)? == 0
            && let Some(json) =
                storage::try_get_item(&legacy_storage_key(board_key)).map_err(|()| {
                    FrbError::Io {
                        message: "read legacy last outcomes from browser storage failed"
                            .to_string(),
                    }
                })? {
            serde_json::from_str(&json).map_err(|err| FrbError::Io {
                message: format!("read legacy last outcomes: {err}"),
            })?
        } else {
            BTreeMap::new()
        };
        let prefix = super::super::pin_web_storage::prefix("last_outcomes", board_key)?;
        let keys = storage::list_keys(&prefix).map_err(|()| FrbError::Io {
            message: "list last outcomes from browser storage failed".to_string(),
        })?;
        for key in keys {
            let Some(pin_id) = key.strip_prefix(&prefix) else {
                continue;
            };
            let Some(raw) = storage::try_get_item(&key).map_err(|()| FrbError::Io {
                message: "read last outcome from browser storage failed".to_string(),
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

    pub(super) fn write_pin(
        board_key: &str,
        pin_id: &str,
        outcome: &serde_json::Value,
    ) -> Result<(), FrbError> {
        let key = format!(
            "{}{}",
            super::super::pin_web_storage::prefix("last_outcomes", board_key)?,
            pin_id
        );
        let json = serde_json::to_string(outcome).map_err(|err| FrbError::Internal {
            message: format!("serialize last outcome: {err}"),
        })?;
        upeg_pegboard_ui::platform::storage::set_item(&key, &json).map_err(|()| FrbError::Io {
            message: "save last outcomes to browser storage failed".to_string(),
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use upeg_sources::store::{LastOutcomeRecord, NewLastOutcome, Store};

    use super::super::boot::FrbError;
    use super::super::tools::{CanonicalOutputEntry, CanonicalToolError, CanonicalToolResult};
    use super::LastOutcomeDto;

    pub(super) fn open_store() -> Result<Store, FrbError> {
        Store::open().map_err(|err| FrbError::Io {
            message: format!("open shared store: {err}"),
        })
    }

    /// Flatten a [`CanonicalToolResult`] into the store's row shape.
    /// Output entries serialize as one JSON array; a serialization
    /// failure is an [`FrbError::Internal`] (the DTO is plain data, so
    /// this indicates a bug rather than bad user input).
    pub(super) fn new_last_outcome_from(
        pin_id: String,
        tool_id: String,
        args_json: Option<String>,
        result: &CanonicalToolResult,
    ) -> Result<NewLastOutcome, FrbError> {
        let outputs_json =
            serde_json::to_string(&result.outputs).map_err(|err| FrbError::Internal {
                message: format!("serialize outputs: {err}"),
            })?;
        Ok(NewLastOutcome {
            pin_id,
            tool_id,
            args_json,
            ok: result.ok,
            primary_output_id: result.primary_output_id.clone(),
            outputs_json,
            error_code: result.error.as_ref().map(|error| error.code.clone()),
            error_message: result.error.as_ref().map(|error| error.message.clone()),
        })
    }

    /// Rebuild the wire DTO from a stored row. `None` when the
    /// persisted `outputs_json` no longer parses as canonical entries
    /// (corrupt row) — the caller skips such rows.
    pub(super) fn dto_from_record(record: LastOutcomeRecord) -> Option<LastOutcomeDto> {
        let outputs: Vec<CanonicalOutputEntry> = serde_json::from_str(&record.outputs_json).ok()?;
        let error = record.error_code.map(|code| CanonicalToolError {
            code,
            message: record.error_message.unwrap_or_default(),
            details: None,
        });
        Some(LastOutcomeDto {
            pin_id: record.pin_id,
            tool_id: record.tool_id,
            args_json: record.args_json,
            result: CanonicalToolResult {
                ok: record.ok,
                primary_output_id: record.primary_output_id,
                outputs,
                error,
            },
            truncated: record.truncated,
            updated_at_ms: record.updated_at_ms,
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use upeg_sources::store::Store;

    use super::super::tools::{
        CanonicalFileContent, CanonicalFileValue, CanonicalOutputEntry, CanonicalOutputValue,
        CanonicalToolError, CanonicalToolResult,
    };
    use super::native::{dto_from_record, new_last_outcome_from};

    fn ok_result() -> CanonicalToolResult {
        CanonicalToolResult {
            ok: true,
            primary_output_id: Some("value".to_string()),
            outputs: vec![CanonicalOutputEntry {
                id: "value".to_string(),
                label: Some("Value".to_string()),
                kind: "string".to_string(),
                value: CanonicalOutputValue::String {
                    value: "42".to_string(),
                },
            }],
            error: None,
        }
    }

    fn error_result() -> CanonicalToolResult {
        CanonicalToolResult {
            ok: false,
            primary_output_id: None,
            outputs: Vec::new(),
            error: Some(CanonicalToolError {
                code: "E_FAIL".to_string(),
                message: "boom".to_string(),
                details: None,
            }),
        }
    }

    fn temp_store() -> (std::path::PathBuf, Store) {
        let dir = std::env::temp_dir().join(format!(
            "upeg-frb-last-outcomes-{}-{:?}",
            std::process::id(),
            std::thread::current().id(),
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(upeg_core::paths::STORE_FILE);
        let store = Store::open_at(&path).expect("temp store");
        (dir, store)
    }

    #[test]
    fn canonical_result_round_trips_through_store_row_to_dto() {
        let (dir, mut store) = temp_store();
        for (pin_id, tool_id, result) in [
            ("pin-hex", "num.hex", ok_result()),
            ("pin-ping", "net.ping", error_result()),
        ] {
            let outcome = new_last_outcome_from(
                pin_id.to_string(),
                tool_id.to_string(),
                Some(r#"{"value":"42"}"#.to_string()),
                &result,
            )
            .expect("row conversion");
            store.record_last_outcome("dev", &outcome).expect("record");
        }

        let loaded: Vec<_> = store
            .load_last_outcomes("dev")
            .expect("load")
            .into_iter()
            .filter_map(dto_from_record)
            .collect();
        assert_eq!(loaded.len(), 2);

        let hex = &loaded[0];
        assert_eq!(hex.pin_id, "pin-hex");
        assert_eq!(hex.tool_id, "num.hex");
        assert_eq!(hex.args_json.as_deref(), Some(r#"{"value":"42"}"#));
        assert_eq!(hex.result, ok_result());
        assert!(!hex.truncated);

        let ping = &loaded[1];
        assert_eq!(ping.pin_id, "pin-ping");
        assert_eq!(ping.tool_id, "net.ping");
        assert_eq!(ping.result, error_result());
        assert!(ping.updated_at_ms > 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn large_file_result_survives_store_reopen_without_truncation() {
        let (dir, mut store) = temp_store();
        let result = CanonicalToolResult {
            ok: true,
            primary_output_id: Some("file".to_string()),
            outputs: vec![CanonicalOutputEntry {
                id: "file".to_string(),
                label: None,
                kind: "file".to_string(),
                value: CanonicalOutputValue::File {
                    value: CanonicalFileValue {
                        name: "large.bin".to_string(),
                        is_dir: false,
                        content: CanonicalFileContent::Bytes {
                            bytes: vec![42; 70 * 1024],
                        },
                        mime: None,
                    },
                },
            }],
            error: None,
        };
        let record = new_last_outcome_from(
            "pin-file".to_string(),
            "file.tool".to_string(),
            Some(r#"{"file":"large.bin"}"#.to_string()),
            &result,
        )
        .expect("serialize large file");
        assert!(record.outputs_json.contains("\"bytes\":\""));
        store
            .record_last_outcome("dev", &record)
            .expect("record large file");
        drop(store);

        let reopened = Store::open_at(&dir.join(upeg_core::paths::STORE_FILE)).expect("reopen");
        let loaded = reopened.load_last_outcomes("dev").expect("load");
        assert_eq!(loaded.len(), 1);
        let dto = dto_from_record(loaded.into_iter().next().expect("row")).expect("dto");
        assert_eq!(dto.result, result);
        assert!(!dto.truncated);
        assert_eq!(dto.args_json.as_deref(), Some(r#"{"file":"large.bin"}"#));
        drop(reopened);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_numeric_file_bytes_still_decode() {
        let json = r#"{"Bytes":{"bytes":[0,1,255]}}"#;
        let parsed: CanonicalFileContent = serde_json::from_str(json).expect("legacy file bytes");
        assert_eq!(
            parsed,
            CanonicalFileContent::Bytes {
                bytes: vec![0, 1, 255]
            }
        );
    }

    #[test]
    fn corrupted_outputs_json_row_is_filtered_out_in_dto_conversion() {
        let record = upeg_sources::store::LastOutcomeRecord {
            pin_id: "pin-hex".to_string(),
            tool_id: "num.hex".to_string(),
            args_json: None,
            ok: true,
            primary_output_id: None,
            outputs_json: "not json".to_string(),
            truncated: false,
            error_code: None,
            error_message: None,
            updated_at_ms: 1,
        };
        assert!(dto_from_record(record).is_none());
    }
}
