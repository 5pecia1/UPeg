#[test]
fn tui_편집_관련_국제화_키는_view_소스에서_실제로_참조된다() {
    // 조용한 카탈로그 불일치 계열 버그를 막는다. `t(locale, "tui.xxx")`
    // 어디든 오타가 있으면 렌더러는 `???`로 폴백하지만, 키가 En과 Ko
    // 양쪽 카탈로그에 존재하므로 카탈로그 동등성 테스트는 계속 통과한다.
    // 이 PR에서 추가한 모든 키가 실제로 읽혀야 하는 소스 파일에 고정한다.
    const VIEW_SRC: &str = concat!(
        include_str!("../../view.rs"),
        include_str!("../../view/confirm.rs"),
        include_str!("../../view/running.rs"),
        include_str!("../../view/tool_picker.rs"),
        include_str!("../../view/pin_color_editor.rs")
    );
    const TUI_KEYS_IN_VIEW: &[&str] = &[
        // 리사이즈 모드 그리드 타이틀(render_pegboard_grid가 읽음).
        "tui.grid.resize_title",
        // 힌트 칩(render_header가 읽음). Modeless: 단일 힌트 묶음.
        "tui.header.edit.hint.new",
        "tui.header.edit.hint.rename",
        "tui.header.edit.hint.delete",
        "tui.header.edit.hint.add",
        "tui.header.edit.hint.pin",
        "tui.header.edit.hint.color",
        "tui.header.edit.hint.move",
        // 6단계 BoardEditor(render_board_editor가 읽음).
        "tui.board.editor.title",
        "tui.board.editor.add_prompt",
        "tui.board.editor.rename_prompt",
        "tui.board.editor.empty_hint",
        // 8단계 ConfirmDeleteBoard.
        "tui.board.delete.confirm.title",
        "tui.board.delete.confirm.prompt",
        "tui.board.delete.confirm.options",
        // 9단계 ToolPicker.
        "tui.toolpicker.title",
        "tui.toolsearch.title",
        "tui.toolpicker.query",
        "tui.toolpicker.empty",
        "tui.toolpicker.pinned_marker",
        "tui.pin_color.editor.title",
        "tui.pin_color.editor.tool",
        "tui.pin_color.editor.current",
        "tui.pin_color.editor.palette",
        "tui.pin_color.editor.hint",
        // 승인 장벽 대화상자(render_confirm_approval이 읽음).
        "tui.approval.confirm.title",
        "tui.approval.confirm.prompt",
        "tui.approval.confirm.options",
        // 실행 중 라이브 출력 pane(running_body / right_pane_content가 읽음).
        "tui.right_pane.running",
        "tui.running.status",
        "tui.running.cancelling",
        "tui.running.empty",
    ];
    for key in TUI_KEYS_IN_VIEW {
        assert!(
            VIEW_SRC.contains(&format!("\"{key}\"")),
            "국제화 키 {key:?}는 카탈로그에 등록되어 있지만 view 모듈에서 읽히지 \
             않는다. 오타이거나 사용되지 않는 키일 가능성이 높다"
        );
    }
}

#[test]
fn 모드리스_힌트_칩은_보드_관리_키를_상시_노출한다() {
    // Modeless: 편집 토글이 사라져 힌트 묶음은 하나뿐이다. 헤더는
    // 언제나 보드 관리(n/R/D/a)와 per-pin 칩을 항해 칩과 함께 노출한다.
    // 여기에는 한국어/영어 리터럴을 두지 않고 국제화 키 참조만 확인한다.
    const SRC: &str = include_str!("../../view.rs");
    let render_header = SRC
        .split_once("\nfn render_header")
        .and_then(|(_, after)| after.split_once("\nfn "))
        .map_or(SRC, |(body, _)| body);
    // 더 이상 편집 모드로 분기하지 않는다.
    assert!(
        !render_header.contains("state.edit_mode"),
        "render_header는 modeless이므로 state.edit_mode로 분기하면 안 된다"
    );
    for key in [
        "tui.header.edit.hint.new",
        "tui.header.edit.hint.rename",
        "tui.header.edit.hint.delete",
        "tui.header.edit.hint.add",
        "tui.header.edit.hint.pin",
    ] {
        assert!(
            render_header.contains(key),
            "render_header는 상시 노출 힌트 키 {key:?}를 참조해야 한다"
        );
    }
}
