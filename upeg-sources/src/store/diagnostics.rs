//! Bounded local failure diagnostics.
//!
//! Unlike the metadata-only execution log, these rows retain a redacted,
//! capped error report so an operator can inspect or export a failed run
//! after its result modal has closed. This is a local store, never a network
//! reporting service.

use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use upeg_core::diagnostics::{
    DIAGNOSTIC_CONTEXT_MAX_BYTES, DiagnosticDraft, redact_diagnostic_text,
};

use super::schema::DIAGNOSTICS_TABLE;
use super::{Store, StoreError};

/// Retain a finite recent window so failure reports cannot grow unbounded.
pub const DIAGNOSTIC_RETENTION: usize = 200;
const DEFAULT_READ_LIMIT: usize = 50;

/// A persisted, redacted diagnostic report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRecord {
    pub id: String,
    pub run_id: String,
    pub occurred_at_ms: u64,
    pub app_version: String,
    pub os: String,
    pub tool_id: Option<String>,
    pub source: String,
    pub cwd: Option<String>,
    pub project: Option<String>,
    pub error_code: String,
    pub error_message: String,
    pub status: String,
    pub stdout: String,
    pub stderr: String,
    pub debug_context: Option<String>,
}

impl Store {
    /// Persist a failed run and evict reports older than the local retention
    /// window. The draft is redacted here, at the last boundary before disk.
    pub fn append_diagnostic(&mut self, draft: DiagnosticDraft) -> Result<String, StoreError> {
        let draft = draft.redacted();
        let id = uuid::Uuid::now_v7().to_string();
        let debug_context = draft.debug_context.and_then(serialize_debug_context);
        let sql = format!(
            "INSERT INTO {DIAGNOSTICS_TABLE} \
             (id, run_id, occurred_at_ms, app_version, os, tool_id, source, cwd, project, \
              error_code, error_message, status, stdout, stderr, debug_context) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"
        );
        let trim_sql = format!(
            "DELETE FROM {DIAGNOSTICS_TABLE} WHERE id NOT IN \
             (SELECT id FROM {DIAGNOSTICS_TABLE} ORDER BY occurred_at_ms DESC, id DESC LIMIT ?1)"
        );
        self.write_tx(|tx| {
            tx.execute(
                &sql,
                params![
                    id,
                    draft.run_id,
                    clamp_to_i64(draft.occurred_at_ms),
                    draft.app_version,
                    draft.os,
                    draft.tool_id,
                    draft.source,
                    draft.cwd,
                    draft.project,
                    draft.error_code,
                    draft.error_message,
                    draft.status,
                    draft.stdout,
                    draft.stderr,
                    debug_context,
                ],
            )?;
            tx.execute(
                &trim_sql,
                [i64::try_from(DIAGNOSTIC_RETENTION).unwrap_or(i64::MAX)],
            )?;
            Ok(())
        })?;
        Ok(id)
    }

    /// Return newest reports first, bounded even when the caller omits a limit.
    pub fn read_diagnostics(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<DiagnosticRecord>, StoreError> {
        let limit = limit
            .unwrap_or(DEFAULT_READ_LIMIT)
            .min(DIAGNOSTIC_RETENTION);
        let sql = format!(
            "SELECT id, run_id, occurred_at_ms, app_version, os, tool_id, source, cwd, project, \
                    error_code, error_message, status, stdout, stderr, debug_context \
             FROM {DIAGNOSTICS_TABLE} ORDER BY occurred_at_ms DESC, id DESC LIMIT ?1"
        );
        let mut statement = self.connection().prepare(&sql)?;
        let rows =
            statement.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], row_to_record)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Read one retained report by its stable export id.
    pub fn diagnostic(&self, id: &str) -> Result<Option<DiagnosticRecord>, StoreError> {
        let sql = format!(
            "SELECT id, run_id, occurred_at_ms, app_version, os, tool_id, source, cwd, project, \
                    error_code, error_message, status, stdout, stderr, debug_context \
             FROM {DIAGNOSTICS_TABLE} WHERE id = ?1"
        );
        self.connection()
            .query_row(&sql, [id], row_to_record)
            .optional()
            .map_err(StoreError::from)
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<DiagnosticRecord> {
    Ok(DiagnosticRecord {
        id: row.get(0)?,
        run_id: row.get(1)?,
        occurred_at_ms: nonnegative_u64(row.get(2)?),
        app_version: row.get(3)?,
        os: row.get(4)?,
        tool_id: row.get(5)?,
        source: row.get(6)?,
        cwd: row.get(7)?,
        project: row.get(8)?,
        error_code: row.get(9)?,
        error_message: row.get(10)?,
        status: row.get(11)?,
        stdout: row.get(12)?,
        stderr: row.get(13)?,
        debug_context: row.get(14)?,
    })
}

fn clamp_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn nonnegative_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

/// Keep debug detail valid JSON even when its aggregate representation would
/// exceed the storage budget. A plain string truncate would make the value
/// unparsable just when a support tool needs it most.
fn serialize_debug_context(value: serde_json::Value) -> Option<String> {
    let serialized = serde_json::to_string(&value).ok()?;
    if serialized.len() <= DIAGNOSTIC_CONTEXT_MAX_BYTES {
        return Some(serialized);
    }
    serde_json::to_string(&serde_json::json!({
        "truncated": true,
        "preview": redact_diagnostic_text(&serialized, DIAGNOSTIC_CONTEXT_MAX_BYTES / 2),
    }))
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(number: usize) -> DiagnosticDraft {
        DiagnosticDraft {
            run_id: format!("run-{number}"),
            occurred_at_ms: u64::try_from(number).expect("usize fits u64"),
            app_version: "test".to_string(),
            os: "test".to_string(),
            tool_id: Some("tool.fail".to_string()),
            source: "cli".to_string(),
            cwd: None,
            project: None,
            error_code: "failed".to_string(),
            error_message: "TOKEN=visible".to_string(),
            status: "tool_error".to_string(),
            stdout: String::new(),
            stderr: String::new(),
            debug_context: None,
        }
    }

    #[test]
    fn diagnostics_are_redacted_and_retained_in_a_bounded_newest_window() {
        let mut store = Store::open_in_memory().expect("store");
        for number in 0..(DIAGNOSTIC_RETENTION + 2) {
            store.append_diagnostic(draft(number)).expect("append");
        }
        let reports = store
            .read_diagnostics(Some(DIAGNOSTIC_RETENTION + 10))
            .expect("read");
        assert_eq!(reports.len(), DIAGNOSTIC_RETENTION);
        assert_eq!(
            reports.first().map(|report| report.run_id.as_str()),
            Some("run-201")
        );
        assert!(!reports[0].error_message.contains("visible"));
        assert!(store.diagnostic(&reports[0].id).expect("show").is_some());
    }

    #[test]
    fn timeout_and_cancellation_debug_context_round_trip_without_raw_secret_values() {
        let mut store = Store::open_in_memory().expect("store");
        let mut timed_out = draft(1);
        timed_out.debug_context = Some(serde_json::json!({
            "timed_out": true,
            "cancelled": true,
            "token": "must-not-persist",
        }));
        let id = store.append_diagnostic(timed_out).expect("append");
        let report = store.diagnostic(&id).expect("read").expect("report");
        let debug: serde_json::Value =
            serde_json::from_str(report.debug_context.as_deref().expect("debug context"))
                .expect("debug context JSON");
        assert_eq!(debug["timed_out"], serde_json::Value::Bool(true));
        assert_eq!(debug["cancelled"], serde_json::Value::Bool(true));
        assert_eq!(
            debug["token"],
            serde_json::Value::String("[REDACTED]".to_string())
        );
    }

    #[test]
    fn oversized_debug_context_remains_valid_json_after_capping() {
        let mut store = Store::open_in_memory().expect("store");
        let mut oversized = draft(1);
        oversized.debug_context = Some(serde_json::json!({
            "stderr": "x".repeat(DIAGNOSTIC_CONTEXT_MAX_BYTES * 2),
        }));
        let id = store.append_diagnostic(oversized).expect("append");
        let report = store.diagnostic(&id).expect("read").expect("report");
        let debug: serde_json::Value =
            serde_json::from_str(report.debug_context.as_deref().expect("debug context"))
                .expect("capped debug context remains JSON");
        assert_eq!(debug["truncated"], serde_json::Value::Bool(true));
    }
}
