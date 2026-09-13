//! Canonical JSON boundary for pegboard state.
//!
//! The desktop backup file and the Flutter storage adapter exchange boards
//! and layouts as JSON. This module pins the canonical wire shape:
//! board guidance, `BTreeMap` key ordering for layouts, and `skip_serializing_if` omission
//! of `None` fields on [`Placement`]. Backup export/import and the drift
//! gates round-trip through here unmodified.

use std::collections::BTreeMap;

use upeg_core::Placement;

use crate::pegboard::BoardData;

pub fn boards_to_json(boards: &[BoardData]) -> serde_json::Result<String> {
    serde_json::to_string(boards)
}

pub fn boards_from_json(json: &str) -> serde_json::Result<Vec<BoardData>> {
    serde_json::from_str(json)
}

pub fn layouts_to_json(layouts: &BTreeMap<String, Vec<Placement>>) -> serde_json::Result<String> {
    serde_json::to_string(layouts)
}

pub fn layouts_from_json(json: &str) -> serde_json::Result<BTreeMap<String, Vec<Placement>>> {
    serde_json::from_str(json)
}
