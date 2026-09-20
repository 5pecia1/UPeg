//! Companion to the `input_tests` module — collects only key-routing
//! (q-back, ignoring filter shortcuts inside forms, result navigation)
//! and crossterm-adapter regression tests. Split out for the workspace
//! 1000-LoC file-size budget.

use super::super::msg::key_stroke_from_crossterm;
use super::*;

#[test]
fn q_in_detail_view_returns_to_list() {
    // The render hint advertised "q back" but the handler let q fall
    // through as Action::None. Pinning the new behavior so a later
    // refactor cannot silently break the hint again.
    let mut s = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Char('q'), t);
    assert_eq!(
        action,
        Action::None,
        "q in Detail must be navigation, not an Action"
    );
    assert_eq!(
        s.view,
        View::List,
        "q in Detail must return to the List view"
    );

    // Uppercase Q behaves the same.
    let mut s2 = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };
    handle_key(&mut s2, Key::Char('Q'), t);
    assert_eq!(s2.view, View::List, "uppercase Q must go back the same way");
}

#[test]
fn q_inside_form_does_not_quit_so_user_can_type() {
    let mut s = State {
        cursor: 1,
        view: View::Form {
            tool_id: "x",
            form: string_form("input", "", false),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Char('q'), t);
    assert_eq!(action, Action::None);
    if let View::Form { form, .. } = &s.view {
        assert_eq!(
            form.fields[0].draft,
            DraftInputValue::Text("q".into()),
            "the character `q` must enter the field unintercepted"
        );
    } else {
        panic!("must stay in the Form view");
    }
}

#[test]
fn filter_shortcut_chars_inside_form_remain_literal_text() {
    let mut s = State {
        cursor: 1,
        view: View::Form {
            tool_id: "x",
            form: string_form("input", "", false),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    handle_key(&mut s, Key::Char('b'), t);
    handle_key(&mut s, Key::Char('t'), t);

    if let View::Form { form, .. } = &s.view {
        assert_eq!(
            form.fields[0].draft,
            DraftInputValue::Text("bt".into()),
            "while editing a form, filter shortcuts must remain text"
        );
    } else {
        panic!("must stay in the Form view");
    }
}

#[test]
fn tab_advances_focused_field_cyclically() {
    let mut s = State {
        cursor: 0,
        view: View::Form {
            tool_id: "x",
            form: string_form_with_names(&["a", "b", "c"]),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Tab, t);
    if let View::Form { form, .. } = &s.view {
        assert_eq!(form.focused, 1);
    }
    handle_key(&mut s, Key::Tab, t);
    if let View::Form { form, .. } = &s.view {
        assert_eq!(form.focused, 2);
    }
    handle_key(&mut s, Key::Tab, t);
    if let View::Form { form, .. } = &s.view {
        assert_eq!(form.focused, 0, "must cycle back to the first field");
    }
}

#[test]
fn apply_outcome_transitions_to_result_view() {
    let mut s = State {
        cursor: 0,
        view: View::Form {
            tool_id: "test.simple",
            form: TuiFormState::new(InputSpec::empty()),
        },
        ..State::default()
    };
    apply_outcome(
        &mut s,
        "test.simple",
        Outcome::Success(crate::domain::execution::dispatch::text_success("done")),
    );
    match s.view {
        View::Result {
            tool_id,
            outputs,
            text,
            is_error,
        } => {
            assert_eq!(tool_id, "test.simple");
            assert_eq!(outputs[0].id, "result");
            assert_eq!(text, "");
            assert!(!is_error);
        }
        _ => panic!("expected the Result view"),
    }
    assert_eq!(s.focus, FocusArea::RightPane);
}

#[test]
fn tool_error_outcome_is_marked_as_error() {
    let mut s = State::default();
    apply_outcome(
        &mut s,
        "x",
        Outcome::Failure(crate::domain::execution::dispatch::dispatch_failure(
            "tool_error",
            "boom",
        )),
    );
    if let View::Result { is_error, text, .. } = &s.view {
        assert!(*is_error);
        assert_eq!(text, "boom");
    } else {
        panic!("expected Result");
    }
}

#[test]
fn unconsumed_key_in_result_view_keeps_result() {
    // Old contract: any key returned to the list. Under the new contract
    // only Enter/F1 (rerun) and Esc/q (close) mean anything; every other
    // key is ignored and the result stays put (pairs with the
    // rerun/close tests in
    // `upeg-cli/src/surfaces/tui/state_tests/navigation.rs`).
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "x",
            outputs: Vec::new(),
            text: "ok".into(),
            is_error: false,
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Char('z'), t);
    assert!(matches!(&s.view, View::Result { tool_id: "x", .. }));
}

#[test]
fn esc_in_result_view_returns_to_list() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "x",
            outputs: Vec::new(),
            text: "ok".into(),
            is_error: false,
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Esc, t);
    assert_eq!(s.view, View::List);
}

#[test]
fn crossterm_adapter_passes_ctrl_k_modifier_to_common_resolver() {
    let event = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('k'),
        crossterm::event::KeyModifiers::CONTROL,
    );
    let stroke = key_stroke_from_crossterm(event).expect("press key event");

    assert!(stroke.modifiers.control);
    assert_eq!(
        upeg_core::resolve_key(
            upeg_core::KeyboardContext::new(upeg_core::KeyboardScope::Board),
            stroke,
        ),
        Some(upeg_core::KeyboardCommand::Search)
    );
}

// ─── Pure helpers ──────────────────────────────────────────
