//! PR #13 회귀 방어: narrow 화면에서 마우스 라우팅과 focus 가
//! 렌더와 정렬되어야 한다.
//!
//! - P1 (Critical): handle_mouse 가 layout.left / layout.right 기반으로
//!   hit-test 하면 narrow + Form 에서 body 가 layout.left 전체를 덮어
//!   클릭이 grid hit 으로 새고 View::Detail 로 점프 (Form 입력 손실).
//!   body_rects + is_dialog 가드로 막는다.
//! - P2: narrow + List 에서 RightPane focus 가 남으면 키 입력이
//!   invisible 패널로 흘러 grid 무응답. clamp_state_to_area 가 매-프레임
//!   focus 를 visible surface 로 재조정하고, Tab 사이클이 invisible pane
//!   을 skip 한다.

use super::*;

// ─────────────────── P1: mouse routing 정렬 ───────────────────

#[test]
fn narrow_form_위_클릭은_form_을_떠나지_않는다() {
    let mut s = State {
        cursor: 0,
        view: View::Form {
            tool_id: "test.with_input",
            form: TuiFormState::new(upeg_core::InputSpec::empty()),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    // 폭 50 < MIN_DUAL_PANE_WIDTH(60) → narrow. Form 은 right_pane_dominates
    // 이므로 body 전체가 우측 패널처럼 동작해야 한다.
    let area = Rect::new(0, 0, 50, 30);
    let layout = tui_layout(area);
    assert!(layout.is_narrow(), "전제: 폭 50은 narrow 여야 한다");

    // 옛 코드 경로 (layout.left 기반) 라면 이 좌표는 grid hit 으로 처리되어
    // state.view 를 Detail 로 바꿨을 것.
    handle_mouse(
        &mut s,
        Mouse {
            column: layout.left.x + 2,
            row: layout.left.y + 2,
            kind: MouseKind::Pointer(PointerPhase::Down),
        },
        t,
        area,
    );

    assert!(
        matches!(s.view, View::Form { .. }),
        "narrow + Form 에서 body 클릭이 Form 을 벗어나면 안 된다. got: {:?}",
        s.view,
    );
    // Form 본문은 right pane 으로 라우팅되어 focus 가 RightPane 으로 가야.
    assert_eq!(s.focus, FocusArea::RightPane);
}

#[test]
fn narrow_form_위_휠스크롤은_right_scroll을_움직인다() {
    let mut s = State {
        cursor: 0,
        view: View::Form {
            tool_id: "test.with_input",
            form: TuiFormState::new(upeg_core::InputSpec::empty()),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 50, 30);
    let layout = tui_layout(area);

    let before = s.right_scroll;
    handle_mouse(
        &mut s,
        Mouse {
            column: layout.left.x + 5,
            row: layout.left.y + 5,
            kind: MouseKind::Scroll(ScrollDelta::VERTICAL_FORWARD),
        },
        t,
        area,
    );
    assert!(
        s.right_scroll > before.get(),
        "narrow + Form 에서 휠은 right_scroll 을 진행시켜야 한다. before={:?} after={:?}",
        before,
        s.right_scroll,
    );
}

#[test]
fn narrow_list_에서_body_클릭은_그리드를_여전히_선택한다() {
    // body_rects 도입이 narrow + List 의 그리드 클릭을 깨뜨리지 않는지.
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 50, 30); // narrow + View::List → 보드가 본진
    let layout = tui_layout(area);

    handle_mouse(
        &mut s,
        Mouse {
            column: layout.left.x + 2,
            row: layout.left.y + 2,
            kind: MouseKind::Pointer(PointerPhase::Down),
        },
        t,
        area,
    );
    assert_eq!(s.focus, FocusArea::Grid);
}

#[test]
fn dialog_뷰_위_body_클릭은_focus를_바꾸지_않는다() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        focus: FocusArea::Boards,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);

    handle_mouse(
        &mut s,
        Mouse {
            column: layout.left.x + 10,
            row: layout.left.y + 10,
            kind: MouseKind::Pointer(PointerPhase::Down),
        },
        t,
        area,
    );
    assert!(
        matches!(s.view, View::Settings { .. }),
        "Settings 위 클릭은 view 를 바꾸면 안 된다"
    );
    assert_eq!(
        s.focus,
        FocusArea::Boards,
        "dialog 위 body 클릭은 focus 도 건드리면 안 된다"
    );
}

// ─────────────────── P2: focus 재조정 + Tab skip ───────────────────

#[test]
fn narrow_list로_resize되면_right_pane_focus가_grid로_재조정된다() {
    let mut s = State {
        focus: FocusArea::RightPane,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 50, 30); // narrow + List → grid 본진

    clamp_state_to_area(&mut s, t, area);
    assert_eq!(
        s.focus,
        FocusArea::Grid,
        "narrow + List 에서 RightPane focus 는 Grid 로 재조정되어야 한다"
    );
}

#[test]
fn narrow_form으로_resize되면_grid_focus가_right_pane으로_재조정된다() {
    let mut s = State {
        focus: FocusArea::Grid,
        view: View::Form {
            tool_id: "test.with_input",
            form: TuiFormState::new(upeg_core::InputSpec::empty()),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 50, 30);

    clamp_state_to_area(&mut s, t, area);
    assert_eq!(
        s.focus,
        FocusArea::RightPane,
        "narrow + Form 에서 Grid focus 는 RightPane 으로 재조정되어야 한다"
    );
}

#[test]
fn 넓은_화면에서는_clamp가_focus를_보존한다() {
    let mut s = State {
        focus: FocusArea::RightPane,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 100, 30);

    clamp_state_to_area(&mut s, t, area);
    assert_eq!(
        s.focus,
        FocusArea::RightPane,
        "wide-mode 에서는 RightPane focus 가 유지되어야 한다"
    );
}

#[test]
fn narrow_list_에서_tab은_invisible_right_pane을_건너뛴다() {
    let mut s = State {
        focus: FocusArea::Tags,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 50, 30); // narrow + List → right 안 보임

    // Tags → (next: RightPane, 건너뜀) → Grid
    update(
        &mut s,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Tab),
            tools: t,
            area: Some(area),
        },
    );
    assert_eq!(
        s.focus,
        FocusArea::Grid,
        "narrow + List 의 Tab 은 RightPane 을 건너뛰고 Grid 로 와야 한다"
    );
}

// Form view 에서 Tab 은 폼 필드 이동을 담당 (handle_focus_command 호출이
// 차단되어 있음 — update.rs 의 `!matches!(state.view, View::Form {..})` 가드).
// 따라서 narrow + Form 의 surface skip 은 Tab 으로는 도달되지 않는다.
// 대응되는 회귀 방어는 clamp_state_to_area 가 Grid focus 를
// RightPane 으로 재조정한다는 위의 `narrow_form으로_resize되면…` 테스트가 담당.

// ─────────────────── cycle-back 종료 가드 ───────────────────

#[test]
fn narrow_list_에서_grid_focus_상태로_tab을_누르면_cycle은_정확히_한_바퀴안에_종료한다() {
    // next_focus 는 cycle-back termination 으로 무한루프를 방지한다.
    // FocusArea variant 가 추가되어도 magic number(0..4) 가 아니라
    // `cursor != focus` 가 종료를 보장하므로, 이 회귀 가드는 단순히
    // "cycle 이 panic/hang 없이 끝난다" 를 단정한다.
    let mut s = State {
        focus: FocusArea::Grid,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 50, 30); // narrow + List → grid 본진, right 접힘

    // Grid → (next: Boards, 보임) → Boards
    update(
        &mut s,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Tab),
            tools: t,
            area: Some(area),
        },
    );
    assert_eq!(
        s.focus,
        FocusArea::Boards,
        "Grid → Boards 첫 visible 로 이동해야 한다"
    );

    // Boards → Tags → (RightPane, 안 보임 → skip) → (Grid, 보임) → Grid
    update(
        &mut s,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Tab),
            tools: t,
            area: Some(area),
        },
    );
    update(
        &mut s,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Tab),
            tools: t,
            area: Some(area),
        },
    );
    assert_eq!(
        s.focus,
        FocusArea::Grid,
        "Tab 두 번에 Tags → Grid (RightPane skip) 으로 와야 한다"
    );
}
