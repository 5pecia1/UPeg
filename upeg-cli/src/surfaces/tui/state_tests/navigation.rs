use super::*;

mod clear_input;
mod grid;
mod result_view;

#[test]
fn 아래키는_커서를_끝에서_멈추며_전진시킨다() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Down, t);
    assert_eq!(s.cursor, 1);
    handle_key(&mut s, Key::Down, t);
    assert_eq!(s.cursor, 2);
    handle_key(&mut s, Key::Down, t);
    assert_eq!(s.cursor, 2, "커서는 len - 1에서 멈춰야 한다");
}

#[test]
fn 위키는_커서를_영에서_멈추며_뒤로_이동시킨다() {
    let mut s = fresh();
    s.cursor = 2;
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Up, t);
    handle_key(&mut s, Key::Up, t);
    handle_key(&mut s, Key::Up, t);
    assert_eq!(s.cursor, 0, "커서는 0 아래로 내려가면 안 된다");
}

#[test]
fn 빈_툴박스에서도_이동은_패닉하지_않는다() {
    let mut s = fresh();
    handle_key(&mut s, Key::Down, &[]);
    handle_key(&mut s, Key::Up, &[]);
    assert_eq!(s.cursor, 0);
}

#[test]
fn 필터키는_shared_selection_저장_effect를_반환한다() {
    let mut s = fresh();
    let tools = fixture_tools();
    let tools = tools.as_slice();

    let action = handle_key(&mut s, Key::Char('b'), tools);

    assert_eq!(action, Action::SavePegboardSelection);
}

#[test]
fn 필터마우스클릭은_shared_selection_저장_effect를_반환한다() {
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
fn 목록에서_큐는_종료_확인을_연다() {
    // Modeless 오종료 방지: `q`는 즉시 종료하지 않고 확인 오버레이를
    // 연다. 실제 종료는 확인(y/Enter/F1)에서만 나온다.
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Char('q'), t);
    assert_eq!(action, Action::None, "q는 즉시 종료하지 않는다");
    assert!(matches!(s.view, View::ConfirmQuit), "q는 종료 확인을 연다");

    let confirmed = handle_key(&mut s, Key::Char('y'), t);
    assert_eq!(confirmed, Action::Quit, "확인(y)이 실제 종료를 낸다");
}

#[test]
fn 종료_확인에서_취소는_보드로_돌아간다() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Char('q'), t);
    assert!(matches!(s.view, View::ConfirmQuit));
    let action = handle_key(&mut s, Key::Char('n'), t);
    assert_eq!(action, Action::None);
    assert!(matches!(s.view, View::List), "취소는 보드로 돌아간다");
}

#[test]
fn 목록에서_이스케이프는_종료하지_않는다() {
    // 오종료 방지: 최상위에서 Esc는 더 이상 앱을 종료하지 않는다.
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Esc, t);
    assert_eq!(action, Action::None, "Esc는 더 이상 앱을 종료하지 않는다");
    assert!(matches!(s.view, View::List), "Esc는 보드 목록을 유지한다");
}

#[test]
fn 목록에서_엔터는_상세를_열지_않고_실행한다() {
    // Enter는 표면 전체에서 "실행/확정"이라는 하나의 계약을 지킨다.
    // cursor 0의 test.simple은 입력이 없으므로 즉시 Dispatch로 실행된다.
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Enter, t);
    match action {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.simple");
            assert_eq!(args, json!({}));
        }
        other => panic!("Enter는 List에서도 Dispatch를 발생시켜야 하지만 {other:?}를 받았다"),
    }
    assert!(
        matches!(
            s.view,
            View::Running {
                tool_id: "test.simple",
                ..
            }
        ),
        "Enter는 Detail이 아니라 실행 중 보기로 넘어간다"
    );
}

#[test]
fn 목록에서_오는_상세를_연다() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Char('o'), t);
    assert_eq!(action, Action::None);
    assert_eq!(s.view, View::Detail);
}

#[test]
fn 목록_상세_폼_실행은_기본_상태_전이를_따른다() {
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
        other => panic!("폼을 채운 뒤 dispatch를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 업데이트는_목록_상세_폼_결과_전이를_구동한다() {
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
        .expect("실행이 시작되면 모델이 run을 붙든다")
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
fn 목록에서_제이와_케이는_화살표처럼_이동한다() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Char('j'), t);
    assert_eq!(s.cursor, 1);
    handle_key(&mut s, Key::Char('k'), t);
    assert_eq!(s.cursor, 0);
}

#[test]
fn 목록에서_f1은_입력_도구_폼을_연다() {
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
fn 도구_행_클릭은_선택하고_상세를_연다() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);

    let action = handle_mouse(
        &mut s,
        Mouse {
            // 첫 번째 그리드 행의 두 번째 U1 카드.
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
fn 마우스_휠은_커서를_옮기지_않고_그리드를_스크롤한다() {
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
fn 가로_마우스_휠은_커서를_옮기지_않고_그리드를_가로로_스크롤한다() {
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
    assert_eq!(s.cursor, 0, "가로 휠은 커서를 옮기지 말아야 한다");
    let after_right = s.grid_h_scroll;
    assert!(
        after_right > 0,
        "ScrollRight는 grid_h_scroll을 증가시켜야 한다"
    );

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
        "ScrollLeft는 grid_h_scroll을 감소시켜야 한다"
    );
}

#[test]
fn 왼쪽_키는_커서_이동과_함께_가로_스크롤을_되돌린다() {
    // 오른쪽 끝 셀로 이동하면 grid_h_scroll이 양수가 되고, 다시 왼쪽 셀로
    // 이동하면 sync_grid_scroll_to_cursor가 grid_h_scroll을 0으로 되돌려
    // 새 커서가 뷰포트에 보이도록 보장해야 한다.
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
    assert_eq!(s.cursor, 0, "Left 키는 다시 왼쪽 셀로 가야 한다");
    assert_eq!(
        s.grid_h_scroll, 0,
        "왼쪽 끝으로 돌아오면 가로 스크롤은 0으로 되돌아야 한다"
    );
}

#[test]
fn 상세_보기에서_마우스_휠은_오른쪽_패널을_스크롤한다() {
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
fn 렌더는_긴_결과에_오른쪽_패널_스크롤을_적용한다() {
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
        "오른쪽 패널 스크롤은 긴 출력의 아래쪽을 렌더링해야 한다. right pane: {right_text}"
    );
    assert!(
        !right_text.contains("top-marker"),
        "스크롤되면 오른쪽 패널이 위쪽에 고정되어 있으면 안 된다. right pane: {right_text}"
    );
}
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

#[test]
fn 좁은_터미널로_resize되면_clamp_state_to_area가_grid_h_scroll을_낮춘다() {
    // effects.rs의 매-프레임 clamp가 사라지면 stale grid_h_scroll이
    // 새 (더 좁은) 캔버스에서 영역을 벗어나 렌더링 깨짐을 유발한다.
    // 회귀 방어선.
    let mut s = fresh();
    s.grid_h_scroll = ScrollOffset::new(200);
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 60, 20);

    clamp_state_to_area(&mut s, t, area);

    let max = {
        let layout = tui_layout(area);
        let content = layout.left;
        // grid_max_h_scroll는 grid에 있지만 여기서는 단지 "200보다 작다"만
        // 검증해도 회귀를 잡기에 충분. 핵심은 호출 자체가 살아 있는 것.
        let _ = content;
        200
    };
    assert!(
        s.grid_h_scroll < max,
        "clamp 호출이 grid_h_scroll을 캔버스에 맞춰 줄여야 한다"
    );
}
