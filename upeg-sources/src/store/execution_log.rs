//! Metadata-only execution log persistence on the SQLite store.
//!
//! The log records which Tool ran, from which surface/Board/Trigger, how
//! it ended, and how long it took — never Tool arguments, outputs, error
//! messages, or credential values (the PRD v2.1 contract enforced by
//! `upeg-cli::adapters::execution_log`). Rows are append-only history:
//! no updates, no tombstones.
//!
//! Concurrent appends from several processes (CLI dispatch + HTTP daemon)
//! are absorbed by WAL + the store-wide busy timeout: each append is one
//! short `BEGIN IMMEDIATE` transaction.
//!
//! The `trigger` column is quoted in SQL because `TRIGGER` is an SQLite
//! keyword.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use upeg_core::{RecentSignal, SearchSignals};

use super::schema::EXECUTION_LOG_TABLE;
use super::{Store, StoreError};

/// How many records a read returns when the filter carries no explicit
/// limit. Matches the CLI `log` command's historical default.
const DEFAULT_READ_LIMIT: usize = 100;

/// One dispatch record. The serde shape (field order, `None` fields
/// omitted) is the wire contract of `upeg log --json` and the HTTP
/// `/v1/logs` payload; the columns of the `execution_log` table mirror
/// it one-to-one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionLogRecord {
    pub schema_version: u8,
    pub started_at_ms: u64,
    pub tool_id: String,
    pub toolkit: String,
    pub invoker: String,
    pub surface: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    /// The [`upeg_core::PrincipalRole`] label the runtime stamped on the
    /// call (`operator` / `agent` / `local`). `None` for rows written
    /// before the column existed, and for a call whose envelope carried
    /// no principal block.
    ///
    /// A role label, never a token: the log stays metadata-only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    pub status: String,
    pub duration_ms: u64,
}

/// Read-side filter. All present fields must match (AND semantics);
/// `since_ms` is an inclusive lower bound on `started_at_ms`; `limit`
/// keeps the newest matching records (default [`DEFAULT_READ_LIMIT`]).
#[derive(Debug, Clone, Default)]
pub struct ExecutionLogFilter {
    pub tool_id: Option<String>,
    pub surface: Option<String>,
    pub status: Option<String>,
    pub trigger: Option<String>,
    pub since_ms: Option<u64>,
    pub limit: Option<usize>,
}

impl Store {
    /// Append one record. Never updates or removes existing rows.
    pub fn append_execution_record(
        &mut self,
        record: &ExecutionLogRecord,
    ) -> Result<(), StoreError> {
        let sql = format!(
            "INSERT INTO {EXECUTION_LOG_TABLE} \
               (schema_version, started_at_ms, tool_id, toolkit, invoker, surface, \
                board, \"trigger\", principal, status, duration_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
        );
        self.write_tx(|tx| {
            tx.execute(
                &sql,
                params![
                    record.schema_version,
                    clamp_to_i64(record.started_at_ms),
                    record.tool_id,
                    record.toolkit,
                    record.invoker,
                    record.surface,
                    record.board,
                    record.trigger,
                    record.principal,
                    record.status,
                    clamp_to_i64(record.duration_ms),
                ],
            )?;
            Ok(())
        })
    }

    /// Read matching records in append order (oldest of the kept window
    /// first), keeping only the newest `filter.limit` matches — the same
    /// window semantics the JSONL predecessor had.
    pub fn read_execution_records(
        &self,
        filter: &ExecutionLogFilter,
    ) -> Result<Vec<ExecutionLogRecord>, StoreError> {
        let mut clauses: Vec<String> = Vec::new();
        let mut params: Vec<rusqlite::types::Value> = Vec::new();
        let equality_columns: [(&str, Option<&String>); 4] = [
            ("tool_id", filter.tool_id.as_ref()),
            ("surface", filter.surface.as_ref()),
            ("status", filter.status.as_ref()),
            ("\"trigger\"", filter.trigger.as_ref()),
        ];
        for (column, value) in equality_columns {
            if let Some(value) = value {
                clauses.push(format!("{column} = ?{}", clauses.len() + 1));
                params.push(value.clone().into());
            }
        }
        if let Some(since_ms) = filter.since_ms {
            clauses.push(format!("started_at_ms >= ?{}", clauses.len() + 1));
            params.push(clamp_to_i64(since_ms).into());
        }
        let where_sql = if clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {} ", clauses.join(" AND "))
        };
        let limit = filter.limit.unwrap_or(DEFAULT_READ_LIMIT);
        params.push(clamp_to_i64(u64::try_from(limit).unwrap_or(u64::MAX)).into());

        // Newest `limit` rows selected by descending insertion id, then
        // reversed so callers see append order.
        let sql = format!(
            "SELECT schema_version, started_at_ms, tool_id, toolkit, invoker, surface, \
                    board, \"trigger\", principal, status, duration_ms \
             FROM {EXECUTION_LOG_TABLE} {where_sql}\
             ORDER BY id DESC LIMIT ?{}",
            clauses.len() + 1
        );
        let mut stmt = self.connection().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            Ok(ExecutionLogRecord {
                schema_version: row.get(0)?,
                started_at_ms: row.get(1)?,
                tool_id: row.get(2)?,
                toolkit: row.get(3)?,
                invoker: row.get(4)?,
                surface: row.get(5)?,
                board: row.get(6)?,
                trigger: row.get(7)?,
                principal: row.get(8)?,
                status: row.get(9)?,
                duration_ms: row.get(10)?,
            })
        })?;
        let mut records = rows.collect::<Result<Vec<_>, _>>()?;
        records.reverse();
        Ok(records)
    }

    /// Recent-tool search signals: unique `tool_id`s ordered newest first
    /// (by `started_at_ms`, insertion order breaking ties), ranked from 0.
    pub fn recent_execution_search_signals(&self) -> Result<SearchSignals, StoreError> {
        let sql = format!(
            "SELECT tool_id FROM ( \
               SELECT tool_id, started_at_ms, id, \
                      ROW_NUMBER() OVER ( \
                        PARTITION BY tool_id \
                        ORDER BY started_at_ms DESC, id DESC \
                      ) AS newest_rank \
               FROM {EXECUTION_LOG_TABLE} \
             ) WHERE newest_rank = 1 \
             ORDER BY started_at_ms DESC, id DESC"
        );
        let mut stmt = self.connection().prepare(&sql)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut recent = Vec::new();
        for tool_id in rows {
            let rank = recent.len();
            recent.push(RecentSignal {
                tool_id: tool_id?,
                rank,
            });
        }
        Ok(SearchSignals {
            pinned: Vec::new(),
            recent,
        })
    }
}

/// SQLite INTEGER is `i64`; clamp instead of erroring on absurd values.
fn clamp_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
