use std::collections::BTreeMap;
use std::path::PathBuf;

use upeg_core::{ArgsPreset, BOARD_COLS, ColSpan, PinColorHex, PinSpan, Placement, RowSpan};

use super::{Store, schema};
use crate::pegboard::{BoardData, PegboardSelection, PegboardState};
use crate::store::export;

fn temp_store_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "upeg-store-{label}-{}-{:?}",
        std::process::id(),
        std::thread::current().id(),
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn store_db_path(dir: &std::path::Path) -> PathBuf {
    dir.join(upeg_core::paths::STORE_FILE)
}

fn board(key: &str, title: &str) -> BoardData {
    BoardData {
        key: key.into(),
        title: title.into(),
        guidance: upeg_core::BoardGuidance::default(),
    }
}

fn one_board_state(placements: Vec<Placement>) -> PegboardState {
    PegboardState {
        boards: vec![board("dev", "Dev")],
        layouts: BTreeMap::from([("dev".to_string(), placements)]),
        selection: PegboardSelection::default(),
    }
}

#[test]
fn user_version_runner_applies_migrations_and_is_idempotent_on_rerun() {
    let store = Store::open_in_memory().expect("in-memory store");
    let version = schema::user_version(store.connection()).expect("read user_version");
    assert_eq!(
        version,
        schema::MIGRATIONS.len(),
        "a fresh store must have a user_version with all migrations applied"
    );

    // File-based reopen: already-applied migrations must be skipped.
    let dir = temp_store_dir("migrate-idempotent");
    let path = store_db_path(&dir);
    drop(Store::open_at(&path).expect("first open"));
    let reopened = Store::open_at(&path).expect("reopen must not re-run migrations");
    assert_eq!(
        schema::user_version(reopened.connection()).expect("reopened user_version"),
        schema::MIGRATIONS.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn schema_span_check_matches_core_constant() {
    // MIGRATIONS is a const string so BOARD_COLS cannot be interpolated
    // directly. This test ties the SQL literal to the Rust constant.
    let v1 = schema::MIGRATIONS[0];
    assert!(
        v1.contains(&format!("span_cols BETWEEN 1 AND {BOARD_COLS}")),
        "placements.span_cols CHECK must equal BOARD_COLS ({BOARD_COLS})"
    );
    assert!(v1.contains("span_rows >= 1"));
    for table in [
        schema::META_TABLE,
        schema::BOARDS_TABLE,
        schema::PLACEMENTS_TABLE,
        schema::SELECTION_TABLE,
    ] {
        assert!(
            v1.contains(&format!("CREATE TABLE {table} ")),
            "v1 schema must contain the {table} table"
        );
    }
}

#[test]
fn schema_rejects_rows_violating_span_check() {
    let store = Store::open_in_memory().expect("in-memory store");
    let insert = format!(
        "INSERT INTO {boards} (key, title, position, updated_at, device_id) \
         VALUES ('dev', 'Dev', 0, 0, 'test')",
        boards = schema::BOARDS_TABLE,
    );
    store
        .connection()
        .execute(&insert, [])
        .expect("insert board");
    let bad_span = format!(
        "INSERT INTO {placements} \
           (board_key, tool_id, x, y, span_cols, span_rows, updated_at, device_id) \
         VALUES ('dev', 't', 0, 0, {over}, 1, 0, 'test')",
        placements = schema::PLACEMENTS_TABLE,
        over = BOARD_COLS + 1,
    );
    assert!(
        store.connection().execute(&bad_span, []).is_err(),
        "span_cols beyond BOARD_COLS must be rejected by CHECK"
    );
}

#[test]
fn change_rev_is_none_before_writes_and_increments_per_write() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    assert_eq!(store.change_rev().expect("read rev"), None);

    store
        .save_state_global(&one_board_state(vec![Placement::new("a", 0, 0)]))
        .expect("first save");
    assert_eq!(store.change_rev().expect("read rev"), Some(1));

    store
        .save_selection(&PegboardSelection::default())
        .expect("save selection");
    assert_eq!(store.change_rev().expect("read rev"), Some(2));
}

#[test]
fn device_id_is_created_on_first_open_and_kept_on_reopen() {
    let dir = temp_store_dir("device-id");
    let path = store_db_path(&dir);
    let first = Store::open_at(&path)
        .expect("first open")
        .device_id()
        .expect("create device_id");
    assert!(
        uuid::Uuid::parse_str(&first).is_ok(),
        "device_id must be a UUID: {first}"
    );
    let second = Store::open_at(&path)
        .expect("reopen")
        .device_id()
        .expect("device_id persists");
    assert_eq!(first, second);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_at_isolates_state_per_path() {
    let dir_a = temp_store_dir("isolation-a");
    let dir_b = temp_store_dir("isolation-b");
    let mut store_a = Store::open_at(&store_db_path(&dir_a)).expect("store A");
    let store_b = Store::open_at(&store_db_path(&dir_b)).expect("store B");

    store_a
        .save_state_global(&one_board_state(vec![Placement::new("a", 0, 0)]))
        .expect("save A");

    assert_eq!(store_a.change_rev().expect("A rev"), Some(1));
    assert_eq!(store_b.change_rev().expect("B rev"), None);
    assert!(
        store_b
            .load_state_global()
            .expect("load B")
            .boards
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}

#[test]
fn unpinned_placement_is_tombstoned_and_filtered_on_load() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&one_board_state(vec![
            Placement::new("keep.tool", 0, 0),
            Placement::new("drop.tool", 1, 0),
        ]))
        .expect("save two placements");

    store
        .save_state_global(&one_board_state(vec![Placement::new("keep.tool", 0, 0)]))
        .expect("save keeping one");

    let loaded = store.load_state_global().expect("load");
    let dev = loaded.layouts.get("dev").expect("dev layout");
    assert_eq!(
        dev.iter().map(|p| p.tool_id.as_str()).collect::<Vec<_>>(),
        vec!["keep.tool"],
        "a tombstoned placement must not appear on load"
    );

    // A tombstone is not a delete: the row itself must remain (for
    // future sync).
    let count: i64 = store
        .connection()
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM {placements}",
                placements = schema::PLACEMENTS_TABLE
            ),
            [],
            |row| row.get(0),
        )
        .expect("count rows");
    assert_eq!(count, 2, "tombstone rows must physically remain");
}

#[test]
fn deleted_board_is_tombstoned_and_filtered_on_load() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&PegboardState {
            boards: vec![board("dev", "Dev"), board("lab", "Lab")],
            layouts: BTreeMap::from([
                ("dev".to_string(), vec![Placement::new("a", 0, 0)]),
                ("lab".to_string(), vec![Placement::new("b", 0, 0)]),
            ]),
            selection: PegboardSelection::default(),
        })
        .expect("save two boards");

    store
        .save_state_global(&one_board_state(vec![Placement::new("a", 0, 0)]))
        .expect("save removing lab");

    let loaded = store.load_state_global().expect("load");
    assert_eq!(loaded.boards, vec![board("dev", "Dev")]);
    assert!(
        !loaded.layouts.contains_key("lab"),
        "a tombstoned board's layout must not load"
    );
}

#[test]
fn empty_layout_round_trips_empty_without_default_revival() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&one_board_state(vec![Placement::new("a", 0, 0)]))
        .expect("save");
    store
        .save_state_global(&one_board_state(Vec::new()))
        .expect("save all unpinned");

    let loaded = store.load_state_global().expect("load");
    assert_eq!(
        loaded.layouts.get("dev").map(Vec::len),
        Some(0),
        "an empty layout must load as an explicit empty entry"
    );
}

#[test]
fn span_args_preset_and_color_round_trip_through_store() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let placement = Placement::new("styled.tool", 2, 1)
        .with_color(Some(PinColorHex::parse("#12abef").expect("test color")))
        .with_span(Some(PinSpan::new(
            ColSpan::new(3).expect("test cols"),
            RowSpan::new(2).expect("test rows"),
        )))
        .with_args_preset(Some(
            ArgsPreset::parse(r#"{"city":"Seoul","days":3}"#).expect("test preset"),
        ));
    store
        .save_state_global(&one_board_state(vec![placement.clone()]))
        .expect("save");

    let loaded = store.load_state_global().expect("load");
    assert_eq!(
        loaded.layouts.get("dev").map(Vec::as_slice),
        Some(std::slice::from_ref(&placement)),
        "color/span/args_preset must round-trip losslessly"
    );
}

#[test]
fn selection_round_trips_as_single_row() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let mut state = one_board_state(vec![Placement::new("a", 0, 0)]);
    state.selection = PegboardSelection {
        board_key: Some("dev".into()),
        tag: "pure".into(),
    };
    store.save_state_global(&state).expect("save");

    assert_eq!(
        store.load_state_global().expect("load").selection,
        state.selection
    );

    store
        .save_selection(&PegboardSelection::default())
        .expect("replace selection");
    assert_eq!(
        store.load_state_global().expect("reload").selection,
        PegboardSelection::default()
    );
}

#[test]
fn row_level_upsert_and_tombstone_are_reflected_in_load_state() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&one_board_state(Vec::new()))
        .expect("prepare board");

    store
        .upsert_placement("dev", &Placement::new("row.tool", 4, 2))
        .expect("row-level upsert");
    let loaded = store.load_state_global().expect("load");
    assert_eq!(
        loaded.layouts.get("dev").and_then(|l| l.first()),
        Some(&Placement::new("row.tool", 4, 2))
    );

    store
        .tombstone_placement("dev", "row.tool")
        .expect("row-level tombstone");
    assert_eq!(
        store
            .load_state_global()
            .expect("reload")
            .layouts
            .get("dev")
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn in_wal_mode_concurrent_writes_from_two_connections_all_survive() {
    const WRITES_PER_CONNECTION: u64 = 5;

    let dir = temp_store_dir("wal-concurrency");
    let path = store_db_path(&dir);
    // Finish schema/device setup first so both threads only race on
    // writes.
    drop(Store::open_at(&path).expect("prepare schema"));

    let writer = |tool_prefix: &'static str, path: PathBuf| {
        std::thread::spawn(move || {
            let mut store = Store::open_at(&path).expect("thread store open");
            for index in 0..WRITES_PER_CONNECTION {
                store
                    .upsert_placement_bootstrap(tool_prefix, index)
                    .expect("concurrent write");
            }
        })
    };
    let a = writer("alpha", path.clone());
    let b = writer("beta", path.clone());
    a.join().expect("alpha thread");
    b.join().expect("beta thread");

    let store = Store::open_at(&path).expect("verification open");
    assert_eq!(
        store.change_rev().expect("rev"),
        Some(WRITES_PER_CONNECTION * 2),
        "no write may be lost under busy_timeout"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

impl Store {
    /// WAL-concurrency test only: bundles a board upsert + placement
    /// upsert in one transaction to create real write contention.
    fn upsert_placement_bootstrap(
        &mut self,
        prefix: &'static str,
        index: u64,
    ) -> Result<(), super::StoreError> {
        let device_id = self.device_id()?;
        let now = super::epoch_millis_now();
        self.write_tx(|tx| {
            let board_sql = format!(
                "INSERT OR IGNORE INTO {boards} (key, title, position, updated_at, device_id) \
                 VALUES (?1, ?1, 0, ?2, ?3)",
                boards = schema::BOARDS_TABLE,
            );
            tx.execute(&board_sql, rusqlite::params![prefix, now, device_id])?;
            let placement_sql = format!(
                "INSERT OR REPLACE INTO {placements} \
                   (board_key, tool_id, x, y, updated_at, device_id) \
                 VALUES (?1, ?2, 0, 0, ?3, ?4)",
                placements = schema::PLACEMENTS_TABLE,
            );
            tx.execute(
                &placement_sql,
                rusqlite::params![prefix, format!("{prefix}.{index}"), now, device_id],
            )?;
            Ok(())
        })
    }
}

fn log_record(tool_id: &str, started_at_ms: u64) -> super::ExecutionLogRecord {
    super::ExecutionLogRecord {
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

#[test]
fn v4_schema_adds_principal_column_to_execution_log() {
    // The history is append-only, so the column is added without
    // recreating the table — older rows must keep principal NULL, not
    // fabricated.
    let v4 = schema::MIGRATIONS[3];
    assert!(
        v4.contains(&format!(
            "ALTER TABLE {} ADD COLUMN principal TEXT",
            schema::EXECUTION_LOG_TABLE
        )),
        "v4 must add the principal column via ALTER TABLE: {v4}"
    );
}

#[test]
fn principal_is_preserved_between_write_and_read() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let mut record = log_record("principal.tool", 10);
    record.principal = Some("agent".to_string());
    store.append_execution_record(&record).expect("append");

    let read = store
        .read_execution_records(&super::ExecutionLogFilter::default())
        .expect("read");

    assert_eq!(read, vec![record]);
}

#[test]
fn record_without_principal_stays_null() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let record = log_record("principal.absent", 10);
    store.append_execution_record(&record).expect("append");

    let read = store
        .read_execution_records(&super::ExecutionLogFilter::default())
        .expect("read");

    assert_eq!(read[0].principal, None);
}

#[test]
fn v2_schema_creates_memos_and_execution_log_tables() {
    let v2 = schema::MIGRATIONS[1];
    for table in [schema::MEMOS_TABLE, schema::EXECUTION_LOG_TABLE] {
        assert!(
            v2.contains(&format!("CREATE TABLE {table} ")),
            "v2 schema must contain the {table} table"
        );
    }
    assert!(
        v2.contains(&format!(
            "ON {} (started_at_ms)",
            schema::EXECUTION_LOG_TABLE
        )),
        "v2 must have a started_at_ms index"
    );
    assert!(
        v2.contains(&format!(
            "ON {} (tool_id, started_at_ms)",
            schema::EXECUTION_LOG_TABLE
        )),
        "v2 must have a (tool_id, started_at_ms) index"
    );
}

#[test]
fn memo_tombstone_keeps_row_and_filters_on_load() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let mut memos = BTreeMap::from([
        ("keep".to_string(), "body".to_string()),
        ("drop".to_string(), "gone".to_string()),
    ]);
    store.save_memos(&memos).expect("save two memos");

    memos.remove("drop");
    store.save_memos(&memos).expect("save removing one");

    assert_eq!(store.load_memos().expect("load"), memos);
    let count: i64 = store
        .connection()
        .query_row(
            &format!("SELECT COUNT(*) FROM {}", schema::MEMOS_TABLE),
            [],
            |row| row.get(0),
        )
        .expect("count rows");
    assert_eq!(count, 2, "tombstone rows must physically remain");
}

#[test]
fn resaving_identical_memo_does_not_move_updated_at() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let memos = BTreeMap::from([("scratch".to_string(), "hello".to_string())]);
    store.save_memos(&memos).expect("first save");
    let first: i64 = store
        .connection()
        .query_row(
            &format!(
                "SELECT updated_at FROM {} WHERE key = 'scratch'",
                schema::MEMOS_TABLE
            ),
            [],
            |row| row.get(0),
        )
        .expect("read updated_at");

    std::thread::sleep(std::time::Duration::from_millis(5));
    store.save_memos(&memos).expect("resave same content");
    let second: i64 = store
        .connection()
        .query_row(
            &format!(
                "SELECT updated_at FROM {} WHERE key = 'scratch'",
                schema::MEMOS_TABLE
            ),
            [],
            |row| row.get(0),
        )
        .expect("reread updated_at");
    assert_eq!(
        first, second,
        "updated_at of an unchanged row must not move"
    );
}

#[test]
fn execution_log_reads_with_filters_and_limit_keeping_append_order() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .append_execution_record(&log_record("a.one", 1))
        .expect("append 1");
    let mut http_record = log_record("b.two", 2);
    http_record.surface = "http".to_string();
    http_record.trigger = Some("hotkey".to_string());
    http_record.status = "tool_error".to_string();
    store
        .append_execution_record(&http_record)
        .expect("append 2");
    store
        .append_execution_record(&log_record("c.three", 3))
        .expect("append 3");

    let filtered = store
        .read_execution_records(&super::ExecutionLogFilter {
            surface: Some("http".into()),
            trigger: Some("hotkey".into()),
            status: Some("tool_error".into()),
            since_ms: Some(2),
            ..super::ExecutionLogFilter::default()
        })
        .expect("filtered read");
    assert_eq!(filtered, vec![http_record]);

    let limited = store
        .read_execution_records(&super::ExecutionLogFilter {
            limit: Some(2),
            ..super::ExecutionLogFilter::default()
        })
        .expect("limited read");
    assert_eq!(
        limited
            .iter()
            .map(|record| record.tool_id.as_str())
            .collect::<Vec<_>>(),
        vec!["b.two", "c.three"],
        "limit must keep the newest rows but return them in append order"
    );
}

#[test]
fn execution_log_recent_signals_are_newest_first_unique() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    for (tool_id, ts) in [
        ("alpha.one", 10),
        ("beta.two", 30),
        ("alpha.one", 50),
        ("gamma.three", 40),
    ] {
        store
            .append_execution_record(&log_record(tool_id, ts))
            .expect("append");
    }

    let signals = store.recent_execution_search_signals().expect("signals");
    assert!(signals.pinned.is_empty());
    assert_eq!(
        signals
            .recent
            .iter()
            .map(|signal| (signal.tool_id.as_str(), signal.rank))
            .collect::<Vec<_>>(),
        vec![("alpha.one", 0), ("gamma.three", 1), ("beta.two", 2)]
    );
}

#[test]
fn execution_log_same_timestamp_puts_later_insert_first() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .append_execution_record(&log_record("first.tool", 7))
        .expect("append 1");
    store
        .append_execution_record(&log_record("second.tool", 7))
        .expect("append 2");

    let signals = store.recent_execution_search_signals().expect("signals");
    assert_eq!(
        signals
            .recent
            .iter()
            .map(|signal| signal.tool_id.as_str())
            .collect::<Vec<_>>(),
        vec!["second.tool", "first.tool"],
        "for equal started_at_ms the later insert is newest"
    );
}

#[test]
fn concurrent_execution_log_appends_from_two_connections_are_not_lost() {
    const APPENDS_PER_CONNECTION: usize = 5;

    let dir = temp_store_dir("log-append-concurrency");
    let path = store_db_path(&dir);
    // Finish schema/device setup first so both threads only race on
    // writes.
    drop(Store::open_at(&path).expect("prepare schema"));

    let writer = |tool_prefix: &'static str, path: PathBuf| {
        std::thread::spawn(move || {
            // Concurrent appends from separate connections, like CLI
            // dispatch and the HTTP daemon.
            let mut store = Store::open_at(&path).expect("thread store open");
            for index in 0..APPENDS_PER_CONNECTION {
                let started = u64::try_from(index).unwrap_or_default();
                store
                    .append_execution_record(&log_record(
                        &format!("{tool_prefix}.{index}"),
                        started,
                    ))
                    .expect("append must succeed under busy_timeout");
            }
        })
    };
    let a = writer("alpha", path.clone());
    let b = writer("beta", path.clone());
    a.join().expect("alpha thread");
    b.join().expect("beta thread");

    let store = Store::open_at(&path).expect("verification open");
    let records = store
        .read_execution_records(&super::ExecutionLogFilter::default())
        .expect("read all");
    assert_eq!(
        records.len(),
        APPENDS_PER_CONNECTION * 2,
        "no record may be lost under concurrent appends"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn canonical_export_preserves_board_guidance_and_layout_shape() {
    // Preserves guidance defaults, BTreeMap key ordering, and
    // None-field omission.
    let boards = vec![board("dev", "Dev"), board("trading", "Trading")];
    assert_eq!(
        export::boards_to_json(&boards).expect("serialize boards"),
        r#"[{"key":"dev","title":"Dev","guidance":{"description":"","instructions":""}},{"key":"trading","title":"Trading","guidance":{"description":"","instructions":""}}]"#,
    );

    let layouts = BTreeMap::from([
        ("zeta".to_string(), vec![Placement::new("plain.tool", 1, 0)]),
        (
            "alpha".to_string(),
            vec![
                Placement::new("styled.tool", 0, 0)
                    .with_color(Some(PinColorHex::parse("#AABBCC").expect("test color")))
                    .with_span(Some(PinSpan::new(
                        ColSpan::new(2).expect("test cols"),
                        RowSpan::new(1).expect("test rows"),
                    )))
                    .with_args_preset(Some(ArgsPreset::parse(r#"{"n":1}"#).expect("test preset"))),
            ],
        ),
    ]);
    let json = export::layouts_to_json(&layouts).expect("serialize layouts");
    assert_eq!(
        json,
        concat!(
            r##"{"alpha":[{"tool_id":"styled.tool","x":0,"y":0,"color":"#AABBCC","##,
            r#""span":{"cols":2,"rows":1},"args_preset":{"n":1}}],"#,
            r#""zeta":[{"tool_id":"plain.tool","x":1,"y":0}]}"#,
        ),
        "the canonical form of BTreeMap ordering + None omission must be kept"
    );

    // The import direction mirrors the same shape.
    assert_eq!(
        export::layouts_from_json(&json).expect("deserialize layouts"),
        layouts
    );
    assert_eq!(
        export::boards_from_json(&export::boards_to_json(&boards).expect("serialize"))
            .expect("deserialize boards"),
        boards
    );
}

// ─── last_outcomes (schema v3) ─────────────────────────────────────────

use crate::store::{NewLastOutcome, OUTPUTS_JSON_MAX_BYTES, cap_outputs_json};

fn outcome(tool_id: &str, ok: bool) -> NewLastOutcome {
    NewLastOutcome {
        tool_id: tool_id.into(),
        ok,
        primary_output_id: Some("value".into()),
        outputs_json: r#"[{"id":"value","kind":"string","value":{"String":{"value":"42"}}}]"#
            .into(),
        error_code: if ok { None } else { Some("E_FAIL".into()) },
        error_message: if ok { None } else { Some("boom".into()) },
    }
}

#[test]
fn v3_schema_creates_last_outcomes_table_keyed_per_placement() {
    let v3 = schema::MIGRATIONS[2];
    assert!(
        v3.contains(&format!("CREATE TABLE {} ", schema::LAST_OUTCOMES_TABLE)),
        "v3 schema must contain the last_outcomes table"
    );
    assert!(
        v3.contains("PRIMARY KEY (board_key, tool_id)"),
        "last_outcomes must be keyed per placement by (board_key, tool_id)"
    );
}

#[test]
fn outputs_json_within_size_cap_passes_unclipped() {
    let json = r#"[{"id":"value","v":1}]"#;
    let capped = cap_outputs_json(json, Some("value"));
    assert_eq!(capped.json, json);
    assert!(!capped.truncated);
}

#[test]
fn over_size_cap_keeps_only_primary_output_and_marks_truncated() {
    let filler = "x".repeat(OUTPUTS_JSON_MAX_BYTES);
    let json = format!(r#"[{{"id":"value","v":"ok"}},{{"id":"noise","v":"{filler}"}}]"#);
    let capped = cap_outputs_json(&json, Some("value"));
    assert_eq!(capped.json, r#"[{"id":"value","v":"ok"}]"#);
    assert!(capped.truncated);

    // Without a primary id the first entry becomes the preview.
    let no_primary = cap_outputs_json(&json, None);
    assert_eq!(no_primary.json, r#"[{"id":"value","v":"ok"}]"#);
    assert!(no_primary.truncated);
}

#[test]
fn caps_to_empty_array_when_even_primary_output_exceeds_cap() {
    let filler = "x".repeat(OUTPUTS_JSON_MAX_BYTES + 1);
    let json = format!(r#"[{{"id":"value","v":"{filler}"}}]"#);
    let capped = cap_outputs_json(&json, Some("value"));
    assert_eq!(capped.json, "[]");
    assert!(capped.truncated);

    // A non-array (corrupted) payload also safely falls to an empty
    // array.
    let not_array = format!(r#""{filler}""#);
    let corrupt = cap_outputs_json(&not_array, Some("value"));
    assert_eq!(corrupt.json, "[]");
    assert!(corrupt.truncated);
}

#[test]
fn last_outcome_round_trips_via_record_load_and_overwrites_same_key() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .record_last_outcome("dev", &outcome("num.hex", true))
        .expect("record success");
    store
        .record_last_outcome("dev", &outcome("net.ping", false))
        .expect("record failure");

    let loaded = store.load_last_outcomes("dev").expect("load");
    assert_eq!(loaded.len(), 2, "both rows must appear in tool_id order");
    let ping = &loaded[0];
    assert_eq!(ping.tool_id, "net.ping");
    assert!(!ping.ok);
    assert_eq!(ping.error_code.as_deref(), Some("E_FAIL"));
    assert_eq!(ping.error_message.as_deref(), Some("boom"));
    let hex = &loaded[1];
    assert_eq!(hex.tool_id, "num.hex");
    assert!(hex.ok);
    assert!(!hex.truncated);
    assert!(hex.updated_at_ms > 0);
    assert_eq!(hex.outputs_json, outcome("num.hex", true).outputs_json);

    // Recording the same (board, tool) key again overwrites.
    store
        .record_last_outcome("dev", &outcome("num.hex", false))
        .expect("overwrite");
    let reloaded = store.load_last_outcomes("dev").expect("reload");
    assert_eq!(reloaded.len(), 2);
    assert!(
        !reloaded[1].ok,
        "the latest record must replace the earlier one"
    );
}

#[test]
fn last_outcome_is_isolated_by_board_key() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .record_last_outcome("dev", &outcome("num.hex", true))
        .expect("record dev");
    store
        .record_last_outcome("trading", &outcome("num.hex", false))
        .expect("record trading");

    assert!(store.load_last_outcomes("dev").expect("load dev")[0].ok);
    assert!(!store.load_last_outcomes("trading").expect("load trading")[0].ok);
    assert!(
        store
            .load_last_outcomes("personal")
            .expect("empty board")
            .is_empty()
    );
}

#[test]
fn over_cap_record_is_stored_with_truncated_flag() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let filler = "x".repeat(OUTPUTS_JSON_MAX_BYTES);
    let big = NewLastOutcome {
        outputs_json: format!(r#"[{{"id":"value","v":"ok"}},{{"id":"noise","v":"{filler}"}}]"#),
        ..outcome("big.tool", true)
    };
    store.record_last_outcome("dev", &big).expect("record");

    let loaded = store.load_last_outcomes("dev").expect("load");
    assert!(loaded[0].truncated);
    assert_eq!(loaded[0].outputs_json, r#"[{"id":"value","v":"ok"}]"#);
}

#[test]
fn clear_last_outcome_deletes_only_that_row() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .record_last_outcome("dev", &outcome("num.hex", true))
        .expect("record 1");
    store
        .record_last_outcome("dev", &outcome("net.ping", true))
        .expect("record 2");

    store.clear_last_outcome("dev", "num.hex").expect("clear");
    let remaining = store.load_last_outcomes("dev").expect("load");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].tool_id, "net.ping");
}

#[test]
fn pin_unpin_cleans_last_outcome_alongside_tombstone() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&one_board_state(vec![
            Placement::new("num.hex", 0, 0),
            Placement::new("net.ping", 1, 0),
        ]))
        .expect("prepare board");
    store
        .record_last_outcome("dev", &outcome("num.hex", true))
        .expect("record 1");
    store
        .record_last_outcome("dev", &outcome("net.ping", true))
        .expect("record 2");

    // The row-level tombstone path.
    store
        .tombstone_placement("dev", "num.hex")
        .expect("row-level unpin");
    let after_row = store.load_last_outcomes("dev").expect("load");
    assert_eq!(
        after_row
            .iter()
            .map(|o| o.tool_id.as_str())
            .collect::<Vec<_>>(),
        vec!["net.ping"],
        "the tombstoned pin's outcome must be deleted in the same transaction"
    );

    // The whole-save save_state path (diff → tombstone) cleans up the
    // same way.
    store
        .save_state_global(&one_board_state(Vec::new()))
        .expect("full unpin");
    assert!(
        store.load_last_outcomes("dev").expect("reload").is_empty(),
        "unpin via the save_state path must clean the outcome too"
    );
}
