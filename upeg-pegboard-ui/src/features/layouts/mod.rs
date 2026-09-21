//! Pegboard layout persistence + Tag filter helpers.
//!
//! Layouts are coordinate-based: each entry is a [`Placement`] carrying
//! the user's chosen `(x, y)` on the [`upeg_core::BOARD_COLS`]-wide canonical grid.
//! Pin size comes from the manifest, so it isn't persisted here.
//! Surfaces render placements by reading `(x, y)` directly; shared pegboard
//! helpers reconcile coordinates after every mutation.

use std::collections::{BTreeMap, HashMap};

use upeg_core::{Placement, SearchSignals, Surface, ToolMeta};
use upeg_runtime::pegboard::{placements_from_ordered_ids, reconcile_placements};
use upeg_runtime::toolbox_tool;
use upeg_runtime::{ToolMetaRuntimeExt, tags_for_surface, toolbox_tools};

use crate::features::boards::Board;

pub const ALL_TAG: &str = "all";

pub type BoardLayouts = HashMap<&'static str, Vec<Placement>>;

pub fn serialize_layouts(layouts: &BoardLayouts) -> String {
    let serializable: BTreeMap<String, Vec<Placement>> = layouts
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect();
    serde_json::to_string(&serializable).unwrap_or_else(|_| "{}".into())
}

/// Parse a layouts JSON blob back into the runtime shape, dropping orphan
/// board keys, tool ids unknown to the toolbox, tools not on the Desktop
/// surface, and any placement that no longer fits the canonical grid.
/// Returns `None` only when the JSON is malformed.
pub fn deserialize_layouts(json: &str, valid_boards: &[Board]) -> Option<BoardLayouts> {
    let parsed = serde_json::from_str::<BTreeMap<String, Vec<Placement>>>(json).ok()?;
    let mut out: BoardLayouts = HashMap::new();
    for (board_key, raw_placements) in parsed {
        let Some(board) = valid_boards.iter().find(|b| b.key == board_key) else {
            continue;
        };
        let desktop_placements = raw_placements.into_iter().filter(|placement| {
            toolbox_tool(&placement.tool_id)
                .is_some_and(|tool| tool.is_on_surface(Surface::Desktop))
        });
        out.insert(board.key, reconcile_placements(desktop_placements));
    }
    Some(out)
}

pub fn save_layouts(layouts: &BoardLayouts) {
    let json = serialize_layouts(layouts);
    let _ = crate::platform::storage::set_item(crate::platform::storage::LAYOUTS_KEY, &json);
}

pub fn load_layouts(valid_boards: &[Board]) -> Option<BoardLayouts> {
    let json = crate::platform::storage::get_item(crate::platform::storage::LAYOUTS_KEY)?;
    deserialize_layouts(&json, valid_boards)
}

pub fn default_layouts(boards: &[Board]) -> BoardLayouts {
    let mut out: BoardLayouts = HashMap::new();
    for b in boards {
        let ids: Vec<&'static str> = toolbox_tools()
            .filter(|t| t.is_on_surface(Surface::Desktop))
            .filter(|t| t.is_on_board(b.key))
            .map(|t| t.id)
            .collect();
        out.insert(b.key, placements_from_ordered_ids(ids.into_iter()));
    }
    out
}

pub fn tag_options(tools: &[&'static ToolMeta]) -> Vec<String> {
    let mut tags: Vec<String> = tools.iter().flat_map(|t| t.tag_labels()).collect();
    tags.sort();
    tags.dedup();
    let mut out = Vec::with_capacity(tags.len() + 1);
    out.push(ALL_TAG.to_string());
    out.extend(tags);
    out
}

pub fn tools_for_tag(tools: &[&'static ToolMeta], selected_tag: &str) -> Vec<&'static ToolMeta> {
    if selected_tag == ALL_TAG {
        return tools.to_vec();
    }
    tools
        .iter()
        .copied()
        .filter(|t| t.has_tag(selected_tag))
        .collect()
}

pub fn count_for_tag(tools: &[&'static ToolMeta], tag: &str) -> usize {
    if tag == ALL_TAG {
        return tools.len();
    }
    tools.iter().filter(|t| t.has_tag(tag)).count()
}

pub fn normalize_selected_tag(tags: &[String], selected_tag: &str) -> String {
    tags.iter()
        .find(|tag| tag.as_str() == selected_tag)
        .cloned()
        .unwrap_or_else(|| ALL_TAG.to_string())
}

pub fn tool_is_pinned_in_layout(layouts: &BoardLayouts, board: &str, id: &str) -> bool {
    layouts
        .get(board)
        .is_some_and(|placements| placements.iter().any(|p| p.tool_id == id))
}

pub fn pinned_search_signals_for_board(layouts: &BoardLayouts, board: &str) -> SearchSignals {
    layouts
        .get(board)
        .map_or_else(SearchSignals::default, |placements| {
            SearchSignals::from_pinned_placements(placements)
        })
}

pub fn tag_from_toolbox(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed == ALL_TAG {
        return Some(ALL_TAG.to_string());
    }
    tags_for_surface(Surface::Desktop)
        .into_iter()
        .find(|tag| tag == trimmed)
}

pub fn save_selected_tag(tag: &str) {
    crate::platform::storage::save_selected_tag(tag);
}

pub fn load_selected_tag() -> Option<String> {
    crate::platform::storage::load_selected_tag().and_then(|tag| tag_from_toolbox(&tag))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::boards::default_boards;

    #[test]
    fn deserialize_after_serialize_restores_original() {
        let boards = default_boards();
        let mut layouts: BoardLayouts = HashMap::new();
        layouts.insert(
            "dev",
            vec![
                Placement::new("num.hex_to_decimal", 0, 0),
                Placement::new("id.uuid_v7", 1, 0),
            ],
        );
        let json = serialize_layouts(&layouts);
        let back = deserialize_layouts(&json, &boards).expect("parse");
        assert_eq!(back.get("dev").map(Vec::len), Some(2));
    }

    #[test]
    fn deserialize_layouts_garbage_returns_none() {
        assert!(deserialize_layouts("not json", &default_boards()).is_none());
        assert!(deserialize_layouts("[]", &default_boards()).is_none());
    }

    #[test]
    fn deserialize_drops_unknown_boards() {
        let boards = default_boards();
        let json = r#"{"dev":[{"tool_id":"num.hex_to_decimal","x":0,"y":0}],"obsolete_board":[]}"#;
        let back = deserialize_layouts(json, &boards).expect("parse");
        assert!(back.contains_key("dev"));
        assert!(!back.contains_key("obsolete_board"));
    }

    #[test]
    fn deserialize_drops_unregistered_tools() {
        let boards = default_boards();
        let json = r#"{"dev":[{"tool_id":"num.hex_to_decimal","x":0,"y":0},{"tool_id":"nonexistent.tool","x":1,"y":0}]}"#;
        let back = deserialize_layouts(json, &boards).expect("parse");
        let dev = back.get("dev").unwrap();
        assert_eq!(dev.len(), 1);
        assert_eq!(dev[0].tool_id, "num.hex_to_decimal");
    }

    #[test]
    fn deserialize_rejects_legacy_string_arrays() {
        let boards = default_boards();
        let json = r#"{"dev":["num.hex_to_decimal","id.uuid_v7"]}"#;
        assert!(deserialize_layouts(json, &boards).is_none());
    }

    #[test]
    fn correctly_detects_whether_tool_is_pinned_in_layout() {
        let mut layouts: BoardLayouts = HashMap::new();
        layouts.insert(
            "dev",
            vec![Placement::new("a", 0, 0), Placement::new("b", 1, 0)],
        );
        assert!(tool_is_pinned_in_layout(&layouts, "dev", "a"));
        assert!(!tool_is_pinned_in_layout(&layouts, "dev", "c"));
        assert!(!tool_is_pinned_in_layout(&layouts, "missing", "a"));
    }

    #[test]
    fn pinned_search_signals_for_board_builds_ranking_from_layout_map() {
        let mut layouts: BoardLayouts = HashMap::new();
        layouts.insert(
            "dev",
            vec![
                Placement::new("z.tool", 2, 1),
                Placement::new("a.tool", 1, 0),
                Placement::new("z.tool", 0, 0),
            ],
        );

        let signals = pinned_search_signals_for_board(&layouts, "dev");

        assert_eq!(signals.pinned.len(), 2);
        assert_eq!(signals.pinned[0].tool_id, "z.tool");
        assert_eq!(signals.pinned[0].rank, 0);
        assert_eq!(signals.pinned[1].tool_id, "a.tool");
        assert_eq!(signals.pinned[1].rank, 1);
        assert!(signals.recent.is_empty());
        assert_eq!(
            pinned_search_signals_for_board(&layouts, "missing"),
            SearchSignals::default()
        );
    }

    #[test]
    fn tag_options_include_all_and_sorted_unique_tags() {
        // Minimal ToolMeta-like setup via real tools from the toolbox.
        let tools: Vec<&'static ToolMeta> = toolbox_tools().collect();
        let tags = tag_options(&tools);
        assert_eq!(tags.first().unwrap(), ALL_TAG);
        let rest = &tags[1..];
        for w in rest.windows(2) {
            assert!(w[0] <= w[1]);
            assert!(w[0] != w[1]);
        }
    }

    #[test]
    fn all_tag_filter_returns_all_tools() {
        let tools: Vec<&'static ToolMeta> = toolbox_tools().collect();
        let filtered = tools_for_tag(&tools, ALL_TAG);
        assert_eq!(filtered.len(), tools.len());
    }

    #[test]
    fn specific_tag_filter_returns_only_tools_with_that_tag() {
        let tools: Vec<&'static ToolMeta> = toolbox_tools().collect();
        let filtered = tools_for_tag(&tools, "convert");
        for t in &filtered {
            assert!(t.has_tag("convert"));
        }
    }

    #[test]
    fn selected_tag_normalization_returns_valid_tag() {
        let tags = vec!["all".into(), "convert".into(), "text".into()];
        assert_eq!(normalize_selected_tag(&tags, "convert"), "convert");
        assert_eq!(normalize_selected_tag(&tags, "missing"), "all");
    }

    #[test]
    fn count_for_tag_matches_filtered_result_length() {
        let tools: Vec<&'static ToolMeta> = toolbox_tools().collect();
        let n = count_for_tag(&tools, ALL_TAG);
        assert_eq!(n, tools.len());
    }
}
