//! Source-adjacent tests for the TUI surface. Split into this sibling
//! file so the production module can focus on the State/Action machine,
//! the ratatui render functions, and the `serve` I/O wrapper.
//!
//! Test scope: fixture-based cursor/form behavior (no inventory
//! coupling), key/mouse routing, layout helpers, surface gating, and
//! manifest text rendering.

use crate::domain::execution::dispatch::Outcome;
use crate::surfaces::tui::model::{
    BoardEditMode, BoardFilter, MoveMode, ResizeMode, ToolPickerMode, ToolPickerReturn,
};
use crate::surfaces::tui::*;
use ratatui::Terminal;
use ratatui::layout::Rect;
use ratatui::text::Span;
use serde_json::json;
use std::collections::BTreeMap;
use upeg_core::{
    DraftInputValue, InputFieldSpec, InputKind, InputName, InputSpec, Invoker, PinKind, Placement,
    Surface, ToolMeta,
};

pub(crate) fn fresh() -> State {
    // Since phase 2, State caches the on-disk boards/layouts originally
    // loaded via `pegboard::load_state`. Tests that verify filter
    // cycling/focus behavior need at least one board, so we seed with
    // `default_state` (dev / trading / personal). Tests that need an
    // *empty* board set must build State explicitly.
    let snapshot = upeg_sources::pegboard::default_state();
    State {
        boards: snapshot.boards,
        layouts: snapshot.layouts,
        ..State::default()
    }
}

#[test]
fn replace_pegboard_caches_external_boards_and_keeps_current_board() {
    let mut s = fresh();
    s.filters.select_board("dev");
    s.filters.select_tag("convert");
    let mut next = upeg_sources::pegboard::default_state();
    next.boards.push(upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "gui".into(),
        title: "GUI".into(),
    });
    next.layouts.insert(
        "gui".into(),
        vec![Placement::new("num.hex_to_decimal", 0, 0)],
    );

    s.replace_pegboard(next);

    assert!(
        s.board_filter_options()
            .iter()
            .any(|option| option == "gui"),
        "an externally added board must appear in the next frame's filter options"
    );
    assert_eq!(s.filters.board.as_deref(), Some("dev"));
    assert_eq!(s.board_cursor, 0);
    assert!(
        s.tag_filter_options()
            .iter()
            .any(|option| option == "convert"),
        "a tag filter still valid on the current board must be kept"
    );
}

#[test]
fn replace_pegboard_resets_removed_board_filter_to_all() {
    let mut s = fresh();
    s.filters.select_board("dev");
    s.cursor = usize::MAX;
    let next = upeg_sources::pegboard::PegboardState {
        boards: vec![upeg_sources::pegboard::BoardData {
            guidance: upeg_core::BoardGuidance::default(),
            key: "gui".into(),
            title: "GUI".into(),
        }],
        layouts: BTreeMap::from([(
            "gui".into(),
            vec![Placement::new("num.hex_to_decimal", 0, 0)],
        )]),
        selection: upeg_sources::pegboard::PegboardSelection::default(),
    };

    s.replace_pegboard(next);

    assert!(
        s.filters.board.is_none(),
        "when the current board is removed externally, the stale board filter must not be kept"
    );
    assert_eq!(s.board_cursor, 0);
    assert!(s.cursor < s.visible_placements().len());
}

#[test]
fn external_pegboard_reload_is_deferred_during_board_edits_and_move_mode() {
    let mut s = fresh();
    assert!(s.can_reload_external_pegboard());

    s.view = View::BoardEditor {
        mode: BoardEditMode::AddBoard,
        buffer: String::new(),
    };
    assert!(!s.can_reload_external_pegboard());

    s.view = View::ConfirmDeleteBoard {
        key: "dev".into(),
        title: "Dev".into(),
    };
    assert!(!s.can_reload_external_pegboard());

    s.view = View::ToolPicker {
        mode: ToolPickerMode::Pin,
        return_to: ToolPickerReturn::list(FocusArea::Grid),
        query: String::new(),
        cursor: 0,
    };
    assert!(!s.can_reload_external_pegboard());

    s.view = View::List;
    s.move_mode = Some(MoveMode {
        board: "dev".into(),
        tool_id: "num.hex_to_decimal",
        target_x: 0,
        target_y: 0,
        base: Vec::new(),
        preview: Vec::new(),
    });
    assert!(!s.can_reload_external_pegboard());

    s.move_mode = None;
    s.resize_mode = Some(ResizeMode {
        board: "dev".into(),
        tool_id: "num.hex_to_decimal",
        cols: 1,
        rows: 1,
        base: Vec::new(),
        preview: Vec::new(),
    });
    assert!(
        !s.can_reload_external_pegboard(),
        "external pegboard reload must also be deferred during a resize preview"
    );
}

fn layout_contains(state: &State, board: &str, tool_id: &str) -> bool {
    state
        .layouts
        .get(board)
        .is_some_and(|placements| placements.iter().any(|p| p.tool_id == tool_id))
}

fn layout_ids_by_position(state: &State, board: &str) -> Vec<String> {
    let mut placements = state.layouts.get(board).cloned().unwrap_or_default();
    placements.sort_by_key(|p| (p.y, p.x));
    placements.into_iter().map(|p| p.tool_id).collect()
}

fn string_field(name: &str, required: bool) -> InputFieldSpec {
    InputFieldSpec::new(
        InputName::new(name).expect("test input name"),
        None,
        None,
        required,
        InputKind::String,
    )
    .expect("test string field")
}

fn string_form(name: &str, value: &str, required: bool) -> TuiFormState {
    let spec = InputSpec::new(vec![string_field(name, required)]).expect("test input spec");
    let mut form = TuiFormState::new(spec);
    set_form_text(&mut form, 0, value);
    form
}

fn set_form_text(form: &mut TuiFormState, index: usize, value: &str) {
    if let Some(field) = form.fields.get_mut(index)
        && let DraftInputValue::Text(text) = &mut field.draft
    {
        *text = value.to_string();
    }
}

/// Builds a fixed fake tool list so cursor/form behavior does not
/// depend on inventory-registered entries.
pub(crate) fn fixture_tools() -> [&'static ToolMeta; 3] {
    static SIMPLE: ToolMeta = ToolMeta {
        id: "test.simple",
        toolkit: "test",
        local_id: "simple",
        tags: &[],
        display_label: "Test tool",
        description: "Simple no-arg tool",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    let with_input = Box::leak(Box::new(ToolMeta {
        id: "test.with_input",
        toolkit: "test",
        local_id: "with_input",
        tags: &[],
        display_label: "Test tool",
        description: "Tool that takes an input string",
        input_spec: upeg_core::InputSpec::new(vec![
            InputFieldSpec::new(
                InputName::new("input").expect("test input name"),
                None,
                Some("Some text".to_string()),
                true,
                InputKind::String,
            )
            .expect("test input field"),
        ])
        .expect("test input spec"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    }));
    static THREE: ToolMeta = ToolMeta {
        id: "test.third",
        toolkit: "test",
        local_id: "third",
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    [&SIMPLE, with_input, &THREE]
}

fn span_fixture_tools() -> [&'static ToolMeta; 4] {
    static WIDE: ToolMeta = ToolMeta {
        id: "test.wide",
        toolkit: "test",
        local_id: "wide",
        tags: &[],
        display_label: "Wide",
        description: "Two columns",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    static TALL: ToolMeta = ToolMeta {
        id: "test.tall",
        toolkit: "test",
        local_id: "tall",
        tags: &[],
        display_label: "Tall",
        description: "Two rows",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    static ONE: ToolMeta = ToolMeta {
        id: "test.one",
        toolkit: "test",
        local_id: "one",
        tags: &[],
        display_label: "One",
        description: "One cell",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    static FOUR: ToolMeta = ToolMeta {
        id: "test.four",
        toolkit: "test",
        local_id: "four",
        tags: &[],
        display_label: "Four",
        description: "One cell",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    [&WIDE, &TALL, &ONE, &FOUR]
}

fn wrapped_grid_fixture_tools() -> [&'static ToolMeta; 5] {
    static ONE: ToolMeta = ToolMeta {
        id: "test.wrap_one",
        toolkit: "test",
        local_id: "wrap_one",
        tags: &[],
        display_label: "One",
        description: "One cell",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    static TWO: ToolMeta = ToolMeta {
        id: "test.wrap_two",
        toolkit: "test",
        local_id: "wrap_two",
        tags: &[],
        display_label: "Two",
        description: "One cell",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    static THREE: ToolMeta = ToolMeta {
        id: "test.wrap_three",
        toolkit: "test",
        local_id: "wrap_three",
        tags: &[],
        display_label: "Three",
        description: "One cell",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    static FOUR: ToolMeta = ToolMeta {
        id: "test.wrap_four",
        toolkit: "test",
        local_id: "wrap_four",
        tags: &[],
        display_label: "Four",
        description: "One cell",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    static FIVE: ToolMeta = ToolMeta {
        id: "test.wrap_five",
        toolkit: "test",
        local_id: "wrap_five",
        tags: &[],
        display_label: "Five",
        description: "One cell",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    [&ONE, &TWO, &THREE, &FOUR, &FIVE]
}

fn buffer_region_text(buffer: &ratatui::buffer::Buffer, area: Rect) -> String {
    let mut text = String::new();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

pub(crate) fn filter_option_column(
    area: Rect,
    prefix: &str,
    options: &[String],
    index: usize,
) -> u16 {
    let preceding_options_width = options
        .iter()
        .take(index)
        .map(|option| Span::raw(format!(" {option} ")).width() as u16)
        .sum::<u16>();
    area.x
        .saturating_add(1)
        .saturating_add(Span::raw(prefix).width() as u16)
        .saturating_add(preceding_options_width)
        .saturating_add(1)
}

mod approval_run;
mod detail_settings;
mod editing;
mod narrow_layout;
mod navigation;

/// Board filter options must not show "all" as a visible option, but
/// must list the real boards. Like the Flutter GUI, the board bar must
/// not render an "all" chip.
#[test]
fn board_filter_options_return_only_real_boards_not_all() {
    let s = fresh();
    let options = s.board_filter_options();

    // "all" must not be a visible option (it exists only as internal
    // state)
    assert!(
        !options.contains(&"all".to_string()),
        "board filter options must not contain 'all'"
    );

    // But the real board keys must be present
    assert!(
        options.iter().any(|o| o == "dev"),
        "board filter options must contain 'dev'"
    );
    assert!(
        options.iter().any(|o| o == "trading"),
        "board filter options must contain 'trading'"
    );
    assert!(
        options.iter().any(|o| o == "personal"),
        "board filter options must contain 'personal'"
    );
}

/// Tag filter options must still include "all" (unlike board filters).
#[test]
fn tag_filter_options_still_include_all() {
    let s = fresh();
    let options = s.tag_filter_options();

    // The tag filter must keep showing "all" (same as the Flutter GUI)
    assert!(
        options.contains(&"all".to_string()),
        "tag filter options must contain 'all'"
    );
}

/// The digit key '0' must still return to the all-boards filter
/// (BoardFilter::All). The clear-board behavior must keep working.
#[test]
fn digit_0_key_returns_to_all_boards_filter() {
    let mut s = fresh();
    s.filters.select_board("dev");

    let result = s.board_for_digit('0');

    assert!(
        result.is_some() && result.unwrap().is_none(),
        "digit 0 key must return BoardFilter::All"
    );
}

/// The digit key '1' must select the first real board (index remapping).
/// Before: '1' pointed at "all"; after: '1' points at the first board
/// ("dev").
#[test]
fn digit_1_key_selects_first_real_board() {
    let s = fresh();

    let result = s.board_for_digit('1');

    assert!(
        result.is_some() && result.unwrap().as_deref() == Some("dev"),
        "digit 1 key must return the first board ('dev')"
    );
}

/// When cycling the board filter with 'b', if current is All it must
/// move to the first real board.
#[test]
fn board_cycle_moves_from_all_to_first_real_board() {
    use crate::surfaces::tui::model::cycle_board_filter;

    let s = fresh();
    let options = s.board_filter_options();

    // Cycle from the All state
    let current = BoardFilter::All;
    let next = cycle_board_filter(&current, &options);

    // Must cycle to the first real board
    assert!(
        next.as_deref() == Some("dev"),
        "cycling from All must move to the first board ('dev')"
    );
}
