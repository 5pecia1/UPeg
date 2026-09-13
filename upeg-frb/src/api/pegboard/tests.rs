//! Unit + fixture tests for the pegboard FRB surface (`super`).
//!
//! Split out of `pegboard.rs` per the workspace test-layout convention
//! (docs/rules/engineering-rules.md §10): the inline `#[cfg(test)]` block had grown
//! past the ~200-LoC budget for colocated tests. Shared fixtures live here;
//! `shared_state_tests.rs` is `include!`d so it reuses them.

use super::*;
use crate::api::test_support::run_with_storage_backup;
use upeg_core::{ALL_SURFACES, InputSpec, Invoker, OutputSpec, PegboardUnits, PinKind, Source};
use upeg_pegboard_ui::features::boards::Board;
use upeg_runtime::toolbox_add_tool_managed;

const EXPECTED_FIXED_BOARD_COLS: u32 = 6;

/// Build a `ToolMeta` for fixture-only PegboardUnits coverage. Each
/// helper creates a fresh `local_id` so collisions never bubble up
/// across tests.
fn fixture_meta(local: &'static str, units: PegboardUnits) -> upeg_core::ToolMeta {
    upeg_core::ToolMeta {
        id: Box::leak(format!("frb_pegboard_test.{local}").into_boxed_str()),
        toolkit: "frb_pegboard_test",
        local_id: local,
        tags: &[],
        display_label: "frb pegboard test",
        description: "",
        input_spec: InputSpec::empty(),
        output_spec: OutputSpec::empty(),
        primary_output_id: None,
        source: Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: units,
        invoker: Invoker::Function,
        surfaces: ALL_SURFACES,
        boards: &[],
    }
}

#[test]
fn placement_dto_가_u1_도구를_1x1로_매핑한다() {
    let _guard = toolbox_add_tool_managed(fixture_meta("u1_tool", PegboardUnits::U1));
    let placement = Placement::new("frb_pegboard_test.u1_tool", 3, 2);
    let dto = placement_dto_from(&placement);
    assert_eq!(dto.w, 1);
    assert_eq!(dto.h, 1);
    // x/y mirror the placement coordinates.
    assert_eq!(dto.x, 3);
    assert_eq!(dto.y, 2);
}

#[test]
fn layout_snapshot은_board_cols를_항상_6으로_노출한다() {
    run_with_storage_backup(|| {
        let _guard = toolbox_add_tool_managed(fixture_meta("legacy_beyond_six", PegboardUnits::U2));
        let tool_id = "frb_pegboard_test.legacy_beyond_six";
        let board_key = create_board("FRB Legacy Six Columns".to_string())
            .expect("create isolated legacy board");
        let legacy_anchor_x = u32::from(upeg_core::BOARD_COLS) + 1;
        let layouts_json = format!(
            r#"{{"{board_key}":[{{"tool_id":"{tool_id}","x":{legacy_anchor_x},"y":0}}]}}"#,
        );
        upeg_sources::pegboard::save_layouts_json(&layouts_json).expect("legacy layout 저장 성공");

        let snapshot = load_layout_snapshot_for_filter(board_key, None);

        assert_eq!(u32::from(upeg_core::BOARD_COLS), EXPECTED_FIXED_BOARD_COLS);
        assert_eq!(snapshot.board_cols, EXPECTED_FIXED_BOARD_COLS);
        let moved = snapshot
            .placements
            .iter()
            .find(|placement| placement.tool_id == tool_id)
            .expect("snapshot must contain reconciled legacy pin");
        assert!(moved.x + moved.w <= EXPECTED_FIXED_BOARD_COLS);
    });
}

#[test]
fn placement_dto_가_u2_도구를_2x1로_매핑한다() {
    let _guard = toolbox_add_tool_managed(fixture_meta("u2_tool", PegboardUnits::U2));
    let placement = Placement::new("frb_pegboard_test.u2_tool", 0, 0);
    let dto = placement_dto_from(&placement);
    assert_eq!(dto.w, 2);
    assert_eq!(dto.h, 1);
}

#[test]
fn placement_dto_가_u2t_도구를_1x2로_매핑한다() {
    let _guard = toolbox_add_tool_managed(fixture_meta("u2t_tool", PegboardUnits::U2T));
    let placement = Placement::new("frb_pegboard_test.u2t_tool", 1, 1);
    let dto = placement_dto_from(&placement);
    assert_eq!(dto.w, 1);
    assert_eq!(dto.h, 2);
}

#[test]
fn placement_dto_가_미등록_도구를_1x1로_대체한다() {
    // Unknown tool ids fall back to (1, 1) per
    // `upeg_runtime::pegboard::placement_size`'s contract.
    let placement = Placement::new("not.registered", 0, 0);
    let dto = placement_dto_from(&placement);
    assert_eq!(dto.w, 1);
    assert_eq!(dto.h, 1);
}

#[test]
fn move_pin_가_u16_범위를_벗어난_anchor를_거부한다() {
    // anchor_x exceeds u16::MAX → Validation error before any layout
    // mutation happens.
    let err = move_pin("dev".to_string(), "any.tool".to_string(), 70_000, 0)
        .expect_err("anchor_x past u16::MAX must validate-fail");
    match err {
        FrbError::Validation { field, .. } => assert_eq!(field, "anchor_x"),
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn move_pin은_6열_밖_anchor를_오른쪽_확장이_아니라_보정한다() {
    run_with_storage_backup(|| {
        let _guard = toolbox_add_tool_managed(fixture_meta("clamped_wide_pin", PegboardUnits::U2));
        let tool_id = "frb_pegboard_test.clamped_wide_pin";
        let board_key =
            create_board("FRB Clamped Width".to_string()).expect("create isolated clamp board");
        pin_tool(board_key.clone(), tool_id.to_string()).expect("pin clamped fixture");

        let anchor_x = u32::from(upeg_core::BOARD_COLS) + 1;
        move_pin(board_key.clone(), tool_id.to_string(), anchor_x, 0)
            .expect("x beyond BOARD_COLS must be clamped inside the fixed board");

        let snapshot = load_layout_snapshot(board_key);
        let moved = snapshot
            .placements
            .iter()
            .find(|placement| placement.tool_id == tool_id)
            .expect("snapshot must contain moved pin");
        assert_eq!(snapshot.board_cols, EXPECTED_FIXED_BOARD_COLS);
        assert_ne!(moved.x, anchor_x);
        assert_eq!(moved.x, EXPECTED_FIXED_BOARD_COLS - moved.w);
        assert!(moved.x + moved.w <= EXPECTED_FIXED_BOARD_COLS);
    });
}

#[test]
fn reorder_pin은_이웃_pin만_원자적으로_교환한다() {
    run_with_storage_backup(|| {
        let _guard_a = toolbox_add_tool_managed(fixture_meta("reorder_a", PegboardUnits::U1));
        let _guard_b = toolbox_add_tool_managed(fixture_meta("reorder_b", PegboardUnits::U1));
        let _guard_c = toolbox_add_tool_managed(fixture_meta("reorder_c", PegboardUnits::U1));
        let tool_a = "frb_pegboard_test.reorder_a";
        let tool_b = "frb_pegboard_test.reorder_b";
        let tool_c = "frb_pegboard_test.reorder_c";
        let board_key =
            create_board("FRB Atomic Reorder".to_string()).expect("create reorder board");

        pin_tool(board_key.clone(), tool_a.to_string()).expect("pin tool a");
        pin_tool(board_key.clone(), tool_b.to_string()).expect("pin tool b");
        pin_tool(board_key.clone(), tool_c.to_string()).expect("pin tool c");

        reorder_pin(
            board_key.clone(),
            tool_a.to_string(),
            OrderDirectionDto::Next,
        )
        .expect("reorder next");

        let snapshot = load_layout_snapshot(board_key);
        let at = |tool_id: &str| {
            snapshot
                .placements
                .iter()
                .find(|placement| placement.tool_id == tool_id)
                .map(|placement| (placement.x, placement.y))
        };
        assert_eq!(at(tool_a), Some((1, 0)));
        assert_eq!(at(tool_b), Some((0, 0)));
        assert_eq!(at(tool_c), Some((2, 0)));
    });
}

#[test]
fn 잘못된_pin_이동은_기존처럼_검증_오류를_반환한다() {
    let oversized_anchor_x = u32::from(u16::MAX) + 1;
    let err = move_pin(
        "dev".to_string(),
        "frb_pegboard_test.invalid_move".to_string(),
        oversized_anchor_x,
        0,
    )
    .expect_err("anchor_x past u16::MAX must validate-fail");
    match err {
        FrbError::Validation { field, .. } => assert_eq!(field, "anchor_x"),
        other => panic!("unexpected error variant: {other:?}"),
    }
}

fn boards_fixture() -> Vec<Board> {
    vec![
        Board {
            key: "dev",
            title: "Dev",
            guidance: upeg_core::BoardGuidance::default(),
        },
        Board {
            key: "media",
            title: "Media",
            guidance: upeg_core::BoardGuidance::default(),
        },
        Board {
            key: "misc",
            title: "Misc",
            guidance: upeg_core::BoardGuidance::default(),
        },
    ]
}

fn layouts_with(map: &[(&'static str, &[&str])]) -> BoardLayouts {
    let mut out = BoardLayouts::new();
    for (key, tools) in map {
        let placements: Vec<Placement> = tools
            .iter()
            .enumerate()
            .map(|(i, t)| Placement::new(*t, u16::try_from(i).unwrap_or(0), 0))
            .collect();
        out.insert(*key, placements);
    }
    out
}

fn placement_signature(placements: &[PlacementDto]) -> Vec<(String, u32, u32, u32, u32)> {
    let mut out: Vec<_> = placements
        .iter()
        .map(|placement| {
            (
                placement.tool_id.clone(),
                placement.x,
                placement.y,
                placement.w,
                placement.h,
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn pinned_boards_for_tool은_도구가_고정된_보드_키를_반환한다() {
    let boards = boards_fixture();
    let layouts = layouts_with(&[
        ("dev", &["num.hex_to_decimal"]),
        ("media", &["id.uuid_v7"]),
        ("misc", &["num.hex_to_decimal", "id.uuid_v7"]),
    ]);
    let pinned: Vec<&'static str> = boards
        .iter()
        .filter(|board| {
            layouts
                .get(board.key)
                .is_some_and(|v| v.iter().any(|p| p.tool_id == "num.hex_to_decimal"))
        })
        .map(|b| b.key)
        .collect();
    assert_eq!(pinned, vec!["dev", "misc"]);
}

#[test]
fn pinned_boards_for_tool은_미고정시_빈_벡터를_반환한다() {
    let boards = boards_fixture();
    let layouts = layouts_with(&[("dev", &["num.hex_to_decimal"]), ("media", &["id.uuid_v7"])]);
    let pinned: Vec<&'static str> = boards
        .iter()
        .filter(|board| {
            layouts
                .get(board.key)
                .is_some_and(|v| v.iter().any(|p| p.tool_id == "memo.scratch"))
        })
        .map(|b| b.key)
        .collect();
    assert!(pinned.is_empty(), "memo.scratch is not pinned anywhere");
}

#[test]
fn count_for_tag_dto는_all_태그에서_전체_count를_반환한다() {
    let n = count_for_tag(ALL_TAG.to_string());
    // Real toolbox has > 0 tools.
    assert!(n > 0, "ALL_TAG count must include every registered tool");
}

#[test]
fn count_for_tag_dto는_미등록_태그에서_0을_반환한다() {
    let n = count_for_tag("__no_such_tag_zzz__".to_string());
    assert_eq!(n, 0);
}

#[test]
fn preview_push는_anchor에_겹치는_pin이_있으면_밀린_placements를_반환한다() {
    run_with_storage_backup(|| {
        // F16 — push-placement preview surfaces the layout that WOULD
        // result if the user dropped `tool_id` at `(anchor_x, anchor_y)`,
        // without mutating the persisted board. The Dart canvas uses
        // this to paint ghosted overlays during the move-mode preview.
        //
        // Register a fixture tool and pre-pin it AND a second fixture
        // at row 0 so they overlap when the dragged tool lands at (0,0).
        let unique_pin_a: &'static str =
            Box::leak("frb_t5_push_preview.tool_a".to_string().into_boxed_str());
        let unique_pin_b: &'static str =
            Box::leak("frb_t5_push_preview.tool_b".to_string().into_boxed_str());
        let meta_a = upeg_core::ToolMeta {
            id: unique_pin_a,
            toolkit: "frb_t5_push_preview",
            local_id: "tool_a",
            tags: &[],
            display_label: "tool a",
            description: "",
            input_spec: InputSpec::empty(),
            output_spec: OutputSpec::empty(),
            primary_output_id: None,
            source: Source::UserInput,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: ALL_SURFACES,
            boards: &[],
        };
        let meta_b = upeg_core::ToolMeta {
            id: unique_pin_b,
            toolkit: "frb_t5_push_preview",
            local_id: "tool_b",
            tags: &[],
            display_label: "tool b",
            description: "",
            input_spec: InputSpec::empty(),
            output_spec: OutputSpec::empty(),
            primary_output_id: None,
            source: Source::UserInput,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: ALL_SURFACES,
            boards: &[],
        };
        let _ga = toolbox_add_tool_managed(meta_a);
        let _gb = toolbox_add_tool_managed(meta_b);
        let board_key =
            create_board("FRB T5 Push Preview".to_string()).expect("create isolated preview board");

        // Seed both tools on the isolated board at (0,0) and
        // (1,0). When tool_b is previewed at (0,0) it should
        // displace tool_a — the returned preview must include
        // BOTH ids and tool_a's new position must differ from
        // (0,0).
        pin_tool(board_key.clone(), unique_pin_a.to_string()).expect("pin tool a");
        pin_tool(board_key.clone(), unique_pin_b.to_string()).expect("pin tool b");
        move_pin(board_key.clone(), unique_pin_a.to_string(), 0, 0).expect("move tool a");
        move_pin(board_key.clone(), unique_pin_b.to_string(), 1, 0).expect("move tool b");

        let preview = preview_push(board_key, unique_pin_b.to_string(), 0, 0);
        assert!(
            !preview.is_empty(),
            "preview_push with a colliding anchor must return placements"
        );
        let dragged = preview
            .iter()
            .find(|p| p.tool_id == unique_pin_b)
            .expect("preview must include the dragged tool");
        assert_eq!((dragged.x, dragged.y), (0, 0));
        let displaced = preview
            .iter()
            .find(|p| p.tool_id == unique_pin_a)
            .expect("preview must include the displaced tool");
        assert!(
            (displaced.x, displaced.y) != (0, 0),
            "displaced tool must move out of the dragged anchor cell"
        );
    });
}

#[test]
fn preview_push는_미등록_도구에_대해_빈_벡터를_반환한다() {
    // F16 — unknown tool ids return an empty preview so the Dart
    // canvas can hide the overlay without a separate "should show"
    // signal. Mirrors the same `placement_size == 0` guard
    // `move_pin` uses.
    let preview = preview_push("dev".to_string(), "no.such.tool.frb_t5".to_string(), 0, 0);
    assert!(preview.is_empty(), "unknown tool must yield empty preview");
}

#[test]
fn preview_push는_commit과_같은_6열_reflow를_반환한다() {
    run_with_storage_backup(|| {
        let _dragged_guard =
            toolbox_add_tool_managed(fixture_meta("preview_commit_dragged", PegboardUnits::U2));
        let _blocker_guard =
            toolbox_add_tool_managed(fixture_meta("preview_commit_blocker", PegboardUnits::U2));
        let dragged_id = "frb_pegboard_test.preview_commit_dragged";
        let blocker_id = "frb_pegboard_test.preview_commit_blocker";
        let board_key =
            create_board("FRB Preview Commit".to_string()).expect("create isolated preview board");
        pin_tool(board_key.clone(), dragged_id.to_string()).expect("pin dragged fixture");
        pin_tool(board_key.clone(), blocker_id.to_string()).expect("pin blocker fixture");
        move_pin(
            board_key.clone(),
            blocker_id.to_string(),
            EXPECTED_FIXED_BOARD_COLS - 2,
            0,
        )
        .expect("place blocker at the last U2 start column");

        let anchor_x = u32::from(upeg_core::BOARD_COLS) + 1;
        let preview = preview_push(board_key.clone(), dragged_id.to_string(), anchor_x, 0);
        assert!(
            !preview.is_empty(),
            "preview beyond BOARD_COLS must return fixed-width projected placements"
        );
        let moved = preview
            .iter()
            .find(|placement| placement.tool_id == dragged_id)
            .expect("preview must contain moved pin");
        assert_eq!(moved.x, EXPECTED_FIXED_BOARD_COLS - moved.w);
        assert!(
            preview
                .iter()
                .all(|placement| placement.x + placement.w <= EXPECTED_FIXED_BOARD_COLS),
            "preview must not horizontally expand beyond six columns"
        );

        move_pin(board_key.clone(), dragged_id.to_string(), anchor_x, 0)
            .expect("commit beyond BOARD_COLS must use the same fixed-width reflow");
        let committed = load_layout_snapshot(board_key);

        assert_eq!(
            placement_signature(&preview),
            placement_signature(&committed.placements)
        );
    });
}

#[test]
fn count_for_tag_dto는_특정_태그를_가진_도구만_센다() {
    // Register a fixture tool with a unique tag; assert the count
    // rises by exactly one.
    let unique_tag: &'static str = Box::leak("frb_q3_test_tag".to_string().into_boxed_str());
    let meta = upeg_core::ToolMeta {
        id: "frb_q3_test.tagged_one",
        toolkit: "frb_q3_test",
        local_id: "tagged_one",
        tags: Box::leak(Box::new([unique_tag])),
        display_label: "frb q3 test",
        description: "",
        input_spec: InputSpec::empty(),
        output_spec: OutputSpec::empty(),
        primary_output_id: None,
        source: Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: ALL_SURFACES,
        boards: &[],
    };
    let _guard = toolbox_add_tool_managed(meta);
    let n = count_for_tag(unique_tag.to_string());
    assert_eq!(n, 1);
}

#[test]
fn tools_for_tag는_runtime_tag_labels를_기준으로_필터한다() {
    let tool_id: &'static str = Box::leak("frb_effective_tag.fixture".to_string().into_boxed_str());
    let meta = upeg_core::ToolMeta {
        id: tool_id,
        toolkit: "frb_effective_tag",
        local_id: "fixture",
        tags: &[],
        display_label: "effective tag fixture",
        description: "",
        input_spec: InputSpec::empty(),
        output_spec: OutputSpec::empty(),
        primary_output_id: None,
        source: Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: ALL_SURFACES,
        boards: &[],
    };
    let _guard = toolbox_add_tool_managed(meta);

    let hits = tools_for_tag("pure".to_string());
    let hit = hits
        .iter()
        .find(|tool| tool.id == tool_id)
        .expect("runtime invoker tag must match tools_for_tag");

    assert!(
        hit.tags.iter().any(|tag| tag == "pure"),
        "ToolDto.tags must expose effective runtime tags, not only raw manifest tags"
    );
}

#[test]
fn filtered_layout_snapshot은_runtime_tag로_placements를_좁힌다() {
    run_with_storage_backup(|| {
        let tool_id: &'static str =
            Box::leak("frb_filtered_layout.fixture".to_string().into_boxed_str());
        let meta = upeg_core::ToolMeta {
            id: tool_id,
            toolkit: "frb_filtered_layout",
            local_id: "fixture",
            tags: &[],
            display_label: "filtered layout fixture",
            description: "",
            input_spec: InputSpec::empty(),
            output_spec: OutputSpec::empty(),
            primary_output_id: None,
            source: Source::UserInput,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: ALL_SURFACES,
            boards: &[],
        };
        let _guard = toolbox_add_tool_managed(meta);
        let board_key = create_board("FRB Filtered Layout".to_string()).expect("create board");
        pin_tool(board_key.clone(), tool_id.to_string()).expect("pin fixture");

        let matched = load_layout_snapshot_for_filter(board_key.clone(), Some("pure".to_string()));
        assert!(
            matched
                .placements
                .iter()
                .any(|placement| placement.tool_id == tool_id),
            "runtime pure tag should include the pinned fixture"
        );

        let missing =
            load_layout_snapshot_for_filter(board_key, Some("__missing_tag__".to_string()));
        assert!(
            missing.placements.is_empty(),
            "unknown tag should filter the layout to zero placements"
        );
    });
}

include!("shared_state_tests.rs");

// ─── span / args-preset overrides ──────────────────────────────────────

#[test]
fn placement_dto는_span_override를_effective_size와_필드로_노출한다() {
    let _guard = toolbox_add_tool_managed(fixture_meta("span_dto_tool", PegboardUnits::U1));
    let placement = Placement::new("frb_pegboard_test.span_dto_tool", 0, 0)
        .with_span(Some(PinSpan::new(
            ColSpan::new(3).expect("테스트 cols"),
            RowSpan::new(2).expect("테스트 rows"),
        )))
        .with_args_preset(Some(
            ArgsPreset::parse(r#"{"city":"Seoul"}"#).expect("테스트 preset"),
        ));
    let dto = placement_dto_from(&placement);
    assert_eq!((dto.w, dto.h), (3, 2), "w/h는 effective span이어야 한다");
    assert_eq!(dto.span_cols, Some(3));
    assert_eq!(dto.span_rows, Some(2));
    assert_eq!(dto.args_preset_json.as_deref(), Some(r#"{"city":"Seoul"}"#));

    let plain = placement_dto_from(&Placement::new("frb_pegboard_test.span_dto_tool", 0, 0));
    assert_eq!((plain.w, plain.h), (1, 1));
    assert_eq!(plain.span_cols, None);
    assert_eq!(plain.span_rows, None);
    assert_eq!(plain.args_preset_json, None);
}

#[test]
fn set_pin_span은_저장되고_clear로_manifest_크기로_돌아간다() {
    run_with_storage_backup(|| {
        let _guard = toolbox_add_tool_managed(fixture_meta("span_rt_tool", PegboardUnits::U1));
        let tool_id = "frb_pegboard_test.span_rt_tool";
        let board_key = create_board("FRB Span RT".to_string()).expect("보드 생성");
        pin_tool(board_key.clone(), tool_id.to_string()).expect("핀 고정");

        set_pin_span(board_key.clone(), tool_id.to_string(), 2, 3).expect("span 설정");
        let snapshot = load_layout_snapshot(board_key.clone());
        let dto = snapshot
            .placements
            .iter()
            .find(|p| p.tool_id == tool_id)
            .expect("핀 존재");
        assert_eq!((dto.w, dto.h), (2, 3));
        assert_eq!((dto.span_cols, dto.span_rows), (Some(2), Some(3)));

        clear_pin_span(board_key.clone(), tool_id.to_string()).expect("span 해제");
        let snapshot = load_layout_snapshot(board_key);
        let dto = snapshot
            .placements
            .iter()
            .find(|p| p.tool_id == tool_id)
            .expect("핀 존재");
        assert_eq!((dto.w, dto.h), (1, 1));
        assert_eq!((dto.span_cols, dto.span_rows), (None, None));
    });
}

#[test]
fn set_pin_span은_범위_밖_값을_validation_오류로_거부한다() {
    let too_wide = u32::from(upeg_core::BOARD_COLS) + 1;
    for (cols, rows, field) in [(0, 1, "cols"), (too_wide, 1, "cols"), (1, 0, "rows")] {
        match set_pin_span("dev".to_string(), "any.tool".to_string(), cols, rows) {
            Err(FrbError::Validation { field: got, .. }) => {
                assert_eq!(got, field, "cols={cols} rows={rows}");
            }
            other => panic!("expected Validation for cols={cols} rows={rows}, got {other:?}"),
        }
    }
}

#[test]
fn set_pin_args_preset은_저장되고_비객체는_거부한다() {
    run_with_storage_backup(|| {
        let _guard = toolbox_add_tool_managed(fixture_meta("preset_rt_tool", PegboardUnits::U1));
        let tool_id = "frb_pegboard_test.preset_rt_tool";
        let board_key = create_board("FRB Preset RT".to_string()).expect("보드 생성");
        pin_tool(board_key.clone(), tool_id.to_string()).expect("핀 고정");

        set_pin_args_preset(
            board_key.clone(),
            tool_id.to_string(),
            r#" { "days" : 3 } "#.to_string(),
        )
        .expect("preset 설정");
        let dto_preset = |board_key: String| {
            load_layout_snapshot(board_key)
                .placements
                .iter()
                .find(|p| p.tool_id == tool_id)
                .and_then(|p| p.args_preset_json.clone())
        };
        assert_eq!(
            dto_preset(board_key.clone()).as_deref(),
            Some(r#"{"days":3}"#),
            "preset은 canonical JSON으로 왕복해야 한다"
        );

        match set_pin_args_preset(board_key.clone(), tool_id.to_string(), "[]".to_string()) {
            Err(FrbError::Validation { field, .. }) => assert_eq!(field, "args_preset"),
            other => panic!("expected Validation for non-object preset, got {other:?}"),
        }

        clear_pin_args_preset(board_key.clone(), tool_id.to_string()).expect("preset 해제");
        assert_eq!(dto_preset(board_key), None);
    });
}

#[test]
fn set_pin_span은_커지면서_겹치는_이웃을_밀어낸다() {
    run_with_storage_backup(|| {
        let _guard_a = toolbox_add_tool_managed(fixture_meta("span_push_a", PegboardUnits::U1));
        let _guard_b = toolbox_add_tool_managed(fixture_meta("span_push_b", PegboardUnits::U1));
        let grown_id = "frb_pegboard_test.span_push_a";
        let neighbour_id = "frb_pegboard_test.span_push_b";
        let board_key = create_board("FRB Span Push".to_string()).expect("보드 생성");
        pin_tool(board_key.clone(), grown_id.to_string()).expect("핀 고정 a");
        pin_tool(board_key.clone(), neighbour_id.to_string()).expect("핀 고정 b");
        // a=(0,0), b=(1,0). a를 2칸 폭으로 키우면 b의 자리를 침범한다.
        set_pin_span(board_key.clone(), grown_id.to_string(), 2, 1).expect("span 설정");

        let snapshot = load_layout_snapshot(board_key);
        let find = |id: &str| {
            snapshot
                .placements
                .iter()
                .find(|p| p.tool_id == id)
                .cloned()
                .expect("핀 존재")
        };
        let grown = find(grown_id);
        let neighbour = find(neighbour_id);
        assert_eq!((grown.x, grown.y, grown.w), (0, 0, 2), "앵커는 유지된다");
        assert!(
            neighbour.x >= 2 || neighbour.y > 0,
            "이웃은 겹치지 않는 칸으로 밀려야 한다: ({}, {})",
            neighbour.x,
            neighbour.y
        );
    });
}

#[test]
fn preview_resize는_commit과_같은_push_결과를_돌려주고_상태를_바꾸지_않는다() {
    run_with_storage_backup(|| {
        let _guard_a = toolbox_add_tool_managed(fixture_meta("resize_prev_a", PegboardUnits::U1));
        let _guard_b = toolbox_add_tool_managed(fixture_meta("resize_prev_b", PegboardUnits::U1));
        let grown_id = "frb_pegboard_test.resize_prev_a";
        let neighbour_id = "frb_pegboard_test.resize_prev_b";
        let board_key = create_board("FRB Resize Preview".to_string()).expect("보드 생성");
        pin_tool(board_key.clone(), grown_id.to_string()).expect("핀 고정 a");
        pin_tool(board_key.clone(), neighbour_id.to_string()).expect("핀 고정 b");

        let before = load_layout_snapshot(board_key.clone());
        let preview = preview_resize(board_key.clone(), grown_id.to_string(), 2, 1);
        assert!(!preview.is_empty(), "핀이 있으면 미리보기가 있어야 한다");

        // 미리보기는 상태를 바꾸지 않는다.
        let after = load_layout_snapshot(board_key.clone());
        let coords = |snap: &LayoutSnapshotDto| {
            snap.placements
                .iter()
                .map(|p| (p.tool_id.clone(), p.x, p.y, p.w, p.h))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            coords(&before),
            coords(&after),
            "preview는 read-only여야 한다"
        );

        // 미리보기와 commit은 같은 push 결과를 낸다.
        set_pin_span(board_key.clone(), grown_id.to_string(), 2, 1).expect("commit");
        let committed = load_layout_snapshot(board_key);
        let committed_neighbour = committed
            .placements
            .iter()
            .find(|p| p.tool_id == neighbour_id)
            .expect("이웃 존재");
        let previewed_neighbour = preview
            .iter()
            .find(|p| p.tool_id == neighbour_id)
            .expect("미리보기에 이웃 존재");
        assert_eq!(
            (previewed_neighbour.x, previewed_neighbour.y),
            (committed_neighbour.x, committed_neighbour.y),
            "preview와 commit의 밀어내기 결과가 같아야 한다"
        );
    });
}

#[test]
fn preview_resize는_범위_밖_span과_미고정_도구에_빈_벡터를_돌려준다() {
    run_with_storage_backup(|| {
        let _guard = toolbox_add_tool_managed(fixture_meta("resize_prev_bad", PegboardUnits::U1));
        let tool_id = "frb_pegboard_test.resize_prev_bad";
        let board_key = create_board("FRB Resize Bad".to_string()).expect("보드 생성");

        // 핀이 없으면 빈 벡터.
        assert!(preview_resize(board_key.clone(), tool_id.to_string(), 2, 1).is_empty());

        pin_tool(board_key.clone(), tool_id.to_string()).expect("핀 고정");
        let too_wide = u32::from(upeg_core::BOARD_COLS) + 1;
        assert!(preview_resize(board_key.clone(), tool_id.to_string(), 0, 1).is_empty());
        assert!(preview_resize(board_key.clone(), tool_id.to_string(), too_wide, 1).is_empty());
        assert!(preview_resize(board_key, tool_id.to_string(), 1, 0).is_empty());
    });
}

#[test]
fn move_pin은_span_override_폭으로_클램프한다() {
    run_with_storage_backup(|| {
        let _guard = toolbox_add_tool_managed(fixture_meta("span_clamp_tool", PegboardUnits::U1));
        let tool_id = "frb_pegboard_test.span_clamp_tool";
        let board_key = create_board("FRB Span Clamp".to_string()).expect("보드 생성");
        pin_tool(board_key.clone(), tool_id.to_string()).expect("핀 고정");
        set_pin_span(board_key.clone(), tool_id.to_string(), 3, 1).expect("span 설정");

        // manifest 폭(1)이라면 x=5까지 허용되지만, override 폭 3이면
        // 최대 시작 열은 6-3=3으로 보정되어야 한다.
        move_pin(board_key.clone(), tool_id.to_string(), 5, 0).expect("이동");
        let snapshot = load_layout_snapshot(board_key);
        let dto = snapshot
            .placements
            .iter()
            .find(|p| p.tool_id == tool_id)
            .expect("핀 존재");
        assert_eq!(dto.x, 3, "override 폭 기준으로 클램프해야 한다");
        assert!(dto.x + dto.w <= EXPECTED_FIXED_BOARD_COLS);
    });
}

/// The Project Manifest owns the boards it declares: it re-merges them
/// on the very next load, so a delete cannot succeed — it can only
/// sweep the board's stored row and tombstone every pin on it.
///
/// TUI and desktop/PWA are different code paths onto the same store
/// (`upeg_sources::pegboard::remove_board` vs. this module's
/// [`delete_board`]), so the refusal is asserted on BOTH here: the FRB
/// side used to call the unguarded `upeg_pegboard_ui` primitive and
/// destroyed exactly those pins.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn 프로젝트_보드_삭제는_tui와_frb_양쪽에서_거부된다() {
    use super::project_scope_test_support::ScopedProjectBoard;

    const PROJECT_BOARD_ID: &str = "frb-parity-proj";

    run_with_storage_backup(|| {
        let _project = ScopedProjectBoard::declare(PROJECT_BOARD_ID, "FRB Parity Project");

        // FRB (desktop/PWA) 경로.
        match delete_board(PROJECT_BOARD_ID.to_string()) {
            Err(crate::api::boot::FrbError::Validation { field, .. }) => {
                assert_eq!(field, "board_key");
            }
            other => panic!("프로젝트 보드 삭제는 거부되어야 한다, got {other:?}"),
        }

        // TUI 경로 — 같은 store 를 만지는 다른 primitive.
        let mut state = upeg_sources::pegboard::load_state();
        assert!(
            !upeg_sources::pegboard::remove_board(&mut state, PROJECT_BOARD_ID),
            "TUI 경로도 프로젝트 보드는 지우지 못한다"
        );

        // 거부는 store 를 건드리지 않아야 한다: 보드가 그대로 남는다.
        let boards = upeg_sources::pegboard::board_keys_in(&upeg_sources::pegboard::load_state());
        assert!(
            boards.iter().any(|key| key == PROJECT_BOARD_ID),
            "거부된 삭제가 보드를 없앴다: {boards:?}"
        );
    });
}

/// A global board is still deletable — the guard must be about project
/// declarations, not about board deletion in general.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn 전역_보드는_여전히_삭제된다() {
    run_with_storage_backup(|| {
        let board_key = create_board("FRB Deletable Board".to_string()).expect("보드 생성");
        delete_board(board_key.clone()).expect("전역 보드는 지워진다");
        let boards = upeg_sources::pegboard::board_keys_in(&upeg_sources::pegboard::load_state());
        assert!(
            !boards.contains(&board_key),
            "삭제한 전역 보드가 남아 있다: {boards:?}"
        );
    });
}
