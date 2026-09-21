use super::*;

mod board_crud;
mod i18n_keys;
mod movement;
mod pin_color_editor;
mod resize;
mod tool_picker;

#[test]
fn full_board_flow_performs_add_pin_reorder_rename_delete_in_sequence() {
    // The end-to-end flow that pre-PR coverage missed. Verifies each
    // key performs its role in order and the accumulated state matches
    // what a real user would see step by step. Modeless: board
    // management keys work immediately without an edit toggle.
    use crate::surfaces::tui::model::BoardEditMode;
    let mut s = fresh();
    let t = fixture_tools();
    let tools = t.as_slice();

    // 1. `n`, `Alpha`, Enter creates a new board and the filter follows.
    handle_key(&mut s, Key::Char('n'), tools);
    assert!(matches!(
        s.view,
        View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            ..
        }
    ));
    for ch in "Alpha".chars() {
        handle_key(&mut s, Key::Char(ch), tools);
    }
    let effect_add = handle_key(&mut s, Key::Enter, tools);
    assert_eq!(effect_add, Effect::SavePegboard);
    assert_eq!(
        s.filters.board.as_deref(),
        Some("alpha"),
        "step 2: the filter must follow to the new board"
    );
    assert!(s.boards.iter().any(|b| b.key == "alpha"));

    // 3. `a` opens the ToolPicker; type a query, pin with Enter, close
    // with ESC.
    handle_key(&mut s, Key::Char('a'), tools);
    let needle = "num.hex_to_decimal";
    let real_tools_for_dispatch = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("alpha"),
        None,
    );
    let _ = real_tools_for_dispatch;
    for ch in needle.chars() {
        handle_key(&mut s, Key::Char(ch), tools);
    }
    let effect_pin = handle_key(&mut s, Key::Enter, tools);
    assert_eq!(effect_pin, Effect::SavePegboard);
    handle_key(&mut s, Key::Esc, tools);
    assert!(matches!(s.view, View::List));
    assert!(
        layout_contains(&s, "alpha", needle),
        "step 3: the tool must be pinned on the alpha board"
    );

    // 4. Pin a second tool so there is a neighbor to reorder, then
    // press `[` / `]`.
    handle_key(&mut s, Key::Char('a'), tools);
    let second = "convert.base64_encode";
    for ch in second.chars() {
        handle_key(&mut s, Key::Char(ch), tools);
    }
    handle_key(&mut s, Key::Enter, tools);
    handle_key(&mut s, Key::Esc, tools);
    // Refresh the visible-tools snapshot the same way the runtime loop
    // does.
    let visible = s.visible_tools();
    assert!(visible.len() >= 2);
    s.cursor = 0;
    let effect_swap = handle_key(&mut s, Key::Char(']'), &visible);
    assert_eq!(effect_swap, Effect::SavePegboard);
    let layout = s.layouts.get("alpha").cloned().unwrap_or_default();
    assert_eq!(
        layout.len(),
        2,
        "step 4: there must still be two pinned tools"
    );
    assert_eq!(
        s.cursor, 1,
        "step 4: after `]` the cursor must follow the moved tool"
    );

    // 5. `R` rename pre-fills the buffer; we edit it to "Beta".
    handle_key(&mut s, Key::Char('R'), &visible);
    for _ in 0..20 {
        handle_key(&mut s, Key::Backspace, &visible);
    }
    for ch in "Beta".chars() {
        handle_key(&mut s, Key::Char(ch), &visible);
    }
    let effect_rename = handle_key(&mut s, Key::Enter, &visible);
    assert_eq!(effect_rename, Effect::SavePegboard);
    let alpha = s
        .boards
        .iter()
        .find(|b| b.key == "alpha")
        .expect("the key must be preserved");
    assert_eq!(
        alpha.title, "Beta",
        "step 5: the title changes while the key stays stable"
    );

    // 6. `D` then `y` deletes the board and returns the filter to a
    // fallback board.
    handle_key(&mut s, Key::Char('D'), &visible);
    assert!(matches!(s.view, View::ConfirmDeleteBoard { .. }));
    let effect_delete = handle_key(&mut s, Key::Char('y'), &visible);
    assert_eq!(effect_delete, Effect::SavePegboard);
    assert!(!s.boards.iter().any(|b| b.key == "alpha"));
    assert!(
        s.filters.board.is_some(),
        "step 6: the filter must return to a remaining board, not None"
    );
}

#[test]
fn edit_flow_emits_save_effect_only_for_committed_changes() {
    // Effect contract: Effect::SavePegboard must fire only when sources
    // state actually changes. Typing into the BoardEditor buffer or
    // pressing Backspace stays in memory only. Pressing Enter on an
    // empty buffer is structurally a no-op. Before this PR a later
    // refactor risked writing to disk on every keypress.
    let mut s = fresh();
    let t = fixture_tools();
    let tools = t.as_slice();

    let mut save_count = 0_usize;
    let mut log = |effect: Effect| {
        if matches!(effect, Effect::SavePegboard) {
            save_count += 1;
        }
    };

    log(handle_key(&mut s, Key::Char('n'), tools)); // open the editor
    for ch in "Beta".chars() {
        log(handle_key(&mut s, Key::Char(ch), tools)); // typing does not save.
    }
    log(handle_key(&mut s, Key::Backspace, tools)); // backspace does not save either.
    log(handle_key(&mut s, Key::Enter, tools)); // commit saves.
    assert_eq!(
        save_count, 1,
        "only the Enter commit may emit Effect::SavePegboard"
    );

    // Committing an empty buffer must not emit a save either.
    handle_key(&mut s, Key::Char('n'), tools);
    let effect_blank = handle_key(&mut s, Key::Enter, tools);
    assert_eq!(
        effect_blank,
        Effect::None,
        "Enter on an empty buffer must not commit or save"
    );
}

#[test]
fn e_does_not_start_resize_without_a_selected_board() {
    // `e` is now resolved as StartResize at board scope (symmetric with
    // `m`/StartMove). But under the "all" board filter it is ambiguous
    // which layout to change, so start_resize silently refuses — the
    // same gate as p/m.
    let mut s = fresh();
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('e'), t.as_slice());
    assert_eq!(effect, Effect::None, "`e` must produce no effect");
    assert!(
        s.resize_mode.is_none(),
        "resize is impossible without a selected board"
    );
    assert!(matches!(s.view, View::List), "`e` must not change the view");
    handle_key(&mut s, Key::Char('E'), t.as_slice());
    assert!(s.resize_mode.is_none(), "`E` passes through the same gate");
}

#[test]
fn e_is_also_a_free_key_in_detail_view() {
    // The Detail scope does not bind `e` (unlike board scope's
    // StartResize). It remains a free key in the detail view.
    let mut s = State {
        view: View::Detail,
        ..fresh()
    };
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('E'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::Detail), "`e` keeps the detail view");
}

#[test]
fn form_view_absorbs_all_destructive_edit_keys_as_text() {
    // The counterpart of the single `e` absorption test. The Form view
    // must sit below the destructive-edit gate. If a later refactor
    // moves any of these keys above the Form match arm, the user's tool
    // input silently disappears or a destructive overlay opens
    // mid-typing. Parameterized so new destructive keys are explicitly
    // considered here.
    let destructive_keys = [
        ('n', "n"),
        ('R', "R"),
        ('D', "D"),
        ('p', "p"),
        ('c', "c"),
        ('m', "m"),
        ('a', "a"),
        ('[', "["),
        (']', "]"),
    ];
    for (ch, expected) in destructive_keys {
        let mut s = State {
            view: View::Form {
                tool_id: "test.with_input",
                form: string_form("input", "", true),
            },
            ..fresh()
        };
        let t = fixture_tools();
        let effect = handle_key(&mut s, Key::Char(ch), t.as_slice());
        assert_eq!(
            effect,
            Effect::None,
            "the Form view must absorb {ch:?} as text and produce no Effect"
        );
        match &s.view {
            View::Form { form, .. } => assert_eq!(
                form.fields[0].draft,
                DraftInputValue::Text(expected.to_string()),
                "the Form input should hold {ch:?} verbatim but got {:?}",
                form.fields[0].draft
            ),
            other => panic!("expected the Form view but got {other:?}"),
        }
    }
}

#[test]
fn e_inside_form_view_is_a_character_not_a_command() {
    // While typing inside the tool-input form, `e` is a character key.
    // No board command may intercept it.
    let mut s = State {
        view: View::Form {
            tool_id: "test.with_input",
            form: string_form("input", "", true),
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('e'), t.as_slice());
    if let View::Form { form, .. } = &s.view {
        assert_eq!(
            form.fields[0].draft,
            DraftInputValue::Text("e".into()),
            "the Form view must absorb `e` as text input"
        );
    } else {
        panic!("expected the Form view but got {:?}", s.view);
    }
}
