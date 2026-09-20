use super::*;
use upeg_core::{ColSpan, PinSpan, RowSpan};

/// Same stage as the movement tests in movement.rs: registers the
/// fixture tools on a single dev board and starts at the given
/// placements.
fn resize_stage(state: &mut State, placements: Vec<Placement>) -> Vec<&'static ToolMeta> {
    state.filters.select_board("dev");
    state.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "dev".into(),
        title: "Dev".into(),
    }];
    for tool in fixture_tools() {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    state.layouts.insert("dev".into(), placements);
    upeg_sources::pegboard::tools_for_board_and_tag_in(
        &state.pegboard_snapshot(),
        Some("dev"),
        None,
    )
}

fn test_span(cols: u16, rows: u16) -> PinSpan {
    PinSpan::new(
        ColSpan::new(cols).expect("test cols"),
        RowSpan::new(rows).expect("test rows"),
    )
}

/// Reads the target pin's span override from the committed dev layout.
/// `None` means "no override (manifest size)".
fn committed_span(state: &State, tool_id: &str) -> Option<(u16, u16)> {
    state
        .layouts
        .get("dev")
        .and_then(|placements| placements.iter().find(|p| p.tool_id == tool_id))
        .expect("the committed placement must remain")
        .span
        .map(PinSpan::grid_span)
}

#[test]
fn resize_keys_change_only_preview_and_enter_commits_span() {
    let mut s = fresh();
    let tools = resize_stage(
        &mut s,
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    s.cursor = 0;
    let committed_before = s.layouts.get("dev").cloned().unwrap();

    assert_eq!(handle_key(&mut s, Key::Char('e'), &tools), Effect::None);
    let resize = s.resize_mode.as_ref().expect("e opens resize mode");
    assert_eq!(
        (resize.cols, resize.rows),
        (1, 1),
        "the starting size is the committed placement's effective_size"
    );

    assert_eq!(handle_key(&mut s, Key::Right, &tools), Effect::None);
    let resize = s.resize_mode.as_ref().expect("resize mode active");
    assert_eq!((resize.cols, resize.rows), (2, 1));
    assert_eq!(
        resize
            .preview
            .iter()
            .find(|p| p.tool_id == "test.simple")
            .and_then(|p| p.span)
            .map(PinSpan::grid_span),
        Some((2, 1)),
        "the preview's target pin must carry a span override"
    );
    assert_eq!(
        s.layouts.get("dev").cloned().unwrap(),
        committed_before,
        "arrow-key previews must not commit before Enter"
    );

    assert_eq!(handle_key(&mut s, Key::Enter, &tools), Effect::SavePegboard);
    assert!(s.resize_mode.is_none(), "Enter must close resize mode");
    assert_eq!(
        committed_span(&s, "test.simple"),
        Some((2, 1)),
        "the committed layout must store span=Some(2x1)"
    );
}

#[test]
fn esc_in_resize_mode_discards_preview() {
    let mut s = fresh();
    let tools = resize_stage(
        &mut s,
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    s.cursor = 0;
    let before = s.layouts.clone();

    handle_key(&mut s, Key::Char('e'), &tools);
    handle_key(&mut s, Key::Right, &tools);
    let effect = handle_key(&mut s, Key::Esc, &tools);

    assert_eq!(effect, Effect::None);
    assert!(s.resize_mode.is_none());
    assert_eq!(s.layouts, before, "Esc must discard the preview");
}

#[test]
fn span_changes_past_limits_are_ignored() {
    let mut s = fresh();
    let tools = resize_stage(&mut s, vec![Placement::new("test.simple", 0, 0)]);
    s.cursor = 0;

    handle_key(&mut s, Key::Char('e'), &tools);
    // -1 at cols 1 and -1 at rows 1 are rejected by newtype validation
    // and ignored.
    handle_key(&mut s, Key::Left, &tools);
    handle_key(&mut s, Key::Up, &tools);
    let resize = s.resize_mode.as_ref().expect("resize mode active");
    assert_eq!(
        (resize.cols, resize.rows),
        (1, 1),
        "ignored below the lower bound"
    );
    assert_eq!(
        resize.preview, resize.base,
        "an ignored key must not change the preview"
    );
    // A no-change commit has no save effect.
    assert_eq!(
        handle_key(&mut s, Key::Enter, &tools),
        Effect::None,
        "when the preview equals base, Enter does not save"
    );
    assert!(s.resize_mode.is_none());

    // cols is capped at BOARD_COLS(6).
    handle_key(&mut s, Key::Char('e'), &tools);
    for _ in 0..usize::from(upeg_core::BOARD_COLS) {
        handle_key(&mut s, Key::Right, &tools);
    }
    let resize = s.resize_mode.as_ref().expect("resize mode active");
    assert_eq!(
        resize.cols,
        upeg_core::BOARD_COLS,
        "+1 at 6 is ignored and stays at the cap"
    );
}

#[test]
fn reset_key_returns_to_manifest_size_and_clears_span_on_commit() {
    let mut s = fresh();
    let tools = resize_stage(
        &mut s,
        vec![Placement::new("test.simple", 0, 0).with_span(Some(test_span(2, 2)))],
    );
    s.cursor = 0;

    handle_key(&mut s, Key::Char('e'), &tools);
    let resize = s.resize_mode.as_ref().expect("resize mode active");
    assert_eq!(
        (resize.cols, resize.rows),
        (2, 2),
        "the committed span override decides the starting size"
    );

    handle_key(&mut s, Key::Char('0'), &tools);
    let resize = s.resize_mode.as_ref().expect("resize mode active");
    assert_eq!(
        (resize.cols, resize.rows),
        (1, 1),
        "0 returns to the manifest placement_size"
    );

    assert_eq!(handle_key(&mut s, Key::Enter, &tools), Effect::SavePegboard);
    assert_eq!(
        committed_span(&s, "test.simple"),
        None,
        "committing at manifest size stores span=None instead of a redundant override"
    );
}

#[test]
fn growing_span_pushes_neighbors_in_preview() {
    let mut s = fresh();
    let tools = resize_stage(
        &mut s,
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    s.cursor = 0;

    handle_key(&mut s, Key::Char('e'), &tools);
    handle_key(&mut s, Key::Right, &tools);
    let preview = s
        .resize_mode
        .as_ref()
        .expect("resize mode active")
        .preview
        .clone();
    assert_eq!(
        preview
            .iter()
            .find(|p| p.tool_id == "test.with_input")
            .map(|p| (p.x, p.y)),
        Some((2, 0)),
        "the colliding neighbor must be pushed away by place_tool_with_push"
    );

    handle_key(&mut s, Key::Enter, &tools);
    assert_eq!(
        layout_ids_by_position(&s, "dev"),
        vec!["test.simple", "test.with_input"],
        "the committed order must follow the pushed coordinates"
    );
    let after = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    let expected = after
        .iter()
        .position(|t| t.id == "test.simple")
        .expect("the resized tool must remain on the board");
    assert_eq!(
        s.cursor, expected,
        "after commit the cursor must point back at the resized tool"
    );
}

#[test]
fn mouse_down_cancels_resize_mode_for_keyboard_consistency() {
    // Same contract as move_mode's mouse-cancel regression test: a Down
    // click closes the hidden resize preview first so Enter cannot
    // commit the wrong thing.
    let mut s = fresh();
    let tools = resize_stage(
        &mut s,
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    s.cursor = 0;
    handle_key(&mut s, Key::Char('e'), &tools);
    assert!(s.resize_mode.is_some(), "precondition: e starts a resize");

    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);
    let mouse = Mouse {
        column: layout.boards.x + 1,
        row: layout.boards.y + 1,
        kind: MouseKind::Pointer(PointerPhase::Down),
    };
    let _ = handle_mouse(&mut s, mouse, &tools, area);

    assert!(
        s.resize_mode.is_none(),
        "mouse Down must cancel the resize preview"
    );
}
