#[test]
fn tui_editing_i18n_keys_are_actually_referenced_in_view_source() {
    // Guards against the silent catalog-mismatch bug class: a typo in
    // any `t(locale, "tui.xxx")` makes the renderer fall back to `???`,
    // yet the catalog-parity tests keep passing because the key exists
    // in both En and Ko catalogs. Pins every key added in this PR to
    // the source file that must actually read it.
    const VIEW_SRC: &str = concat!(
        include_str!("../../view.rs"),
        include_str!("../../view/confirm.rs"),
        include_str!("../../view/running.rs"),
        include_str!("../../view/tool_picker.rs"),
        include_str!("../../view/pin_color_editor.rs")
    );
    const TUI_KEYS_IN_VIEW: &[&str] = &[
        // Resize-mode grid title (read by render_pegboard_grid).
        "tui.grid.resize_title",
        // Hint chips (read by render_header). Modeless: a single hint set.
        "tui.header.edit.hint.new",
        "tui.header.edit.hint.rename",
        "tui.header.edit.hint.delete",
        "tui.header.edit.hint.add",
        "tui.header.edit.hint.pin",
        "tui.header.edit.hint.color",
        "tui.header.edit.hint.move",
        // Phase-6 BoardEditor (read by render_board_editor).
        "tui.board.editor.title",
        "tui.board.editor.add_prompt",
        "tui.board.editor.rename_prompt",
        "tui.board.editor.empty_hint",
        // Phase-8 ConfirmDeleteBoard.
        "tui.board.delete.confirm.title",
        "tui.board.delete.confirm.prompt",
        "tui.board.delete.confirm.options",
        // Phase-9 ToolPicker.
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
        // Approval-barrier dialog (read by render_confirm_approval).
        "tui.approval.confirm.title",
        "tui.approval.confirm.prompt",
        "tui.approval.confirm.options",
        // Running live-output pane (read by running_body / right_pane_content).
        "tui.right_pane.running",
        "tui.running.status",
        "tui.running.cancelling",
        "tui.running.empty",
    ];
    for key in TUI_KEYS_IN_VIEW {
        assert!(
            VIEW_SRC.contains(&format!("\"{key}\"")),
            "i18n key {key:?} is registered in the catalog but never read in \
             the view module — likely a typo or an unused key"
        );
    }
}

#[test]
fn modeless_hint_chips_always_expose_board_management_keys() {
    // Modeless: with the edit toggle gone there is only one hint set.
    // The header always shows the board-management (n/R/D/a) and
    // per-pin chips next to the navigation chips. We check only i18n
    // key references here — no Korean/English literals.
    const SRC: &str = include_str!("../../view.rs");
    let render_header = SRC
        .split_once("\nfn render_header")
        .and_then(|(_, after)| after.split_once("\nfn "))
        .map_or(SRC, |(body, _)| body);
    // No longer branches on edit mode.
    assert!(
        !render_header.contains("state.edit_mode"),
        "render_header is modeless and must not branch on state.edit_mode"
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
            "render_header must reference the always-on hint key {key:?}"
        );
    }
}
