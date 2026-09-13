use super::*;
use crate::surfaces::tui::model::{
    PIN_COLOR_PALETTE, PinColorEditorDraft, pin_color_palette_for_digit,
};
use upeg_core::PinColorHex;

const BOARD_KEY: &str = "dev";
const TOOL_ID: &str = "num.hex_to_decimal";
const ORIGINAL_COLOR: &str = "#112233";
const CUSTOM_COLOR_TYPED: &str = "#12abcf";
const CUSTOM_COLOR_AFTER_BACKSPACE: &str = "#12abc";
const CUSTOM_COLOR_REPLACEMENT_SUFFIX: char = 'e';
const CUSTOM_COLOR_APPLIED: &str = "#12ABCE";
const PALETTE_DIGIT: char = '2';

fn pin_color_state(color: Option<PinColorHex>) -> State {
    let mut state = fresh();
    state.filters.select_board(BOARD_KEY);
    state.layouts.insert(
        BOARD_KEY.to_string(),
        vec![Placement::new(TOOL_ID, 0, 0).with_color(color)],
    );
    state.cursor = 0;
    state
}

fn visible_tools_for(state: &State) -> Vec<&'static ToolMeta> {
    state.visible_tools()
}

fn open_editor(state: &mut State) -> Vec<&'static ToolMeta> {
    let tools = visible_tools_for(state);
    let effect = handle_key(state, Key::Char('c'), &tools);
    assert_eq!(effect, Effect::None);
    tools
}

fn placement_color(state: &State) -> Option<&str> {
    state
        .layouts
        .get(BOARD_KEY)
        .and_then(|placements| {
            placements
                .iter()
                .find(|placement| placement.tool_id == TOOL_ID)
        })
        .and_then(|placement| placement.color.as_ref())
        .map(PinColorHex::as_str)
}

#[test]
fn 편집_모드에서_포커스된_핀의_c는_색상_편집기를_연다() {
    let original = PinColorHex::parse(ORIGINAL_COLOR).expect("테스트 원본 색상");
    let mut state = pin_color_state(Some(original.clone()));
    let tools = visible_tools_for(&state);

    let effect = handle_key(&mut state, Key::Char('c'), &tools);

    assert_eq!(effect, Effect::None);
    match &state.view {
        View::PinColorEditor(editor) => {
            assert_eq!(editor.board, BOARD_KEY);
            assert_eq!(editor.tool_id, TOOL_ID);
            assert_eq!(editor.original.as_ref(), Some(&original));
            assert_eq!(editor.draft, PinColorEditorDraft::Existing(original));
        }
        other => panic!("색상 편집기를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 편집_모드여도_포커스된_핀이_없으면_c는_아무_일도_하지_않는다() {
    let mut state = fresh();
    state.filters.select_board(BOARD_KEY);
    state.layouts.insert(BOARD_KEY.to_string(), Vec::new());
    let tools = visible_tools_for(&state);

    let effect = handle_key(&mut state, Key::Char('c'), &tools);

    assert_eq!(effect, Effect::None);
    assert!(matches!(state.view, View::List));
}

#[test]
fn 색상_편집기에서_팔레트_숫자는_해당_색상_초안을_고른다() {
    let mut state = pin_color_state(None);
    let tools = open_editor(&mut state);

    let effect = handle_key(&mut state, Key::Char(PALETTE_DIGIT), &tools);

    assert_eq!(effect, Effect::None);
    let expected = pin_color_palette_for_digit(PALETTE_DIGIT).expect("테스트 팔레트 숫자");
    match &state.view {
        View::PinColorEditor(editor) => {
            assert_eq!(editor.draft, PinColorEditorDraft::Palette(expected));
            assert_eq!(
                PIN_COLOR_PALETTE.len(),
                9,
                "팔레트 숫자 키는 1-9와 대응해야 한다"
            );
        }
        other => panic!("색상 편집기를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 색상_편집기는_커스텀_hex를_입력하고_백스페이스로_수정한다() {
    let mut state = pin_color_state(None);
    let tools = open_editor(&mut state);

    for ch in CUSTOM_COLOR_TYPED.chars() {
        handle_key(&mut state, Key::Char(ch), &tools);
    }
    handle_key(&mut state, Key::Backspace, &tools);
    handle_key(
        &mut state,
        Key::Char(CUSTOM_COLOR_REPLACEMENT_SUFFIX),
        &tools,
    );

    match &state.view {
        View::PinColorEditor(editor) => {
            assert_ne!(CUSTOM_COLOR_TYPED, CUSTOM_COLOR_APPLIED);
            assert_eq!(
                editor.draft,
                PinColorEditorDraft::Custom(format!(
                    "{CUSTOM_COLOR_AFTER_BACKSPACE}{CUSTOM_COLOR_REPLACEMENT_SUFFIX}"
                ))
            );
        }
        other => panic!("색상 편집기를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 색상_편집기에서_ctrl_u는_입력_중인_초안을_지운다() {
    let mut state = pin_color_state(None);
    let tools = open_editor(&mut state);
    for ch in CUSTOM_COLOR_TYPED.chars() {
        handle_key(&mut state, Key::Char(ch), &tools);
    }

    let ctrl_u = upeg_core::KeyStroke::modified(
        Key::Char('u'),
        upeg_core::KeyModifiers {
            control: true,
            ..upeg_core::KeyModifiers::NONE
        },
    );
    let effect = update(
        &mut state,
        Msg::KeyPress {
            stroke: ctrl_u,
            tools: &tools,
            area: None,
        },
    );

    assert_eq!(effect, Effect::None);
    match &state.view {
        View::PinColorEditor(editor) => {
            assert_eq!(editor.draft, PinColorEditorDraft::Empty);
        }
        other => panic!("색상 편집기를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 색상_편집기에서_f1은_enter와_동일하게_적용한다() {
    let original = PinColorHex::parse(ORIGINAL_COLOR).expect("테스트 원본 색상");
    let mut state = pin_color_state(Some(original));
    let tools = open_editor(&mut state);
    handle_key(&mut state, Key::Char(PALETTE_DIGIT), &tools);

    let effect = handle_key(&mut state, Key::F(1), &tools);

    assert_eq!(effect, Effect::SavePegboard);
    assert_eq!(state.view, View::List);
    assert_eq!(
        placement_color(&state),
        Some(PIN_COLOR_PALETTE[PALETTE_DIGIT.to_digit(10).unwrap() as usize - 1])
    );
}

#[test]
fn 색상_편집기는_enter_적용_r_초기화_esc와_q_취소를_구분한다() {
    let original = PinColorHex::parse(ORIGINAL_COLOR).expect("테스트 원본 색상");

    let mut apply_state = pin_color_state(Some(original.clone()));
    let tools = open_editor(&mut apply_state);
    for ch in CUSTOM_COLOR_TYPED.chars() {
        handle_key(&mut apply_state, Key::Char(ch), &tools);
    }
    handle_key(&mut apply_state, Key::Backspace, &tools);
    handle_key(
        &mut apply_state,
        Key::Char(CUSTOM_COLOR_REPLACEMENT_SUFFIX),
        &tools,
    );
    let apply_effect = handle_key(&mut apply_state, Key::Enter, &tools);
    assert_eq!(apply_effect, Effect::SavePegboard);
    assert_eq!(placement_color(&apply_state), Some(CUSTOM_COLOR_APPLIED));
    assert!(matches!(apply_state.view, View::List));

    let mut reset_state = pin_color_state(Some(original.clone()));
    let tools = open_editor(&mut reset_state);
    let reset_effect = handle_key(&mut reset_state, Key::Char('r'), &tools);
    assert_eq!(reset_effect, Effect::SavePegboard);
    assert_eq!(placement_color(&reset_state), None);
    assert!(matches!(reset_state.view, View::List));

    for cancel_key in [Key::Esc, Key::Char('q')] {
        let mut cancel_state = pin_color_state(Some(original.clone()));
        let tools = open_editor(&mut cancel_state);
        let cancel_effect = handle_key(&mut cancel_state, cancel_key, &tools);
        assert_eq!(cancel_effect, Effect::None);
        assert_eq!(placement_color(&cancel_state), Some(ORIGINAL_COLOR));
        assert!(matches!(cancel_state.view, View::List));
    }
}
