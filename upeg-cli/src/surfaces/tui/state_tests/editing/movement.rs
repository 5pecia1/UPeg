use super::*;

#[test]
fn 포커스가_없으면_대괄호키는_아무_일도_하지_않는다() {
    // Modeless: 편집 게이트는 사라졌지만 `[`/`]`는 여전히 포커스된 핀을
    // 요구한다. 보이는 도구가 없으면(빈 그리드) 재정렬은 조용히 무시된다.
    let mut s = fresh();
    s.filters.select_board("dev");
    s.cursor = 0;
    let before = s.layouts.get("dev").cloned().unwrap_or_default();
    assert_eq!(handle_key(&mut s, Key::Char(']'), &[]), Effect::None);
    assert_eq!(s.layouts.get("dev").cloned().unwrap_or_default(), before);
}

#[test]
fn 닫는_대괄호키는_커서가_있는_도구를_뒤쪽으로_옮긴다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    assert!(tools.len() >= 2);
    let first_id = tools[0].id;
    let second_id = tools[1].id;
    s.cursor = 0;
    let effect = handle_key(&mut s, Key::Char(']'), &tools);
    assert_eq!(effect, Effect::SavePegboard);
    let layout = layout_ids_by_position(&s, "dev");
    assert_eq!(layout[0], second_id);
    assert_eq!(layout[1], first_id);
    assert_eq!(s.cursor, 1, "커서는 이동한 도구를 따라가야 한다");
}

#[test]
fn 첫_칸에서_여는_대괄호키는_아무_일도_하지_않는다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    s.cursor = 0;
    let before = s.layouts.get("dev").cloned().unwrap_or_default();
    let effect = handle_key(&mut s, Key::Char('['), &tools);
    assert_eq!(effect, Effect::None);
    assert_eq!(s.layouts.get("dev").cloned().unwrap_or_default(), before);
    assert_eq!(s.cursor, 0);
}

#[test]
fn 마지막_칸에서_닫는_대괄호키는_아무_일도_하지_않는다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    let last_idx = tools.len() - 1;
    s.cursor = last_idx;
    let before = s.layouts.get("dev").cloned().unwrap_or_default();
    let effect = handle_key(&mut s, Key::Char(']'), &tools);
    assert_eq!(effect, Effect::None);
    assert_eq!(s.layouts.get("dev").cloned().unwrap_or_default(), before);
    assert_eq!(s.cursor, last_idx);
}

#[test]
fn 이동키는_밀어내기_미리보기를_시작하고_엔터는_커밋한다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    s.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "dev".into(),
        title: "Dev".into(),
    }];
    let fixtures = fixture_tools();
    for tool in fixtures {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    s.layouts.insert(
        "dev".into(),
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
            Placement::new("test.third", 2, 0),
        ],
    );
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    s.cursor = 2;
    let committed_before = s.layouts.get("dev").cloned().unwrap();

    assert_eq!(handle_key(&mut s, Key::Char('m'), &tools), Effect::None);
    assert!(s.move_mode.is_some(), "m은 좌표 이동 모드를 시작해야 한다");
    assert_eq!(handle_key(&mut s, Key::Left, &tools), Effect::None);

    let preview = s
        .move_mode
        .as_ref()
        .expect("move mode active")
        .preview
        .clone();
    assert_eq!(
        preview
            .iter()
            .find(|p| p.tool_id == "test.third")
            .map(|p| (p.x, p.y)),
        Some((1, 0)),
        "이동한 위젯은 대상 좌표를 차지해야 한다"
    );
    assert_eq!(
        preview
            .iter()
            .find(|p| p.tool_id == "test.with_input")
            .map(|p| (p.x, p.y)),
        Some((2, 0)),
        "충돌한 위젯은 미리보기에서 앞으로 밀려야 한다"
    );
    assert_eq!(
        s.layouts.get("dev").cloned().unwrap(),
        committed_before,
        "화살표 키 미리보기는 Enter 전에는 커밋되면 안 된다"
    );

    assert_eq!(handle_key(&mut s, Key::Enter, &tools), Effect::SavePegboard);
    assert!(s.move_mode.is_none(), "Enter는 이동 모드를 닫아야 한다");
    assert_eq!(
        layout_ids_by_position(&s, "dev"),
        vec!["test.simple", "test.third", "test.with_input"],
        "커밋된 순서는 밀어낸 좌표를 따라야 한다"
    );
}

#[test]
fn 마우스_다운은_이동_모드를_해제하여_키보드와_정합성을_유지한다() {
    // Codex 회귀: m 으로 좌표 이동 미리보기를 켜둔 상태에서 마우스로
    // 보드/태그/그리드를 클릭하면 키보드는 여전히 이동 모드로 잠겨
    // Enter 가 숨은 미리보기를 커밋할 위험이 있었다. 마우스 Down 은
    // 이동 모드를 먼저 닫고 일반 입력으로 돌려놓는다.
    let mut s = fresh();
    s.filters.select_board("dev");
    s.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "dev".into(),
        title: "Dev".into(),
    }];
    for tool in fixture_tools() {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    s.layouts.insert(
        "dev".into(),
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    s.cursor = 1;
    handle_key(&mut s, Key::Char('m'), &tools);
    assert!(s.move_mode.is_some(), "사전조건: m 은 좌표 이동을 시작한다");

    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);
    // 보드 바 클릭은 흔히 다른 보드/필터로 점프하는 동작이다.
    let mouse = Mouse {
        column: layout.boards.x + 1,
        row: layout.boards.y + 1,
        kind: MouseKind::Pointer(PointerPhase::Down),
    };
    let _ = handle_mouse(&mut s, mouse, &tools, area);

    assert!(
        s.move_mode.is_none(),
        "마우스 Down 은 좌표 이동을 취소해야 키보드와 정합이 유지된다"
    );
}

#[test]
fn 커밋_후_커서는_재정렬된_새_위치를_따라간다() {
    // Codex 회귀: commit_move 가 레이아웃만 갱신하고 cursor 를 그대로
    // 두어, 다음 키 입력이 엉뚱한 도구에 적용되었다. push 정렬 후 새
    // (y, x) 위치를 찾아 cursor 를 재고정한다.
    let mut s = fresh();
    s.filters.select_board("dev");
    s.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "dev".into(),
        title: "Dev".into(),
    }];
    for tool in fixture_tools() {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    s.layouts.insert(
        "dev".into(),
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
            Placement::new("test.third", 2, 0),
        ],
    );
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    s.cursor = 2; // test.third

    handle_key(&mut s, Key::Char('m'), &tools);
    handle_key(&mut s, Key::Left, &tools);
    let effect = handle_key(&mut s, Key::Enter, &tools);
    assert_eq!(effect, Effect::SavePegboard);

    let after = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    let expected = after
        .iter()
        .position(|t| t.id == "test.third")
        .expect("이동한 도구는 보드에 남아 있어야 한다");
    assert_eq!(
        s.cursor, expected,
        "커밋 후 커서는 방금 옮긴 도구의 새 인덱스를 가리켜야 한다"
    );
}

#[test]
fn 이동_모드에서_이스케이프는_미리보기를_버린다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    s.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "dev".into(),
        title: "Dev".into(),
    }];
    let fixtures = fixture_tools();
    for tool in fixtures {
        upeg_runtime::toolbox_add_tool((*tool).clone());
    }
    s.layouts.insert(
        "dev".into(),
        vec![
            Placement::new("test.simple", 0, 0),
            Placement::new("test.with_input", 1, 0),
        ],
    );
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    s.cursor = 1;
    let before = s.layouts.clone();

    handle_key(&mut s, Key::Char('m'), &tools);
    handle_key(&mut s, Key::Left, &tools);
    let effect = handle_key(&mut s, Key::Esc, &tools);

    assert_eq!(effect, Effect::None);
    assert!(s.move_mode.is_none());
    assert_eq!(s.layouts, before, "Esc는 미리보기를 버려야 한다");
}

#[test]
fn 포커스가_없으면_핀키는_아무_일도_하지_않는다() {
    // Modeless: `p`는 포커스된 핀을 요구한다. 보이는 도구가 없으면
    // 조용히 무시된다 — 편집 모드가 아니라 포커스가 유일한 게이트다.
    let mut s = fresh();
    s.filters.select_board("dev");
    s.cursor = 0;
    let before = s.layouts.get("dev").cloned().unwrap_or_default();
    let effect = handle_key(&mut s, Key::Char('p'), &[]);
    assert_eq!(effect, Effect::None);
    assert_eq!(s.layouts.get("dev").cloned().unwrap_or_default(), before);
}

#[test]
fn 전체_필터이면_핀키는_아무_일도_하지_않는다() {
    // "all" 보드 필터는 어떤 레이아웃을 바꿀지 모호하다. 사용자가
    // 먼저 구체적인 보드를 고르도록 작업을 거부해야 한다.
    let mut s = fresh();
    s.cursor = 0;
    let tools = s.visible_tools();
    let before_layouts = s.layouts.clone();
    let effect = handle_key(&mut s, Key::Char('p'), &tools);
    assert_eq!(effect, Effect::None);
    assert_eq!(s.layouts, before_layouts);
}

#[test]
fn 편집_모드에서_핀키는_현재_고정된_도구를_해제한다() {
    // 토글 의미: 그리드는 고정된 도구만 보여주므로 커서 아래 항목에서
    // 첫 `p` 입력은 해당 도구의 고정을 해제한다.
    let mut s = fresh();
    s.filters.select_board("dev");
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    assert!(
        !tools.is_empty(),
        "dev 보드는 시작 시 고정된 도구를 가져야 한다"
    );
    let target_id = tools[0].id;
    s.cursor = 0;

    let effect = handle_key(&mut s, Key::Char('p'), &tools);

    assert_eq!(effect, Effect::SavePegboard);
    assert!(
        !layout_contains(&s, "dev", target_id),
        "첫 번째 p 입력은 커서가 있는 도구의 고정을 해제해야 한다"
    );
}

#[test]
fn 고정_해제로_그리드가_줄면_핀키는_커서를_안쪽으로_고정한다() {
    // 마지막 행의 고정을 해제하면 커서가 새 마지막 행으로 올라와야 한다.
    // 그렇지 않으면 다음 렌더에서 레이아웃 끝을 넘어 인덱싱한다.
    let mut s = fresh();
    s.filters.select_board("dev");
    let tools = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("dev"),
        None,
    );
    assert!(
        tools.len() >= 2,
        "이 테스트에는 고정된 dev 도구가 적어도 두 개 필요하다"
    );
    s.cursor = tools.len() - 1;

    handle_key(&mut s, Key::Char('p'), &tools);

    let new_len = s.layouts.get("dev").map(Vec::len).unwrap_or_default();
    assert!(s.cursor < new_len, "커서는 그리드 안쪽으로 고정되어야 한다");
}
