use super::*;

#[test]
fn unpinning_one_duplicate_keeps_the_sibling_placement_and_outcome() {
    let first = Placement::new("num.hex", 0, 0)
        .with_pin_id(PinId::parse("pin-first").expect("valid pin id"));
    let second = Placement::new("num.hex", 1, 0)
        .with_pin_id(PinId::parse("pin-second").expect("valid pin id"));
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&one_board_state(vec![first, second]))
        .expect("save duplicate pins");
    store
        .record_last_outcome("dev", &outcome_for_pin("pin-first", "num.hex", true))
        .expect("record first outcome");
    store
        .record_last_outcome("dev", &outcome_for_pin("pin-second", "num.hex", false))
        .expect("record second outcome");

    store
        .tombstone_placement("dev", "pin-first")
        .expect("unpin first duplicate");

    let state = store.load_state_global().expect("reload state");
    let placements = &state.layouts["dev"];
    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].pin_id.as_str(), "pin-second");
    assert_eq!(placements[0].tool_id, "num.hex");
    let outcomes = store.load_last_outcomes("dev").expect("reload outcomes");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].pin_id, "pin-second");
    assert_eq!(outcomes[0].tool_id, "num.hex");
    assert!(!outcomes[0].ok);
}

#[test]
fn v7_migrates_v5_rows_with_all_placement_fields_and_outcomes() {
    let dir = temp_store_dir("v5-to-v7");
    std::fs::create_dir_all(&dir).expect("create temp directory");
    let path = store_db_path(&dir);
    {
        let conn = rusqlite::Connection::open(&path).expect("open v5 database");
        for migration in &schema::MIGRATIONS[..5] {
            conn.execute_batch(migration).expect("apply v5 migration");
        }
        conn.pragma_update(None, "user_version", 5)
            .expect("mark v5 schema");
        conn.execute(
            "INSERT INTO boards (key, title, position, updated_at, device_id, deleted, description, instructions) \
             VALUES ('dev', 'Dev', 0, 10, 'device-a', 0, 'description', 'instructions')",
            [],
        )
        .expect("insert board");
        conn.execute(
            "INSERT INTO placements \
             (board_key, tool_id, x, y, color, span_cols, span_rows, args_preset, updated_at, device_id, deleted) \
             VALUES ('dev', 'num.hex', 2, 3, '#AABBCC', 2, 3, '{\"base\":16}', 11, 'device-a', 0)",
            [],
        )
        .expect("insert live placement");
        conn.execute(
            "INSERT INTO placements \
             (board_key, tool_id, x, y, color, span_cols, span_rows, args_preset, updated_at, device_id, deleted) \
             VALUES ('dev', 'dead.tool', 4, 5, NULL, NULL, NULL, NULL, 12, 'device-b', 1)",
            [],
        )
        .expect("insert tombstone");
        conn.execute(
            "INSERT INTO last_outcomes \
             (board_key, tool_id, ok, primary_output_id, outputs_json, truncated, error_code, error_message, updated_at, device_id) \
             VALUES ('dev', 'num.hex', 0, 'value', '[{\"id\":\"value\"}]', 1, 'E_FAIL', 'boom', 13, 'device-a')",
            [],
        )
        .expect("insert outcome");
    }

    let store = Store::open_at(&path).expect("migrate v5 database");
    assert_eq!(
        schema::user_version(store.connection()).expect("version"),
        7
    );

    let live = store.load_state_global().expect("load migrated state");
    let placement = &live.layouts["dev"][0];
    assert_eq!(placement.pin_id.as_str(), "num.hex");
    assert_eq!(placement.tool_id, "num.hex");
    assert_eq!((placement.x, placement.y), (2, 3));
    assert_eq!(
        placement.color.as_ref().map(PinColorHex::as_str),
        Some("#AABBCC")
    );
    assert_eq!(
        placement.span,
        Some(PinSpan::new(
            ColSpan::new(2).unwrap(),
            RowSpan::new(3).unwrap()
        ))
    );
    assert_eq!(
        placement.args_preset.as_ref().map(ArgsPreset::as_str),
        Some("{\"base\":16}")
    );

    let tombstone: (String, String, i64, i64, String) = store
        .connection()
        .query_row(
            "SELECT pin_id, tool_id, updated_at, deleted, device_id FROM placements WHERE tool_id = 'dead.tool'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .expect("read tombstone");
    assert_eq!(
        tombstone,
        (
            "dead.tool".into(),
            "dead.tool".into(),
            12,
            1,
            "device-b".into()
        )
    );

    let outcomes = store
        .load_last_outcomes("dev")
        .expect("load migrated outcome");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].pin_id, "num.hex");
    assert_eq!(outcomes[0].tool_id, "num.hex");
    assert!(!outcomes[0].ok);
    assert!(outcomes[0].truncated);
    assert_eq!(outcomes[0].error_code.as_deref(), Some("E_FAIL"));
    assert_eq!(outcomes[0].error_message.as_deref(), Some("boom"));
    assert_eq!(outcomes[0].updated_at_ms, 13);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn v7_migrates_existing_v6_diagnostics_without_losing_reports() {
    let dir = temp_store_dir("v6-to-v7-diagnostics");
    std::fs::create_dir_all(&dir).expect("create temp directory");
    let path = store_db_path(&dir);
    {
        let conn = rusqlite::Connection::open(&path).expect("open v6 database");
        for migration in &schema::MIGRATIONS[..6] {
            conn.execute_batch(migration).expect("apply v6 migration");
        }
        conn.pragma_update(None, "user_version", 6)
            .expect("mark v6 schema");
        conn.execute(
            "INSERT INTO diagnostics \
             (id, run_id, occurred_at_ms, app_version, os, source, error_code, error_message, status, stdout, stderr) \
             VALUES ('report-1', 'run-1', 42, 'test', 'linux', 'tool', 'E_FAIL', 'boom', 'failed', '', '')",
            [],
        )
        .expect("insert existing diagnostic");
    }

    let store = Store::open_at(&path).expect("migrate v6 database");
    assert_eq!(
        schema::user_version(store.connection()).expect("version"),
        7
    );
    let diagnostic: (String, String) = store
        .connection()
        .query_row(
            "SELECT run_id, error_message FROM diagnostics WHERE id = 'report-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("existing diagnostic survives migration");
    assert_eq!(diagnostic, ("run-1".into(), "boom".into()));

    let _ = std::fs::remove_dir_all(&dir);
}
