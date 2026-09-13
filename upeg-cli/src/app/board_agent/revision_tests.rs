use upeg_core::{ArgsPreset, Placement};
use upeg_sources::pegboard::BoardVisibility;
use upeg_sources::store::Store;

use super::*;

#[test]
fn 지연_임포트_완료는_보드_리비전을_바꾸지_않는다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-revision-deferred",
        |_| {},
        || {
            const DEFERRED_TOOL: &str = "test.deferred_revision_import";
            let board = BoardKey::parse("dev").expect("보드");
            let path = pegboard::state_path_from_env().expect("스토어 경로");
            let visibility = BoardVisibility::from_process();
            let mut store = Store::open_at(&path).expect("스토어");
            let mut raw = store.load_state(&visibility).expect("저장된 상태");
            raw.layouts.insert(
                "dev".into(),
                vec![Placement::new(DEFERRED_TOOL, 0, 0).with_args_preset(Some(
                    ArgsPreset::parse(r#"{"input":"ff"}"#).expect("프리셋"),
                ))],
            );
            store
                .save_state(&raw, &visibility)
                .expect("등록 전 핀 저장");
            let before = board_context(&board).expect("등록 전 보드");
            assert!(before.tools.iter().all(|tool| tool.id != DEFERRED_TOOL));
            assert_eq!(before.unresolved_pins, vec![DEFERRED_TOOL]);

            let mut meta = upeg_runtime::toolbox_tool("num.hex_to_decimal")
                .expect("기본 도구")
                .clone();
            meta.id = DEFERRED_TOOL;
            meta.toolkit = "test";
            meta.local_id = "deferred_revision_import";
            let _registration = upeg_runtime::toolbox_add_tool_managed(meta);
            let after = board_context(&board).expect("등록 후 보드");
            assert!(after.tools.iter().any(|tool| tool.id == DEFERRED_TOOL));
            assert!(after.unresolved_pins.is_empty());
            assert_eq!(before.revision, after.revision);

            raw.layouts.get_mut("dev").expect("배치")[0].args_preset =
                Some(ArgsPreset::parse(r#"{"input":"10"}"#).expect("변경한 프리셋"));
            store
                .save_state(&raw, &visibility)
                .expect("프리셋 변경 저장");
            assert_ne!(
                after.revision,
                board_context(&board).expect("변경 후 보드").revision
            );
        },
    );
}

#[test]
fn 저장하지_않은_기본_배치를_처음_저장해도_리비전은_같다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-revision-first-save",
        |_| {},
        || {
            let path = pegboard::state_path_from_env().expect("스토어 경로");
            let visibility = BoardVisibility::from_process();
            // A fresh store has no saved row; the facade supplies the default pins.
            Store::open_at(&path)
                .expect("스토어")
                .save_state(
                    &pegboard::PegboardState {
                        boards: Vec::new(),
                        layouts: std::collections::BTreeMap::default(),
                        selection: pegboard::PegboardSelection::default(),
                    },
                    &visibility,
                )
                .expect("저장된 보드 제거");
            let board = BoardKey::parse("dev").expect("보드");
            let before = board_context(&board).expect("기본 보드");
            let state = pegboard::load_state_from_path(&path).expect("기본 상태");
            pegboard::save_state_to_path(&path, &state).expect("첫 저장");

            assert_eq!(
                before.revision,
                board_context(&board).expect("저장 후 보드").revision
            );
        },
    );
}
