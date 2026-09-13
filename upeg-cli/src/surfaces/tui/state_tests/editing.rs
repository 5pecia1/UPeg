use super::*;

mod board_crud;
mod i18n_keys;
mod movement;
mod pin_color_editor;
mod resize;
mod tool_picker;

#[test]
fn 전체_보드_흐름은_추가_고정_재정렬_이름변경_삭제를_차례로_수행한다() {
    // 기존 PR 전 커버리지가 놓쳤던 엔드투엔드 흐름이다. 각 키가
    // 순서대로 역할을 수행하고, 누적 상태가 실제 사용자가 단계별로
    // 보게 될 모습과 일치하는지 확인한다. Modeless: 편집 토글 없이
    // 곧바로 보드 관리 키를 쓴다.
    use crate::surfaces::tui::model::BoardEditMode;
    let mut s = fresh();
    let t = fixture_tools();
    let tools = t.as_slice();

    // 1. `n`, `Alpha`, Enter 순서로 새 보드를 만들고 필터가 따라간다.
    handle_key(&mut s, Key::Char('n'), tools);
    assert!(matches!(
        s.view,
        View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            ..
        }
    ));
    for ch in "Alpha".chars() {
        handle_key(&mut s, Key::Char(ch), tools);
    }
    let effect_add = handle_key(&mut s, Key::Enter, tools);
    assert_eq!(effect_add, Effect::SavePegboard);
    assert_eq!(
        s.filters.board.as_deref(),
        Some("alpha"),
        "2단계: 필터는 새 보드로 따라가야 한다"
    );
    assert!(s.boards.iter().any(|b| b.key == "alpha"));

    // 3. `a`가 ToolPicker를 열고, 검색어를 입력한 뒤 Enter로 고정하고 ESC로 닫는다.
    handle_key(&mut s, Key::Char('a'), tools);
    let needle = "num.hex_to_decimal";
    let real_tools_for_dispatch = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &s.pegboard_snapshot(),
        Some("alpha"),
        None,
    );
    let _ = real_tools_for_dispatch;
    for ch in needle.chars() {
        handle_key(&mut s, Key::Char(ch), tools);
    }
    let effect_pin = handle_key(&mut s, Key::Enter, tools);
    assert_eq!(effect_pin, Effect::SavePegboard);
    handle_key(&mut s, Key::Esc, tools);
    assert!(matches!(s.view, View::List));
    assert!(
        layout_contains(&s, "alpha", needle),
        "3단계: alpha 보드에 도구가 고정되어야 한다"
    );

    // 4. 재정렬할 이웃이 있도록 두 번째 도구를 고정한 뒤 `[` / `]`를 누른다.
    handle_key(&mut s, Key::Char('a'), tools);
    let second = "convert.base64_encode";
    for ch in second.chars() {
        handle_key(&mut s, Key::Char(ch), tools);
    }
    handle_key(&mut s, Key::Enter, tools);
    handle_key(&mut s, Key::Esc, tools);
    // 런타임 루프와 같은 방식으로 보이는 도구 스냅샷을 갱신한다.
    let visible = s.visible_tools();
    assert!(visible.len() >= 2);
    s.cursor = 0;
    let effect_swap = handle_key(&mut s, Key::Char(']'), &visible);
    assert_eq!(effect_swap, Effect::SavePegboard);
    let layout = s.layouts.get("alpha").cloned().unwrap_or_default();
    assert_eq!(layout.len(), 2, "4단계: 고정된 도구는 계속 두 개여야 한다");
    assert_eq!(
        s.cursor, 1,
        "4단계: `]` 뒤에는 커서가 이동한 도구를 따라가야 한다"
    );

    // 5. `R` 이름 변경은 버퍼를 미리 채우고, 이를 "Beta"로 편집한다.
    handle_key(&mut s, Key::Char('R'), &visible);
    for _ in 0..20 {
        handle_key(&mut s, Key::Backspace, &visible);
    }
    for ch in "Beta".chars() {
        handle_key(&mut s, Key::Char(ch), &visible);
    }
    let effect_rename = handle_key(&mut s, Key::Enter, &visible);
    assert_eq!(effect_rename, Effect::SavePegboard);
    let alpha = s
        .boards
        .iter()
        .find(|b| b.key == "alpha")
        .expect("키는 보존되어야 한다");
    assert_eq!(
        alpha.title, "Beta",
        "5단계: 제목은 바뀌고 키는 안정적으로 유지되어야 한다"
    );

    // 6. `D` 다음 `y`는 보드를 지우고 필터를 대체 보드로 돌린다.
    handle_key(&mut s, Key::Char('D'), &visible);
    assert!(matches!(s.view, View::ConfirmDeleteBoard { .. }));
    let effect_delete = handle_key(&mut s, Key::Char('y'), &visible);
    assert_eq!(effect_delete, Effect::SavePegboard);
    assert!(!s.boards.iter().any(|b| b.key == "alpha"));
    assert!(
        s.filters.board.is_some(),
        "6단계: 필터는 None이 아니라 남은 보드로 돌아가야 한다"
    );
}

#[test]
fn 편집_흐름은_커밋된_변경에서만_저장_효과를_발생시킨다() {
    // Effect 계약: Effect::SavePegboard는 sources 상태가 실제로 바뀔 때만
    // 발생해야 한다. BoardEditor 버퍼에 입력하거나 Backspace를 누르는 일은
    // 메모리 안에서만 끝난다. 빈 버퍼에서 Enter를 누르는 것도 구조적으로
    // 아무 일도 아니다. 기존 PR 전에는 이후 리팩터가 키 입력마다 디스크
    // 쓰기를 남발할 위험이 있었다.
    let mut s = fresh();
    let t = fixture_tools();
    let tools = t.as_slice();

    let mut save_count = 0_usize;
    let mut log = |effect: Effect| {
        if matches!(effect, Effect::SavePegboard) {
            save_count += 1;
        }
    };

    log(handle_key(&mut s, Key::Char('n'), tools)); // 편집기 열기
    for ch in "Beta".chars() {
        log(handle_key(&mut s, Key::Char(ch), tools)); // 입력은 저장하지 않는다.
    }
    log(handle_key(&mut s, Key::Backspace, tools)); // 백스페이스도 저장하지 않는다.
    log(handle_key(&mut s, Key::Enter, tools)); // 커밋은 저장한다.
    assert_eq!(
        save_count, 1,
        "Enter 커밋만 Effect::SavePegboard를 발생시켜야 한다"
    );

    // 빈 버퍼 커밋도 저장을 발생시키면 안 된다.
    handle_key(&mut s, Key::Char('n'), tools);
    let effect_blank = handle_key(&mut s, Key::Enter, tools);
    assert_eq!(
        effect_blank,
        Effect::None,
        "빈 버퍼 Enter는 커밋하거나 저장하면 안 된다"
    );
}

#[test]
fn 보드_미선택이면_e는_리사이즈를_시작하지_않는다() {
    // `e`는 이제 보드 스코프에서 StartResize로 해석된다(`m`/StartMove와
    // 대칭). 다만 "all" 보드 필터에서는 어떤 레이아웃을 바꿀지 모호하므로
    // start_resize가 조용히 거부한다 — p/m과 같은 게이트.
    let mut s = fresh();
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('e'), t.as_slice());
    assert_eq!(effect, Effect::None, "`e`는 아무 효과도 내면 안 된다");
    assert!(s.resize_mode.is_none(), "보드 미선택이면 리사이즈 불가");
    assert!(matches!(s.view, View::List), "`e`는 보기를 바꾸면 안 된다");
    handle_key(&mut s, Key::Char('E'), t.as_slice());
    assert!(s.resize_mode.is_none(), "`E`도 같은 게이트를 지난다");
}

#[test]
fn 상세_보기에서도_e는_자유키다() {
    // Detail 스코프는 `e`를 바인딩하지 않는다(보드 스코프의 StartResize와
    // 달리). 상세 보기에서는 여전히 자유키다.
    let mut s = State {
        view: View::Detail,
        ..fresh()
    };
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('E'), t.as_slice());
    assert_eq!(effect, Effect::None);
    assert!(matches!(s.view, View::Detail), "`e`는 상세 보기를 유지한다");
}

#[test]
fn 폼_보기는_모든_파괴적_편집키를_텍스트로_받아들인다() {
    // 단일 `e` 흡수 테스트의 짝이다. Form 보기는 파괴적 편집 게이트 아래에
    // 있어야 한다. 이후 리팩터가 이 키들 중 하나라도 Form match arm 위로
    // 옮기면 사용자의 도구 입력이 조용히 사라지거나 입력 도중 파괴적
    // 오버레이가 열린다. 새 파괴적 키를 추가할 때 여기서 명시적으로
    // 판단하도록 매개변수화한다.
    let destructive_keys = [
        ('n', "n"),
        ('R', "R"),
        ('D', "D"),
        ('p', "p"),
        ('c', "c"),
        ('m', "m"),
        ('a', "a"),
        ('[', "["),
        (']', "]"),
    ];
    for (ch, expected) in destructive_keys {
        let mut s = State {
            view: View::Form {
                tool_id: "test.with_input",
                form: string_form("input", "", true),
            },
            ..fresh()
        };
        let t = fixture_tools();
        let effect = handle_key(&mut s, Key::Char(ch), t.as_slice());
        assert_eq!(
            effect,
            Effect::None,
            "Form 보기는 {ch:?}를 텍스트로 흡수해야 하며 Effect는 없어야 한다"
        );
        match &s.view {
            View::Form { form, .. } => assert_eq!(
                form.fields[0].draft,
                DraftInputValue::Text(expected.to_string()),
                "Form 입력은 {ch:?}를 그대로 담아야 하지만 {:?}를 받았다",
                form.fields[0].draft
            ),
            other => panic!("Form 보기를 기대했지만 {other:?}를 받았다"),
        }
    }
}

#[test]
fn 폼_보기_안의_e는_명령이_아니라_문자이다() {
    // 도구 입력 폼 안에서 입력하는 동안 `e`는 문자 키다. 어떤 보드
    // 명령도 이를 가로채면 안 된다.
    let mut s = State {
        view: View::Form {
            tool_id: "test.with_input",
            form: string_form("input", "", true),
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('e'), t.as_slice());
    if let View::Form { form, .. } = &s.view {
        assert_eq!(
            form.fields[0].draft,
            DraftInputValue::Text("e".into()),
            "Form 보기는 `e`를 텍스트 입력으로 흡수해야 한다"
        );
    } else {
        panic!("Form 보기를 기대했지만 {:?}를 받았다", s.view);
    }
}
