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
fn 도구추가키는_편집모드_없이도_선택기를_연다() {
    // Modeless: 구체 보드가 선택돼 있으면 `a`는 곧바로 핀 선택기를 연다.
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
fn 전체_필터이면_도구추가키는_아무_일도_하지_않는다() {
    let mut s = fresh();
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('a'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::List));
}

#[test]
fn 편집_모드에서_소문자_또는_대문자_a는_도구_선택기를_빈_검색어로_연다() {
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
fn 도구_선택기에서_입력하면_검색어가_필터링된다() {
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
        panic!("ToolPicker를 기대했다");
    }
}

#[test]
fn 도구_선택기는_공유_검색으로_도구킷과_태그를_매칭한다() {
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
fn 도구_선택기는_활성_보드의_고정_도구를_먼저_정렬한다() {
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
fn 도구_선택기는_아홉개를_넘는_검색_결과를_자르지_않는다() {
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
        "도구 선택기는 예전 9개 제한을 적용하지 않아야 한다: {ids:?}"
    );
    assert!(
        ids.len() > OLD_NINE_RESULT_CAP,
        "검색 결과는 9개를 넘어도 유지되어야 한다"
    );
}

#[test]
fn 도구_선택기는_최근_로그가_없어도_열리고_검색한다() {
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
fn 도구_선택기에서_엔터는_고정을_토글하고_보기를_열어_둔다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    // 토글 결과를 관찰할 수 있도록 dev의 기본 레이아웃을 비운다.
    s.layouts.insert("dev".into(), Vec::new());
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('a'), t.as_slice());
    let needle = "num.hex_to_decimal";
    for ch in needle.chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    // 정확히 일치하는 검색어는 해당 도구를 커서 0에 둔다.
    let effect = handle_key(&mut s, Key::Enter, t.as_slice());
    assert_eq!(effect, Effect::SavePegboard);
    let pinned = layout_contains(&s, "dev", needle);
    assert!(
        pinned,
        "선택기에서 커서가 있는 행에 Enter를 누르면 도구가 고정되어야 한다"
    );
    assert!(
        matches!(s.view, View::ToolPicker { .. }),
        "여러 도구를 고정할 수 있도록 고정 후에도 선택기는 열려 있어야 한다"
    );
}

#[test]
fn 도구_선택기에서_이미_고정된_도구에_엔터를_누르면_고정이_해제된다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    let needle = "num.hex_to_decimal";
    // 도구가 정확히 한 번 고정된 상태로 시작하게 한다.
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
        "고정된 도구에서 Enter를 누르면 토글로 고정이 해제되어야 한다"
    );
}

#[test]
fn 도구_선택기에서_이스케이프는_오버레이를_닫는다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('a'), t.as_slice());
    handle_key(&mut s, Key::Esc, t.as_slice());
    assert!(matches!(s.view, View::List));
}

#[test]
fn slash는_edit_mode_없이_search_picker를_연다() {
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
fn 오른쪽_패널_focus에서_ctrl_k는_search_picker를_연다() {
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
        "Ctrl+K는 RightPane 스크롤로 소비되지 않고 search picker를 열어야 한다. got: {:?}",
        s.view
    );
}

#[test]
fn search_picker는_열릴_때_focus를_초기화해서_키입력을_picker로_보낸다() {
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
                "Down은 RightPane 스크롤이 아니라 picker cursor를 내려야 한다"
            );
        }
        other => panic!("expected search ToolPicker, got {other:?}"),
    }

    handle_key(&mut s, Key::Char('j'), t.as_slice());
    match &s.view {
        View::ToolPicker { query, cursor, .. } => {
            assert_eq!(
                query, "j",
                "j는 RightPane 스크롤이 아니라 picker 검색어여야 한다"
            );
            assert_eq!(
                *cursor, 0,
                "검색어 입력 후 picker cursor는 맨 위로 돌아가야 한다"
            );
        }
        other => panic!("expected search ToolPicker, got {other:?}"),
    }
}

#[test]
fn detail에서_연_search_picker는_esc로_detail에_돌아간다() {
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
fn search_picker_enter는_핀을_바꾸지_않고_상세를_연다() {
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
