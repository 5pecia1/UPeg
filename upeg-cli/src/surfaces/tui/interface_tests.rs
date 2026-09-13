//! `tui_tests.rs`를 저장소 파일 크기 예산 안에 두기 위해 분리한 TUI
//! 인터페이스 인벤토리 회귀 테스트.

use crate::surfaces::tui::interface_inventory_entries;
use std::collections::HashSet;
use upeg_core::Surface;
use upeg_core::interface_inventory::{
    Compatibility, INTERFACE_INVENTORY_SCHEMA_VERSION, InterfaceInventory, InterfaceKind,
};

#[test]
fn tui_인터페이스_인벤토리는_안정적인_형태로_직렬화된다() {
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: interface_inventory_entries(),
    };

    inventory.validate().expect("유효한 TUI 인벤토리");

    let ids: HashSet<_> = inventory
        .entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    for id in [
        "tui.board.list",
        "tui.tag.filter",
        "tui.tool.detail",
        "tui.tool.form",
        "tui.tool.dispatch",
        "tui.tool.approval",
        "tui.tool.live_output",
    ] {
        assert!(ids.contains(id), "인벤토리 항목 {id}가 없다");
    }

    for entry in &inventory.entries {
        assert!(entry.surfaces.contains(&Surface::Tui));
        assert_eq!(entry.kind, InterfaceKind::TuiInteraction);
        assert_eq!(entry.version, "v1");
        assert_eq!(entry.compatibility, Compatibility::Stable);
        assert!(entry.owner.path.is_some(), "{} owner가 없다", entry.id);
        assert!(entry.docs.path.is_some(), "{} docs가 없다", entry.id);
        assert!(entry.source.path.is_some(), "{} source가 없다", entry.id);
        assert!(
            entry.tests.path().is_some_and(|path| !path.is_empty()),
            "{} tests가 없다",
            entry.id
        );
    }
}
