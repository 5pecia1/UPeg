use super::*;
use upeg_core::{ColSpan, PinSpan, RowSpan};

/// movement.rs의 이동 테스트와 같은 무대: dev 보드 하나에 fixture 도구를
/// 등록하고 지정한 placement로 시작한다.
fn resize_stage(state: &mut State, placements: Vec<Placement>) -> Vec<&'static ToolMeta> {
    state.filters.select_board("dev");
    state.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "dev".into(),
        title: "Dev".into(),
    }];
    for tool in fixture_tools() {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    state.layouts.insert("dev".into(), placements);
    upeg_sources::pegboard::tools_for_board_and_tag_in(
        &state.pegboard_snapshot(),
        Some("dev"),
        None,
    )
}

fn test_span(cols: u16, rows: u16) -> PinSpan {
    PinSpan::new(
        ColSpan::new(cols).expect("테스트 cols"),
        RowSpan::new(rows).expect("테스트 rows"),
    )
}

/// 커밋된 dev 레이아웃에서 대상 핀의 span override를 읽는다.
/// `None`은 "override 없음(manifest 크기)"을 뜻한다.
fn committed_span(state: &State, tool_id: &str) -> Option<(u16, u16)> {
    state
        .layouts
        .get("dev")
        .and_then(|placements| placements.iter().find(|p| p.tool_id == tool_id))
        .expect("커밋된 placement가 남아 있어야 한다")
        .span
        .map(PinSpan::grid_span)
}

#[test]
fn 리사이즈_키는_미리보기만_바꾸고_엔터는_span을_커밋한다() {
    let mut s = fresh();
    let tools = resize_stage(
        &mut s,
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    s.cursor = 0;
    let committed_before = s.layouts.get("dev").cloned().unwrap();

    assert_eq!(handle_key(&mut s, Key::Char('e'), &tools), Effect::None);
    let resize = s.resize_mode.as_ref().expect("e는 리사이즈 모드를 연다");
    assert_eq!(
        (resize.cols, resize.rows),
        (1, 1),
        "시작 크기는 committed placement의 effective_size다"
    );

    assert_eq!(handle_key(&mut s, Key::Right, &tools), Effect::None);
    let resize = s.resize_mode.as_ref().expect("resize mode active");
    assert_eq!((resize.cols, resize.rows), (2, 1));
    assert_eq!(
        resize
            .preview
            .iter()
            .find(|p| p.tool_id == "test.simple")
            .and_then(|p| p.span)
            .map(PinSpan::grid_span),
        Some((2, 1)),
        "미리보기의 대상 핀은 span override를 담아야 한다"
    );
    assert_eq!(
        s.layouts.get("dev").cloned().unwrap(),
        committed_before,
        "화살표 키 미리보기는 Enter 전에는 커밋되면 안 된다"
    );

    assert_eq!(handle_key(&mut s, Key::Enter, &tools), Effect::SavePegboard);
    assert!(
        s.resize_mode.is_none(),
        "Enter는 리사이즈 모드를 닫아야 한다"
    );
    assert_eq!(
        committed_span(&s, "test.simple"),
        Some((2, 1)),
        "커밋된 레이아웃은 span=Some(2x1)을 저장해야 한다"
    );
}

#[test]
fn 리사이즈_모드에서_이스케이프는_미리보기를_버린다() {
    let mut s = fresh();
    let tools = resize_stage(
        &mut s,
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    s.cursor = 0;
    let before = s.layouts.clone();

    handle_key(&mut s, Key::Char('e'), &tools);
    handle_key(&mut s, Key::Right, &tools);
    let effect = handle_key(&mut s, Key::Esc, &tools);

    assert_eq!(effect, Effect::None);
    assert!(s.resize_mode.is_none());
    assert_eq!(s.layouts, before, "Esc는 미리보기를 버려야 한다");
}

#[test]
fn 스팬_한도를_넘는_증감은_무시된다() {
    let mut s = fresh();
    let tools = resize_stage(&mut s, vec![Placement::new("test.simple", 0, 0)]);
    s.cursor = 0;

    handle_key(&mut s, Key::Char('e'), &tools);
    // cols 1에서 -1, rows 1에서 -1은 newtype 검증에 걸려 무시된다.
    handle_key(&mut s, Key::Left, &tools);
    handle_key(&mut s, Key::Up, &tools);
    let resize = s.resize_mode.as_ref().expect("resize mode active");
    assert_eq!((resize.cols, resize.rows), (1, 1), "하한 밑으로는 무시");
    assert_eq!(
        resize.preview, resize.base,
        "무시된 키는 미리보기를 바꾸면 안 된다"
    );
    // 변화 없는 커밋은 저장 효과가 없다.
    assert_eq!(
        handle_key(&mut s, Key::Enter, &tools),
        Effect::None,
        "미리보기가 base와 같으면 Enter는 저장하지 않는다"
    );
    assert!(s.resize_mode.is_none());

    // cols는 BOARD_COLS(6)가 상한이다.
    handle_key(&mut s, Key::Char('e'), &tools);
    for _ in 0..usize::from(upeg_core::BOARD_COLS) {
        handle_key(&mut s, Key::Right, &tools);
    }
    let resize = s.resize_mode.as_ref().expect("resize mode active");
    assert_eq!(
        resize.cols,
        upeg_core::BOARD_COLS,
        "6에서 +1은 무시되어 상한에 머문다"
    );
}

#[test]
fn 리셋키는_manifest_크기로_되돌리고_커밋_시_span을_지운다() {
    let mut s = fresh();
    let tools = resize_stage(
        &mut s,
        vec![Placement::new("test.simple", 0, 0).with_span(Some(test_span(2, 2)))],
    );
    s.cursor = 0;

    handle_key(&mut s, Key::Char('e'), &tools);
    let resize = s.resize_mode.as_ref().expect("resize mode active");
    assert_eq!(
        (resize.cols, resize.rows),
        (2, 2),
        "committed span override가 시작 크기를 결정한다"
    );

    handle_key(&mut s, Key::Char('0'), &tools);
    let resize = s.resize_mode.as_ref().expect("resize mode active");
    assert_eq!(
        (resize.cols, resize.rows),
        (1, 1),
        "0은 manifest placement_size로 되돌린다"
    );

    assert_eq!(handle_key(&mut s, Key::Enter, &tools), Effect::SavePegboard);
    assert_eq!(
        committed_span(&s, "test.simple"),
        None,
        "manifest 크기로 커밋하면 중복 override 대신 span=None을 저장한다"
    );
}

#[test]
fn 커지는_스팬은_미리보기에서_이웃을_밀어낸다() {
    let mut s = fresh();
    let tools = resize_stage(
        &mut s,
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    s.cursor = 0;

    handle_key(&mut s, Key::Char('e'), &tools);
    handle_key(&mut s, Key::Right, &tools);
    let preview = s
        .resize_mode
        .as_ref()
        .expect("resize mode active")
        .preview
        .clone();
    assert_eq!(
        preview
            .iter()
            .find(|p| p.tool_id == "test.with_input")
            .map(|p| (p.x, p.y)),
        Some((2, 0)),
        "충돌한 이웃은 place_tool_with_push로 밀려나야 한다"
    );

    handle_key(&mut s, Key::Enter, &tools);
    assert_eq!(
        layout_ids_by_position(&s, "dev"),
        vec!["test.simple", "test.with_input"],
        "커밋된 순서는 밀어낸 좌표를 따라야 한다"
    );
    let after = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    let expected = after
        .iter()
        .position(|t| t.id == "test.simple")
        .expect("리사이즈한 도구는 보드에 남아 있어야 한다");
    assert_eq!(
        s.cursor, expected,
        "커밋 후 커서는 리사이즈한 도구를 다시 가리켜야 한다"
    );
}

#[test]
fn 마우스_다운은_리사이즈_모드를_해제하여_키보드와_정합성을_유지한다() {
    // move_mode의 마우스 취소 회귀 테스트와 같은 계약: Down 클릭은 숨은
    // 리사이즈 미리보기를 먼저 닫아 Enter가 엉뚱한 커밋을 하지 못하게 한다.
    let mut s = fresh();
    let tools = resize_stage(
        &mut s,
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    s.cursor = 0;
    handle_key(&mut s, Key::Char('e'), &tools);
    assert!(s.resize_mode.is_some(), "사전조건: e는 리사이즈를 시작한다");

    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);
    let mouse = Mouse {
        column: layout.boards.x + 1,
        row: layout.boards.y + 1,
        kind: MouseKind::Pointer(PointerPhase::Down),
    };
    let _ = handle_mouse(&mut s, mouse, &tools, area);

    assert!(
        s.resize_mode.is_none(),
        "마우스 Down은 리사이즈 미리보기를 취소해야 한다"
    );
}
