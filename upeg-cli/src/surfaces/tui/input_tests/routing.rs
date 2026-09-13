//! `input_tests` 모듈의 짝 — 키 라우팅(q-back, 폼 안 필터 단축키
//! 무시, 결과 이동)과 crossterm 어댑터 회귀 테스트만 모은다.
//! 워크스페이스 1000-LoC 파일 크기 예산 때문에 분리했다.

use super::super::msg::key_stroke_from_crossterm;
use super::*;

#[test]
fn 상세_보기에서_큐는_목록으로_돌아간다() {
    // 렌더 힌트는 "q back"을 안내했지만 핸들러에서는 q가 Action::None으로
    // 흘러갔다. 이후 리팩터가 힌트를 다시 조용히 깨뜨리지 못하도록
    // 새 동작을 고정한다.
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
        "Detail의 q는 Action이 아니라 이동이어야 한다"
    );
    assert_eq!(s.view, View::List, "Detail의 q는 List 보기로 돌아가야 한다");

    // 대문자 Q도 같다.
    let mut s2 = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };
    handle_key(&mut s2, Key::Char('Q'), t);
    assert_eq!(s2.view, View::List, "대문자 Q도 동일하게 뒤로 가야 한다");
}

#[test]
fn 폼_안의_큐는_사용자가_입력할_수_있도록_종료하지_않는다() {
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
            "문자 `q`는 가로채이지 않고 필드에 들어가야 한다"
        );
    } else {
        panic!("Form 보기에 머물러야 한다");
    }
}

#[test]
fn 폼_안의_필터_단축키_문자는_그대로_텍스트로_남는다() {
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
            "폼 편집 중에는 필터 단축키가 텍스트로 남아야 한다"
        );
    } else {
        panic!("Form 보기에 머물러야 한다");
    }
}

#[test]
fn 탭은_포커스된_필드를_순환하며_전진시킨다() {
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
        assert_eq!(form.focused, 0, "첫 필드로 순환해야 한다");
    }
}

#[test]
fn 결과_적용은_결과_보기로_전이한다() {
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
        _ => panic!("Result 보기를 기대했다"),
    }
    assert_eq!(s.focus, FocusArea::RightPane);
}

#[test]
fn 도구_오류_결과는_오류로_표시된다() {
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
        panic!("Result를 기대했다");
    }
}

#[test]
fn 결과_보기에서_소비되지_않는_키는_결과를_유지한다() {
    // 예전 계약: 어떤 키를 눌러도 목록으로 돌아갔다. 새 계약에서는
    // Enter/F1(재실행)과 Esc/q(닫기)만 의미가 있고, 그 외 키는 무시되어
    // 결과가 그대로 남는다 (`upeg-cli/src/surfaces/tui/state_tests/navigation.rs`
    // 의 재실행/닫기 테스트와 짝을 이룬다).
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
fn 결과_보기에서_이스케이프는_목록으로_돌아간다() {
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
fn crossterm_어댑터는_ctrl_k_수식어를_공통_resolver에_전달한다() {
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

// ─── 순수 헬퍼 ───────────────────────────────────────────
