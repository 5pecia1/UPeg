use super::*;
use upeg_core::BoardKey;
use upeg_runtime::pegboard_project::{ProjectBoardDecl, ProjectBoardScope};

#[test]
fn current_input_and_result_input_survive_reopen_independently() {
    let dir = temp_store_dir("pin-input-reopen");
    let path = store_db_path(&dir);
    let pin = Placement::new("num.hex", 0, 0)
        .with_args_preset(Some(ArgsPreset::parse(r#"{"base":16}"#).expect("preset")));
    {
        let mut store = Store::open_at(&path).expect("create store");
        store
            .save_state_global(&one_board_state(vec![pin]))
            .expect("save placement");
        store
            .save_pin_draft(
                "dev",
                &PinDraftRecord {
                    pin_id: "num.hex".into(),
                    tool_id: "num.hex".into(),
                    args_json: r#"{"value":"4"}"#.into(),
                },
            )
            .expect("save current input");
        store
            .record_last_outcome(
                "dev",
                &NewLastOutcome {
                    args_json: Some(r#"{"value":"4"}"#.into()),
                    ..outcome("num.hex", true)
                },
            )
            .expect("save result");
        store
            .save_pin_draft(
                "dev",
                &PinDraftRecord {
                    pin_id: "num.hex".into(),
                    tool_id: "num.hex".into(),
                    args_json: r#"{"value":"5"}"#.into(),
                },
            )
            .expect("edit current input");
    }
    let mut reopened = Store::open_at(&path).expect("reopen store");
    assert_eq!(
        reopened.load_pin_drafts("dev").expect("drafts")[0].args_json,
        r#"{"value":"5"}"#
    );
    assert_eq!(
        reopened.load_last_outcomes("dev").expect("outcomes")[0]
            .args_json
            .as_deref(),
        Some(r#"{"value":"4"}"#)
    );
    assert_eq!(
        reopened.load_state_global().expect("state").layouts["dev"][0]
            .args_preset
            .as_ref()
            .map(ArgsPreset::as_str),
        Some(r#"{"base":16}"#)
    );

    reopened
        .tombstone_placement("dev", "num.hex")
        .expect("remove pin");
    assert!(
        reopened
            .load_pin_drafts("dev")
            .expect("drafts after remove")
            .is_empty()
    );
    assert!(
        reopened
            .load_last_outcomes("dev")
            .expect("outcomes after remove")
            .is_empty()
    );
    drop(reopened);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn v8_migration_preserves_old_results_without_inventing_input() {
    let dir = temp_store_dir("v8-input-migration");
    let path = store_db_path(&dir);
    std::fs::create_dir_all(&dir).expect("directory");
    {
        let conn = rusqlite::Connection::open(&path).expect("old store");
        for migration in &schema::MIGRATIONS[..7] {
            conn.execute_batch(migration).expect("old migration");
        }
        conn.pragma_update(None, "user_version", 7)
            .expect("old version");
        conn.execute(
            "INSERT INTO last_outcomes (board_key,pin_id,tool_id,ok,outputs_json,truncated,updated_at,device_id) \
             VALUES ('dev','pin-a','num.hex',1,'[]',1,1,'device-a')",
            [],
        ).expect("old result");
    }
    let store = Store::open_at(&path).expect("migrate v8");
    let outcomes = store.load_last_outcomes("dev").expect("old results");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].args_json, None);
    assert!(outcomes[0].truncated);
    assert!(
        store
            .load_pin_drafts("dev")
            .expect("new drafts table")
            .is_empty()
    );
    drop(store);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn duplicate_tool_drafts_are_isolated_and_board_delete_clears_state() {
    let mut store = Store::open_in_memory().expect("store");
    store
        .save_state_global(&one_board_state(vec![
            Placement::new("num.hex", 0, 0).with_pin_id(PinId::parse("pin-a").expect("pin id")),
            Placement::new("num.hex", 1, 0).with_pin_id(PinId::parse("pin-b").expect("pin id")),
        ]))
        .expect("save duplicate pins");
    for (pin_id, value) in [("pin-a", "4"), ("pin-b", "5")] {
        store
            .save_pin_draft(
                "dev",
                &PinDraftRecord {
                    pin_id: pin_id.into(),
                    tool_id: "num.hex".into(),
                    args_json: format!(r#"{{"value":"{value}"}}"#),
                },
            )
            .expect("save draft");
    }
    store
        .record_last_outcome("dev", &outcome_for_pin("pin-a", "num.hex", true))
        .expect("save outcome");
    let drafts = store.load_pin_drafts("dev").expect("load drafts");
    assert_eq!(
        drafts
            .iter()
            .map(|row| row.pin_id.as_str())
            .collect::<Vec<_>>(),
        vec!["pin-a", "pin-b"]
    );
    assert_ne!(drafts[0].args_json, drafts[1].args_json);

    store
        .save_state_global(&PegboardState {
            boards: Vec::new(),
            layouts: BTreeMap::new(),
            selection: PegboardSelection::default(),
        })
        .expect("delete board");
    assert!(
        store
            .load_pin_drafts("dev")
            .expect("load deleted drafts")
            .is_empty()
    );
    assert!(
        store
            .load_last_outcomes("dev")
            .expect("load deleted outcomes")
            .is_empty()
    );
}

#[test]
fn pin_cache_edits_do_not_publish_pegboard_layout_revisions() {
    let mut store = Store::open_in_memory().expect("store");
    let before = store.change_rev().expect("revision before draft");
    store
        .save_pin_draft(
            "dev",
            &PinDraftRecord {
                pin_id: "pin-a".into(),
                tool_id: "num.hex".into(),
                args_json: r#"{"value":"4"}"#.into(),
            },
        )
        .expect("save draft");
    assert_eq!(store.change_rev().expect("revision after draft"), before);
    store.clear_pin_draft("dev", "pin-a").expect("clear draft");
    assert_eq!(store.change_rev().expect("revision after clear"), before);
    store
        .record_last_outcome("dev", &outcome_for_pin("pin-a", "num.hex", true))
        .expect("record result");
    assert_eq!(store.change_rev().expect("revision after result"), before);
    store
        .clear_last_outcome("dev", "pin-a")
        .expect("clear result");
    assert_eq!(
        store.change_rev().expect("revision after result clear"),
        before
    );
}

#[test]
fn same_visible_board_and_pin_in_two_projects_keep_separate_state() {
    let board_key = BoardKey::parse("shared").expect("board key");
    let project_key = |manifest: &str| {
        let scope = ProjectBoardScope::for_manifest(
            std::path::Path::new(manifest),
            vec![ProjectBoardDecl::new(
                board_key.clone(),
                "Shared".to_string(),
            )],
        );
        crate::pegboard::BoardVisibility::for_project(scope).store_key("shared")
    };
    let a = project_key("/project-a/upeg.toml");
    let b = project_key("/project-b/upeg.toml");
    assert_ne!(a, b);

    let mut store = Store::open_in_memory().expect("store");
    for (key, value) in [(&a, "4"), (&b, "5")] {
        let args_json = format!(r#"{{"value":"{value}"}}"#);
        store
            .save_pin_draft(
                key,
                &PinDraftRecord {
                    pin_id: "same-pin".into(),
                    tool_id: "num.hex".into(),
                    args_json: args_json.clone(),
                },
            )
            .expect("save project draft");
        store
            .record_last_outcome(
                key,
                &NewLastOutcome {
                    args_json: Some(args_json),
                    ..outcome_for_pin("same-pin", "num.hex", true)
                },
            )
            .expect("save project result");
    }
    assert_eq!(
        store.load_pin_drafts(&a).expect("project a draft")[0].args_json,
        r#"{"value":"4"}"#
    );
    assert_eq!(
        store.load_pin_drafts(&b).expect("project b draft")[0].args_json,
        r#"{"value":"5"}"#
    );
    assert_eq!(
        store.load_last_outcomes(&a).expect("project a result")[0]
            .args_json
            .as_deref(),
        Some(r#"{"value":"4"}"#)
    );
    assert_eq!(
        store.load_last_outcomes(&b).expect("project b result")[0]
            .args_json
            .as_deref(),
        Some(r#"{"value":"5"}"#)
    );
}
