    use super::*;

    fn stroke(key: Key) -> KeyStroke {
        KeyStroke::plain(key)
    }

    #[test]
    fn board_scope_resolves_common_navigation_and_commands() {
        let ctx = KeyboardContext::new(KeyboardScope::Board);

        assert_eq!(
            resolve_key(ctx, stroke(Key::Left)),
            Some(KeyboardCommand::Move(NavDirection::Left))
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('h'))),
            Some(KeyboardCommand::Move(NavDirection::Left))
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Enter)),
            Some(KeyboardCommand::Run)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Space)),
            Some(KeyboardCommand::Run)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('/'))),
            Some(KeyboardCommand::Search)
        );
    }

    #[test]
    fn board_scope_enter_runs_instead_of_opening_details() {
        // Enter keeps a single "run/confirm" contract across every surface.
        // Previously only List's Enter opened details as an exception, which
        // collided with the Enter=confirm convention of Result/Form/BoardEditor.
        let ctx = KeyboardContext::new(KeyboardScope::Board);
        assert_eq!(
            resolve_key(ctx, stroke(Key::Enter)),
            Some(KeyboardCommand::Run),
            "Enter must resolve to Run, not Open"
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::F(1))),
            Some(KeyboardCommand::Run),
            "F1 must resolve to Run as well"
        );
    }

    #[test]
    fn board_scope_o_opens_the_detail_view() {
        let ctx = KeyboardContext::new(KeyboardScope::Board);
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('o'))),
            Some(KeyboardCommand::Open)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('O'))),
            Some(KeyboardCommand::Open)
        );
    }

    #[test]
    fn board_and_detail_scopes_f2_is_the_copy_command() {
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        let detail_ctx = KeyboardContext::new(KeyboardScope::Detail);
        assert_eq!(
            resolve_key(board_ctx, stroke(Key::F(2))),
            Some(KeyboardCommand::Copy)
        );
        assert_eq!(
            resolve_key(detail_ctx, stroke(Key::F(2))),
            Some(KeyboardCommand::Copy)
        );
    }

    #[test]
    fn f1_commits_in_boardeditor_pincolor_toolpicker_moving_and_confirms_in_confirmdelete() {
        // PinColorEditor shares BoardEditor's scope in
        // keyboard_scope_for_view, so it needs no separate case.
        let cases = [
            (KeyboardScope::BoardEditor, KeyboardCommand::Commit),
            (KeyboardScope::ToolPicker, KeyboardCommand::Commit),
            (KeyboardScope::Moving, KeyboardCommand::Commit),
        ];
        for (scope, expected) in cases {
            assert_eq!(
                resolve_key(KeyboardContext::new(scope), stroke(Key::F(1))),
                Some(expected),
                "F1 in {scope:?} must be {expected:?}"
            );
        }
        assert_eq!(
            resolve_key(
                KeyboardContext::new(KeyboardScope::ConfirmDelete),
                stroke(Key::F(1))
            ),
            Some(KeyboardCommand::Confirm)
        );
    }

    #[test]
    fn ctrl_u_clears_input_in_form_boardeditor_and_toolpicker() {
        let ctrl_u = KeyStroke::modified(
            Key::Char('u'),
            KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        );
        for scope in [
            KeyboardScope::Form,
            KeyboardScope::BoardEditor,
            KeyboardScope::ToolPicker,
        ] {
            assert_eq!(
                resolve_key(KeyboardContext::new(scope), ctrl_u),
                Some(KeyboardCommand::ClearInput),
                "Ctrl+U in {scope:?} must be ClearInput"
            );
        }
    }

    #[test]
    fn board_change_commands_are_always_active_in_modeless_board() {
        // With the edit mode gone (modeless), `n`/`R`/`D`/`a` always resolve
        // in the board scope. No focus needed — they are board-level work.
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);

        assert_eq!(
            resolve_key(board_ctx, stroke(Key::Char('n'))),
            Some(KeyboardCommand::NewBoard)
        );
        assert_eq!(
            resolve_key(board_ctx, stroke(Key::Char('R'))),
            Some(KeyboardCommand::RenameBoard)
        );
        assert_eq!(
            resolve_key(board_ctx, stroke(Key::Char('D'))),
            Some(KeyboardCommand::DeleteBoard)
        );
        assert_eq!(
            resolve_key(board_ctx, stroke(Key::Char('a'))),
            Some(KeyboardCommand::OpenToolPicker)
        );
    }

    #[test]
    fn e_starts_resize_on_the_focused_pin() {
        // `e`, freed by the old edit toggle, was rebound to start resize.
        // Symmetric with `m`/StartMove, it resolves only on a focused pin.
        let focused_pin_ctx = KeyboardContext {
            has_tool_focus: true,
            ..KeyboardContext::new(KeyboardScope::Board)
        };
        assert_eq!(
            resolve_key(focused_pin_ctx, stroke(Key::Char('e'))),
            Some(KeyboardCommand::StartResize)
        );
        assert_eq!(
            resolve_key(focused_pin_ctx, stroke(Key::Char('E'))),
            Some(KeyboardCommand::StartResize)
        );
    }

    #[test]
    fn e_is_not_resolved_in_unfocused_board_and_detail_scopes() {
        // Without a focused pin there is nothing to resize, so `e` is
        // ignored. It stays a free key in the Detail scope.
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        let detail_ctx = KeyboardContext::new(KeyboardScope::Detail);
        assert_eq!(resolve_key(board_ctx, stroke(Key::Char('e'))), None);
        assert_eq!(resolve_key(board_ctx, stroke(Key::Char('E'))), None);
        assert_eq!(resolve_key(detail_ctx, stroke(Key::Char('e'))), None);
    }

    #[test]
    fn resize_scope_resolves_arrow_keys_to_span_deltas() {
        let ctx = KeyboardContext::new(KeyboardScope::Resize);
        let grow_cases = [
            (Key::Right, 1, 0),
            (Key::Char('l'), 1, 0),
            (Key::Left, -1, 0),
            (Key::Char('h'), -1, 0),
            (Key::Down, 0, 1),
            (Key::Char('j'), 0, 1),
            (Key::Up, 0, -1),
            (Key::Char('k'), 0, -1),
        ];
        for (key, cols, rows) in grow_cases {
            assert_eq!(
                resolve_key(ctx, stroke(key)),
                Some(KeyboardCommand::ResizeBy { cols, rows }),
                "{key:?} must be ResizeBy(cols {cols}, rows {rows})"
            );
        }
    }

    #[test]
    fn resize_scope_resolves_commit_cancel_and_reset_to_the_same_keys_as_moving() {
        let ctx = KeyboardContext::new(KeyboardScope::Resize);
        assert_eq!(
            resolve_key(ctx, stroke(Key::Enter)),
            Some(KeyboardCommand::Commit)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::F(1))),
            Some(KeyboardCommand::Commit)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Esc)),
            Some(KeyboardCommand::Cancel)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('q'))),
            Some(KeyboardCommand::Cancel)
        );
        // `0` follows the board scope's filter-clear convention and resets
        // to the manifest default size.
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('0'))),
            Some(KeyboardCommand::ResetSpan)
        );
    }

    #[test]
    fn c_opens_color_editing_on_the_focused_pin() {
        let focused_pin_ctx = KeyboardContext {
            has_tool_focus: true,
            ..KeyboardContext::new(KeyboardScope::Board)
        };

        assert_eq!(
            resolve_key(focused_pin_ctx, stroke(Key::Char('c'))),
            Some(KeyboardCommand::EditPinColor)
        );
    }

    #[test]
    fn c_on_an_unfocused_board_does_not_open_pin_color_editing() {
        // Without a focused pin there is no target, so `c` is ignored —
        // this gate is the only condition left after going modeless.
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        assert_eq!(resolve_key(board_ctx, stroke(Key::Char('c'))), None);
    }

    #[test]
    fn board_scope_slash_keeps_search() {
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);

        assert_eq!(
            resolve_key(board_ctx, stroke(Key::Char('/'))),
            Some(KeyboardCommand::Search)
        );
    }

    #[test]
    fn board_scope_primary_k_keeps_search() {
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        let ctrl = KeyModifiers {
            control: true,
            ..KeyModifiers::NONE
        };
        let meta = KeyModifiers {
            meta: true,
            ..KeyModifiers::NONE
        };

        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('k'), ctrl)),
            Some(KeyboardCommand::Search)
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('K'), meta)),
            Some(KeyboardCommand::Search)
        );
    }

    #[test]
    fn primary_w_follows_the_scopes_esc_behavior() {
        // Ctrl+W (Windows/Linux) and Cmd+W (macOS) mean "close the current
        // surface" by desktop convention. Each scope delegates to whatever
        // dismissal it bound to Esc — Close for Board/Form, Cancel for
        // editor/confirm/picker/Moving.
        let ctrl = KeyModifiers {
            control: true,
            ..KeyModifiers::NONE
        };
        let meta = KeyModifiers {
            meta: true,
            ..KeyModifiers::NONE
        };

        let close_scopes = [
            KeyboardScope::Board,
            KeyboardScope::Detail,
            KeyboardScope::Form,
            KeyboardScope::Settings,
        ];
        for scope in close_scopes {
            let ctx = KeyboardContext::new(scope);
            assert_eq!(
                resolve_key(ctx, KeyStroke::modified(Key::Char('w'), ctrl)),
                Some(KeyboardCommand::Close),
                "Ctrl+W in {scope:?} scope must be Close"
            );
            assert_eq!(
                resolve_key(ctx, KeyStroke::modified(Key::Char('W'), meta)),
                Some(KeyboardCommand::Close),
                "Cmd+W in {scope:?} scope must be Close"
            );
        }

        let cancel_scopes = [
            KeyboardScope::Moving,
            KeyboardScope::Resize,
            KeyboardScope::BoardEditor,
            KeyboardScope::ConfirmDelete,
            KeyboardScope::ToolPicker,
        ];
        for scope in cancel_scopes {
            let ctx = KeyboardContext::new(scope);
            assert_eq!(
                resolve_key(ctx, KeyStroke::modified(Key::Char('w'), ctrl)),
                Some(KeyboardCommand::Cancel),
                "Ctrl+W in {scope:?} scope must be Cancel"
            );
        }
    }

    #[test]
    fn board_scope_primary_q_keeps_quit() {
        // Ctrl+Q (Windows/Linux) and Cmd+Q (macOS) both mean quit by
        // desktop convention. It must be Quit not only for bare `q` but
        // for primary-modifier chords too.
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        let ctrl = KeyModifiers {
            control: true,
            ..KeyModifiers::NONE
        };
        let meta = KeyModifiers {
            meta: true,
            ..KeyModifiers::NONE
        };

        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('q'), ctrl)),
            Some(KeyboardCommand::Quit)
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('Q'), meta)),
            Some(KeyboardCommand::Quit)
        );
    }

    #[test]
    fn shared_keyboard_contract_pins_commands_across_major_scopes() {
        let board_with_focus = KeyboardContext {
            has_tool_focus: true,
            ..KeyboardContext::new(KeyboardScope::Board)
        };
        let cases = [
            (
                KeyboardContext::new(KeyboardScope::Board),
                KeyStroke::modified(
                    Key::Char('k'),
                    KeyModifiers {
                        control: true,
                        ..KeyModifiers::NONE
                    },
                ),
                Some(KeyboardCommand::Search),
            ),
            (
                KeyboardContext::new(KeyboardScope::Board),
                stroke(Key::Char('q')),
                Some(KeyboardCommand::Quit),
            ),
            (
                KeyboardContext::new(KeyboardScope::Board),
                stroke(Key::Char('b')),
                Some(KeyboardCommand::CycleBoardFilter),
            ),
            (
                KeyboardContext::new(KeyboardScope::Board),
                stroke(Key::Char('t')),
                Some(KeyboardCommand::CycleTagFilter),
            ),
            (
                KeyboardContext::new(KeyboardScope::Board),
                stroke(Key::Char('9')),
                BoardSlot::new(9).map(KeyboardCommand::SwitchBoard),
            ),
            (
                board_with_focus,
                stroke(Key::Char('a')),
                Some(KeyboardCommand::OpenToolPicker),
            ),
            (
                board_with_focus,
                stroke(Key::Char('p')),
                Some(KeyboardCommand::TogglePin),
            ),
            (
                board_with_focus,
                stroke(Key::Char('c')),
                Some(KeyboardCommand::EditPinColor),
            ),
            (
                board_with_focus,
                stroke(Key::Char('m')),
                Some(KeyboardCommand::StartMove),
            ),
            (
                board_with_focus,
                stroke(Key::Char('e')),
                Some(KeyboardCommand::StartResize),
            ),
            (
                board_with_focus,
                stroke(Key::Char(']')),
                Some(KeyboardCommand::Reorder(OrderDirection::Next)),
            ),
            (
                KeyboardContext::new(KeyboardScope::Detail),
                stroke(Key::Char('r')),
                Some(KeyboardCommand::Run),
            ),
            (
                KeyboardContext::new(KeyboardScope::Detail),
                stroke(Key::Char('q')),
                Some(KeyboardCommand::Close),
            ),
            (
                KeyboardContext::new(KeyboardScope::Form),
                stroke(Key::Char('b')),
                Some(KeyboardCommand::Text('b')),
            ),
            (
                KeyboardContext::new(KeyboardScope::Form),
                stroke(Key::BackTab),
                Some(KeyboardCommand::FocusPrevious),
            ),
            (
                KeyboardContext::new(KeyboardScope::Settings),
                stroke(Key::Char('l')),
                Some(KeyboardCommand::Move(NavDirection::Right)),
            ),
            (
                KeyboardContext::new(KeyboardScope::BoardEditor),
                stroke(Key::Enter),
                Some(KeyboardCommand::Commit),
            ),
            (
                KeyboardContext::new(KeyboardScope::ConfirmDelete),
                stroke(Key::Char('y')),
                Some(KeyboardCommand::Confirm),
            ),
            (
                KeyboardContext::new(KeyboardScope::ConfirmDelete),
                stroke(Key::Char('Q')),
                Some(KeyboardCommand::Cancel),
            ),
            (
                KeyboardContext::new(KeyboardScope::ToolPicker),
                stroke(Key::Backspace),
                Some(KeyboardCommand::Backspace),
            ),
            (
                KeyboardContext::new(KeyboardScope::FilterBar),
                stroke(Key::End),
                Some(KeyboardCommand::End),
            ),
            (
                KeyboardContext::new(KeyboardScope::RightPane),
                stroke(Key::PageDown),
                Some(KeyboardCommand::Page(PageDirection::Down)),
            ),
            (
                KeyboardContext::new(KeyboardScope::Moving),
                stroke(Key::Char('h')),
                Some(KeyboardCommand::Move(NavDirection::Left)),
            ),
            (
                KeyboardContext::new(KeyboardScope::Moving),
                stroke(Key::Esc),
                Some(KeyboardCommand::Cancel),
            ),
            (
                KeyboardContext::new(KeyboardScope::Resize),
                stroke(Key::Char('l')),
                Some(KeyboardCommand::ResizeBy { cols: 1, rows: 0 }),
            ),
            (
                KeyboardContext::new(KeyboardScope::Resize),
                stroke(Key::Char('0')),
                Some(KeyboardCommand::ResetSpan),
            ),
        ];

        for (context, stroke, expected) in cases {
            assert_eq!(
                resolve_key(context, stroke),
                expected,
                "{context:?} + {stroke:?} must keep the shared key binding contract"
            );
        }
    }

    #[test]
    fn command_modifier_blocks_plain_bindings_unless_an_explicit_shortcut() {
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        let focus_ctx = KeyboardContext {
            has_tool_focus: true,
            ..KeyboardContext::new(KeyboardScope::Board)
        };
        let ctrl = KeyModifiers {
            control: true,
            ..KeyModifiers::NONE
        };
        let alt = KeyModifiers {
            alt: true,
            ..KeyModifiers::NONE
        };
        let meta = KeyModifiers {
            meta: true,
            ..KeyModifiers::NONE
        };
        let ctrl_alt = KeyModifiers {
            control: true,
            alt: true,
            ..KeyModifiers::NONE
        };
        let shift = KeyModifiers {
            shift: true,
            ..KeyModifiers::NONE
        };

        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('k'), ctrl)),
            Some(KeyboardCommand::Search),
            "Ctrl/Cmd+K is the only global command-modified board shortcut"
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('k'), ctrl_alt)),
            None,
            "Alt must not combine with the Ctrl/Cmd+K primary shortcut"
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('e'), ctrl)),
            None,
            "Ctrl+E is unbound — bare `e` starts resize, but no chord does"
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('b'), alt)),
            None,
            "Alt+B must not cycle board filters"
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('1'), meta)),
            None,
            "Meta+1 must not switch boards"
        );
        assert_eq!(
            resolve_key(
                KeyboardContext::new(KeyboardScope::Form),
                KeyStroke::modified(Key::Char('b'), ctrl),
            ),
            None,
            "Ctrl+B in form scope is a command chord, not text input"
        );
        assert_eq!(
            resolve_key(
                KeyboardContext::new(KeyboardScope::Moving),
                KeyStroke::modified(Key::Left, alt),
            ),
            None,
            "Alt+arrow must not move a board item"
        );
        assert_eq!(
            resolve_key(focus_ctx, KeyStroke::modified(Key::Char('R'), shift)),
            Some(KeyboardCommand::RenameBoard),
            "Shift only produces case/symbol keys and remains eligible"
        );
    }

    #[test]
    fn board_slot_allows_only_one_through_nine() {
        assert_eq!(BoardSlot::new(0), None);
        assert_eq!(BoardSlot::new(1).map(BoardSlot::zero_based_index), Some(0));
        assert_eq!(BoardSlot::new(9).map(BoardSlot::zero_based_index), Some(8));
        assert_eq!(BoardSlot::new(10), None);
    }

    #[test]
    fn key_label_parser_resolves_key_strings_shared_by_surface_adapters() {
        assert_eq!(key_from_label("ArrowLeft", false), Some(Key::Left));
        assert_eq!(key_from_label(" ", false), Some(Key::Space));
        assert_eq!(key_from_label("Space", false), Some(Key::Space));
        assert_eq!(key_from_label("F1", false), Some(Key::F(1)));
        assert_eq!(key_from_label("F24", false), Some(Key::F(24)));
        assert_eq!(key_from_label("Tab", true), Some(Key::BackTab));
        assert_eq!(key_from_label("Shift", false), None);
    }
