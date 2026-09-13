use super::*;

#[test]
fn 마지막_남은_보드에서_삭제키는_아무_일도_하지_않는다() {
    // 마지막 보드 동기화 보호 장치: 보드가 하나만 남았을 때 TUI는
    // 삭제를 거부해야 한다. 그렇지 않으면 sources::sanitize_state가
    // 디스크에서는 default_boards()를 조용히 복원하지만 메모리 캐시는
    // 빈 상태로 남아, 사용자가 재시작 전까지 빈 페그보드를 보게 된다.
    let mut s = fresh();
    s.boards = vec![upeg_sources::pegboard::BoardData {
        guidance: upeg_core::BoardGuidance::default(),
        key: "solo".into(),
        title: "Solo".into(),
    }];
    s.layouts.clear();
    s.layouts.insert("solo".into(), Vec::new());
    s.filters.select_board("solo");
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('D'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(
        matches!(s.view, View::List),
        "마지막 보드에서 D는 ConfirmDeleteBoard를 열면 안 된다"
    );
    assert_eq!(s.boards.len(), 1, "보드 목록은 바뀌면 안 된다");
    assert!(
        s.boards.iter().any(|b| b.key == "solo"),
        "유일한 보드는 그대로 남아 있어야 한다"
    );
}

#[test]
fn 보드가_여러_개이면_삭제키는_확인창을_연다() {
    // 보호 장치의 정상 경로 회귀 테스트: 보드가 여러 개일 때는
    // 확인 오버레이가 계속 열려야 한다. 보호 장치가 일반적인 경우까지
    // 실수로 막으면 안 된다.
    let mut s = fresh();
    s.filters.select_board("dev");
    assert!(
        s.boards.len() > 1,
        "fresh()는 3개 항목이 있는 default_boards()를 준비한다"
    );
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('D'), t.as_slice());
    assert!(
        matches!(s.view, View::ConfirmDeleteBoard { .. }),
        "보드가 여러 개이면 D는 확인 오버레이를 열어야 한다"
    );
}

#[test]
fn 삭제키는_편집모드_없이도_확인창을_연다() {
    // Modeless: 편집 토글이 사라졌으므로 구체 보드가 선택돼 있고 보드가
    // 여러 개면 `D`는 곧바로 삭제 확인 오버레이를 연다 — 별도 모드 진입
    // 없이. (오종료 방지는 확인 오버레이 자체가 담당한다.)
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('D'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::ConfirmDeleteBoard { .. }));
}

#[test]
fn 전체_필터이면_삭제키는_아무_일도_하지_않는다() {
    let mut s = fresh();
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('D'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::List));
}

#[test]
fn 삭제키는_보드_삭제_확인_오버레이를_연다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('D'), t.as_slice());
    match &s.view {
        View::ConfirmDeleteBoard { key, title } => {
            assert_eq!(key, "dev");
            assert_eq!(title, "Dev");
        }
        other => panic!("ConfirmDeleteBoard를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 삭제_확인에서_예를_누르면_보드를_지우고_첫_남은_보드로_돌아간다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('D'), t.as_slice());
    let effect = handle_key(&mut s, Key::Char('y'), t.as_slice());
    assert_eq!(effect, Effect::SavePegboard);
    assert!(!s.boards.iter().any(|b| b.key == "dev"));
    assert!(!s.layouts.contains_key("dev"));
    let new_filter = s.filters.board.as_deref();
    assert!(
        new_filter.is_some(),
        "필터는 None으로 사라지지 않고 첫 번째 남은 보드로 돌아가야 한다"
    );
    assert_ne!(new_filter, Some("dev"));
    assert!(matches!(s.view, View::List));
}

#[test]
fn 삭제_확인에서_아니오를_누르면_상태_변경_없이_취소한다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('D'), t.as_slice());
    let before_boards = s.boards.clone();
    let before_layouts = s.layouts.clone();
    let effect = handle_key(&mut s, Key::Char('n'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::List));
    assert_eq!(s.boards, before_boards);
    assert_eq!(s.layouts, before_layouts);
}

#[test]
fn 삭제_확인에서_이스케이프는_취소한다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('D'), t.as_slice());
    let before_boards = s.boards.clone();
    handle_key(&mut s, Key::Esc, t.as_slice());
    assert!(matches!(s.view, View::List));
    assert_eq!(s.boards, before_boards);
}

#[test]
fn 이름변경키는_편집모드_없이도_편집기를_연다() {
    // Modeless: 구체 보드가 선택돼 있으면 `R`은 곧바로 이름변경 편집기를
    // 연다 — 편집 모드 진입 단계 없이.
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('R'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(
        s.view,
        View::BoardEditor {
            mode: crate::surfaces::tui::model::BoardEditMode::Rename { .. },
            ..
        }
    ));
}

#[test]
fn 전체_필터이면_이름변경키는_아무_일도_하지_않는다() {
    let mut s = fresh();
    // filter.board를 None으로 두면 "all"이다.
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('R'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::List));
}

#[test]
fn 이름변경키는_현재_제목을_담은_이름변경_보드_편집기를_연다() {
    use crate::surfaces::tui::model::BoardEditMode;
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('R'), t.as_slice());
    match &s.view {
        View::BoardEditor {
            mode:
                BoardEditMode::Rename {
                    key,
                    original_title,
                },
            buffer,
        } => {
            assert_eq!(key, "dev");
            assert_eq!(original_title, "Dev");
            assert_eq!(
                buffer, "Dev",
                "이름 변경 버퍼는 현재 제목으로 미리 채워져야 한다"
            );
        }
        other => panic!("BoardEditor::Rename을 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 보드_편집기의_이름변경_커밋은_제목만_바꾸고_키를_유지한다() {
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('R'), t.as_slice());
    // 버퍼 내용을 교체한다.
    for _ in 0..10 {
        handle_key(&mut s, Key::Backspace, t.as_slice());
    }
    for ch in "Development".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    let effect = handle_key(&mut s, Key::Enter, t.as_slice());
    assert_eq!(effect, Effect::SavePegboard);
    let dev = s
        .boards
        .iter()
        .find(|b| b.key == "dev")
        .expect("dev 보드는 그대로 있어야 한다");
    assert_eq!(dev.title, "Development");
    assert_eq!(
        s.filters.board.as_deref(),
        Some("dev"),
        "필터는 같은 키에 계속 붙어 있어야 한다"
    );
}

#[test]
fn 새_보드키는_보드_필터가_있어도_추가_편집기를_연다() {
    // Modeless: `n`은 보드 필터 유무와 무관하게 항상 추가 편집기를 연다
    // (보드 생성은 보드 레벨 작업이라 포커스/필터를 요구하지 않는다).
    use crate::surfaces::tui::model::BoardEditMode;
    let mut s = fresh();
    s.filters.select_board("dev");
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('n'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(
        s.view,
        View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            ..
        }
    ));
}

#[test]
fn 새_보드키는_편집모드_없이_추가_편집기를_연다() {
    use crate::surfaces::tui::model::BoardEditMode;
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    match &s.view {
        View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            buffer,
        } => assert!(buffer.is_empty()),
        other => panic!("BoardEditor::AddBoard를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 보드_편집기에서_입력한_문자는_버퍼에_쌓인다() {
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    for ch in "test".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    if let View::BoardEditor { buffer, .. } = &s.view {
        assert_eq!(buffer, "test");
    } else {
        panic!("BoardEditor 보기를 기대했지만 {:?}를 받았다", s.view);
    }
}

#[test]
fn 보드_편집기에서_백스페이스는_버퍼의_마지막_문자를_지운다() {
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    for ch in "abc".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    handle_key(&mut s, Key::Backspace, t.as_slice());
    if let View::BoardEditor { buffer, .. } = &s.view {
        assert_eq!(buffer, "ab");
    } else {
        panic!("BoardEditor 보기를 기대했다");
    }
}

#[test]
fn 보드_편집기에서_엔터는_새_보드를_커밋하고_필터를_이동한다() {
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    for ch in "Project Alpha".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    let effect = handle_key(&mut s, Key::Enter, t.as_slice());
    assert_eq!(effect, Effect::SavePegboard);
    assert!(matches!(s.view, View::List));
    assert_eq!(
        s.filters.board.as_deref(),
        Some("project-alpha"),
        "필터는 새로 추가한 보드로 따라가야 한다"
    );
    assert!(
        s.boards.iter().any(|b| b.key == "project-alpha"),
        "새 보드는 캐시에 나타나야 한다"
    );
}

#[test]
fn 보드_편집기에서_빈_버퍼로_엔터를_누르면_아무_일도_하지_않는다() {
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    let effect = handle_key(&mut s, Key::Enter, t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::BoardEditor { .. }));
}

#[test]
fn 보드_편집기에서_이스케이프는_취소하고_목록으로_돌아간다() {
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('n'), t.as_slice());
    for ch in "discard".chars() {
        handle_key(&mut s, Key::Char(ch), t.as_slice());
    }
    let before_boards = s.boards.clone();
    handle_key(&mut s, Key::Esc, t.as_slice());
    assert!(matches!(s.view, View::List));
    assert_eq!(s.boards, before_boards, "ESC는 버퍼를 저장하면 안 된다");
}
