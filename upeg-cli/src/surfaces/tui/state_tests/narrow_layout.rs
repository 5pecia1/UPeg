//! PR #13 regression guard: on narrow screens mouse routing and focus
//! must stay aligned with the render.
//!
//! - P1 (Critical): if handle_mouse hit-tests against layout.left /
//!   layout.right, then in narrow + Form the body covers all of
//!   layout.left and clicks leak into grid hits, jumping to
//!   View::Detail (Form input loss). Guarded by body_rects + the
//!   is_dialog check.
//! - P2: in narrow + List a leftover RightPane focus sends key input
//!   to the invisible panel and the grid stops responding.
//!   clamp_state_to_area re-adjusts focus to a visible surface every
//!   frame, and the Tab cycle skips invisible panes.

use super::*;

// ─────────────────── P1: mouse routing alignment ───────────────────

#[test]
fn click_over_narrow_form_does_not_leave_form() {
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
    // Width 50 < MIN_DUAL_PANE_WIDTH(60) → narrow. Form is
    // right_pane_dominates, so the whole body must behave like the
    // right pane.
    let area = Rect::new(0, 0, 50, 30);
    let layout = tui_layout(area);
    assert!(layout.is_narrow(), "precondition: width 50 must be narrow");

    // Under the old code path (layout.left-based) this coordinate would
    // have been treated as a grid hit and changed state.view to Detail.
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
        "in narrow + Form a body click must not leave the Form. got: {:?}",
        s.view,
    );
    // The Form body is routed as the right pane, so focus must go to
    // RightPane.
    assert_eq!(s.focus, FocusArea::RightPane);
}

#[test]
fn wheel_scroll_over_narrow_form_moves_right_scroll() {
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
        "in narrow + Form the wheel must advance right_scroll. before={:?} after={:?}",
        before,
        s.right_scroll,
    );
}

#[test]
fn body_click_in_narrow_list_still_selects_grid() {
    // Verify the body_rects introduction did not break grid clicks in
    // narrow + List.
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 50, 30); // narrow + View::List → the board is home base
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
fn body_click_over_dialog_view_does_not_change_focus() {
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
        "a click over Settings must not change the view"
    );
    assert_eq!(
        s.focus,
        FocusArea::Boards,
        "a body click over a dialog must not touch focus either"
    );
}

// ─────────────────── P2: focus re-adjustment + Tab skip ───────────────────

#[test]
fn resize_to_narrow_list_readjusts_right_pane_focus_to_grid() {
    let mut s = State {
        focus: FocusArea::RightPane,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 50, 30); // narrow + List → grid home base

    clamp_state_to_area(&mut s, t, area);
    assert_eq!(
        s.focus,
        FocusArea::Grid,
        "in narrow + List, RightPane focus must be re-adjusted to Grid"
    );
}

#[test]
fn resize_to_narrow_form_readjusts_grid_focus_to_right_pane() {
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
        "in narrow + Form, Grid focus must be re-adjusted to RightPane"
    );
}

#[test]
fn clamp_preserves_focus_on_wide_screen() {
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
        "in wide mode RightPane focus must be preserved"
    );
}

#[test]
fn tab_in_narrow_list_skips_invisible_right_pane() {
    let mut s = State {
        focus: FocusArea::Tags,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 50, 30); // narrow + List → right pane not visible

    // Tags → (next: RightPane, skipped) → Grid
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
        "Tab in narrow + List must skip RightPane and land on Grid"
    );
}

// In the Form view, Tab handles form-field movement (the
// handle_focus_command call is blocked — the
// `!matches!(state.view, View::Form {..})` guard in update.rs). So the
// narrow + Form surface skip is unreachable via Tab. The corresponding
// regression guard is covered by the
// `resize_to_narrow_form_readjusts…` test above, where
// clamp_state_to_area re-adjusts Grid focus to RightPane.

// ─────────────────── cycle-back termination guard ───────────────────

#[test]
fn tab_cycle_in_narrow_list_terminates_within_one_cycle() {
    // next_focus prevents infinite loops via cycle-back termination.
    // Since `cursor != focus` guarantees termination even if a
    // FocusArea variant is added (rather than a magic number like
    // 0..4), this regression guard simply asserts "the cycle ends
    // without panic/hang".
    let mut s = State {
        focus: FocusArea::Grid,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 50, 30); // narrow + List → grid home base, right collapsed

    // Grid → (next: Boards, visible) → Boards
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
        "must move Grid → Boards, the first visible"
    );

    // Boards → Tags → (RightPane, not visible → skip) → (Grid, visible) → Grid
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
        "two Tabs must go Tags → Grid (RightPane skip)"
    );
}
