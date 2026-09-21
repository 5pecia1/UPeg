use super::*;
use crate::surfaces::tui::model::PresentationHost;

mod clear_input;
mod grid;
mod result_view;

#[test]
fn down_key_advances_cursor_and_stops_at_end() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Down, t);
    assert_eq!(s.cursor, 1);
    handle_key(&mut s, Key::Down, t);
    assert_eq!(s.cursor, 2);
    handle_key(&mut s, Key::Down, t);
    assert_eq!(s.cursor, 2, "the cursor must stop at len - 1");
}

#[test]
fn up_key_moves_cursor_back_and_stops_at_zero() {
    let mut s = fresh();
    s.cursor = 2;
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Up, t);
    handle_key(&mut s, Key::Up, t);
    handle_key(&mut s, Key::Up, t);
    assert_eq!(s.cursor, 0, "the cursor must not go below 0");
}

#[test]
fn movement_does_not_panic_on_empty_toolbox() {
    let mut s = fresh();
    handle_key(&mut s, Key::Down, &[]);
    handle_key(&mut s, Key::Up, &[]);
    assert_eq!(s.cursor, 0);
}

#[test]
fn filter_key_returns_save_shared_selection_effect() {
    let mut s = fresh();
    let tools = fixture_tools();
    let tools = tools.as_slice();

    let action = handle_key(&mut s, Key::Char('b'), tools);

    assert_eq!(action, Action::SavePegboardSelection);
}

#[test]
fn filter_mouse_click_returns_save_shared_selection_effect() {
    let mut s = fresh();
    let tools = fixture_tools();
    let tools = tools.as_slice();
    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);
    let options = s.board_filter_options();
    let column = filter_option_column(layout.boards, BOARD_FILTER_PREFIX, &options, 1);

    let action = handle_mouse(
        &mut s,
        Mouse {
            column,
            row: layout.boards.y + 1,
            kind: MouseKind::Pointer(PointerPhase::Down),
        },
        tools,
        area,
    );

    assert_eq!(action, Action::SavePegboardSelection);
}

#[test]
fn q_in_list_opens_quit_confirm() {
    // Modeless quit guard: `q` does not quit immediately but opens a
    // confirm overlay. Actual quit only comes from confirmation
    // (y/Enter/F1).
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Char('q'), t);
    assert_eq!(action, Action::None, "q does not quit immediately");
    assert!(
        matches!(s.view, View::ConfirmQuit),
        "q opens the quit confirmation"
    );

    let confirmed = handle_key(&mut s, Key::Char('y'), t);
    assert_eq!(
        confirmed,
        Action::Quit,
        "confirm (y) performs the actual quit"
    );
}

#[test]
fn cancel_on_quit_confirm_returns_to_board() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Char('q'), t);
    assert!(matches!(s.view, View::ConfirmQuit));
    let action = handle_key(&mut s, Key::Char('n'), t);
    assert_eq!(action, Action::None);
    assert!(matches!(s.view, View::List), "cancel returns to the board");
}

#[test]
fn esc_in_list_does_not_quit() {
    // Quit guard: Esc at the top level no longer quits the app.
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Esc, t);
    assert_eq!(action, Action::None, "Esc no longer quits the app");
    assert!(matches!(s.view, View::List), "Esc keeps the board list");
}

#[test]
fn enter_in_list_runs_instead_of_opening_detail() {
    // Enter keeps a single "run/confirm" contract across the surface.
    // test.simple at cursor 0 has no inputs, so it dispatches
    // immediately.
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Enter, t);
    match action {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.simple");
            assert_eq!(args, json!({}));
        }
        other => panic!("Enter must produce Dispatch in List too but got {other:?}"),
    }
    assert!(
        matches!(
            s.view,
            View::Running {
                tool_id: "test.simple",
                ..
            }
        ),
        "Enter moves to the running view, not Detail"
    );
}

#[test]
fn o_in_list_opens_detail() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Char('o'), t);
    assert_eq!(action, Action::None);
    assert_eq!(s.view, View::Detail);
}

#[test]
fn list_detail_form_run_follows_default_state_transitions() {
    let mut s = State {
        cursor: 1,
        ..State::default()
    };
    let tools = fixture_tools();
    let tools = tools.as_slice();

    assert_eq!(handle_key(&mut s, Key::Char('o'), tools), Action::None);
    assert_eq!(s.view, View::Detail);

    assert_eq!(handle_key(&mut s, Key::Char('r'), tools), Action::None);
    assert!(matches!(
        s.view,
        View::Form {
            tool_id: "test.with_input",
            ..
        }
    ));

    for ch in "0xff".chars() {
        assert_eq!(handle_key(&mut s, Key::Char(ch), tools), Action::None);
    }

    match handle_key(&mut s, Key::Enter, tools) {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.with_input");
            assert_eq!(args, json!({"input": "0xff"}));
        }
        other => panic!("expected dispatch after filling the form but got {other:?}"),
    }
}

#[test]
fn update_drives_list_detail_form_result_transitions() {
    let mut state = State {
        cursor: 1,
        ..State::default()
    };
    let tools = fixture_tools();
    let tools = tools.as_slice();

    assert_eq!(
        update(
            &mut state,
            Msg::KeyPress {
                stroke: upeg_core::KeyStroke::from(Key::Char('o')),
                tools,
                area: None,
            },
        ),
        Effect::None
    );
    assert_eq!(state.view, View::Detail);

    assert_eq!(
        update(
            &mut state,
            Msg::KeyPress {
                stroke: upeg_core::KeyStroke::from(Key::Char('r')),
                tools,
                area: None,
            },
        ),
        Effect::None
    );
    assert!(matches!(
        state.view,
        View::Form {
            tool_id: "test.with_input",
            ..
        }
    ));

    assert_eq!(
        update(
            &mut state,
            Msg::KeyPress {
                stroke: upeg_core::KeyStroke::from(Key::Char('x')),
                tools,
                area: None,
            },
        ),
        Effect::None
    );
    let effect = update(
        &mut state,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Enter),
            tools,
            area: None,
        },
    );
    let run = state
        .active_run
        .expect("once a run starts the model attaches a run")
        .run;
    assert_eq!(
        effect,
        Effect::Dispatch {
            run,
            tool_id: "test.with_input",
            args: json!({"input": "x"}),
        }
    );

    let success = crate::domain::execution::dispatch::text_success("done");
    let outputs = success.outputs.clone();
    assert_eq!(
        update(
            &mut state,
            Msg::ToolDone {
                run,
                host: PresentationHost::LocalTui,
                tool_id: "test.with_input",
                outcome: Outcome::Success(success),
            },
        ),
        Effect::None
    );
    assert_eq!(
        state.view,
        View::Result {
            tool_id: "test.with_input",
            outputs,
            text: String::new(),
            is_error: false,
        }
    );
}

#[test]
fn j_and_k_move_like_arrows_in_list() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Char('j'), t);
    assert_eq!(s.cursor, 1);
    handle_key(&mut s, Key::Char('k'), t);
    assert_eq!(s.cursor, 0);
}

#[test]
fn f1_in_list_opens_input_tool_form() {
    let mut s = State {
        cursor: 1,
        view: View::List,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::F(1), t);
    assert_eq!(action, Action::None);
    assert!(matches!(
        s.view,
        View::Form {
            tool_id: "test.with_input",
            ..
        }
    ));
}

#[test]
fn tool_row_click_selects_and_opens_detail() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);

    let action = handle_mouse(
        &mut s,
        Mouse {
            // The second U1 card on the first grid row.
            column: layout.left.x + 1 + 19 + 2,
            row: layout.left.y + 2,
            kind: MouseKind::Pointer(PointerPhase::Down),
        },
        t,
        area,
    );
    assert_eq!(action, Action::None);
    assert_eq!(s.cursor, 1);
    assert_eq!(s.view, View::Detail);
}

#[test]
fn mouse_wheel_scrolls_grid_without_moving_cursor() {
    // Force vertical overflow by pinning a tool five rows down. The
    // canvas height then exceeds the 20-row viewport and the wheel can
    // exercise grid_scroll without cursor movement.
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 60, 20);
    let layout = tui_layout(area);
    let _guard = PlacementHintsGuard::set(vec![
        Some(PlacementHint::unit(0, 0)),
        Some(PlacementHint::unit(0, 5)),
        Some(PlacementHint::unit(1, 0)),
    ]);

    handle_mouse(
        &mut s,
        Mouse {
            column: layout.left.x + 1,
            row: layout.left.y + 1,
            kind: MouseKind::Scroll(ScrollDelta::VERTICAL_FORWARD),
        },
        t,
        area,
    );
    let scroll_after_down = s.grid_scroll;
    handle_mouse(
        &mut s,
        Mouse {
            column: layout.left.x + 1,
            row: layout.left.y + 1,
            kind: MouseKind::Scroll(ScrollDelta::VERTICAL_BACK),
        },
        t,
        area,
    );
    let scroll_after_up = s.grid_scroll;
    let cursor = s.cursor;

    assert_eq!(cursor, 0);
    assert!(scroll_after_down > 0);
    assert_eq!(scroll_after_up, 0);
}

#[test]
fn horizontal_mouse_wheel_scrolls_grid_without_moving_cursor() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 60, 20);
    let layout = tui_layout(area);

    handle_mouse(
        &mut s,
        Mouse {
            column: layout.left.x + 1,
            row: layout.left.y + 1,
            kind: MouseKind::Scroll(ScrollDelta::HORIZONTAL_FORWARD),
        },
        t,
        area,
    );
    assert_eq!(s.cursor, 0, "the horizontal wheel must not move the cursor");
    let after_right = s.grid_h_scroll;
    assert!(after_right > 0, "ScrollRight must increase grid_h_scroll");

    handle_mouse(
        &mut s,
        Mouse {
            column: layout.left.x + 1,
            row: layout.left.y + 1,
            kind: MouseKind::Scroll(ScrollDelta::HORIZONTAL_BACK),
        },
        t,
        area,
    );
    assert_eq!(s.cursor, 0);
    assert!(
        s.grid_h_scroll < after_right.get(),
        "ScrollLeft must decrease grid_h_scroll"
    );
}

#[test]
fn left_key_rewinds_horizontal_scroll_with_cursor_move() {
    // Moving to the rightmost cell makes grid_h_scroll positive; moving
    // back left must make sync_grid_scroll_to_cursor return
    // grid_h_scroll to 0 so the new cursor stays visible in the
    // viewport.
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 60, 20);
    let _guard = PlacementHintsGuard::set(vec![
        Some(PlacementHint::unit(0, 0)),
        Some(PlacementHint::unit(5, 0)),
        Some(PlacementHint::unit(0, 1)),
    ]);

    update(
        &mut s,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Right),
            tools: t,
            area: Some(area),
        },
    );
    assert_eq!(s.cursor, 1);
    let scroll_after_right = s.grid_h_scroll;
    assert!(scroll_after_right > 0);

    update(
        &mut s,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Left),
            tools: t,
            area: Some(area),
        },
    );
    assert_eq!(s.cursor, 0, "the Left key must go back to the left cell");
    assert_eq!(
        s.grid_h_scroll, 0,
        "back at the left edge, horizontal scroll must return to 0"
    );
}

#[test]
fn mouse_wheel_in_detail_view_scrolls_right_pane() {
    let mut s = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);

    handle_mouse(
        &mut s,
        Mouse {
            column: layout.right.x + 1,
            row: layout.right.y + 1,
            kind: MouseKind::Scroll(ScrollDelta::VERTICAL_FORWARD),
        },
        t,
        area,
    );
    assert!(s.right_scroll > 0);
    handle_mouse(
        &mut s,
        Mouse {
            column: layout.right.x + 1,
            row: layout.right.y + 1,
            kind: MouseKind::Scroll(ScrollDelta::VERTICAL_BACK),
        },
        t,
        area,
    );
    assert_eq!(s.right_scroll, 0);
}

#[test]
fn render_applies_right_pane_scroll_to_long_result() {
    use ratatui::backend::TestBackend;

    static TOOL: ToolMeta = ToolMeta {
        id: "test.scroll_result",
        toolkit: "test",
        local_id: "scroll_result",
        tags: &[],
        display_label: "Scroll result",
        description: "Right pane scroll fixture",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    let tools = [&TOOL];
    let mut text = String::from("top-marker\n");
    for i in 0..20 {
        text.push_str(&format!("filler-{i:02}\n"));
    }
    text.push_str("bottom-marker");
    let state = State {
        cursor: 0,
        right_scroll: ScrollOffset::new(u16::MAX),
        view: View::Result {
            tool_id: "test.scroll_result",
            outputs: Vec::new(),
            text,
            is_error: false,
        },
        ..State::default()
    };
    let area = Rect::new(0, 0, 90, 16);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let right_text = buffer_region_text(terminal.backend().buffer(), tui_layout(area).right);

    assert!(
        right_text.contains("bottom-marker"),
        "the right-pane scroll must render the bottom of long output. right pane: {right_text}"
    );
    assert!(
        !right_text.contains("top-marker"),
        "when scrolled, the right pane must not stay pinned to the top. right pane: {right_text}"
    );
}
#[test]
fn f1_button_click_runs_selected_no_arg_tool() {
    let mut s = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);

    let action = handle_mouse(
        &mut s,
        Mouse {
            column: layout.right.x + 3,
            row: layout.right.y + layout.right.height - 2,
            kind: MouseKind::Pointer(PointerPhase::Down),
        },
        t,
        area,
    );
    match action {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.simple");
            assert_eq!(args, json!({}));
        }
        other => panic!("expected Dispatch on F1 mouse click but got {other:?}"),
    }
}

#[test]
fn esc_in_detail_returns_to_list_without_quitting() {
    let mut s = State {
        cursor: 1,
        view: View::Detail,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Esc, t);
    assert_eq!(action, Action::None);
    assert_eq!(s.view, View::List);
}

#[test]
fn enter_on_no_arg_tool_runs_immediately() {
    // simple has empty args, so Enter in Detail runs it with {}.
    let mut s = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Enter, t);
    match action {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.simple");
            assert_eq!(args, json!({}));
        }
        other => panic!("expected Dispatch but got {other:?}"),
    }
}

#[test]
fn run_key_opens_form_for_tool_requiring_input() {
    let mut s = State {
        cursor: 1,
        view: View::Detail,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Char('r'), t);
    match &s.view {
        View::Form { tool_id, form } => {
            assert_eq!(*tool_id, "test.with_input");
            assert_eq!(form.len(), 1);
            assert_eq!(form.fields[0].name.as_str(), "input");
            let input = form.spec_for_field(&form.fields[0]).unwrap();
            assert!(input.required);
            assert_eq!(form.focused, 0);
        }
        other => panic!("expected the Form view but got {other:?}"),
    }
}

#[test]
fn typing_in_form_appends_to_focused_field() {
    let mut s = State {
        cursor: 1,
        view: View::Form {
            tool_id: "test.with_input",
            form: string_form("input", "", true),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    for c in "0xff".chars() {
        handle_key(&mut s, Key::Char(c), t);
    }
    if let View::Form { form, .. } = &s.view {
        assert_eq!(form.fields[0].draft, DraftInputValue::Text("0xff".into()));
    } else {
        panic!("expected the Form view");
    }
}

#[test]
fn backspace_in_form_removes_last_char() {
    let mut s = State {
        cursor: 0,
        view: View::Form {
            tool_id: "x",
            form: string_form("input", "abc", false),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Backspace, t);
    if let View::Form { form, .. } = &s.view {
        assert_eq!(form.fields[0].draft, DraftInputValue::Text("ab".into()));
    } else {
        panic!("expected the Form view");
    }
}

#[test]
fn enter_in_form_triggers_run() {
    let mut s = State {
        cursor: 1,
        view: View::Form {
            tool_id: "test.with_input",
            form: string_form("input", "0xff", true),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    match handle_key(&mut s, Key::Enter, t) {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.with_input");
            assert_eq!(args, json!({"input": "0xff"}));
        }
        other => panic!("expected Dispatch but got {other:?}"),
    }
}

#[test]
fn esc_in_form_returns_to_list() {
    let mut s = State {
        cursor: 1,
        view: View::Form {
            tool_id: "x",
            form: TuiFormState::new(InputSpec::empty()),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Esc, t);
    assert_eq!(s.view, View::List);
}

#[test]
fn clamp_state_to_area_lowers_grid_h_scroll_when_resized_narrower() {
    // If the per-frame clamp in effects.rs disappears, a stale
    // grid_h_scroll goes out of bounds on the new (narrower) canvas and
    // causes render breakage. Regression guard.
    let mut s = fresh();
    s.grid_h_scroll = ScrollOffset::new(200);
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 60, 20);

    clamp_state_to_area(&mut s, t, area);

    let max = {
        let layout = tui_layout(area);
        let content = layout.left;
        // grid_max_h_scroll lives in grid, but here checking just
        // "less than 200" is enough to catch the regression. The point
        // is that the call itself stays alive.
        let _ = content;
        200
    };
    assert!(
        s.grid_h_scroll < max,
        "the clamp call must shrink grid_h_scroll to fit the canvas"
    );
}
