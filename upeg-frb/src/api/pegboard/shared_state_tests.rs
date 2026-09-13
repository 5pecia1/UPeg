#[test]
fn filtered_layout_snapshot은_all에서_모든_placements를_반환한다() {
    run_with_storage_backup(|| {
        let board_key = create_board("FRB All Filter Layout".to_string()).expect("create board");
        pin_tool(board_key.clone(), "num.hex_to_decimal".to_string())
            .expect("pin encoding fixture");
        pin_tool(board_key.clone(), "id.uuid_v7".to_string()).expect("pin id fixture");

        let all = load_layout_snapshot_for_filter(board_key.clone(), Some(ALL_TAG.to_string()));
        let unfiltered = load_layout_snapshot_for_filter(board_key, None);

        assert_eq!(all.placements.len(), 2);
        assert_eq!(unfiltered.placements.len(), 2);
        assert_eq!(
            all.placements
                .iter()
                .map(|placement| placement.tool_id.as_str())
                .collect::<Vec<_>>(),
            unfiltered
                .placements
                .iter()
                .map(|placement| placement.tool_id.as_str())
                .collect::<Vec<_>>()
        );
    });
}

#[test]
fn create_board는_shared_pegboard_state에_board와_빈_layout을_저장한다() {
    run_with_storage_backup(|| {
        let board_key =
            create_board("FRB Shared Empty Board".to_string()).expect("create board through FRB");

        let state = upeg_sources::pegboard::load_state();

        assert!(
            state.boards.iter().any(|board| board.key == board_key),
            "FRB-created board must be visible through shared pegboard state"
        );
        let layout = state
            .layouts
            .get(&board_key)
            .expect("FRB-created board must have a layout entry TUI can reload");
        assert!(
            layout.is_empty(),
            "new custom board layout must start empty"
        );
    });
}

#[test]
fn pin_color_저장은_shared_layout_storage에_반영된다() {
    run_with_storage_backup(|| {
        let _guard = toolbox_add_tool_managed(fixture_meta("color_storage", PegboardUnits::U1));
        let board_key = "dev";
        let tool_id = "frb_pegboard_test.color_storage";
        pin_tool(board_key.to_string(), tool_id.to_string()).expect("pin fixture");

        set_pin_color(
            board_key.to_string(),
            tool_id.to_string(),
            Some("#112233".to_string()),
        )
        .expect("pin color 저장 성공");

        let state = upeg_sources::pegboard::load_state();
        let stored = state
            .layouts
            .get(board_key)
            .and_then(|placements| placements.iter().find(|placement| placement.tool_id == tool_id))
            .and_then(|placement| placement.color.as_ref())
            .map(upeg_core::PinColorHex::as_str);
        assert_eq!(stored, Some("#112233"));

        let snapshot = load_layout_snapshot(board_key.to_string());
        let dto_color = snapshot
            .placements
            .iter()
            .find(|placement| placement.tool_id == tool_id)
            .and_then(|placement| placement.color.as_deref());
        assert_eq!(dto_color, Some("#112233"));
    });
}

#[test]
fn pegboard_selection_dto는_shared_selection을_저장하고_로드한다() {
    run_with_storage_backup(|| {
        let board_key = create_board("FRB Selection Board".to_string()).expect("create board");
        pin_tool(board_key.clone(), "num.hex_to_decimal".to_string()).expect("pin fixture");

        save_pegboard_selection(PegboardSelectionDto {
            board_key: Some(board_key.clone()),
            tag: "pure".to_string(),
        })
        .expect("selection 저장 성공");

        let loaded = load_pegboard_selection();

        assert_eq!(loaded.board_key, Some(board_key));
        assert_eq!(loaded.tag, "pure");
    });
}

#[test]
fn board_scoped_tag_options는_보드에_핀된_도구의_태그만_센다() {
    let boards = vec![
        Board {
            key: "dev",
            title: "Dev",
            guidance: upeg_core::BoardGuidance::default(),
        },
        Board {
            key: "ids",
            title: "IDs",
            guidance: upeg_core::BoardGuidance::default(),
        },
    ];
    let layouts: BoardLayouts = [
        ("dev", vec![Placement::new("convert.base64_encode", 0, 0)]),
        ("ids", vec![Placement::new("id.uuid_v7", 0, 0)]),
    ]
    .into_iter()
    .collect();

    let dev_tools = pinned_tools_for_board_in(&boards, &layouts, Some("dev"));
    let tags = tag_options_inner(&dev_tools);

    assert_eq!(tags.first().map(String::as_str), Some(ALL_TAG));
    assert!(tags.iter().any(|tag| tag == "convert"));
    assert!(
        !tags.iter().any(|tag| tag == "id"),
        "다른 보드에만 핀된 도구의 태그는 제외해야 한다"
    );
    assert_eq!(count_for_tag_inner(&dev_tools, ALL_TAG), 1);
    assert_eq!(count_for_tag_inner(&dev_tools, "convert"), 1);
    assert_eq!(count_for_tag_inner(&dev_tools, "id"), 0);
}
