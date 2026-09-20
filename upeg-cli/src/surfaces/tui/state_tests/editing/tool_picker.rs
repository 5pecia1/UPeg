use super::super::super::update::{tool_picker_results, tool_picker_search_results};
use super::*;
use crate::surfaces::tui::model::{ToolPickerMode, ToolPickerReturnView};
use upeg_core::search::SearchField;

const OLD_NINE_RESULT_CAP: usize = 9;
const OVERFLOW_TOOL_COUNT: usize = OLD_NINE_RESULT_CAP + 1;

fn register_tui_picker_tool(
    id: &'static str,
    toolkit: &'static str,
    local_id: &'static str,
    tags: &'static [&'static str],
    display_label: &'static str,
) -> impl Drop {
    upeg_runtime::toolbox_add_tool_managed(ToolMeta {
        id,
        toolkit,
        local_id,
        tags,
        display_label,
        description: "TUI picker shared search fixture",
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
    })
}

fn leaked_static_string(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn missing_recent_log_path(name: &str) -> std::path::PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("upeg-{name}-{}-{now}.jsonl", std::process::id()))
}

#[test]
fn add_tool_key_opens_picker_without_edit_mode() {
    // Modeless: with a concrete board selected, `a` opens the pin
    // picker directly.
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('a'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(
        s.view,
        View::ToolPicker {
            mode: crate::surfaces::tui::model::ToolPickerMode::Pin,
            ..
        }
    ));
}

#[test]
fn add_tool_key_does_nothing_on_all_filter() {
    let mut s = fresh();
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('a'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::List));
}

#[test]
fn lowercase_or_uppercase_a_opens_tool_picker_with_empty_query() {
    for key in ['a', 'A'] {
        let mut s = fresh();
        s.filters.select_board("dev");
        let t = fixture_tools();
        let effect = handle_key(&mut s, Key::Char(key), t.as_slice());
        assert_eq!(effect, Effect::None);
        match &s.view {
            View::ToolPicker {
                mode,
                query,
                cursor,
                ..
            } => {
                assert_eq!(*mode, ToolPickerMode::Pin);
                assert!(query.is_empty(), "{key} must open with an empty query");
                assert_eq!(*cursor, 0, "{key} must place the picker cursor at 0");
            }
            other => panic!("expected ToolPicker for {key}, got {other:?}"),
        }
    }
}

#[test]
fn typing_in_tool_picker_filters_by_query() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('a'), t.as_slice());
    for ch in "hex".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    if let View::ToolPicker { query, .. } = &s.view {
        assert_eq!(query, "hex");
    } else {
        panic!("expected ToolPicker");
    }
}

#[test]
fn tool_picker_matches_toolkit_and_tags_via_shared_search() {
    const TAGS: &[&str] = &["zz-tui-tag-only-needle"];
    let tool_id = "zz_tui_toolkit_match.alpha";
    let _tool = register_tui_picker_tool(
        tool_id,
        "zz_tui_toolkit_match",
        "alpha",
        TAGS,
        "Toolkit match alpha",
    );
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();

    handle_key(&mut s, Key::Char('a'), t.as_slice());

    let toolkit_results = tool_picker_search_results(&s, "zz_tui_toolkit_match");
    let toolkit_result = toolkit_results
        .iter()
        .find(|result| result.tool.id == tool_id)
        .expect("registered TUI tool should be found by toolkit");
    assert!(
        toolkit_result
            .matched_fields
            .contains(&SearchField::Toolkit),
        "shared search metadata must report toolkit matching"
    );

    let tag_results = tool_picker_search_results(&s, "zz-tui-tag-only-needle");
    let tag_result = tag_results
        .iter()
        .find(|result| result.tool.id == tool_id)
        .expect("registered TUI tool should be found by tag");
    assert!(
        tag_result.matched_fields.contains(&SearchField::Tags),
        "shared search metadata must report tag matching"
    );
}

#[test]
fn tool_picker_sorts_active_board_pinned_tools_first() {
    const TAGS: &[&str] = &["zz-tui-pinned-rank-tag"];
    let unpinned_id = "zz_tui_rank.first";
    let pinned_id = "zz_tui_rank.pinned";
    let _unpinned =
        register_tui_picker_tool(unpinned_id, "zz_tui_rank", "first", TAGS, "Rank first");
    let _pinned = register_tui_picker_tool(pinned_id, "zz_tui_rank", "pinned", TAGS, "Rank pinned");
    let mut s = fresh();
    s.filters.select_board("dev");
    s.layouts
        .insert("dev".into(), vec![Placement::new(pinned_id, 0, 0)]);
    let t = fixture_tools();

    handle_key(&mut s, Key::Char('a'), t.as_slice());

    let ids: Vec<_> = tool_picker_results(
        &s,
        ToolPickerMode::Pin,
        "zz-tui-pinned-rank-tag",
        t.as_slice(),
    )
    .into_iter()
    .take(2)
    .map(|tool| tool.id)
    .collect();
    assert_eq!(ids, vec![pinned_id, unpinned_id]);
}

#[test]
fn tool_picker_does_not_truncate_search_results_over_nine() {
    const TAGS: &[&str] = &["zz-tui-no-cap-tag"];
    let mut guards = Vec::with_capacity(OVERFLOW_TOOL_COUNT);
    for index in 0..OVERFLOW_TOOL_COUNT {
        let ordinal = format!("{index:02}");
        guards.push(register_tui_picker_tool(
            leaked_static_string(format!("zz_tui_no_cap.{ordinal}")),
            "zz_tui_no_cap",
            leaked_static_string(ordinal.clone()),
            TAGS,
            leaked_static_string(format!("No cap tool {ordinal}")),
        ));
    }
    assert_eq!(guards.len(), OVERFLOW_TOOL_COUNT);
    let mut state = fresh();
    state.filters.select_board("dev");
    let t = fixture_tools();

    handle_key(&mut state, Key::Char('a'), t.as_slice());

    let ids: Vec<_> = tool_picker_results(
        &state,
        ToolPickerMode::Pin,
        "zz-tui-no-cap-tag",
        t.as_slice(),
    )
    .into_iter()
    .map(|tool| tool.id)
    .collect();
    assert_eq!(
        ids.len(),
        OVERFLOW_TOOL_COUNT,
        "the tool picker must not apply the old 9-item cap: {ids:?}"
    );
    assert!(
        ids.len() > OLD_NINE_RESULT_CAP,
        "search results must be kept beyond 9 items"
    );
}

#[test]
fn tool_picker_opens_and_searches_without_recent_log() {
    const TAGS: &[&str] = &["zz-tui-missing-recent-tag"];
    let tool_id = "zz_tui_missing_recent.tool";
    let _tool = register_tui_picker_tool(
        tool_id,
        "zz_tui_missing_recent",
        "tool",
        TAGS,
        "Missing recent tool",
    );
    let missing_path = missing_recent_log_path("tool-picker-missing-recent");
    let _ = std::fs::remove_file(&missing_path);
    let _ = std::fs::remove_dir_all(&missing_path);
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    s.tool_picker_recent =
        crate::adapters::execution_log::recent_search_signals_from_path(&missing_path).recent;

    let effect = handle_key(&mut s, Key::Char('a'), t.as_slice());
    assert_eq!(effect, Effect::None);

    assert!(matches!(s.view, View::ToolPicker { .. }));
    assert!(
        tool_picker_results(
            &s,
            ToolPickerMode::Pin,
            "zz-tui-missing-recent-tag",
            t.as_slice()
        )
        .iter()
        .any(|tool| tool.id == tool_id),
        "missing execution log must collapse to empty recent signals, not break shared search"
    );
    let _ = std::fs::remove_file(&missing_path);
    let _ = std::fs::remove_dir_all(&missing_path);
}

#[test]
fn enter_in_tool_picker_toggles_pin_and_keeps_view_open() {
    let mut s = fresh();
    s.filters.select_board("dev");
    // Empty dev's default layout so the toggle result is observable.
    s.layouts.insert("dev".into(), Vec::new());
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('a'), t.as_slice());
    let needle = "num.hex_to_decimal";
    for ch in needle.chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    // An exact-match query puts that tool at cursor 0.
    let effect = handle_key(&mut s, Key::Enter, t.as_slice());
    assert_eq!(effect, Effect::SavePegboard);
    let pinned = layout_contains(&s, "dev", needle);
    assert!(
        pinned,
        "Enter on the row under the picker cursor must pin the tool"
    );
    assert!(
        matches!(s.view, View::ToolPicker { .. }),
        "the picker must stay open after pinning so several tools can be pinned"
    );
}

#[test]
fn enter_on_already_pinned_tool_in_picker_unpins_it() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    let needle = "num.hex_to_decimal";
    // Start with the tool pinned exactly once.
    s.layouts
        .insert("dev".into(), vec![Placement::new(needle, 0, 0)]);
    handle_key(&mut s, Key::Char('a'), t.as_slice());
    for ch in needle.chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    let effect = handle_key(&mut s, Key::Enter, t.as_slice());
    assert_eq!(effect, Effect::SavePegboard);
    let still_pinned = layout_contains(&s, "dev", needle);
    assert!(
        !still_pinned,
        "Enter on a pinned tool must toggle the pin off"
    );
}

#[test]
fn esc_in_tool_picker_closes_overlay() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('a'), t.as_slice());
    handle_key(&mut s, Key::Esc, t.as_slice());
    assert!(matches!(s.view, View::List));
}

#[test]
fn slash_opens_search_picker_without_edit_mode() {
    let mut s = fresh();
    let t = fixture_tools();

    let effect = handle_key(&mut s, Key::Char('/'), t.as_slice());

    assert_eq!(effect, Effect::None);
    match &s.view {
        View::ToolPicker {
            mode,
            return_to,
            query,
            cursor,
        } => {
            assert_eq!(*mode, ToolPickerMode::Search);
            assert_eq!(return_to.view, ToolPickerReturnView::List);
            assert!(query.is_empty());
            assert_eq!(*cursor, 0);
        }
        other => panic!("expected search ToolPicker, got {other:?}"),
    }
}

#[test]
fn ctrl_k_from_right_pane_focus_opens_search_picker() {
    let mut s = fresh();
    s.view = View::Detail;
    s.focus = FocusArea::RightPane;
    let t = fixture_tools();
    let ctrl = upeg_core::KeyModifiers {
        control: true,
        ..upeg_core::KeyModifiers::NONE
    };

    update(
        &mut s,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::modified(Key::Char('k'), ctrl),
            tools: t.as_slice(),
            area: None,
        },
    );

    assert!(
        matches!(
            s.view,
            View::ToolPicker {
                mode: ToolPickerMode::Search,
                ..
            }
        ),
        "Ctrl+K must open the search picker, not be consumed by RightPane scroll. got: {:?}",
        s.view
    );
}

#[test]
fn search_picker_resets_focus_on_open_so_keys_reach_picker() {
    let mut s = fresh();
    s.view = View::Detail;
    s.focus = FocusArea::RightPane;
    let t = fixture_tools();

    handle_key(&mut s, Key::Char('/'), t.as_slice());
    assert_eq!(s.focus, FocusArea::Grid);

    handle_key(&mut s, Key::Down, t.as_slice());
    match &s.view {
        View::ToolPicker { cursor, .. } => {
            assert_eq!(
                *cursor, 1,
                "Down must move the picker cursor, not RightPane scroll"
            );
        }
        other => panic!("expected search ToolPicker, got {other:?}"),
    }

    handle_key(&mut s, Key::Char('j'), t.as_slice());
    match &s.view {
        View::ToolPicker { query, cursor, .. } => {
            assert_eq!(
                query, "j",
                "j must go to the picker query, not RightPane scroll"
            );
            assert_eq!(
                *cursor, 0,
                "after typing a query the picker cursor must return to the top"
            );
        }
        other => panic!("expected search ToolPicker, got {other:?}"),
    }
}

#[test]
fn search_picker_opened_from_detail_returns_to_detail_on_esc() {
    let mut s = fresh();
    s.view = View::Detail;
    s.focus = FocusArea::RightPane;
    s.cursor = 2;
    let t = fixture_tools();

    handle_key(&mut s, Key::Char('/'), t.as_slice());
    handle_key(&mut s, Key::Esc, t.as_slice());

    assert_eq!(s.view, View::Detail);
    assert_eq!(s.cursor, 2);
    assert_eq!(s.focus, FocusArea::RightPane);
}

#[test]
fn search_picker_enter_opens_detail_without_changing_pins() {
    let mut s = fresh();
    let t = fixture_tools();
    let before = s.layouts.clone();

    handle_key(&mut s, Key::Char('/'), t.as_slice());
    for ch in "with".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    let effect = handle_key(&mut s, Key::Enter, t.as_slice());

    assert_eq!(effect, Effect::None);
    assert_eq!(s.view, View::Detail);
    assert_eq!(s.cursor, 1);
    assert_eq!(s.layouts, before);
}
