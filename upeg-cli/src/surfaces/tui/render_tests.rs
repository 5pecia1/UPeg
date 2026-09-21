//! TUI render/source contract tests split out of `tui_tests.rs`.

use super::state_tests::fresh;
use crate::Outcome;
use crate::domain::execution::dispatch::dispatch_failure;
use crate::surfaces::tui::*;
use ratatui::{
    Terminal,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
};
use upeg_core::{
    InputFieldSpec, InputKind, InputName, InputSpec, Invoker, OutputEntry, OutputKind, OutputValue,
    PinColorHex, PinKind, Placement, Surface, ToolMeta, ToolSuccess,
};

fn buffer_region_has_modifier(buffer: &Buffer, area: Rect, modifier: Modifier) -> bool {
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if buffer[(x, y)].modifier.contains(modifier) {
                return true;
            }
        }
    }
    false
}

fn buffer_region_has_symbol_with_fg(buffer: &Buffer, area: Rect, symbol: &str, fg: Color) -> bool {
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            let cell = &buffer[(x, y)];
            if cell.symbol() == symbol && cell.fg == fg {
                return true;
            }
        }
    }
    false
}

#[test]
fn test_backend_render_includes_title() {
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State::default();

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());
    assert!(buf_string.contains("Pegboard"));
    assert!(buf_string.contains("Boards"));
    assert!(buf_string.contains("Tags"));
    assert!(buf_string.contains("navigate"));
}

#[test]
fn render_marks_keyboard_focused_filter_option() {
    use ratatui::backend::TestBackend;

    static TAGS: &[&str] = &["zz-focus-render"];
    upeg_runtime::toolbox_add_tool(ToolMeta {
        id: "test.tui_focus_render",
        toolkit: "test",
        local_id: "tui_focus_render",
        tags: TAGS,
        display_label: "Focus render",
        description: "Forces a tag option for focus rendering",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    });

    let area = Rect::new(0, 0, 100, 24);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let tools = list_tools();
    let state = State {
        focus: FocusArea::Tags,
        tag_cursor: 1,
        ..fresh()
    };

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let layout = tui_layout(area);
    let buffer = terminal.backend().buffer();

    assert!(
        buffer_region_has_modifier(buffer, layout.tags, Modifier::UNDERLINED),
        "the focused tag option must render with a visible focus mark"
    );
    assert!(
        !buffer_region_has_modifier(buffer, layout.boards, Modifier::UNDERLINED),
        "the unfocused board bar must not render the keyboard-focus underline"
    );
}

#[test]
fn wide_board_list_renders_board_scrollbar() {
    use ratatui::backend::TestBackend;

    static BOARDS: &[&str] = &[
        "zz-tui-overflow-00",
        "zz-tui-overflow-01",
        "zz-tui-overflow-02",
        "zz-tui-overflow-03",
    ];
    upeg_runtime::toolbox_add_tool(ToolMeta {
        id: "test.tui_board_scrollbar",
        toolkit: "test",
        local_id: "tui_board_scrollbar",
        tags: &[],
        display_label: "Board scrollbar",
        description: "Forces the board filter bar to overflow",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: BOARDS,
    });

    let backend = TestBackend::new(36, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = fresh();

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());
    assert!(
        buf_string.contains('#') && buf_string.contains('>'),
        "a wide board filter bar must show a horizontal scrollbar. rendered: {buf_string}"
    );
}

#[test]
fn filtered_render_shows_active_board_and_tag() {
    use ratatui::backend::TestBackend;

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let filters = TuiFilters::from_options(Some("dev"), Some("pure"));
    let tools = list_tools_for_board_and_tag(filters.board.as_deref(), filters.tag.as_deref());
    let state = fresh();

    terminal
        .draw(|f| render_with_filters(f, &state, &tools, &filters))
        .unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());
    assert!(buf_string.contains("Pegboard grid"));
    assert!(buf_string.contains("Boards"));
    assert!(buf_string.contains(" dev "));
    assert!(buf_string.contains("Tags"));
    assert!(buf_string.contains("pure"));
}

#[test]
fn render_uses_shared_display_labels_and_result_status() {
    use ratatui::backend::TestBackend;

    let hex: &'static ToolMeta = Box::leak(Box::new(ToolMeta {
        id: "num.hex_to_decimal",
        toolkit: "convert",
        local_id: "hex_to_decimal",
        tags: &[],
        display_label: "Hex → Dec",
        description: "Convert hexadecimal to decimal",
        input_spec: InputSpec::new(vec![
            InputFieldSpec::new(
                InputName::new("input").expect("test input name"),
                None,
                None,
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
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    }));
    let tools = [hex];

    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    terminal
        .draw(|f| render(f, &State::default(), &tools))
        .unwrap();
    let list_buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        list_buf.contains("Hex → Dec"),
        "the TUI list must use the ToolMeta display label. rendered: {list_buf}"
    );
    assert!(
        list_buf.contains("↵ run") && list_buf.contains("o inspect"),
        "Enter runs and o is the separate detail inspect. rendered: {list_buf}"
    );
    assert!(
        list_buf.contains("MCP off") && list_buf.contains("HTTP off"),
        "the footer must expose real interface state instead of a fake background-task placeholder. rendered: {list_buf}"
    );
    assert!(
        !list_buf.contains("0 bg tasks") && !list_buf.contains("net OK"),
        "the footer must not reintroduce the inactive-state placeholder. rendered: {list_buf}"
    );
    assert!(
        list_buf.contains("num.hex_to_decimal"),
        "the TUI must keep exposing the technical tool id as secondary metadata. rendered: {list_buf}"
    );

    let ok_state = State {
        cursor: 0,
        view: View::Result {
            tool_id: "num.hex_to_decimal",
            outputs: Vec::new(),
            text: "255".into(),
            is_error: false,
        },
        ..State::default()
    };
    terminal.draw(|f| render(f, &ok_state, &tools)).unwrap();
    let ok_buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        ok_buf.contains("OK"),
        "a success result must show OK. rendered: {ok_buf}"
    );

    let err_state = State {
        cursor: 0,
        view: View::Result {
            tool_id: "num.hex_to_decimal",
            outputs: Vec::new(),
            text: "bad input".into(),
            is_error: true,
        },
        ..State::default()
    };
    terminal.draw(|f| render(f, &err_state, &tools)).unwrap();
    let err_buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        err_buf.contains("ERROR"),
        "an error result must show ERROR. rendered: {err_buf}"
    );
}

#[test]
fn tui_result_status_label_comes_from_core_ux_contract() {
    let view_src = include_str!("view.rs");
    assert!(
        view_src.contains("upeg_core::ux::result_status_label"),
        "the TUI view must import the shared ux result-status helper"
    );
    assert!(
        !view_src.contains("const fn result_status_label"),
        "the TUI view must not keep a local result_status_label copy"
    );
}

#[test]
fn success_result_draws_all_canonical_output_labels_and_values() {
    use ratatui::backend::TestBackend;

    let success = ToolSuccess::new(
        Some("primary".into()),
        vec![
            OutputEntry {
                id: "primary".into(),
                label: Some("Primary label".into()),
                kind: OutputKind::String,
                value: OutputValue::String("main value".into()),
            },
            OutputEntry {
                id: "secondary".into(),
                label: Some("Secondary label".into()),
                kind: OutputKind::Integer,
                value: OutputValue::Integer(42),
            },
        ],
    )
    .unwrap();
    let mut state = State::default();
    apply_outcome(
        &mut state,
        "test.multi_output",
        crate::domain::execution::dispatch::Outcome::Success(success),
    );
    match &state.view {
        View::Result { outputs, text, .. } => {
            assert_eq!(outputs.len(), 2);
            assert!(text.is_empty());
        }
        other => panic!("expected the Result view: {other:?}"),
    }

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    assert!(buf.contains("Primary label"), "rendered: {buf}");
    assert!(buf.contains("main value"), "rendered: {buf}");
    assert!(buf.contains("Secondary label"), "rendered: {buf}");
    assert!(buf.contains("42"), "rendered: {buf}");
}

#[test]
fn form_view_title_and_body_use_consistent_back_verb() {
    // The Form view title ("esc back") and body footer ("[Esc] back")
    // must use the same verb. The handler is `back_one_level`, so
    // "back" matches Detail's "q back" and Result's "↩ list". Pinning
    // the contract so a mismatch stands out whichever string is edited
    // later.
    use ratatui::backend::TestBackend;
    let mut s = State {
        cursor: 0,
        view: View::Form {
            tool_id: "test.iter160",
            form: TuiFormState::new(
                InputSpec::new(vec![
                    InputFieldSpec::new(
                        InputName::new("input").expect("test input name"),
                        None,
                        None,
                        true,
                        InputKind::String,
                    )
                    .expect("test input field"),
                ])
                .expect("test input spec"),
            ),
        },
        ..State::default()
    };
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, &s, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // Both surfaces (block-border title + body footer) must say
    // "back". Pin both substrings.
    assert!(
        buf.contains("Esc] back"),
        "the Form body must say `[Esc] back` since iteration 160. got: {buf}"
    );
    assert!(
        buf.contains("esc back"),
        "the Form title must say `esc back`. got: {buf}"
    );
    // Negative pin: the old "cancel" verb must not come back.
    assert!(
        !buf.contains("Esc] cancel"),
        "`[Esc] cancel` was removed in iteration 160; reverting it brings back the title/body mismatch"
    );
    let _ = &mut s; // suppresses unused-mut even if later State edits stop mutating.
}

// ─── Phase 6-9 overlay renderers ─────────────────────────────
//
// Each new full-body view (BoardEditor / ConfirmDeleteBoard /
// ToolPicker) gets a TestBackend snapshot so render-side regressions
// (missing prompt, wrong title, missing pin marker) fail the test
// instead of slipping through CI.

#[test]
fn board_editor_add_mode_render_shows_prompt_and_buffer() {
    use crate::surfaces::tui::model::BoardEditMode;
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            buffer: "Alpha".to_string(),
        },
        ..fresh()
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // The full-body panel must carry the title, the add prompt, and the
    // typed buffer. The pegboard grid must not be visible.
    assert!(
        buf.contains("Edit board"),
        "the BoardEditor title must be visible. got: {buf}"
    );
    assert!(
        buf.contains("New board title:"),
        "the add-mode prompt must appear. got: {buf}"
    );
    assert!(
        buf.contains("Alpha"),
        "the typed buffer must render. got: {buf}"
    );
    assert!(
        !buf.contains("Pegboard grid"),
        "BoardEditor must occupy the whole body; the grid title must not leak"
    );
}

#[test]
fn board_editor_rename_mode_render_prefills_original_title() {
    use crate::surfaces::tui::model::BoardEditMode;
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::BoardEditor {
            mode: BoardEditMode::Rename {
                key: "dev".to_string(),
                original_title: "Dev".to_string(),
            },
            buffer: "Development".to_string(),
        },
        ..fresh()
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // The rename prompt carries the original title so the user sees
    // what they are changing; the buffer carries the edited value.
    assert!(
        buf.contains("Dev"),
        "the rename prompt must contain the original title. got: {buf}"
    );
    assert!(
        buf.contains("Development"),
        "the rename buffer must render. got: {buf}"
    );
}

#[test]
fn board_editor_empty_buffer_render_shows_hint_not_just_cursor() {
    use crate::surfaces::tui::model::BoardEditMode;
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            buffer: String::new(),
        },
        ..fresh()
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // An empty buffer must show a hint so the user knows what to do.
    // Before the fix the panel rendered only a prompt and a lonely
    // cursor block with no guidance.
    assert!(
        buf.contains("type a title"),
        "the empty-buffer hint must say what to do. got: {buf}"
    );
}

#[test]
fn board_delete_confirm_render_shows_target_title_and_choices() {
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::ConfirmDeleteBoard {
            key: "trading".to_string(),
            title: "Trading".to_string(),
        },
        ..fresh()
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // The prompt must quote the target title verbatim so the user can
    // re-check before pressing y.
    assert!(
        buf.contains("Trading"),
        "the confirm prompt must contain the target title. got: {buf}"
    );
    assert!(
        buf.contains("[y/F1] delete"),
        "the confirm prompt must show the [y/F1] action (F1 confirms everywhere). got: {buf}"
    );
    assert!(
        buf.contains("[n] keep"),
        "the confirm prompt must show the [n] action. got: {buf}"
    );
}

#[test]
fn tool_picker_render_shows_query_and_pin_marker() {
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();

    // Plant a known pinned tool under `dev` so the picker has a target
    // to mark ★.
    let mut state = fresh();
    state.filters.select_board("dev");
    state.layouts.insert(
        "dev".to_string(),
        vec![Placement::new("num.hex_to_decimal", 0, 0)],
    );
    state.view = View::ToolPicker {
        mode: crate::surfaces::tui::model::ToolPickerMode::Pin,
        return_to: crate::surfaces::tui::model::ToolPickerReturn::list(FocusArea::Grid),
        query: "hex".to_string(),
        cursor: 0,
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // Title + query + pin marker + one filtered result.
    assert!(
        buf.contains("Add tool"),
        "the ToolPicker title must be visible. got: {buf}"
    );
    assert!(
        buf.contains("Filter:"),
        "the query input label must be visible. got: {buf}"
    );
    assert!(
        buf.contains("hex"),
        "the typed query must render. got: {buf}"
    );
    assert!(
        buf.contains('★'),
        "the pinned-on-this-board marker must render next to num.hex_to_decimal. got: {buf}"
    );
    assert!(
        buf.contains("num.hex_to_decimal"),
        "the filtered toolbox id must render. got: {buf}"
    );
}

#[test]
fn tool_picker_draws_pinned_tools_above_search_results() {
    use ratatui::backend::TestBackend;

    const TAGS: &[&str] = &["zz-tui-render-pinned-rank-tag"];
    let _unpinned = upeg_runtime::toolbox_add_tool_managed(ToolMeta {
        id: "zz_tui_render_rank.first",
        toolkit: "zz_tui_render_rank",
        local_id: "first",
        tags: TAGS,
        display_label: "Render rank first",
        description: "Render shared search rank fixture",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    });
    let _pinned = upeg_runtime::toolbox_add_tool_managed(ToolMeta {
        id: "zz_tui_render_rank.pinned",
        toolkit: "zz_tui_render_rank",
        local_id: "pinned",
        tags: TAGS,
        display_label: "Render rank pinned",
        description: "Render shared search rank fixture",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    });
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let mut state = fresh();
    state.filters.select_board("dev");
    state.layouts.insert(
        "dev".to_string(),
        vec![Placement::new("zz_tui_render_rank.pinned", 0, 0)],
    );
    state.view = View::ToolPicker {
        mode: crate::surfaces::tui::model::ToolPickerMode::Pin,
        return_to: crate::surfaces::tui::model::ToolPickerReturn::list(FocusArea::Grid),
        query: "zz-tui-render-pinned-rank-tag".to_string(),
        cursor: 0,
    };

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    let pinned = buf
        .find("zz_tui_render_rank.pinned")
        .expect("pinned fixture should render");
    let unpinned = buf
        .find("zz_tui_render_rank.first")
        .expect("unpinned fixture should render");

    assert!(
        pinned < unpinned,
        "pinned shared-search result must render before unpinned result. got: {buf}"
    );
    assert!(
        buf.contains('★'),
        "pinned marker should stay visible on ranked shared-search rows. got: {buf}"
    );
}

#[test]
fn tool_picker_empty_query_render_does_not_leave_body_empty() {
    // An empty query must not leave the user facing an empty panel.
    // Scan the buffer cells inside the panel body for at least one
    // glyph that is neither blank nor border — either the empty-state
    // hint or an unfiltered toolbox row.
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        filters: crate::surfaces::tui::model::TuiFilters::from_options(Some("dev"), None),
        view: View::ToolPicker {
            mode: crate::surfaces::tui::model::ToolPickerMode::Pin,
            return_to: crate::surfaces::tui::model::ToolPickerReturn::list(FocusArea::Grid),
            query: String::new(),
            cursor: 0,
        },
        ..fresh()
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buffer = terminal.backend().buffer();
    let border_chars = ['╭', '╮', '╰', '╯', '─', '│', '█'];
    let mut content_cells = 0_usize;
    // The body sits below the header/filter row. Scanning rows 8..20
    // skips the header, filter bar, and panel border.
    for y in 8_u16..20 {
        for x in 1_u16..79 {
            let sym = buffer[(x, y)].symbol();
            if sym.trim().is_empty() {
                continue;
            }
            if sym.chars().all(|c| border_chars.contains(&c)) {
                continue;
            }
            content_cells += 1;
        }
    }
    assert!(
        content_cells > 0,
        "an empty-query picker must render content in the body area; \
         found {content_cells} cells that are neither border nor blank",
    );
}

// ──────────────────────────────────────────────────────────────
// Narrow-body render branch. As in the GUI (`overflow:auto +
// width:max-content`), the board is the main body. In List/Detail the
// right pane folds and the board takes the whole body; in Form/Result
// the right-hand content the user is working on goes full-body.

#[test]
fn narrow_list_draws_board_full_body_and_hides_right_pane() {
    use ratatui::backend::TestBackend;

    let width: u16 = 50; // < MIN_DUAL_PANE_WIDTH(60)
    let backend = TestBackend::new(width, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State::default(); // View::List

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        buf.contains("Pegboard"),
        "the board title must still be visible in narrow List. got: {buf}"
    );
    // The right pane's list title
    // (" Tool · ↵ run · o inspect · Space run · / search ") must not
    // leak. "inspect" also appears in the modeless header hint
    // ("o inspect"), so we discriminate on "Space run", which only
    // exists in the right-pane title.
    assert!(
        !buf.contains("Space run"),
        "the right list pane must be hidden in narrow List. got: {buf}"
    );
}

#[test]
fn narrow_detail_draws_board_full_body() {
    use ratatui::backend::TestBackend;

    let width: u16 = 50;
    let backend = TestBackend::new(width, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::Detail,
        ..State::default()
    };

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        buf.contains("Pegboard"),
        "the board must stay the main body in narrow Detail. got: {buf}"
    );
    // The Detail pane title "Manifest · F1/↵ ..." must not leak.
    assert!(
        !buf.contains("Manifest"),
        "the manifest right pane must be hidden in narrow Detail. got: {buf}"
    );
}

#[test]
fn narrow_form_draws_full_body_and_hides_board() {
    use crate::surfaces::tui::model::TuiFormState;
    use ratatui::backend::TestBackend;

    let width: u16 = 50;
    let backend = TestBackend::new(width, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    // The surface the user is working on (Form) is the main body; the
    // board yields.
    let state = State {
        view: View::Form {
            tool_id: "num.hex_to_decimal",
            form: TuiFormState::new(upeg_core::InputSpec::empty()),
        },
        ..State::default()
    };

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    // Form's right-pane title ("Form · ↵ run · ...") must draw
    // full-body.
    assert!(
        buf.contains("Form"),
        "the form must be visible full-body in narrow Form. got: {buf}"
    );
    // The board grid title must not leak.
    assert!(
        !buf.contains("Pegboard grid"),
        "the board grid must be hidden in narrow Form. got: {buf}"
    );
}

#[test]
fn narrow_result_draws_result_full_body() {
    use ratatui::backend::TestBackend;

    let width: u16 = 50;
    let backend = TestBackend::new(width, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::Result {
            tool_id: "num.hex_to_decimal",
            outputs: Vec::new(),
            text: "0x12 = 18".into(),
            is_error: false,
        },
        ..State::default()
    };

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        buf.contains("Result") || buf.contains("결과"),
        "the result panel must be visible full-body in narrow Result. got: {buf}"
    );
    assert!(
        !buf.contains("Pegboard grid"),
        "the board grid must be hidden in narrow Result. got: {buf}"
    );
}

#[test]
fn wide_screen_still_shows_dual_panes() {
    use ratatui::backend::TestBackend;

    // Regression guard: the narrow branch was introduced but standard
    // 80x24 keeps dual panes.
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State::default();

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    assert!(buf.contains("Pegboard"), "the board title must be visible");
    assert!(
        buf.contains("inspect") || buf.contains("Tool"),
        "wide List must also expose the right pane. got: {buf}"
    );
}

#[test]
fn grid_card_renders_actual_rgb_color_when_pin_color_set() {
    use ratatui::backend::TestBackend;

    static TAGS: &[&str] = &[];
    let tool: &'static ToolMeta = Box::leak(Box::new(ToolMeta {
        id: "test.pin_color_card",
        toolkit: "test",
        local_id: "pin_color_card",
        tags: TAGS,
        display_label: "Pin Color Card",
        description: "Test card for pin color rendering",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    }));

    let area = Rect::new(0, 0, 80, 24);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let tools = [tool];
    let state = fresh();
    let pin_color = PinColorHex::parse("#112233").expect("test pin color");
    let pin_colors = [Some(pin_color)];
    let expected = Color::Rgb(0x11, 0x22, 0x33);

    terminal
        .draw(|f| render_with_pin_colors(f, &state, &tools, &pin_colors))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let layout = tui_layout(area);
    let grid = grid_content_area(layout.body());

    assert!(
        buffer_region_has_symbol_with_fg(buffer, grid, "I", expected),
        "the pin-kind label must use the custom RGB foreground"
    );
    assert!(
        buffer_region_has_symbol_with_fg(buffer, grid, "─", expected)
            || buffer_region_has_symbol_with_fg(buffer, grid, "│", expected),
        "the card border must use the custom RGB foreground"
    );
}

#[test]
fn grid_card_keeps_existing_accent_rendering_without_pin_color() {
    use ratatui::backend::TestBackend;

    static TAGS: &[&str] = &[];
    let tool: &'static ToolMeta = Box::leak(Box::new(ToolMeta {
        id: "test.pin_color_default_card",
        toolkit: "test",
        local_id: "pin_color_default_card",
        tags: TAGS,
        display_label: "Default Pin Color Card",
        description: "Test card for default pin color rendering",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    }));

    let area = Rect::new(0, 0, 80, 24);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let tools = [tool];
    let state = fresh();
    let pin_colors = [None];

    terminal
        .draw(|f| render_with_pin_colors(f, &state, &tools, &pin_colors))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let layout = tui_layout(area);
    let grid = grid_content_area(layout.body());

    assert!(
        buffer_region_has_symbol_with_fg(buffer, grid, "─", Color::DarkGray)
            || buffer_region_has_symbol_with_fg(buffer, grid, "│", Color::DarkGray),
        "without a custom color the unselected border must keep the existing DarkGray"
    );
}

mod approval_running;
mod controlled_embed;
mod form_constraints;
