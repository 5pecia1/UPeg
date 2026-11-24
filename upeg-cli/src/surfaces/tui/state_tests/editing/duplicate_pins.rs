use super::*;
use upeg_core::{ArgsPreset, PinId, PinSpan};

const BOARD: &str = "dev";
const TOOL: &str = "test.with_input";
const FIRST: &str = "first-instance";
const SECOND: &str = "second-instance";

fn duplicate_board() -> State {
    for tool in fixture_tools() {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    let mut state = fresh();
    state.filters.select_board(BOARD);
    let mut first = Placement::new(TOOL, 2, 0).with_args_preset(Some(
        ArgsPreset::parse(r#"{"input":"first"}"#).expect("first preset"),
    ));
    first.pin_id = PinId::parse(FIRST).expect("first pin id");
    let mut second = Placement::new(TOOL, 0, 1).with_args_preset(Some(
        ArgsPreset::parse(r#"{"input":"second"}"#).expect("second preset"),
    ));
    second.pin_id = PinId::parse(SECOND).expect("second pin id");
    // Storage order deliberately differs from the grid's row-major order.
    state.layouts.insert(BOARD.into(), vec![second, first]);
    state.cursor = 1;
    state
}

fn press(state: &mut State, key: Key) -> Effect {
    let tools = state.visible_tools();
    handle_key(state, key, &tools)
}

fn pin<'a>(state: &'a State, id: &str) -> &'a Placement {
    state.layouts[BOARD]
        .iter()
        .find(|pin| pin.pin_id.as_str() == id)
        .expect("pin present")
}

fn assert_focus(state: &State, id: &str) {
    let selected = state.selected_board_pin().expect("selected pin");
    assert_eq!(selected.pin_id.as_str(), id);
}

#[test]
fn duplicate_pin_edits_keep_identity_through_preview_reorder_and_unpin() {
    let mut state = duplicate_board();
    let original = pin(&state, FIRST).clone();
    assert_focus(&state, SECOND);

    assert_eq!(press(&mut state, Key::Char('c')), Effect::None);
    assert_eq!(press(&mut state, Key::Char('2')), Effect::None);
    assert_eq!(press(&mut state, Key::Enter), Effect::SavePegboard);
    assert!(pin(&state, SECOND).color.is_some());
    assert_eq!(pin(&state, FIRST), &original);

    for exit in [Key::Esc, Key::Enter] {
        assert_eq!(press(&mut state, Key::Char('m')), Effect::None);
        assert_eq!(press(&mut state, Key::Up), Effect::None);
        assert_eq!(
            state.cursor, 0,
            "cursor follows the preview across grid order"
        );
        assert_focus(&state, SECOND);
        let effect = press(&mut state, exit);
        assert_focus(&state, SECOND);
        assert_eq!(pin(&state, FIRST), &original);
        if exit == Key::Esc {
            assert_eq!(effect, Effect::None);
            assert_eq!(state.cursor, 1);
            assert_eq!(pin(&state, SECOND).y, 1);
        } else {
            assert_eq!(effect, Effect::SavePegboard);
            assert_eq!(pin(&state, SECOND).y, 0);
        }
    }

    assert_eq!(press(&mut state, Key::Char(']')), Effect::SavePegboard);
    assert_focus(&state, SECOND);
    assert_eq!(pin(&state, SECOND).x, 2);
    assert_eq!(pin(&state, FIRST).x, 0);
    let first_after_swap = pin(&state, FIRST).clone();
    assert_eq!(first_after_swap.args_preset, original.args_preset);

    assert_eq!(press(&mut state, Key::Char('e')), Effect::None);
    assert_eq!(press(&mut state, Key::Right), Effect::None);
    assert_eq!(press(&mut state, Key::Enter), Effect::SavePegboard);
    assert_focus(&state, SECOND);
    assert_eq!(
        pin(&state, SECOND).span.map(PinSpan::grid_span),
        Some((2, 1))
    );
    assert_eq!(pin(&state, FIRST), &first_after_swap);

    assert_eq!(press(&mut state, Key::Char('p')), Effect::SavePegboard);
    assert_eq!(state.layouts[BOARD], [first_after_swap]);
    assert_focus(&state, FIRST);
}

#[test]
fn duplicate_pin_form_uses_the_selected_instances_saved_preset() {
    let mut state = duplicate_board();
    assert_eq!(press(&mut state, Key::Enter), Effect::None);
    let View::Form { form, .. } = &state.view else {
        panic!("expected form for selected pin");
    };
    assert_eq!(
        form.args().expect("preset supplies required argument"),
        json!({"input":"second"})
    );
    assert!(!state.can_reload_external_pegboard());

    let effect = press(&mut state, Key::F(1));
    assert!(matches!(
        effect,
        Effect::Dispatch { tool_id: TOOL, args, .. } if args == json!({"input":"second"})
    ));
    let first = pin(&state, FIRST)
        .args_preset
        .as_ref()
        .expect("first preset");
    assert_eq!(first.to_object()["input"], "first");
}
