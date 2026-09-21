use super::*;

#[test]
fn bracket_keys_do_nothing_without_focus() {
    // Modeless: the edit gate is gone but `[`/`]` still require a
    // focused pin. With no visible tools (empty grid) the reorder is
    // silently ignored.
    let mut s = fresh();
    s.filters.select_board("dev");
    s.cursor = 0;
    let before = s.layouts.get("dev").cloned().unwrap_or_default();
    assert_eq!(handle_key(&mut s, Key::Char(']'), &[]), Effect::None);
    assert_eq!(s.layouts.get("dev").cloned().unwrap_or_default(), before);
}

#[test]
fn close_bracket_moves_cursor_tool_later() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    assert!(tools.len() >= 2);
    let first_id = tools[0].id;
    let second_id = tools[1].id;
    s.cursor = 0;
    let effect = handle_key(&mut s, Key::Char(']'), &tools);
    assert_eq!(effect, Effect::SavePegboard);
    let layout = layout_ids_by_position(&s, "dev");
    assert_eq!(layout[0], second_id);
    assert_eq!(layout[1], first_id);
    assert_eq!(s.cursor, 1, "the cursor must follow the moved tool");
}

#[test]
fn open_bracket_at_first_cell_does_nothing() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    s.cursor = 0;
    let before = s.layouts.get("dev").cloned().unwrap_or_default();
    let effect = handle_key(&mut s, Key::Char('['), &tools);
    assert_eq!(effect, Effect::None);
    assert_eq!(s.layouts.get("dev").cloned().unwrap_or_default(), before);
    assert_eq!(s.cursor, 0);
}

#[test]
fn close_bracket_at_last_cell_does_nothing() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    let last_idx = tools.len() - 1;
    s.cursor = last_idx;
    let before = s.layouts.get("dev").cloned().unwrap_or_default();
    let effect = handle_key(&mut s, Key::Char(']'), &tools);
    assert_eq!(effect, Effect::None);
    assert_eq!(s.layouts.get("dev").cloned().unwrap_or_default(), before);
    assert_eq!(s.cursor, last_idx);
}

#[test]
fn move_key_starts_push_preview_and_enter_commits() {
    let mut s = fresh();
    s.filters.select_board("dev");
    s.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "dev".into(),
        title: "Dev".into(),
    }];
    let fixtures = fixture_tools();
    for tool in fixtures {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    s.layouts.insert(
        "dev".into(),
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
            Placement::new("test.third", 2, 0),
        ],
    );
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    s.cursor = 2;
    let committed_before = s.layouts.get("dev").cloned().unwrap();

    assert_eq!(handle_key(&mut s, Key::Char('m'), &tools), Effect::None);
    assert!(s.move_mode.is_some(), "m must start coordinate-move mode");
    assert_eq!(handle_key(&mut s, Key::Left, &tools), Effect::None);

    let preview = s
        .move_mode
        .as_ref()
        .expect("move mode active")
        .preview
        .clone();
    assert_eq!(
        preview
            .iter()
            .find(|p| p.tool_id == "test.third")
            .map(|p| (p.x, p.y)),
        Some((1, 0)),
        "the moved widget must occupy the target coordinates"
    );
    assert_eq!(
        preview
            .iter()
            .find(|p| p.tool_id == "test.with_input")
            .map(|p| (p.x, p.y)),
        Some((2, 0)),
        "the colliding widget must be pushed forward in the preview"
    );
    assert_eq!(
        s.layouts.get("dev").cloned().unwrap(),
        committed_before,
        "arrow-key previews must not commit before Enter"
    );

    assert_eq!(handle_key(&mut s, Key::Enter, &tools), Effect::SavePegboard);
    assert!(s.move_mode.is_none(), "Enter must close move mode");
    assert_eq!(
        layout_ids_by_position(&s, "dev"),
        vec!["test.simple", "test.third", "test.with_input"],
        "the committed order must follow the pushed coordinates"
    );
}

#[test]
fn mouse_down_cancels_move_mode_for_keyboard_consistency() {
    // Codex regression: with the coordinate-move preview armed via m, a
    // mouse click on board/tag/grid left the keyboard locked in move
    // mode and Enter could commit the hidden preview. Mouse Down now
    // closes move mode first and returns to normal input.
    let mut s = fresh();
    s.filters.select_board("dev");
    s.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "dev".into(),
        title: "Dev".into(),
    }];
    for tool in fixture_tools() {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    s.layouts.insert(
        "dev".into(),
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    s.cursor = 1;
    handle_key(&mut s, Key::Char('m'), &tools);
    assert!(
        s.move_mode.is_some(),
        "precondition: m starts a coordinate move"
    );

    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);
    // Clicking the board bar commonly jumps to another board/filter.
    let mouse = Mouse {
        column: layout.boards.x + 1,
        row: layout.boards.y + 1,
        kind: MouseKind::Pointer(PointerPhase::Down),
    };
    let _ = handle_mouse(&mut s, mouse, &tools, area);

    assert!(
        s.move_mode.is_none(),
        "mouse Down must cancel the coordinate move to stay consistent with the keyboard"
    );
}

#[test]
fn cursor_follows_reordered_position_after_commit() {
    // Codex regression: commit_move updated only the layout and left
    // cursor alone, so the next keypress applied to the wrong tool.
    // After the push reorders, we find the new (y, x) position and
    // re-pin cursor.
    let mut s = fresh();
    s.filters.select_board("dev");
    s.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "dev".into(),
        title: "Dev".into(),
    }];
    for tool in fixture_tools() {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    s.layouts.insert(
        "dev".into(),
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
            Placement::new("test.third", 2, 0),
        ],
    );
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    s.cursor = 2; // test.third

    handle_key(&mut s, Key::Char('m'), &tools);
    handle_key(&mut s, Key::Left, &tools);
    let effect = handle_key(&mut s, Key::Enter, &tools);
    assert_eq!(effect, Effect::SavePegboard);

    let after = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    let expected = after
        .iter()
        .position(|t| t.id == "test.third")
        .expect("the moved tool must remain on the board");
    assert_eq!(
        s.cursor, expected,
        "after commit the cursor must point at the moved tool's new index"
    );
}

#[test]
fn esc_in_move_mode_discards_preview() {
    let mut s = fresh();
    s.filters.select_board("dev");
    s.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "dev".into(),
        title: "Dev".into(),
    }];
    let fixtures = fixture_tools();
    for tool in fixtures {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    s.layouts.insert(
        "dev".into(),
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    s.cursor = 1;
    let before = s.layouts.clone();

    handle_key(&mut s, Key::Char('m'), &tools);
    handle_key(&mut s, Key::Left, &tools);
    let effect = handle_key(&mut s, Key::Esc, &tools);

    assert_eq!(effect, Effect::None);
    assert!(s.move_mode.is_none());
    assert_eq!(s.layouts, before, "Esc must discard the preview");
}

#[test]
fn pin_key_does_nothing_without_focus() {
    // Modeless: `p` requires a focused pin. With no visible tools it is
    // silently ignored — focus is the only gate, not edit mode.
    let mut s = fresh();
    s.filters.select_board("dev");
    s.cursor = 0;
    let before = s.layouts.get("dev").cloned().unwrap_or_default();
    let effect = handle_key(&mut s, Key::Char('p'), &[]);
    assert_eq!(effect, Effect::None);
    assert_eq!(s.layouts.get("dev").cloned().unwrap_or_default(), before);
}

#[test]
fn pin_key_does_nothing_on_all_filter() {
    // The "all" board filter is ambiguous about which layout to change.
    // The operation must refuse so the user picks a concrete board
    // first.
    let mut s = fresh();
    s.cursor = 0;
    let tools = s.visible_tools();
    let before_layouts = s.layouts.clone();
    let effect = handle_key(&mut s, Key::Char('p'), &tools);
    assert_eq!(effect, Effect::None);
    assert_eq!(s.layouts, before_layouts);
}

#[test]
fn pin_key_unpins_currently_pinned_tool() {
    // Toggle semantics: the grid shows only pinned tools, so the first
    // `p` press on the item under the cursor unpins that tool.
    let mut s = fresh();
    s.filters.select_board("dev");
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    assert!(
        !tools.is_empty(),
        "the dev board must start with pinned tools"
    );
    let target_id = tools[0].id;
    s.cursor = 0;

    let effect = handle_key(&mut s, Key::Char('p'), &tools);

    assert_eq!(effect, Effect::SavePegboard);
    assert!(
        !layout_contains(&s, "dev", target_id),
        "the first p press must unpin the tool under the cursor"
    );
}

#[test]
fn pin_key_clamps_cursor_inward_when_unpin_shrinks_grid() {
    // Unpinning the last row must move the cursor up to the new last
    // row; otherwise the next render indexes past the end of the
    // layout.
    let mut s = fresh();
    s.filters.select_board("dev");
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    assert!(
        tools.len() >= 2,
        "this test needs at least two pinned dev tools"
    );
    s.cursor = tools.len() - 1;

    handle_key(&mut s, Key::Char('p'), &tools);

    let new_len = s.layouts.get("dev").map(Vec::len).unwrap_or_default();
    assert!(
        s.cursor < new_len,
        "the cursor must be clamped inside the grid"
    );
}
