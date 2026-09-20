use upeg_core::{ArgsPreset, Placement};
use upeg_sources::pegboard::BoardVisibility;
use upeg_sources::store::Store;

use super::*;

#[test]
fn completing_a_deferred_import_does_not_change_the_board_revision() {
    crate::test_support::with_seeded_pegboard_home(
        "board-revision-deferred",
        |_| {},
        || {
            const DEFERRED_TOOL: &str = "test.deferred_revision_import";
            let board = BoardKey::parse("dev").expect("board");
            let path = pegboard::state_path_from_env().expect("store path");
            let visibility = BoardVisibility::from_process();
            let mut store = Store::open_at(&path).expect("store");
            let mut raw = store.load_state(&visibility).expect("saved state");
            raw.layouts.insert(
                "dev".into(),
                vec![Placement::new(DEFERRED_TOOL, 0, 0).with_args_preset(Some(
                    ArgsPreset::parse(r#"{"input":"ff"}"#).expect("preset"),
                ))],
            );
            store
                .save_state(&raw, &visibility)
                .expect("save pins before registration");
            let before = board_context(&board).expect("board before registration");
            assert!(before.tools.iter().all(|tool| tool.id != DEFERRED_TOOL));
            assert_eq!(before.unresolved_pins, vec![DEFERRED_TOOL]);

            let mut meta = upeg_runtime::toolbox_tool("num.hex_to_decimal")
                .expect("base tool")
                .clone();
            meta.id = DEFERRED_TOOL;
            meta.toolkit = "test";
            meta.local_id = "deferred_revision_import";
            let _registration = upeg_runtime::toolbox_add_tool_managed(meta);
            let after = board_context(&board).expect("board after registration");
            assert!(after.tools.iter().any(|tool| tool.id == DEFERRED_TOOL));
            assert!(after.unresolved_pins.is_empty());
            assert_eq!(before.revision, after.revision);

            raw.layouts.get_mut("dev").expect("placement")[0].args_preset =
                Some(ArgsPreset::parse(r#"{"input":"10"}"#).expect("changed preset"));
            store
                .save_state(&raw, &visibility)
                .expect("save preset change");
            assert_ne!(
                after.revision,
                board_context(&board).expect("board after change").revision
            );
        },
    );
}

#[test]
fn first_save_of_the_unsaved_default_layout_keeps_the_revision() {
    crate::test_support::with_seeded_pegboard_home(
        "board-revision-first-save",
        |_| {},
        || {
            let path = pegboard::state_path_from_env().expect("store path");
            let visibility = BoardVisibility::from_process();
            // A fresh store has no saved row; the facade supplies the default pins.
            Store::open_at(&path)
                .expect("store")
                .save_state(
                    &pegboard::PegboardState {
                        boards: Vec::new(),
                        layouts: std::collections::BTreeMap::default(),
                        selection: pegboard::PegboardSelection::default(),
                    },
                    &visibility,
                )
                .expect("remove the saved board");
            let board = BoardKey::parse("dev").expect("board");
            let before = board_context(&board).expect("default board");
            let state = pegboard::load_state_from_path(&path).expect("default state");
            pegboard::save_state_to_path(&path, &state).expect("first save");

            assert_eq!(
                before.revision,
                board_context(&board).expect("board after save").revision
            );
        },
    );
}
