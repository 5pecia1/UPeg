use super::*;
use upeg_core::{OutputEntry, OutputKind, OutputValue};

// ─── Result 뷰 시맨틱: 재실행 / 닫기 / 유지, F2 복사 ──────────

#[test]
fn 결과_화면에서_엔터는_같은_도구를_재실행한다() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: Vec::new(),
            text: "이전 결과".into(),
            is_error: false,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    match handle_key(&mut s, Key::Enter, t) {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.simple");
            assert_eq!(args, json!({}));
        }
        other => panic!("Result에서 Enter는 같은 도구를 재실행해야 하지만 {other:?}를 받았다"),
    }
}

#[test]
fn 결과_화면에서_f1도_재실행이다() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: Vec::new(),
            text: String::new(),
            is_error: false,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    assert!(matches!(
        handle_key(&mut s, Key::F(1), t),
        Action::Dispatch {
            tool_id: "test.simple",
            ..
        }
    ));
}

#[test]
fn 결과_화면에서_이스케이프와_큐는_목록으로_닫는다() {
    for key in [Key::Esc, Key::Char('q')] {
        let mut s = State {
            cursor: 0,
            view: View::Result {
                tool_id: "test.simple",
                outputs: Vec::new(),
                text: "결과".into(),
                is_error: false,
            },
            ..fresh()
        };
        let t = fixture_tools();
        let t = t.as_slice();

        let effect = handle_key(&mut s, key, t);
        assert_eq!(effect, Action::None);
        assert_eq!(s.view, View::List, "{key:?}는 Result를 List로 닫아야 한다");
    }
}

#[test]
fn 결과_화면에서_소비되지_않는_키는_결과를_유지한다() {
    // 예전 계약: 아무 키나 누르면 무조건 목록으로 닫혔다. 새 계약에서는
    // Enter/F1(재실행)과 Esc/q(닫기)를 제외한 나머지 키는 무시되고
    // 결과가 그대로 남아야 한다.
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: Vec::new(),
            text: "결과 유지".into(),
            is_error: false,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    let effect = handle_key(&mut s, Key::Char('x'), t);
    assert_eq!(effect, Action::None);
    assert!(
        matches!(&s.view, View::Result { text, .. } if text == "결과 유지"),
        "무관한 키는 결과 화면을 유지해야 하지만 {:?}를 받았다",
        s.view
    );
}

#[test]
fn 결과_화면_f2는_에러_메시지를_최우선으로_복사한다() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: vec![OutputEntry {
                id: "result".into(),
                label: None,
                kind: OutputKind::String,
                value: OutputValue::String("무시되어야 함".into()),
            }],
            text: "boom".into(),
            is_error: true,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    let effect = handle_key(&mut s, Key::F(2), t);
    assert_eq!(effect, Action::CopyToClipboard("boom".to_string()));
}

#[test]
fn 결과_화면_f2는_에러가_아니면_주요_출력_텍스트를_복사한다() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: vec![OutputEntry {
                id: "result".into(),
                label: None,
                kind: OutputKind::String,
                value: OutputValue::String("42".into()),
            }],
            text: String::new(),
            is_error: false,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    let effect = handle_key(&mut s, Key::F(2), t);
    assert_eq!(effect, Action::CopyToClipboard("42".to_string()));
}

#[test]
fn 결과_화면_f2는_출력도_텍스트도_없으면_복사하지_않는다() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: Vec::new(),
            text: String::new(),
            is_error: false,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    assert_eq!(handle_key(&mut s, Key::F(2), t), Action::None);
}

#[test]
fn 상세_화면에서_f2는_도구_id를_복사한다() {
    let mut s = State {
        cursor: 1,
        view: View::Detail,
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    let effect = handle_key(&mut s, Key::F(2), t);
    assert_eq!(
        effect,
        Action::CopyToClipboard("test.with_input".to_string())
    );
}
