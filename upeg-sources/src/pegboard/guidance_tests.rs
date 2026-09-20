use super::*;

#[test]
fn board_guidance_survives_json_round_trip() {
    let json = r##"[{"key":"dev","title":"Dev","guidance":{"description":"개발 작업","instructions":"# 순서\n\n    cargo test\n"}}]"##;
    let boards = export::boards_from_json(json).expect("read boards");
    let exported: serde_json::Value =
        serde_json::from_str(&export::boards_to_json(&boards).expect("export boards"))
            .expect("parse json");

    assert_eq!(exported[0]["guidance"]["description"], "개발 작업");
    assert_eq!(
        exported[0]["guidance"]["instructions"],
        "# 순서\n\n    cargo test\n"
    );
}

#[test]
fn omitted_board_guidance_reads_as_empty_strings() {
    let boards = export::boards_from_json(r#"[{"key":"dev","title":"Dev"}]"#).expect("read boards");
    let exported: serde_json::Value =
        serde_json::from_str(&export::boards_to_json(&boards).expect("export boards"))
            .expect("parse json");

    assert_eq!(exported[0]["guidance"]["description"], "");
    assert_eq!(exported[0]["guidance"]["instructions"], "");
}

#[test]
fn omitted_fields_of_board_guidance_read_as_empty_strings() {
    for (json, description, instructions) in [
        (
            r#"[{"key":"dev","title":"Dev","guidance":{"description":"설명만"}}]"#,
            "설명만",
            "",
        ),
        (
            r#"[{"key":"dev","title":"Dev","guidance":{"instructions":"지침만"}}]"#,
            "",
            "지침만",
        ),
    ] {
        let boards = export::boards_from_json(json).expect("read boards");
        assert_eq!(boards[0].guidance.description, description);
        assert_eq!(boards[0].guidance.instructions, instructions);
    }
}

#[test]
fn personal_board_guidance_survives_save_and_reload() {
    let root = std::env::temp_dir().join(format!("upeg-guidance-roundtrip-{}", std::process::id()));
    let path = state_path_from_root(&root);
    let _ = std::fs::remove_dir_all(&root);
    let visibility = BoardVisibility::global_only();
    let mut state = default_state();
    state.boards = export::boards_from_json(
        r##"[{"key":"dev","title":"Dev","guidance":{"description":"개발 작업","instructions":"# 순서\n\n    cargo test\n"}}]"##,
    ).expect("read boards");
    save_state_to_path_in(&path, &state, &visibility).expect("save");
    let loaded = load_state_from_path_in(&path, &visibility).expect("reload");
    let exported: serde_json::Value =
        serde_json::from_str(&export::boards_to_json(&loaded.boards).expect("export"))
            .expect("parse json");

    assert_eq!(exported[0]["guidance"]["description"], "개발 작업");
    assert_eq!(
        exported[0]["guidance"]["instructions"],
        "# 순서\n\n    cargo test\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn editing_only_board_guidance_reaches_existing_rows_and_export() {
    let root = std::env::temp_dir().join(format!("upeg-guidance-edit-{}", std::process::id()));
    let path = state_path_from_root(&root);
    let _ = std::fs::remove_dir_all(&root);
    let visibility = BoardVisibility::global_only();
    let state = default_state();
    save_state_to_path_in(&path, &state, &visibility).expect("save default state");
    set_board_guidance_to_path_in(
        &path,
        "personal",
        BoardGuidance {
            description: "일상 작업".into(),
            instructions: "# 매일\n\n    정리\n".into(),
        },
        &visibility,
    )
    .expect("save guidance only");

    let reloaded = load_state_from_path_in(&path, &visibility).expect("reload");
    let imported =
        export::boards_from_json(&export::boards_to_json(&reloaded.boards).expect("export"))
            .expect("import");
    let guidance = &imported
        .iter()
        .find(|board| board.key == "personal")
        .expect("personal board")
        .guidance;
    assert_eq!(guidance.description, "일상 작업");
    assert_eq!(guidance.instructions, "# 매일\n\n    정리\n");
    assert_eq!(reloaded.layouts, state.layouts);
    set_board_guidance_to_path_in(&path, "personal", BoardGuidance::default(), &visibility)
        .expect("clear guidance");
    let cleared =
        load_state_from_path_in(&path, &visibility).expect("reload after clearing guidance");
    assert_eq!(
        board_guidance_in(&cleared, "personal"),
        Some(&BoardGuidance::default())
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn board_guidance_edit_applies_only_to_named_personal_board() {
    let mut state = default_state();
    let layouts = state.layouts.clone();
    set_board_guidance_in(
        &mut state,
        "personal",
        BoardGuidance {
            description: "개인 작업".into(),
            instructions: "# 시작\n\n    확인\n".into(),
        },
        &BoardVisibility::global_only(),
    )
    .expect("edit guidance");

    let guidance = board_guidance_in(&state, "personal").expect("personal board");
    assert_eq!(guidance.description, "개인 작업");
    assert_eq!(guidance.instructions, "# 시작\n\n    확인\n");
    assert_eq!(
        board_guidance_in(&state, "dev"),
        Some(&BoardGuidance::default())
    );
    assert_eq!(state.layouts, layouts);
}

#[test]
fn editing_missing_board_guidance_leaves_state_unchanged() {
    let mut state = default_state();
    let before = state.clone();
    let error = set_board_guidance_in(
        &mut state,
        "missing",
        BoardGuidance::default(),
        &BoardVisibility::global_only(),
    )
    .expect_err("unknown board");

    assert!(matches!(error, BoardGuidanceEditError::UnknownBoard { board } if board == "missing"));
    assert_eq!(state, before);
}

#[test]
fn guidance_edit_preserves_not_yet_registered_pins_and_presets() {
    let root = std::env::temp_dir().join(format!("upeg-guidance-deferred-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);
    let visibility = BoardVisibility::global_only();
    let mut store = crate::store::Store::open_at(&path).expect("store");
    let mut state = default_state();
    state.layouts.insert(
        "personal".into(),
        vec![
            Placement::new("not_loaded.echo", 0, 0).with_args_preset(Some(
                upeg_core::ArgsPreset::parse(r#"{"message":"유지"}"#).expect("preset"),
            )),
        ],
    );
    store
        .save_state(&state, &visibility)
        .expect("save deferred-tool pin");

    set_board_guidance_to_path_in(
        &path,
        "personal",
        BoardGuidance {
            description: "새 설명".into(),
            instructions: "새 지침".into(),
        },
        &visibility,
    )
    .expect("save guidance");

    let reloaded = store.load_state(&visibility).expect("reload original");
    assert_eq!(reloaded.layouts, state.layouts);
    assert_eq!(
        board_guidance_in(&reloaded, "personal")
            .expect("guidance")
            .description,
        "새 설명"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn first_guidance_save_keeps_all_default_boards_and_pins() {
    let root =
        std::env::temp_dir().join(format!("upeg-guidance-first-edit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);
    let visibility = BoardVisibility::global_only();
    let expected = default_state();

    set_board_guidance_to_path_in(
        &path,
        "personal",
        BoardGuidance {
            description: "첫 설명".into(),
            instructions: String::new(),
        },
        &visibility,
    )
    .expect("save first guidance");

    let state = crate::store::Store::open_at(&path)
        .expect("store")
        .load_state(&visibility)
        .expect("saved state");
    assert_eq!(state.boards.len(), expected.boards.len());
    assert_eq!(state.layouts, expected.layouts);
    assert_eq!(
        board_guidance_in(&state, "personal")
            .expect("guidance")
            .description,
        "첫 설명"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn guidance_save_does_not_affect_newer_placements_from_other_connection() {
    let root =
        std::env::temp_dir().join(format!("upeg-guidance-other-writer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);
    let visibility = BoardVisibility::global_only();
    let mut editor = crate::store::Store::open_at(&path).expect("guidance edit connection");
    editor
        .save_state(&default_state(), &visibility)
        .expect("save defaults");
    let _previous = editor
        .load_state(&visibility)
        .expect("existing screen state");
    let mut other = crate::store::Store::open_at(&path).expect("other connection");
    let changed = Placement::new("not_loaded.other_writer", 5, 7);
    other
        .upsert_placement("personal", &changed)
        .expect("other connection change");

    assert!(
        editor
            .update_board_guidance(
                "personal",
                &BoardGuidance {
                    description: "새 안내".into(),
                    instructions: String::new(),
                },
                &visibility
            )
            .expect("edit guidance only")
    );

    let state = other.load_state(&visibility).expect("latest state");
    assert!(state.layouts["personal"].contains(&changed));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn saving_missing_board_guidance_does_not_change_store_revision() {
    let root = std::env::temp_dir().join(format!(
        "upeg-guidance-missing-write-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);
    let store = crate::store::Store::open_at(&path).expect("store");
    let before = store.change_rev().expect("existing revision");

    let error = set_board_guidance_to_path_in(
        &path,
        "missing",
        BoardGuidance::default(),
        &BoardVisibility::global_only(),
    )
    .expect_err("missing board");

    assert!(matches!(error, BoardGuidanceEditError::UnknownBoard { .. }));
    assert_eq!(store.change_rev().expect("revision after change"), before);
    let _ = std::fs::remove_dir_all(root);
}
