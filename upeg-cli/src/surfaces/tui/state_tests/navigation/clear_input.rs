use super::*;

fn ctrl_u() -> upeg_core::KeyStroke {
    upeg_core::KeyStroke::modified(
        Key::Char('u'),
        upeg_core::KeyModifiers {
            control: true,
            ..upeg_core::KeyModifiers::NONE
        },
    )
}

// ─── Ctrl+U (ClearInput) ────────────────────────────────

#[test]
fn ctrl_u_in_form_clears_focused_field() {
    let mut s = State {
        cursor: 1,
        view: View::Form {
            tool_id: "test.with_input",
            form: string_form("input", "0xff", true),
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    let effect = update(
        &mut s,
        Msg::KeyPress {
            stroke: ctrl_u(),
            tools: t,
            area: None,
        },
    );

    assert_eq!(effect, Effect::None);
    if let View::Form { form, .. } = &s.view {
        assert_eq!(form.fields[0].draft, DraftInputValue::Text(String::new()));
    } else {
        panic!("expected Form view, got {:?}", s.view);
    }
}

#[test]
fn ctrl_u_in_board_editor_clears_buffer() {
    use crate::surfaces::tui::model::BoardEditMode;

    let mut s = State {
        view: View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            buffer: "Alpha".into(),
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    update(
        &mut s,
        Msg::KeyPress {
            stroke: ctrl_u(),
            tools: t,
            area: None,
        },
    );

    assert!(matches!(&s.view, View::BoardEditor { buffer, .. } if buffer.is_empty()));
}

#[test]
fn ctrl_u_in_tool_picker_clears_query_and_cursor() {
    let mut s = State {
        view: View::ToolPicker {
            mode: ToolPickerMode::Search,
            return_to: ToolPickerReturn::list(FocusArea::Grid),
            query: "hex".into(),
            cursor: 2,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    update(
        &mut s,
        Msg::KeyPress {
            stroke: ctrl_u(),
            tools: t,
            area: None,
        },
    );

    match &s.view {
        View::ToolPicker { query, cursor, .. } => {
            assert!(query.is_empty());
            assert_eq!(*cursor, 0);
        }
        other => panic!("expected ToolPicker view, got {other:?}"),
    }
}
