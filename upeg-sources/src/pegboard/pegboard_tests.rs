use super::*;
use upeg_core::{BOARD_COLS, PinColorHex, Surface};
use upeg_runtime::pegboard::{placement_rect, placement_size, rects_overlap};

#[test]
fn default_state_has_shared_boards_and_layouts() {
    let state = default_state();
    let keys: Vec<&str> = state
        .boards
        .iter()
        .map(|board| board.key.as_str())
        .collect();
    assert_eq!(keys, vec!["dev", "trading", "personal"]);
    assert!(state.layouts.contains_key("dev"));
    assert!(state.layouts.contains_key("trading"));
    assert!(state.layouts.contains_key("personal"));
    assert!(
        state
            .layouts
            .get("trading")
            .is_some_and(|placements| placements.iter().any(|p| p.tool_id == "eth.gas")),
        "the shared default layout must include the GUI-only trading metadata"
    );
}

#[test]
fn save_load_round_trip_prunes_unknown_entries() {
    let root = std::env::temp_dir().join(format!("upeg-pegboard-{}", std::process::id()));
    let path = state_path_from_root(&root);
    let _ = std::fs::remove_dir_all(&root);
    let mut layouts = BTreeMap::new();
    layouts.insert(
        "dev".into(),
        vec![
            Placement::new("num.hex_to_decimal", 0, 0),
            Placement::new("missing.tool", 1, 0),
        ],
    );
    layouts.insert(
        "obsolete".into(),
        vec![Placement::new("num.hex_to_decimal", 0, 0)],
    );
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts,
        selection: PegboardSelection {
            board_key: Some(" dev ".into()),
            tag: " pure ".into(),
        },
    };

    save_state_to_path(&path, &state).expect("save");
    let loaded = load_state_from_path(&path).expect("load");

    assert_eq!(loaded.boards.len(), 1);
    assert_eq!(
        loaded.selection,
        PegboardSelection {
            board_key: Some("dev".into()),
            tag: "pure".into(),
        },
    );
    let dev = loaded.layouts.get("dev").cloned().unwrap_or_default();
    assert_eq!(dev.len(), 1);
    assert_eq!(dev[0].tool_id, "num.hex_to_decimal");
    assert!(!loaded.layouts.contains_key("obsolete"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn selection_normalizes_missing_board_and_tag_to_all() {
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![Placement::new("num.hex_to_decimal", 0, 0)],
        )]),
        selection: PegboardSelection {
            board_key: Some("missing".into()),
            tag: "__missing__".into(),
        },
    };

    let sanitized = sanitize_state(state);

    assert_eq!(sanitized.selection.board_key, None);
    assert_eq!(sanitized.selection.tag, ALL_TAG);
}

#[test]
fn state_change_rev_grows_per_save_and_is_none_before_save() {
    let root = std::env::temp_dir().join(format!("upeg-pegboard-rev-{}", std::process::id()));
    let path = state_path_from_root(&root);
    let _ = std::fs::remove_dir_all(&root);
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![Placement::new("num.hex_to_decimal", 0, 0)],
        )]),
        selection: PegboardSelection::default(),
    };

    assert_eq!(
        state_change_rev_from_path(&path),
        None,
        "before saving (no file) rev must be absent and the lookup must not create the store"
    );
    assert!(
        !path.exists(),
        "the rev lookup must not create the store file"
    );

    save_state_to_path(&path, &state).expect("save");
    let first = state_change_rev_from_path(&path).expect("rev after first save");

    save_state_to_path(&path, &state).expect("resave");
    let second = state_change_rev_from_path(&path).expect("rev after resave");

    assert!(
        second > first,
        "rev must grow per write: {first} -> {second}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn pegboard_state_path_delegates_to_core_paths() {
    let source = include_str!("../pegboard.rs");
    let delegated = ["upeg_core", "paths", "store_path_in(root)"].join("::");
    assert!(
        source.contains(&delegated),
        "Pegboard state path must use the shared core path builder"
    );
}

#[test]
fn move_placement_pushes_colliding_entry_forward() {
    let mut state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![
                Placement::new("num.hex_to_decimal", 0, 0),
                Placement::new("id.uuid_v7", 1, 0),
            ],
        )]),
        selection: PegboardSelection::default(),
    };
    // Move id.uuid_v7 over num.hex_to_decimal. The moved tool takes the
    // target cell and the colliding tool is pushed forward.
    assert!(move_placement(&mut state, "dev", "id.uuid_v7", 0, 0));
    let dev = state.layouts.get("dev").unwrap();
    let moved = dev.iter().find(|p| p.tool_id == "id.uuid_v7").unwrap();
    let pushed = dev
        .iter()
        .find(|p| p.tool_id == "num.hex_to_decimal")
        .unwrap();
    let (moved_w, _) = placement_size("id.uuid_v7");
    assert_eq!((moved.x, moved.y), (0, 0));
    assert_eq!((pushed.x, pushed.y), (moved_w, 0));
    assert!(
        dev.iter().enumerate().all(|(index, left)| {
            dev.iter()
                .skip(index + 1)
                .all(|right| !rects_overlap(placement_rect(left), placement_rect(right)))
        }),
        "a push-out move must not leave overlapping placements"
    );
}

#[test]
fn remove_tool_everywhere_empties_all_boards() {
    let mut state = PegboardState {
        boards: vec![
            BoardData {
                key: "a".into(),
                title: "A".into(),
                guidance: upeg_core::BoardGuidance::default(),
            },
            BoardData {
                key: "b".into(),
                title: "B".into(),
                guidance: upeg_core::BoardGuidance::default(),
            },
        ],
        layouts: BTreeMap::from([
            ("a".into(), vec![Placement::new("num.hex_to_decimal", 0, 0)]),
            ("b".into(), vec![Placement::new("num.hex_to_decimal", 0, 0)]),
        ]),
        selection: PegboardSelection::default(),
    };
    assert!(remove_tool_everywhere(&mut state, "num.hex_to_decimal"));
    assert!(state.layouts.get("a").unwrap().is_empty());
    assert!(state.layouts.get("b").unwrap().is_empty());
}

#[test]
fn sanitize_moves_collisions_to_first_free_cell() {
    // Two U1 widgets at the same (x, y) mimic a corrupted saved value or
    // a manifest size change that now overlaps. sanitize must keep the
    // first placement at its stored coordinates and move the colliding
    // one to the next row-major free cell.
    let mut layouts = BTreeMap::new();
    layouts.insert(
        "dev".into(),
        vec![
            Placement::new("num.hex_to_decimal", 0, 0),
            Placement::new("id.uuid_v7", 0, 0), // collides
        ],
    );
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts,
        selection: PegboardSelection::default(),
    };
    let sanitized = sanitize_state(state);
    let dev = sanitized.layouts.get("dev").cloned().unwrap_or_default();
    assert_eq!(dev.len(), 2);
    let first = dev
        .iter()
        .find(|p| p.tool_id == "num.hex_to_decimal")
        .unwrap();
    let second = dev.iter().find(|p| p.tool_id == "id.uuid_v7").unwrap();
    assert_eq!(
        (first.x, first.y),
        (0, 0),
        "the first placement must keep its stored coordinates"
    );
    assert_ne!(
        (second.x, second.y),
        (0, 0),
        "the collision must be adjusted"
    );
    // After adjustment the two placements must not overlap.
    assert!(!rects_overlap(
        placement_rect(first),
        placement_rect(second)
    ));
}

#[test]
fn sanitize_moves_out_of_board_items_inside_fixed_board() {
    const BEYOND_BOARD_OFFSET: u16 = 4;

    // When a stored x exceeds BOARD_COLS, sanitize must relocate it to
    // the first valid position inside the fixed 6 columns instead of
    // expanding horizontally.
    let invalid_x = BOARD_COLS.saturating_add(BEYOND_BOARD_OFFSET);
    let mut layouts = BTreeMap::new();
    layouts.insert(
        "dev".into(),
        vec![Placement::new("num.hex_to_decimal", invalid_x, 0)],
    );
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts,
        selection: PegboardSelection::default(),
    };
    let sanitized = sanitize_state(state);
    let dev = sanitized.layouts.get("dev").unwrap();
    assert_eq!(dev.len(), 1);
    let p = &dev[0];
    let (w, _) = placement_size(&p.tool_id);
    assert_eq!(
        (p.x, p.y),
        (0, 0),
        "a badly stored x must be corrected to the first valid position inside the fixed board"
    );
    assert!(p.x.saturating_add(w) <= BOARD_COLS);
}

#[test]
fn placements_for_board_and_tag_return_yx_sorted_pairs() {
    // Deliberately keep the Vec storage order different from (y, x)
    // order so the test fails if sorting is dropped.
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![
                Placement::new("num.hex_to_decimal", 3, 1), // (y=1, x=3)
                Placement::new("id.uuid_v7", 0, 0),         // (y=0, x=0)
            ],
        )]),
        selection: PegboardSelection::default(),
    };
    let pairs = placements_for_board_and_tag_in(&state, Some("dev"), None);
    assert_eq!(pairs.len(), 2);
    assert_eq!(
        pairs[0].0.tool_id, "id.uuid_v7",
        "(y=0, x=0) must sort first"
    );
    assert_eq!(pairs[1].0.tool_id, "num.hex_to_decimal");
    // Each pair must be a (Placement, ToolMeta) with matching tool_id.
    for (placement, tool) in &pairs {
        assert_eq!(placement.tool_id, tool.id);
    }
}

#[test]
fn placements_for_board_and_tag_without_board_return_union() {
    let state = PegboardState {
        boards: vec![
            BoardData {
                key: "a".into(),
                title: "A".into(),
                guidance: upeg_core::BoardGuidance::default(),
            },
            BoardData {
                key: "b".into(),
                title: "B".into(),
                guidance: upeg_core::BoardGuidance::default(),
            },
        ],
        layouts: BTreeMap::from([
            ("a".into(), vec![Placement::new("num.hex_to_decimal", 0, 0)]),
            (
                "b".into(),
                vec![
                    Placement::new("num.hex_to_decimal", 5, 5), // duplicate ids are removed.
                    Placement::new("id.uuid_v7", 0, 0),
                ],
            ),
        ]),
        selection: PegboardSelection::default(),
    };
    let pairs = placements_for_board_and_tag_in(&state, None, None);
    let ids: Vec<&str> = pairs.iter().map(|(_, t)| t.id).collect();
    // Deduplication keeps the first entry, board a's num.hex_to_decimal.
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"num.hex_to_decimal"));
    assert!(ids.contains(&"id.uuid_v7"));
}

#[test]
fn placements_for_board_and_tag_filter_by_tag() {
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![Placement::new("num.hex_to_decimal", 0, 0)],
        )]),
        selection: PegboardSelection::default(),
    };
    // "pure" must match num.hex_to_decimal (registry fact).
    let with_tag = placements_for_board_and_tag_in(&state, Some("dev"), Some("pure"));
    assert!(
        with_tag.iter().all(|(_, t)| t.has_tag("pure")),
        "the filtered set must only contain tools with that tag"
    );
    // A fake tag yields an empty result.
    let none = placements_for_board_and_tag_in(&state, Some("dev"), Some("__no-such-tag"));
    assert!(none.is_empty());
}

/// `board_entries_on_surface_in` is the board enumeration entry point
/// shared by CLI/HTTP/MCP (before the app/board_scope refactor the
/// three surfaces reimplemented it and only the CLI copy was missing
/// the `is_on_surface` filter). `num.hex_to_decimal` defaults to
/// ALL_SURFACES while `memo.scratch` only has GUI_SURFACES
/// (Desktop/Pwa/Ext), so it is absent on Cli/Mcp/Http — exactly the
/// combination the bug reproduced on.
#[test]
fn surface_filtered_board_entries_exclude_pins_off_surface() {
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![
                Placement::new("num.hex_to_decimal", 0, 0),
                Placement::new("memo.scratch", 1, 0),
            ],
        )]),
        selection: PegboardSelection::default(),
    };

    for headless_surface in [Surface::Cli, Surface::Mcp, Surface::Http] {
        let ids: Vec<&str> = board_entries_on_surface_in(&state, "dev", None, headless_surface)
            .iter()
            .map(|(_, tool)| tool.id)
            .collect();
        assert_eq!(
            ids,
            vec!["num.hex_to_decimal"],
            "{headless_surface:?} surface must not contain the GUI-only memo.scratch"
        );
    }

    let desktop_ids: Vec<&str> = board_entries_on_surface_in(&state, "dev", None, Surface::Desktop)
        .iter()
        .map(|(_, tool)| tool.id)
        .collect();
    assert!(
        desktop_ids.contains(&"memo.scratch") && desktop_ids.contains(&"num.hex_to_decimal"),
        "the Desktop surface must see both pins: {desktop_ids:?}"
    );
}

#[test]
fn surface_filtered_board_entries_are_subset_of_unfiltered() {
    let state = default_state();
    let unfiltered = placements_for_board_and_tag_in(&state, Some("dev"), None);
    let filtered = board_entries_on_surface_in(&state, "dev", None, Surface::Cli);

    assert!(filtered.len() <= unfiltered.len());
    for (placement, tool) in &filtered {
        assert!(
            tool.is_on_surface(Surface::Cli),
            "every tool in the filtered result must be on that surface"
        );
        assert!(
            unfiltered
                .iter()
                .any(|(p, t)| p.tool_id == placement.tool_id && t.id == tool.id),
            "the filtered result must be a subset of the unfiltered result"
        );
    }
}

#[test]
fn board_catalog_listing_deduplicates_tool_instances_using_the_call_representative() {
    let first = Placement::new("num.hex_to_decimal", 0, 0).with_args_preset(Some(
        upeg_core::ArgsPreset::parse(r#"{"base":16}"#).expect("preset"),
    ));
    let second = Placement::new("num.hex_to_decimal", 1, 0)
        .with_pin_id(upeg_core::PinId::parse("duplicate-pin").expect("pin id"))
        .with_args_preset(Some(
            upeg_core::ArgsPreset::parse(r#"{"base":2}"#).expect("preset"),
        ));
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([("dev".into(), vec![second, first])]),
        selection: PegboardSelection::default(),
    };

    let listed = board_entries_on_surface_in(&state, "dev", None, Surface::Cli);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0.pin_id.as_str(), "num.hex_to_decimal");
    let representative =
        board_placement_on_surface_in(&state, "dev", "num.hex_to_decimal", Surface::Cli)
            .expect("listed tool must be callable");
    assert_eq!(representative.pin_id, listed[0].0.pin_id);
    assert_eq!(representative.args_preset, listed[0].0.args_preset);
}

/// `board_placement_on_surface_in` (single-tool gate) must reach the
/// same verdict as `board_entries_on_surface_in` (the list) — there
/// must be no "absent from the list but callable" or the reverse.
#[test]
fn single_placement_surface_gate_matches_list_verdict() {
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![
                Placement::new("num.hex_to_decimal", 0, 0),
                Placement::new("memo.scratch", 1, 0),
            ],
        )]),
        selection: PegboardSelection::default(),
    };

    assert!(
        board_placement_on_surface_in(&state, "dev", "num.hex_to_decimal", Surface::Cli).is_some(),
        "a CLI-surface tool must also pass the single gate"
    );
    assert!(
        board_placement_on_surface_in(&state, "dev", "memo.scratch", Surface::Cli).is_none(),
        "a GUI-only tool must not pass the CLI single gate (it was dropped from the list too)"
    );
    assert!(
        board_placement_on_surface_in(&state, "dev", "memo.scratch", Surface::Desktop).is_some(),
        "on the Desktop surface memo.scratch must also pass"
    );
    assert!(
        board_placement_on_surface_in(&state, "dev", "no.such.tool", Surface::Cli).is_none(),
        "an unpinned tool must be absent regardless of surface"
    );
}

#[test]
fn board_and_tag_queries_support_explicit_state() {
    // The structural invariants (ALL_TAG comes first and the
    // has_tag/is_on_board facts are preserved) are verified through the
    // `_in` variants that take the shared default state (default_state)
    // explicitly. Going through `load_state()` directly would make the
    // results depend on the developer's local
    // `~/.upeg/pegboard-state.v2.json` contents (e.g. a stale tool id
    // from before a rename), turning this into an environment-dependent
    // test that passes in CI but fails locally. For the same reason
    // documented on `placements_for_board_and_tag_in` ("so parallel
    // tests do not race over UPEG_HOME") we use explicit state here
    // too.
    let state = default_state();
    let tags = tag_options_for_board_in(&state, Some("dev"));
    assert_eq!(tags.first().map(String::as_str), Some(ALL_TAG));
    let tools = tools_for_board_and_tag_in(&state, Some("dev"), Some("pure"));
    assert!(
        tools
            .iter()
            .all(|tool| tool.has_tag("pure") && tool.is_on_board("dev")),
        "the default dev + pure query must preserve both registry facts"
    );

    // Still, the disk-path wiring itself (the public API really going
    // through `load_state()`) must keep running to catch regressions.
    // Only comparisons that must hold no matter what is on disk are
    // made: the public API result must always match the result of the
    // `_in` variant given "the load_state() result read at the same
    // instant". This comparison does not depend on the local state
    // file's contents (even stale or corrupted ones), so it is
    // environment-independent.
    let disk_state = load_state();
    assert_eq!(
        tag_options_for_board(Some("dev")),
        tag_options_for_board_in(&disk_state, Some("dev")),
        "public tag_options_for_board must not differ from the _in variant beyond going through load_state()"
    );
    let public_ids: Vec<&str> = tools_for_board_and_tag(Some("dev"), Some("pure"))
        .iter()
        .map(|tool| tool.id)
        .collect();
    let in_ids: Vec<&str> = tools_for_board_and_tag_in(&disk_state, Some("dev"), Some("pure"))
        .iter()
        .map(|tool| tool.id)
        .collect();
    assert_eq!(
        public_ids, in_ids,
        "public tools_for_board_and_tag must not differ from the _in variant beyond going through load_state()"
    );
}

// ─── Mutation helpers ────────────────────────────────────
//
// Step 1 of unifying cross-surface edits. Desktop and TUI must run
// every state change through these helpers so that (a) the saved shape
// is always normalized the same way, and (b) future bugs have one
// place to fix instead of two forked copies per surface.

fn fresh_state() -> PegboardState {
    default_state()
}

fn layout_ids_by_position(state: &PegboardState, board: &str) -> Vec<String> {
    let mut placements = state.layouts.get(board).cloned().unwrap_or_default();
    placements.sort_by_key(|p| (p.y, p.x));
    placements.into_iter().map(|p| p.tool_id).collect()
}

#[test]
fn slugify_normalizes_arbitrary_titles() {
    assert_eq!(slugify(""), "board");
    assert_eq!(slugify("   "), "board");
    assert_eq!(slugify("Hello World"), "hello-world");
    assert_eq!(slugify("UPPER_case"), "upper-case");
    assert_eq!(slugify("special!!!chars"), "special-chars");
    assert_eq!(slugify("trailing---"), "trailing");
    assert_eq!(slugify("already-slug"), "already-slug");
}

#[test]
fn add_board_slugifies_and_deduplicates() {
    let mut state = fresh_state();
    let key = add_board(&mut state, "My Board!").expect("must be added");
    assert_eq!(key, "my-board");
    assert!(state.boards.iter().any(|b| b.key == "my-board"));
    assert!(state.layouts.contains_key("my-board"));

    let key2 = add_board(&mut state, "My Board").expect("must be added");
    assert_eq!(key2, "my-board-2");
}

#[test]
fn add_board_rejects_empty_title() {
    let mut state = fresh_state();
    let before = state.boards.len();
    assert!(add_board(&mut state, "").is_none());
    assert!(add_board(&mut state, "   ").is_none());
    assert_eq!(state.boards.len(), before);
}

#[test]
fn remove_board_removes_layout_entries_too() {
    let mut state = fresh_state();
    assert!(remove_board(&mut state, "dev"));
    assert!(!state.boards.iter().any(|b| b.key == "dev"));
    assert!(!state.layouts.contains_key("dev"));
    // Idempotence: removing again does nothing.
    assert!(!remove_board(&mut state, "dev"));
}

#[test]
fn rename_board_changes_title_and_keeps_key() {
    let mut state = fresh_state();
    assert!(rename_board(&mut state, "dev", "Development"));
    let dev = state.boards.iter().find(|b| b.key == "dev").unwrap();
    assert_eq!(dev.title, "Development");
    assert_eq!(dev.key, "dev");
}

#[test]
fn rename_board_rejects_empty_title_or_missing_board() {
    let mut state = fresh_state();
    assert!(!rename_board(&mut state, "dev", ""));
    assert!(!rename_board(&mut state, "dev", "  "));
    assert!(!rename_board(&mut state, "nonexistent", "X"));
}

#[test]
fn pin_tool_is_idempotent_and_rejects_unknown_tool() {
    let mut state = fresh_state();
    let tool_id = "num.hex_to_decimal";
    // Remove the default so the pin transition is clearly observable.
    state
        .layouts
        .entry("dev".into())
        .or_default()
        .retain(|p| p.tool_id != tool_id);
    assert_eq!(pin_tool(&mut state, "dev", tool_id), PinAction::Pinned);
    assert_eq!(pin_tool(&mut state, "dev", tool_id), PinAction::NoOp);
    assert_eq!(
        pin_tool(&mut state, "dev", "no.such.tool.exists"),
        PinAction::NoOp
    );
    assert_eq!(pin_tool(&mut state, "no-board", tool_id), PinAction::NoOp);
}

#[test]
fn unpin_tool_reports_the_outcome() {
    let mut state = fresh_state();
    let tool_id = "num.hex_to_decimal";
    state
        .layouts
        .entry("dev".into())
        .or_default()
        .push(Placement::new(tool_id, 0, 0));
    assert_eq!(unpin_tool(&mut state, "dev", tool_id), PinAction::Unpinned);
    assert_eq!(unpin_tool(&mut state, "dev", tool_id), PinAction::NoOp);
    assert_eq!(unpin_tool(&mut state, "no-board", tool_id), PinAction::NoOp);
}

#[test]
fn pin_color_string_boundary_normalizes_and_keeps_placement() {
    const RAW_COLOR: &str = "#12abef";
    const NORMALIZED_COLOR: &str = "#12ABEF";

    let mut state = fresh_state();
    let tool_id = "num.hex_to_decimal";
    state
        .layouts
        .insert("dev".into(), vec![Placement::new(tool_id, 0, 0)]);

    assert!(set_pin_color_hex(&mut state, "dev", tool_id, Some(RAW_COLOR)).expect("set color"));
    let placement = state
        .layouts
        .get("dev")
        .and_then(|placements| {
            placements
                .iter()
                .find(|placement| placement.tool_id == tool_id)
        })
        .expect("placement kept");
    assert_eq!(
        placement.color.as_ref().map(PinColorHex::as_str),
        Some(NORMALIZED_COLOR)
    );
    assert_eq!((placement.x, placement.y), (0, 0));

    assert!(set_pin_color_hex(&mut state, "dev", tool_id, None).expect("reset color"));
    let placement = state
        .layouts
        .get("dev")
        .and_then(|placements| {
            placements
                .iter()
                .find(|placement| placement.tool_id == tool_id)
        })
        .expect("placement kept after reset");
    assert_eq!(placement.color, None);
}

#[test]
fn pin_color_save_load_round_trips_typed_color() {
    const RAW_COLOR: &str = "#aabbcc";
    const NORMALIZED_COLOR: &str = "#AABBCC";

    let root = std::env::temp_dir().join(format!("upeg-pin-color-{}", std::process::id()));
    let path = state_path_from_root(&root);
    let _ = std::fs::remove_dir_all(&root);
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![
                Placement::new("num.hex_to_decimal", 0, 0)
                    .with_color(Some(PinColorHex::parse(RAW_COLOR).expect("test color"))),
            ],
        )]),
        selection: PegboardSelection::default(),
    };

    save_state_to_path(&path, &state).expect("save color");
    let loaded = load_state_from_path(&path).expect("load color");
    let placement = loaded
        .layouts
        .get("dev")
        .and_then(|placements| placements.first())
        .expect("stored placement");

    assert_eq!(
        placement.color.as_ref().map(PinColorHex::as_str),
        Some(NORMALIZED_COLOR)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn toggle_pin_flips_state() {
    let mut state = fresh_state();
    let tool_id = "num.hex_to_decimal";
    state
        .layouts
        .entry("dev".into())
        .or_default()
        .retain(|p| p.tool_id != tool_id);
    assert_eq!(toggle_pin(&mut state, "dev", tool_id), PinAction::Pinned);
    assert_eq!(toggle_pin(&mut state, "dev", tool_id), PinAction::Unpinned);
    assert_eq!(toggle_pin(&mut state, "dev", tool_id), PinAction::Pinned);
    assert_eq!(
        toggle_pin(&mut state, "dev", "no.such.tool"),
        PinAction::NoOp
    );
}

#[test]
fn move_layout_entry_moves_within_bounds() {
    let mut state = fresh_state();
    let a = "convert.base64_encode";
    let b = "convert.base64_decode";
    let c = "convert.base32_encode";
    state.layouts.insert(
        "dev".into(),
        vec![
            Placement::new(a, 0, 0),
            Placement::new(b, 1, 0),
            Placement::new(c, 2, 0),
        ],
    );
    assert!(move_layout_entry(&mut state, "dev", b, Direction::Prev));
    assert_eq!(layout_ids_by_position(&state, "dev"), vec![b, a, c]);
    assert!(move_layout_entry(&mut state, "dev", b, Direction::Next));
    assert_eq!(layout_ids_by_position(&state, "dev"), vec![a, b, c]);
    // Moving past the start does nothing.
    assert!(!move_layout_entry(&mut state, "dev", a, Direction::Prev));
    // Moving past the end does nothing.
    assert!(!move_layout_entry(&mut state, "dev", c, Direction::Next));
    // Unknown tool/board does nothing.
    assert!(!move_layout_entry(
        &mut state,
        "dev",
        "ghost",
        Direction::Prev
    ));
    assert!(!move_layout_entry(
        &mut state,
        "no-board",
        a,
        Direction::Next
    ));
}

#[test]
fn swap_layout_pair_swaps_or_does_nothing() {
    let mut state = fresh_state();
    let a = "convert.base64_encode";
    let b = "convert.base64_decode";
    let c = "convert.base32_encode";
    state.layouts.insert(
        "dev".into(),
        vec![
            Placement::new(a, 0, 0),
            Placement::new(b, 1, 0),
            Placement::new(c, 2, 0),
        ],
    );
    assert!(swap_layout_pair(&mut state, "dev", a, c));
    assert_eq!(layout_ids_by_position(&state, "dev"), vec![c, b, a]);
    // The same id does nothing.
    assert!(!swap_layout_pair(&mut state, "dev", b, b));
    // An unknown id pair does nothing.
    assert!(!swap_layout_pair(&mut state, "dev", "ghost", a));
    assert!(!swap_layout_pair(&mut state, "no-board", a, c));
}

// Cross-surface round trip: an edit applied through the TUI mutation
// surface lands in the same on-disk shape the desktop UI reads.
// save_state_to_path / load_state_from_path are used so the test does
// not have to change the process env.
#[test]
fn tui_style_edit_round_trips_through_desktop_load_path() {
    let root = std::env::temp_dir().join(format!("upeg-pegboard-roundtrip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);

    // TUI session: start from the defaults, add a board, pin a known
    // registry tool, then reorder one slot back.
    let mut tui_state = default_state();
    let new_key = add_board(&mut tui_state, "Project Alpha").expect("add_board");
    assert_eq!(new_key, "project-alpha");
    let tool_id = "num.hex_to_decimal";
    assert_eq!(
        pin_tool(&mut tui_state, &new_key, tool_id),
        PinAction::Pinned
    );
    // Pin a second tool so move_layout_entry has a neighbor to swap.
    let second_id = "convert.base64_encode";
    assert_eq!(
        pin_tool(&mut tui_state, &new_key, second_id),
        PinAction::Pinned
    );
    assert!(move_layout_entry(
        &mut tui_state,
        &new_key,
        second_id,
        Direction::Prev,
    ));
    save_state_to_path(&path, &tui_state).expect("TUI save_state");

    // Desktop session: a fresh load must read everything TUI wrote.
    let desktop_state = load_state_from_path(&path).expect("desktop load_state");
    assert!(
        desktop_state.boards.iter().any(|b| b.key == new_key),
        "desktop must see the board TUI added"
    );
    assert_eq!(
        layout_ids_by_position(&desktop_state, &new_key),
        vec![second_id.to_string(), tool_id.to_string()],
        "desktop must see the same pin order TUI committed"
    );

    // A desktop-side edit comes back to TUI the same way.
    let mut desktop_state = desktop_state;
    assert!(rename_board(&mut desktop_state, &new_key, "Project Beta",));
    assert_eq!(
        toggle_pin(&mut desktop_state, &new_key, tool_id),
        PinAction::Unpinned,
        "the desktop unpin must be registered"
    );
    save_state_to_path(&path, &desktop_state).expect("desktop save_state");

    let tui_state_again = load_state_from_path(&path).expect("TUI reload");
    let renamed = tui_state_again
        .boards
        .iter()
        .find(|b| b.key == new_key)
        .expect("the board key must survive the rename");
    assert_eq!(renamed.title, "Project Beta");
    assert_eq!(
        layout_ids_by_position(&tui_state_again, &new_key),
        vec![second_id.to_string()],
        "TUI must see the desktop unpin"
    );

    let _ = std::fs::remove_dir_all(&root);
}
