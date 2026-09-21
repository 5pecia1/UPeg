//! TUI interface-inventory regression tests, split out of `tui_tests.rs`
//! to stay under the repository file-size budget.

use crate::surfaces::tui::interface_inventory_entries;
use std::collections::HashSet;
use upeg_core::Surface;
use upeg_core::interface_inventory::{
    Compatibility, INTERFACE_INVENTORY_SCHEMA_VERSION, InterfaceInventory, InterfaceKind,
};

#[test]
fn tui_interface_inventory_serializes_to_stable_shape() {
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: interface_inventory_entries(),
    };

    inventory.validate().expect("valid TUI inventory");

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
        assert!(ids.contains(id), "missing inventory entry {id}");
    }

    for entry in &inventory.entries {
        assert!(entry.surfaces.contains(&Surface::Tui));
        assert_eq!(entry.kind, InterfaceKind::TuiInteraction);
        assert_eq!(entry.version, "v1");
        assert_eq!(entry.compatibility, Compatibility::Stable);
        assert!(entry.owner.path.is_some(), "{} missing owner", entry.id);
        assert!(entry.docs.path.is_some(), "{} missing docs", entry.id);
        assert!(entry.source.path.is_some(), "{} missing source", entry.id);
        assert!(
            entry.tests.path().is_some_and(|path| !path.is_empty()),
            "{} missing tests",
            entry.id
        );
    }
}
