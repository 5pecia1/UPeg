//! Persisted per-pin last dispatch outcomes.
//!
//! The Dart `lastOutcomeProvider` cache is write-through: every recorded
//! outcome lands here as well, keyed per placement (`board_key`,
//! `pin_id`) in the shared SQLite store (`upeg-sources::store`,
//! schema v6). On boot / board switch the provider hydrates from
//! [`load_last_outcomes`], so a restart restores the most recent result
//! of every pinned tool as a "restored" (not fresh) state.
//!
//! Outputs cross the store as a JSON array of
//! [`CanonicalOutputEntry`] (serde derives on the DTO); oversized
//! payloads are capped store-side (`OUTPUTS_JSON_MAX_BYTES`) and come
//! back with `truncated = true` so the pin can render an ellipsis.
//! `CanonicalToolError::details` is deliberately not persisted — the
//! store carries only `error_code` / `error_message`.
//!
//! Wasm32 has no shared native store: `record_last_outcome` is a no-op
//! and `load_last_outcomes` returns an empty list, following the
//! `shared_state` cfg pattern.

use super::boot::FrbError;
use super::tools::CanonicalToolResult;

/// One persisted outcome for a pinned tool, as returned by
/// [`load_last_outcomes`].
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct LastOutcomeDto {
    /// The placement identity, independent from `tool_id`.
    pub pin_id: String,
    pub tool_id: String,
    /// The canonical result as recorded (minus `error.details`, which
    /// the store does not persist).
    pub result: CanonicalToolResult,
    /// `true` when the store's size cap dropped output entries.
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
    result: CanonicalToolResult,
) -> Result<(), FrbError> {
    let outcome = native::new_last_outcome_from(pin_id, tool_id, &result)?;
    let mut store = native::open_store()?;
    store
        .record_last_outcome(&board_key, &outcome)
        .map_err(|err| FrbError::Io {
            message: format!("record last outcome: {err}"),
        })
}

/// Wasm32 no-op twin of the native [`record_last_outcome`]: the PWA has
/// no shared store to write through to.
#[flutter_rust_bridge::frb(sync)]
#[cfg(target_arch = "wasm32")]
pub fn record_last_outcome(
    board_key: String,
    pin_id: String,
    tool_id: String,
    result: CanonicalToolResult,
) -> Result<(), FrbError> {
    let _ = (board_key, pin_id, tool_id, result);
    Ok(())
}

/// All persisted outcomes for `board_key`, sorted by pin id.
///
/// Best-effort read (mirrors `list_boards`): an unopenable store or a
/// corrupt row yields an empty / shorter list instead of an error —
/// hydration is a cache warm-up, never a boot blocker.
#[flutter_rust_bridge::frb(sync)]
#[cfg(not(target_arch = "wasm32"))]
pub fn load_last_outcomes(board_key: String) -> Vec<LastOutcomeDto> {
    let Ok(store) = native::open_store() else {
        return Vec::new();
    };
    store
        .load_last_outcomes(&board_key)
        .unwrap_or_default()
        .into_iter()
        .filter_map(native::dto_from_record)
        .collect()
}

/// Wasm32 twin of the native [`load_last_outcomes`]: nothing was ever
/// persisted, so there is nothing to restore.
#[flutter_rust_bridge::frb(sync)]
#[cfg(target_arch = "wasm32")]
pub fn load_last_outcomes(board_key: String) -> Vec<LastOutcomeDto> {
    let _ = board_key;
    Vec::new()
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
        result: &CanonicalToolResult,
    ) -> Result<NewLastOutcome, FrbError> {
        let outputs_json =
            serde_json::to_string(&result.outputs).map_err(|err| FrbError::Internal {
                message: format!("serialize outputs: {err}"),
            })?;
        Ok(NewLastOutcome {
            pin_id,
            tool_id,
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
        CanonicalOutputEntry, CanonicalOutputValue, CanonicalToolError, CanonicalToolResult,
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
            let outcome = new_last_outcome_from(pin_id.to_string(), tool_id.to_string(), &result)
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
    fn corrupted_outputs_json_row_is_filtered_out_in_dto_conversion() {
        let record = upeg_sources::store::LastOutcomeRecord {
            pin_id: "pin-hex".to_string(),
            tool_id: "num.hex".to_string(),
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
