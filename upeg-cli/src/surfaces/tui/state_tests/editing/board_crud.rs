use super::*;

#[test]
fn delete_key_does_nothing_on_last_remaining_board() {
    // Last-board sync guard: the TUI must refuse deletion when only one
    // board remains. Otherwise sources::sanitize_state quietly restores
    // default_boards() on disk while the in-memory cache stays empty,
    // leaving the user staring at an empty pegboard until restart.
    let mut s = fresh();
    s.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "solo".into(),
        title: "Solo".into(),
    }];
    s.layouts.clear();
    s.layouts.insert("solo".into(), Vec::new());
    s.filters.select_board("solo");
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('D'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(
        matches!(s.view, View::List),
        "D on the last board must not open ConfirmDeleteBoard"
    );
    assert_eq!(s.boards.len(), 1, "the board list must not change");
    assert!(
        s.boards.iter().any(|b| b.key == "solo"),
        "the only board must remain intact"
    );
}

#[test]
fn delete_key_opens_confirm_dialog_when_multiple_boards_exist() {
    // Normal-path regression test for the guard: with multiple boards
    // the confirm overlay must still open. The guard must not
    // accidentally block the common case.
    let mut s = fresh();
    s.filters.select_board("dev");
    assert!(
        s.boards.len() > 1,
        "fresh() seeds default_boards() with 3 entries"
    );
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('D'), t.as_slice());
    assert!(
        matches!(s.view, View::ConfirmDeleteBoard { .. }),
        "with multiple boards, D must open the confirm overlay"
    );
}

#[test]
fn delete_key_opens_confirm_dialog_without_edit_mode() {
    // Modeless: now that the edit toggle is gone, `D` opens the
    // delete-confirm overlay immediately when a concrete board is
    // selected and multiple boards exist — no separate mode entry.
    // (The confirm overlay itself prevents accidental termination.)
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('D'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::ConfirmDeleteBoard { .. }));
}

#[test]
fn delete_key_does_nothing_on_all_filter() {
    let mut s = fresh();
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('D'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::List));
}

#[test]
fn delete_key_opens_board_delete_confirm_overlay() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('D'), t.as_slice());
    match &s.view {
        View::ConfirmDeleteBoard { key, title } => {
            assert_eq!(key, "dev");
            assert_eq!(title, "Dev");
        }
        other => panic!("expected ConfirmDeleteBoard but got {other:?}"),
    }
}

#[test]
fn yes_on_delete_confirm_removes_board_and_returns_to_first_remaining() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('D'), t.as_slice());
    let effect = handle_key(&mut s, Key::Char('y'), t.as_slice());
    assert_eq!(effect, Effect::SavePegboard);
    assert!(!s.boards.iter().any(|b| b.key == "dev"));
    assert!(!s.layouts.contains_key("dev"));
    let new_filter = s.filters.board.as_deref();
    assert!(
        new_filter.is_some(),
        "the filter must return to the first remaining board, not vanish to None"
    );
    assert_ne!(new_filter, Some("dev"));
    assert!(matches!(s.view, View::List));
}

#[test]
fn no_on_delete_confirm_cancels_without_state_change() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('D'), t.as_slice());
    let before_boards = s.boards.clone();
    let before_layouts = s.layouts.clone();
    let effect = handle_key(&mut s, Key::Char('n'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::List));
    assert_eq!(s.boards, before_boards);
    assert_eq!(s.layouts, before_layouts);
}

#[test]
fn esc_on_delete_confirm_cancels() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('D'), t.as_slice());
    let before_boards = s.boards.clone();
    handle_key(&mut s, Key::Esc, t.as_slice());
    assert!(matches!(s.view, View::List));
    assert_eq!(s.boards, before_boards);
}

#[test]
fn rename_key_opens_editor_without_edit_mode() {
    // Modeless: with a concrete board selected, `R` opens the rename
    // editor immediately — no edit-mode entry step.
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('R'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(
        s.view,
        View::BoardEditor {
            mode: crate::surfaces::tui::model::BoardEditMode::Rename { .. },
            ..
        }
    ));
}

#[test]
fn rename_key_does_nothing_on_all_filter() {
    let mut s = fresh();
    // Leaving filter.board as None means "all".
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('R'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::List));
}

#[test]
fn rename_key_opens_board_editor_prefilled_with_current_title() {
    use crate::surfaces::tui::model::BoardEditMode;
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('R'), t.as_slice());
    match &s.view {
        View::BoardEditor {
            mode:
                BoardEditMode::Rename {
                    key,
                    original_title,
                },
            buffer,
        } => {
            assert_eq!(key, "dev");
            assert_eq!(original_title, "Dev");
            assert_eq!(
                buffer, "Dev",
                "the rename buffer must be pre-filled with the current title"
            );
        }
        other => panic!("expected BoardEditor::Rename but got {other:?}"),
    }
}

#[test]
fn board_editor_rename_commit_changes_only_title_and_keeps_key() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('R'), t.as_slice());
    // Replace the buffer contents.
    for _ in 0..10 {
        handle_key(&mut s, Key::Backspace, t.as_slice());
    }
    for ch in "Development".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    let effect = handle_key(&mut s, Key::Enter, t.as_slice());
    assert_eq!(effect, Effect::SavePegboard);
    let dev = s
        .boards
        .iter()
        .find(|b| b.key == "dev")
        .expect("the dev board must still exist");
    assert_eq!(dev.title, "Development");
    assert_eq!(
        s.filters.board.as_deref(),
        Some("dev"),
        "the filter must stay attached to the same key"
    );
}

#[test]
fn new_board_key_opens_add_editor_even_with_board_filter() {
    // Modeless: `n` always opens the add editor regardless of board
    // filter (board creation is a board-level operation and requires no
    // focus/filter).
    use crate::surfaces::tui::model::BoardEditMode;
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('n'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(
        s.view,
        View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            ..
        }
    ));
}

#[test]
fn new_board_key_opens_add_editor_without_edit_mode() {
    use crate::surfaces::tui::model::BoardEditMode;
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    match &s.view {
        View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            buffer,
        } => assert!(buffer.is_empty()),
        other => panic!("expected BoardEditor::AddBoard but got {other:?}"),
    }
}

#[test]
fn typed_chars_accumulate_in_board_editor_buffer() {
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    for ch in "test".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    if let View::BoardEditor { buffer, .. } = &s.view {
        assert_eq!(buffer, "test");
    } else {
        panic!("expected the BoardEditor view but got {:?}", s.view);
    }
}

#[test]
fn backspace_in_board_editor_removes_last_buffer_char() {
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    for ch in "abc".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    handle_key(&mut s, Key::Backspace, t.as_slice());
    if let View::BoardEditor { buffer, .. } = &s.view {
        assert_eq!(buffer, "ab");
    } else {
        panic!("expected the BoardEditor view");
    }
}

#[test]
fn enter_in_board_editor_commits_new_board_and_moves_filter() {
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    for ch in "Project Alpha".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    let effect = handle_key(&mut s, Key::Enter, t.as_slice());
    assert_eq!(effect, Effect::SavePegboard);
    assert!(matches!(s.view, View::List));
    assert_eq!(
        s.filters.board.as_deref(),
        Some("project-alpha"),
        "the filter must follow to the newly added board"
    );
    assert!(
        s.boards.iter().any(|b| b.key == "project-alpha"),
        "the new board must appear in the cache"
    );
}

#[test]
fn enter_with_empty_buffer_in_board_editor_does_nothing() {
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    let effect = handle_key(&mut s, Key::Enter, t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::BoardEditor { .. }));
}

#[test]
fn esc_in_board_editor_cancels_and_returns_to_list() {
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    for ch in "discard".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    let before_boards = s.boards.clone();
    handle_key(&mut s, Key::Esc, t.as_slice());
    assert!(matches!(s.view, View::List));
    assert_eq!(s.boards, before_boards, "ESC must not save the buffer");
}
