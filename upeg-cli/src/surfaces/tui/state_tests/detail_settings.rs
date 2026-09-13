use super::*;

#[test]
fn f1_버튼_클릭은_선택된_무인자_도구를_실행한다() {
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
        other => panic!("F1 마우스 클릭에서 Dispatch를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 상세에서_이스케이프는_종료하지_않고_목록으로_돌아간다() {
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
fn 무인자_도구에서_엔터는_즉시_실행한다() {
    // simple은 속성이 비어 있으므로 Detail에서 Enter를 누르면 {}로 실행한다.
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
        other => panic!("Dispatch를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 실행키는_입력이_필요한_도구의_폼을_연다() {
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
        other => panic!("Form 보기를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 폼에서_입력하면_포커스된_필드에_추가된다() {
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
        panic!("Form 보기를 기대했다");
    }
}

#[test]
fn 폼에서_백스페이스는_마지막_문자를_지운다() {
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
        panic!("Form 보기를 기대했다");
    }
}

#[test]
fn 폼에서_엔터는_실행을_발생시킨다() {
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
        other => panic!("Dispatch를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 폼에서_이스케이프는_목록으로_돌아간다() {
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

// ─── E-1단계: View::Settings 진입 / 이동 ────────────────

#[test]
fn 목록에서_설정키는_로케일에_포커스된_설정을_연다() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('s'), t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Locale,
        }
    );
}

#[test]
fn tui_설정_저장은_도구킷_디렉터리가_아니라_설정_루트를_사용한다() {
    const EFFECTS_SRC: &str = include_str!("../effects.rs");
    assert!(
        EFFECTS_SRC.contains("upeg_core::paths::tweaks_path()"),
        "TUI prefs는 toolkits_dir가 아니라 설정 루트(Unix에서는 `~/.upeg`) 아래에 저장되어야 한다"
    );
    assert!(
        !EFFECTS_SRC.contains("paths::toolkits_dir()"),
        "TUI prefs는 UPEG_TOOLKITS_DIR를 따라가면 안 된다. 그 환경 변수는 도구 소스 로딩용이다"
    );
}

#[test]
fn 상세에서_설정키는_설정을_연다() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Detail,
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('s'), t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Locale,
        }
    );
}

#[test]
fn 설정에서_이스케이프는_목록으로_돌아간다() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Theme,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Esc, t.as_slice());
    assert_eq!(s.view, View::List);
}

#[test]
fn 설정에서_아래키는_포커스된_필드를_순환한다() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Down, t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Theme,
        }
    );
    handle_key(&mut s, Key::Down, t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Accent,
        }
    );
    // Locale로 다시 순환한다.
    handle_key(&mut s, Key::Down, t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Locale,
        }
    );
}

#[test]
fn 설정에서_위키는_뒤로_순환한다() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Up, t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Accent,
        }
    );
}

#[test]
fn 설정에서_큐는_종료하지_않고_목록으로_돌아간다() {
    // List 보기의 `q`는 TUI를 종료하지만, Settings 보기의 `q`는 한 단계
    // 뒤로 가야 한다. 그래야 사용자가 강조색 토글을 시험하다가 실수로
    // 바이너리를 종료하지 않는다.
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('q'), t.as_slice());
    assert!(matches!(effect, Action::None));
    assert_eq!(s.view, View::List);
}

// ─── E-2단계: 포커스된 필드에서 ←/→ 값 순환 ──────────

#[test]
fn 오른쪽_화살표는_포커스된_로케일_값을_순환한다() {
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::Locale;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    assert_eq!(s.tweaks.locale, Locale::En);
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Right, t.as_slice());
    assert_eq!(s.tweaks.locale, Locale::Ko);
    // E-3단계: 모든 값 순환 키는 SaveTweaks를 반환하므로 이벤트 루프가
    // 변경을 디스크에 저장한다.
    assert!(
        matches!(effect, Action::SaveTweaks),
        "값 순환은 Effect::SaveTweaks를 발생시켜야 한다"
    );
    // 영어로 다시 순환한다.
    handle_key(&mut s, Key::Right, t.as_slice());
    assert_eq!(s.tweaks.locale, Locale::En);
}

#[test]
fn 왼쪽_화살표는_로케일을_뒤로_순환한다() {
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::Locale;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Left, t.as_slice());
    assert_eq!(s.tweaks.locale, Locale::Ko);
}

#[test]
fn 테마에_포커스되면_오른쪽_화살표는_테마를_순환한다() {
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::Theme;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Theme,
        },
        ..fresh()
    };
    assert_eq!(s.tweaks.theme, Theme::Light);
    let t = fixture_tools();
    handle_key(&mut s, Key::Right, t.as_slice());
    assert_eq!(s.tweaks.theme, Theme::Dark);
    handle_key(&mut s, Key::Right, t.as_slice());
    assert_eq!(
        s.tweaks.theme,
        Theme::Light,
        "두 값짜리 필드는 순환해야 한다"
    );
}

#[test]
fn 오른쪽_화살표는_강조색_네_값을_순환한다() {
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::Accent;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Accent,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let expected = [Accent::Amber, Accent::Cyan, Accent::Pink, Accent::Green];
    for want in expected {
        handle_key(&mut s, Key::Right, t.as_slice());
        assert_eq!(s.tweaks.accent, want);
    }
}

#[test]
fn 왼쪽_화살표는_강조색을_뒤로_순환한다() {
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::Accent;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Accent,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Left, t.as_slice());
    assert_eq!(s.tweaks.accent, Accent::Pink);
}

#[test]
fn 설정_보기는_오른쪽_패널만_아니라_전체_본문_패널에_렌더링된다() {
    // 표시성 수정 고정: 수정 전 Settings는 `right_pane_content`로 렌더링되어,
    // 본문이 아직 활성인 페그보드 그리드 옆의 좁은 오른쪽 패널에 들어갔다.
    // 그래서 `s`를 누른 사용자가 화면을 찾을 수 없었다. 6단계 이후
    // BoardEditor / ConfirmDeleteBoard / ToolPicker 변형이 같은 전체 본문
    // 분기를 공유하므로, 소스 고정은 통합 디스패처 안에서 Settings 변형을
    // 매칭하는지 확인한다.
    const SRC: &str = include_str!("../view.rs");
    assert!(
        SRC.contains("View::Settings { focused_field }"),
        "render_with_context는 View::Settings를 전체 본문 렌더 경로로 보내야 \
         한다. 그렇지 않으면 화면이 사이드 패널 안에서 보이지 않는다"
    );
    assert!(
        SRC.contains("fn render_settings_panel"),
        "Settings 전용 렌더러인 render_settings_panel이 있어야 한다"
    );
    // 전체 본문 점유: render_with_context 는 dispatch 진입에서
    // `let body = layout.body();` 로 좌+우 합 rect 을 만들고, 모든
    // dialog 변형 arm 이 그 `body` 로 전용 렌더러를 호출한다. 인라인
    // 인자 형태로 핀.
    assert!(
        SRC.contains("let body = layout.body();"),
        "render_with_context 는 본문 진입에서 layout.body() (전체 본문) 을 바인딩해야 한다"
    );
    assert!(
        SRC.contains("render_settings_panel(frame, body,"),
        "Settings 렌더러는 body (전체 본문) 으로 호출되어야 한다"
    );
}

#[test]
fn 값_순환은_포커스된_필드만_대상으로_삼는다() {
    // Locale에 포커스된 동안 ← / →를 눌러도 Theme나 Accent는 바뀌면 안 된다.
    // 그렇지 않으면 혼란스러운 사용자가 모든 값을 한 번에 바꾸게 된다.
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::{Accent, Locale, Theme};
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Right, t.as_slice());
    assert_eq!(s.tweaks.locale, Locale::Ko);
    assert_eq!(s.tweaks.theme, Theme::Light, "Theme는 움직이면 안 된다");
    assert_eq!(s.tweaks.accent, Accent::Green, "Accent는 움직이면 안 된다");
}
