//! Metadata-only Execution Log.
//!
//! The log is intentionally narrower than a generic audit trail: it records
//! which Tool ran, from which surface/Board/Trigger, how it ended, and how
//! long it took. It never persists Tool arguments, outputs, error messages, or
//! credential values. This keeps the Execution Log useful for
//! history/debugging without turning upeg into a secret store.
//!
//! Storage is the shared SQLite store (`upeg-sources::store`,
//! `execution_log` table) — this adapter owns the dispatch→record mapping
//! and the CLI/HTTP presentation; row I/O lives with the store. Concurrent
//! appends from the CLI and the HTTP daemon are absorbed by the store's
//! WAL mode + busy timeout.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use upeg_core::{
    EXECUTION_CONTEXT_ARG, EXECUTION_CONTEXT_PRINCIPAL, PRINCIPAL_ROLE_KEY, SearchSignals,
};
use upeg_runtime::toolbox_tool;
use upeg_sources::store::Store;

use crate::domain::execution::dispatch::Outcome;

pub use upeg_sources::store::{ExecutionLogFilter as LogFilter, ExecutionLogRecord};

#[path = "execution_log_time.rs"]
mod execution_log_time;
use execution_log_time::{epoch_ms_to_rfc3339_local, local_utc_offset_seconds_at};

const SCHEMA_VERSION: u8 = 1;

/// Database path the log lives in: `$UPEG_LOG_PATH` (a standalone store
/// database file) or the shared per-user store
/// (`<config_root>/upeg.db`).
pub fn default_log_path() -> Option<PathBuf> {
    crate::infrastructure::paths::execution_log_path()
}

pub fn record_dispatch(id: &str, args: &Value, outcome: &Outcome, duration: Duration) {
    let Some(path) = default_log_path() else {
        return;
    };
    let record = record_from_dispatch(id, args, outcome, duration);
    let _ = append_record_to_path(&path, &record);
}

pub fn record_from_dispatch(
    id: &str,
    args: &Value,
    outcome: &Outcome,
    duration: Duration,
) -> ExecutionLogRecord {
    let meta = toolbox_tool(id);
    let toolkit = meta
        .map(|tool| tool.toolkit_id().to_string())
        .unwrap_or_default();
    let invoker = meta.map_or_else(
        || "unknown".to_string(),
        |tool| tool.invoker.label().to_string(),
    );
    let context = execution_context(args);
    ExecutionLogRecord {
        schema_version: SCHEMA_VERSION,
        started_at_ms: now_ms(),
        tool_id: id.to_string(),
        toolkit,
        invoker,
        surface: context
            .and_then(|ctx| ctx.get("surface"))
            .and_then(Value::as_str)
            .map_or_else(|| "cli".to_string(), str::to_string),
        board: context
            .and_then(|ctx| ctx.get("board"))
            .and_then(Value::as_str)
            .map(str::to_string),
        trigger: context
            .and_then(|ctx| ctx.get("trigger"))
            .and_then(Value::as_str)
            .map(str::to_string),
        principal: context
            .and_then(|ctx| ctx.get(EXECUTION_CONTEXT_PRINCIPAL))
            .and_then(|principal| principal.get(PRINCIPAL_ROLE_KEY))
            .and_then(Value::as_str)
            .map(str::to_string),
        status: match outcome {
            Outcome::Success(_) => "ok",
            Outcome::Failure(_) => "tool_error",
            Outcome::NotFound => "not_found",
        }
        .to_string(),
        duration_ms: duration.as_millis().min(u128::from(u64::MAX)) as u64,
    }
}

/// Append one record to the store database at `path` (created, including
/// its schema, when absent).
pub fn append_record_to_path(path: &Path, record: &ExecutionLogRecord) -> std::io::Result<()> {
    let mut store = Store::open_at(path).map_err(std::io::Error::other)?;
    store
        .append_execution_record(record)
        .map_err(std::io::Error::other)
}

pub fn read_records(filter: &LogFilter) -> std::io::Result<Vec<ExecutionLogRecord>> {
    let Some(path) = default_log_path() else {
        return Ok(Vec::new());
    };
    read_records_from_path(&path, filter)
}

/// Read from the store database at `path`. A missing database means "no
/// history yet" — it is reported empty, not created as a side effect.
pub fn read_records_from_path(
    path: &Path,
    filter: &LogFilter,
) -> std::io::Result<Vec<ExecutionLogRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let store = Store::open_at(path).map_err(std::io::Error::other)?;
    store
        .read_execution_records(filter)
        .map_err(std::io::Error::other)
}

pub(crate) fn recent_search_signals() -> SearchSignals {
    let Some(path) = default_log_path() else {
        return SearchSignals::default();
    };
    recent_search_signals_from_path(&path)
}

/// Recent-tool signals from the store database at `path`; a missing or
/// unreadable database yields the empty default (and is never created
/// as a side effect).
pub(crate) fn recent_search_signals_from_path(path: &Path) -> SearchSignals {
    if !path.exists() {
        return SearchSignals::default();
    }
    let Ok(store) = Store::open_at(path) else {
        return SearchSignals::default();
    };
    store.recent_execution_search_signals().unwrap_or_default()
}

pub fn format_records(records: &[ExecutionLogRecord], json: bool) -> String {
    if json {
        let mut out = serde_json::to_string_pretty(records).unwrap_or_else(|_| "[]".into());
        out.push('\n');
        return out;
    }
    format_records_human(records, local_utc_offset_seconds_at)
}

/// The human (non-JSON) half of [`format_records`], with the UTC-offset
/// source injected: given a record's instant (Unix epoch ms), it answers
/// the offset that row renders in.
///
/// The offset is resolved *per record* rather than hoisted out of the
/// loop. A log can span a DST transition, and the rows before it did not
/// run in the offset that is in effect today — hoisting would stamp every
/// row with the current one and shift the older half by an hour.
/// Production passes [`local_utc_offset_seconds_at`]; tests pass a fixed
/// closure so the rendering stays checkable without touching `TZ`.
fn format_records_human(
    records: &[ExecutionLogRecord],
    utc_offset_seconds_at: impl Fn(i64) -> i32,
) -> String {
    let mut out = String::new();
    for record in records {
        let started_at_ms = i64::try_from(record.started_at_ms).unwrap_or(i64::MAX);
        let started_at =
            epoch_ms_to_rfc3339_local(started_at_ms, utc_offset_seconds_at(started_at_ms));
        out.push_str(&format!(
            "{started_at}\t{}\t{}\t{}\t{}ms",
            record.surface, record.tool_id, record.status, record.duration_ms
        ));
        if let Some(board) = &record.board {
            out.push_str(&format!("\tboard={board}"));
        }
        if let Some(trigger) = &record.trigger {
            out.push_str(&format!("\ttrigger={trigger}"));
        }
        // Same `key=value` shape as `board=` / `trigger=`, and omitted
        // the same way when absent: a row from before principals existed
        // says nothing rather than guessing a role for it.
        if let Some(principal) = &record.principal {
            out.push_str(&format!("\tprincipal={principal}"));
        }
        out.push('\n');
    }
    out
}

fn execution_context(args: &Value) -> Option<&serde_json::Map<String, Value>> {
    args.get(EXECUTION_CONTEXT_ARG).and_then(Value::as_object)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use upeg_core::RecentSignal;

    /// 2026-11-01T06:00:00Z — when US Eastern exits DST (02:00 EDT local).
    const DST_TRANSITION_MS: i64 = 1_793_512_800_000;
    /// 30 minutes before the transition: 2026-11-01T05:30:00Z, 01:30 EDT local.
    const BEFORE_DST_TRANSITION_MS: u64 = 1_793_511_000_000;
    /// 30 minutes after the transition: 2026-11-01T06:30:00Z, (rewound) 01:30 EST local.
    const AFTER_DST_TRANSITION_MS: u64 = 1_793_514_600_000;
    /// Daylight time (EDT) offset.
    const SUMMER_OFFSET_SECONDS: i32 = -4 * 3_600;
    /// Standard time (EST) offset.
    const WINTER_OFFSET_SECONDS: i32 = -5 * 3_600;

    fn test_record(tool_id: &str, started_at_ms: u64) -> ExecutionLogRecord {
        ExecutionLogRecord {
            schema_version: 1,
            started_at_ms,
            tool_id: tool_id.to_string(),
            toolkit: String::new(),
            invoker: "function".to_string(),
            surface: "cli".to_string(),
            board: None,
            trigger: None,
            principal: None,
            status: "ok".to_string(),
            duration_ms: 1,
        }
    }

    fn temp_log_db_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "upeg_{label}_{}_{}.db",
            std::process::id(),
            now_ms()
        ))
    }

    #[test]
    fn record_from_dispatch_keeps_metadata_only() {
        let record = record_from_dispatch(
            "missing.tool",
            &json!({
                "input": "SECRET",
                "_upeg": {
                    "surface": "http",
                    "board": "dev",
                    "trigger": "webhook",
                    "principal": { "role": "agent", "surface": "http" }
                }
            }),
            &Outcome::Failure(crate::domain::execution::dispatch::dispatch_failure(
                "tool_error",
                "SECRET in error",
            )),
            Duration::from_millis(7),
        );
        let encoded = serde_json::to_string(&record).unwrap();
        assert!(!encoded.contains("SECRET"));
        assert_eq!(record.surface, "http");
        assert_eq!(record.board.as_deref(), Some("dev"));
        assert_eq!(record.trigger.as_deref(), Some("webhook"));
        assert_eq!(
            record.principal.as_deref(),
            Some("agent"),
            "the stamped principal's role must be recorded — never the token"
        );
        assert_eq!(record.status, "tool_error");
    }

    #[test]
    fn record_without_principal_does_not_fabricate_role() {
        let record = record_from_dispatch(
            "missing.tool",
            &json!({ "_upeg": { "surface": "cli" } }),
            &Outcome::NotFound,
            Duration::from_millis(1),
        );

        assert_eq!(record.principal, None);
        let encoded = serde_json::to_string(&record).unwrap();
        assert!(
            !encoded.contains("principal"),
            "an absent principal is omitted from the wire: {encoded}"
        );
    }

    #[test]
    fn principal_stamped_by_execution_context_is_recorded_verbatim() {
        // Run the whole pipeline at once: the surface stamps the principal
        // while applying the context, and the log reads only that role.
        // The token appears nowhere.
        let args = upeg_runtime::apply_execution_context(
            &upeg_runtime::RegistryProjectContext,
            json!({ "input": "SECRET" }),
            &upeg_runtime::ExecutionContext::global(upeg_core::Surface::Http).with_principal(
                upeg_core::Principal::new(
                    upeg_core::PrincipalRole::Agent,
                    upeg_core::Surface::Http,
                ),
            ),
            None,
        );

        let record = record_from_dispatch(
            "missing.tool",
            &args,
            &Outcome::NotFound,
            Duration::from_millis(1),
        );

        assert_eq!(record.principal.as_deref(), Some("agent"));
        assert_eq!(record.surface, "http");
        assert!(!serde_json::to_string(&record).unwrap().contains("SECRET"));
    }

    #[test]
    fn human_output_shows_principal_role() {
        let mut record = test_record("principal.render", 1_786_957_503_000);
        record.principal = Some("operator".to_string());

        let out = format_records(&[record], false);

        assert!(out.contains("\tprincipal=operator"), "got:\n{out}");
    }

    #[test]
    fn store_records_are_read_with_filters_and_limits() {
        let path = temp_log_db_path("execution_log_filter");
        let _ = std::fs::remove_file(&path);
        append_record_to_path(&path, &test_record("a.one", 1)).unwrap();
        let mut http_record = test_record("b.two", 2);
        http_record.toolkit = "b".into();
        http_record.invoker = "http".into();
        http_record.surface = "http".into();
        http_record.trigger = Some("hotkey".into());
        http_record.status = "tool_error".into();
        append_record_to_path(&path, &http_record).unwrap();

        let got = read_records_from_path(
            &path,
            &LogFilter {
                surface: Some("http".into()),
                trigger: Some("hotkey".into()),
                since_ms: Some(2),
                ..LogFilter::default()
            },
        )
        .unwrap();
        assert_eq!(got, vec![http_record]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn execution_log_recent_signal_is_newest_first_unique() {
        let path = temp_log_db_path("execution_log_recent");
        let _ = std::fs::remove_file(&path);
        for record in [
            test_record("alpha.one", 10),
            test_record("beta.two", 30),
            test_record("alpha.one", 50),
            test_record("gamma.three", 40),
        ] {
            append_record_to_path(&path, &record).unwrap();
        }

        let signals = recent_search_signals_from_path(&path);

        assert!(signals.pinned.is_empty());
        assert_eq!(
            signals.recent,
            vec![
                RecentSignal {
                    tool_id: "alpha.one".to_string(),
                    rank: 0,
                },
                RecentSignal {
                    tool_id: "gamma.three".to_string(),
                    rank: 1,
                },
                RecentSignal {
                    tool_id: "beta.two".to_string(),
                    rank: 2,
                },
            ]
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_and_unreadable_log_returns_empty_recent_signals() {
        let missing_path = temp_log_db_path("execution_log_missing_recent");
        let _ = std::fs::remove_file(&missing_path);
        let _ = std::fs::remove_dir_all(&missing_path);

        assert_eq!(
            recent_search_signals_from_path(&missing_path),
            SearchSignals::default()
        );
        assert!(
            !missing_path.exists(),
            "recent-signal lookup must not create the database"
        );

        let unreadable_path = temp_log_db_path("execution_log_unreadable_recent");
        let _ = std::fs::remove_file(&unreadable_path);
        let _ = std::fs::remove_dir_all(&unreadable_path);
        std::fs::create_dir(&unreadable_path).unwrap();

        assert_eq!(
            recent_search_signals_from_path(&unreadable_path),
            SearchSignals::default()
        );
        let _ = std::fs::remove_dir(&unreadable_path);
    }

    #[test]
    fn json_output_preserves_raw_epoch_ms_values() {
        // 2026-08-17T09:05:03Z
        let record = test_record("time.check", 1_786_957_503_000);
        let out = format_records(&[record], true);
        assert!(
            out.contains("1786957503000"),
            "json output must keep the raw epoch-ms number; got:\n{out}"
        );
        assert!(
            !out.contains("2026-08-17T09:05:03"),
            "json output must not be rewritten to RFC3339; got:\n{out}"
        );
    }

    #[test]
    fn human_output_renders_local_rfc3339_timestamp_instead_of_epoch_ms() {
        // 2026-08-17T09:05:03Z
        let record = test_record("time.check", 1_786_957_503_000);
        let out = format_records(&[record], false);
        let timestamp = out.split('\t').next().expect("timestamp column");
        let parsed = chrono::DateTime::parse_from_rfc3339(timestamp)
            .expect("human output must lead with an RFC3339 local timestamp");
        assert_eq!(parsed.timestamp_millis(), 1_786_957_503_000);
        assert!(
            !out.contains("1786957503000"),
            "human output must not print the raw epoch-ms number; got:\n{out}"
        );
    }

    #[test]
    fn records_spanning_dst_transition_render_with_each_instants_offset() {
        let out = format_records_human(
            &[
                test_record("dst.before", BEFORE_DST_TRANSITION_MS),
                test_record("dst.after", AFTER_DST_TRANSITION_MS),
            ],
            |ms| {
                if ms < DST_TRANSITION_MS {
                    SUMMER_OFFSET_SECONDS
                } else {
                    WINTER_OFFSET_SECONDS
                }
            },
        );

        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2, "got:\n{out}");
        assert!(
            lines[0].starts_with("2026-11-01T01:30:00-04:00\t"),
            "the pre-transition row must render with the summer offset actually in effect at that instant; got:\n{out}"
        );
        assert!(
            lines[1].starts_with("2026-11-01T01:30:00-05:00\t"),
            "the post-transition row must render with its own instant's winter offset, not inherit the first row's; got:\n{out}"
        );
    }
}
