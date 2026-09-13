//! TUI 표면의 소스 인접 테스트. 프로덕션 모듈이 State/Action 머신,
//! ratatui 렌더 함수, I/O 래퍼 `serve`에 집중할 수 있도록 이 형제 파일로
//! 분리했다.
//!
//! 테스트 범위: fixture 기반 커서/폼 동작(인벤토리 결합 없음), 키/마우스
//! 라우팅, 레이아웃 헬퍼, 표면 게이팅, 매니페스트 텍스트 렌더링.

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
    // 2단계 이후 State는 원래 `pegboard::load_state`를 통해 디스크에 있던
    // 보드/레이아웃을 캐시한다. 필터 순환/포커스 동작을 검증하는 테스트는
    // 보드가 적어도 하나 필요하므로 `default_state`(dev / trading /
    // personal)로 준비한다. *빈* 보드 집합이 필요한 테스트는 State를
    // 명시적으로 만들어야 한다.
    let snapshot = upeg_sources::pegboard::default_state();
    State {
        boards: snapshot.boards,
        layouts: snapshot.layouts,
        ..State::default()
    }
}

#[test]
fn replace_pegboard는_외부_보드를_캐시에_반영하고_현재_보드를_유지한다() {
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
        "외부에서 추가된 보드는 다음 프레임의 필터 옵션에 보여야 한다"
    );
    assert_eq!(s.filters.board.as_deref(), Some("dev"));
    assert_eq!(s.board_cursor, 0);
    assert!(
        s.tag_filter_options()
            .iter()
            .any(|option| option == "convert"),
        "현재 보드에 유효한 tag filter는 유지되어야 한다"
    );
}

#[test]
fn replace_pegboard는_사라진_보드_필터를_all로_되돌린다() {
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
        "외부에서 현재 보드가 삭제되면 stale board filter를 유지하면 안 된다"
    );
    assert_eq!(s.board_cursor, 0);
    assert!(s.cursor < s.visible_placements().len());
}

#[test]
fn 외부_pegboard_reload는_진행_중인_보드_편집과_move_mode에서_보류된다() {
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
        "리사이즈 프리뷰 중에도 외부 pegboard reload는 보류되어야 한다"
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
        InputName::new(name).expect("테스트 입력 이름"),
        None,
        None,
        required,
        InputKind::String,
    )
    .expect("테스트 문자열 필드")
}

fn string_form(name: &str, value: &str, required: bool) -> TuiFormState {
    let spec = InputSpec::new(vec![string_field(name, required)]).expect("테스트 입력 명세");
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

/// 커서/폼 동작이 인벤토리에 등록된 항목에 의존하지 않도록 고정된 가짜
/// 도구 목록을 만든다.
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
                InputName::new("input").expect("테스트 입력 이름"),
                None,
                Some("Some text".to_string()),
                true,
                InputKind::String,
            )
            .expect("테스트 입력 필드"),
        ])
        .expect("테스트 입력 명세"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
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

/// 보드 필터 옵션에 "all"이.visible로 표시되지 않지만 실제 보드는 표시되어야 한다.
/// Flutter GUI와 동일하게 보드 막대에 "all" 칩이 렌더링되지 않아야 한다.
#[test]
fn 보드_필터_옵션은_all이_아닌_실제_보드만_반환한다() {
    let s = fresh();
    let options = s.board_filter_options();

    // "all"은.visible 옵션에 있으면 안 된다 (내부 상태로만 존재)
    assert!(
        !options.contains(&"all".to_string()),
        "보드 필터 옵션에 'all'이.visible 있으면 안 된다"
    );

    // 하지만 실제 보드 키는 존재해야 한다
    assert!(
        options.iter().any(|o| o == "dev"),
        "보드 필터 옵션에 'dev'가 있어야 한다"
    );
    assert!(
        options.iter().any(|o| o == "trading"),
        "보드 필터 옵션에 'trading'이 있어야 한다"
    );
    assert!(
        options.iter().any(|o| o == "personal"),
        "보드 필터 옵션에 'personal'이 있어야 한다"
    );
}

/// 태그 필터 옵션은 여전히 "all"을 포함해야 한다 (보드 필터와 다르게).
#[test]
fn 태그_필터_옵션은_all을_여전히_포함한다() {
    let s = fresh();
    let options = s.tag_filter_options();

    // 태그 필터는 "all"을 계속 표시해야 한다 (Flutter GUI와 동일하게)
    assert!(
        options.contains(&"all".to_string()),
        "태그 필터 옵션에 'all'이 있어야 한다"
    );
}

/// 숫자 키 '0'은 여전히 전체 보드 필터(BoardFilter::All)로 돌아가야 한다.
/// clear 보드 동작이 계속 작동해야 한다.
#[test]
fn 숫자_0_키는_전체_보드_필터로_돌아간다() {
    let mut s = fresh();
    s.filters.select_board("dev");

    let result = s.board_for_digit('0');

    assert!(
        result.is_some() && result.unwrap().is_none(),
        "숫자 0 키는 BoardFilter::All을 반환해야 한다"
    );
}

/// 숫자 키 '1'은 첫 번째 실제 보드를 선택해야 한다 (인덱스 재매핑).
/// 이전: '1'은 "all"을 가리켰다, 이후: '1'은 첫 번째 보드("dev")를 가리킨다.
#[test]
fn 숫자_1_키는_첫_번째_실제_보드를_선택한다() {
    let s = fresh();

    let result = s.board_for_digit('1');

    assert!(
        result.is_some() && result.unwrap().as_deref() == Some("dev"),
        "숫자 1 키는 첫 번째 보드('dev')를 반환해야 한다"
    );
}

/// 'b' 키로 보드 필터를 순환할 때, 현재가 All 상태이면 첫 번째 실제 보드로 이동해야 한다.
#[test]
fn 보드_순환은_all에서_첫_번째_실제_보드로_이동한다() {
    use crate::surfaces::tui::model::cycle_board_filter;

    let s = fresh();
    let options = s.board_filter_options();

    // All 상태에서 순환
    let current = BoardFilter::All;
    let next = cycle_board_filter(&current, &options);

    // 첫 번째 실제 보드로 순환해야 함
    assert!(
        next.as_deref() == Some("dev"),
        "All에서 순환하면 첫 번째 보드('dev')로 이동해야 한다"
    );
}
