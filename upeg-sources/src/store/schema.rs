//! Versioned schema for the shared state store.
//!
//! Migrations are append-only: each entry in [`MIGRATIONS`] moves the
//! database from `PRAGMA user_version == index` to `index + 1`, inside its
//! own transaction. Later steps (e.g. last outcomes) extend the list —
//! existing entries never change.

use rusqlite::Connection;

use super::StoreError;

pub(crate) const META_TABLE: &str = "meta";
pub(crate) const BOARDS_TABLE: &str = "boards";
pub(crate) const PLACEMENTS_TABLE: &str = "placements";
pub(crate) const SELECTION_TABLE: &str = "selection";
pub(crate) const MEMOS_TABLE: &str = "memos";
pub(crate) const EXECUTION_LOG_TABLE: &str = "execution_log";
pub(crate) const LAST_OUTCOMES_TABLE: &str = "last_outcomes";

/// The selection table holds exactly one row, pinned to this id.
pub(crate) const SELECTION_ROW_ID: i64 = 1;

const PRAGMA_USER_VERSION: &str = "user_version";

/// v1 — pegboard state plus the `meta` key-value table (change rev,
/// device id). Every user-data table carries `updated_at` (epoch millis) /
/// `device_id` provenance and, where rows can be removed, a `deleted`
/// tombstone flag so a future sync step can merge instead of clobber.
///
/// The literal span bounds mirror `upeg_core::BOARD_COLS` and the
/// one-cell minimum of `PinSpan`; a test in `store::tests` keeps the SQL
/// and the Rust constants in lockstep.
const MIGRATION_V1: &str = "\
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE boards (
    key        TEXT PRIMARY KEY,
    title      TEXT NOT NULL,
    position   INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    device_id  TEXT NOT NULL,
    deleted    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE placements (
    board_key   TEXT NOT NULL REFERENCES boards(key),
    tool_id     TEXT NOT NULL,
    x           INTEGER NOT NULL,
    y           INTEGER NOT NULL,
    color       TEXT,
    span_cols   INTEGER CHECK (span_cols BETWEEN 1 AND 6),
    span_rows   INTEGER CHECK (span_rows >= 1),
    args_preset TEXT,
    updated_at  INTEGER NOT NULL,
    device_id   TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (board_key, tool_id),
    CHECK ((span_cols IS NULL) = (span_rows IS NULL))
);
CREATE TABLE selection (
    id         INTEGER PRIMARY KEY CHECK (id = 1),
    board_key  TEXT,
    tag        TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    device_id  TEXT NOT NULL
);
";

/// v2 — memos and the execution log move off their JSON/JSONL files into
/// the shared store.
///
/// `memos` follows the v1 user-data convention (`updated_at` / `device_id`
/// provenance plus a `deleted` tombstone) so a future sync step can merge.
/// `execution_log` is append-only history, so rows are never removed and
/// carry no tombstone; the two indexes back "recent, newest first" reads
/// and per-tool history filters. The log stores dispatch **metadata only**
/// — never Tool arguments, outputs, error messages, or credential values
/// (see `upeg-cli::adapters::execution_log`) — so the column set is closed
/// by design. `trigger` is quoted because it is an SQLite keyword.
const MIGRATION_V2: &str = "\
CREATE TABLE memos (
    key        TEXT PRIMARY KEY,
    body       TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    device_id  TEXT NOT NULL,
    deleted    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE execution_log (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    schema_version INTEGER NOT NULL,
    started_at_ms  INTEGER NOT NULL,
    tool_id        TEXT NOT NULL,
    toolkit        TEXT NOT NULL,
    invoker        TEXT NOT NULL,
    surface        TEXT NOT NULL,
    board          TEXT,
    \"trigger\"     TEXT,
    status         TEXT NOT NULL,
    duration_ms    INTEGER NOT NULL
);
CREATE INDEX execution_log_started_at_ms
    ON execution_log (started_at_ms);
CREATE INDEX execution_log_tool_id_started_at_ms
    ON execution_log (tool_id, started_at_ms);
";

/// v3 — per-pin last dispatch outcomes move off the in-memory Dart cache
/// into the shared store so a restart (or another surface) can restore
/// the most recent result of every pinned tool.
///
/// The key is per placement (`board_key`, `tool_id`) — the same tool
/// pinned on two boards keeps two independent outcomes. Rows are a
/// device-local cache, not sync-merged user data: an unpin deletes the
/// row outright (no tombstone). `outputs_json` is capped before insert
/// (see `store::last_outcomes::OUTPUTS_JSON_MAX_BYTES`); `truncated`
/// records that the cap fired so the UI can render an ellipsis.
const MIGRATION_V3: &str = "\
CREATE TABLE last_outcomes (
    board_key         TEXT NOT NULL,
    tool_id           TEXT NOT NULL,
    ok                INTEGER NOT NULL,
    primary_output_id TEXT,
    outputs_json      TEXT NOT NULL,
    truncated         INTEGER NOT NULL DEFAULT 0,
    error_code        TEXT,
    error_message     TEXT,
    updated_at        INTEGER NOT NULL,
    device_id         TEXT NOT NULL,
    PRIMARY KEY (board_key, tool_id)
);
";

/// v4 — the execution log records *who* ran a tool, not only which
/// surface the call arrived on.
///
/// `surface` answers "which door"; `principal` answers "with what
/// authority" — the [`upeg_core::PrincipalRole`] label the runtime
/// stamped into `_upeg.principal` (`operator` / `agent` / `local`). The
/// two are independent: an operator and an agent can both arrive on
/// `http`, and only one of them could have approved a gated Chain step.
///
/// Nullable and added by `ALTER TABLE` because the log is append-only
/// history: rows written before this column existed carry no principal
/// and must not be invented one. Still metadata-only — a role label is
/// not a credential, and no token value ever reaches this table.
const MIGRATION_V4: &str = "\
ALTER TABLE execution_log ADD COLUMN principal TEXT;
";

/// v5 — personal board guidance shares the board row's write provenance.
/// Project guidance stays in its manifest and uses empty stored values.
const MIGRATION_V5: &str = "\
ALTER TABLE boards ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE boards ADD COLUMN instructions TEXT NOT NULL DEFAULT '';
";

/// Append-only migration list. `MIGRATIONS.len()` is the current schema
/// version as recorded in `PRAGMA user_version`.
pub(crate) const MIGRATIONS: &[&str] = &[
    MIGRATION_V1,
    MIGRATION_V2,
    MIGRATION_V3,
    MIGRATION_V4,
    MIGRATION_V5,
];

/// Bring `conn` up to the latest schema version. Idempotent: already-
/// applied migrations (tracked via `PRAGMA user_version`) are skipped.
pub(crate) fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let applied = user_version(conn)?;
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(migration)?;
        tx.pragma_update(None, PRAGMA_USER_VERSION, index + 1)?;
        tx.commit()?;
    }
    Ok(())
}

/// Applied schema version (`PRAGMA user_version`), 0 for a fresh database.
pub(crate) fn user_version(conn: &Connection) -> Result<usize, StoreError> {
    let version: i64 = conn.query_row(&format!("PRAGMA {PRAGMA_USER_VERSION}"), [], |row| {
        row.get(0)
    })?;
    Ok(usize::try_from(version).unwrap_or(0))
}
