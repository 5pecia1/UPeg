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
fn 폼에서_ctrl_u는_포커스된_필드를_지운다() {
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
        panic!("Form 보기를 기대했지만 {:?}를 받았다", s.view);
    }
}

#[test]
fn 보드편집기에서_ctrl_u는_버퍼를_지운다() {
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
fn 도구선택기에서_ctrl_u는_검색어와_커서를_지운다() {
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
        other => panic!("ToolPicker 보기를 기대했지만 {other:?}를 받았다"),
    }
}
