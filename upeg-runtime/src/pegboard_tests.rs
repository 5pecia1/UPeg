use super::*;

#[test]
fn duplicate_tool_pins_keep_distinct_identity_through_reflow_and_removal() {
    register_test_tools();
    let second_id = PinId::parse("01995a93-5165-7ee0-8bd6-d0cfba6f2727").unwrap();
    let mut layout = vec![Placement::new(U1_TOOL, 0, 0)];
    assert_eq!(
        add_placement(&mut layout, U1_TOOL, second_id.clone()),
        PlacementEdit::Changed
    );
    assert_eq!(layout.len(), 2);
    assert_eq!(
        place_tool_with_push(&mut layout, second_id.as_str(), 0, 0),
        PlacementEdit::Changed
    );
    assert_eq!(layout.len(), 2);
    assert_eq!(layout.iter().filter(|p| p.tool_id == U1_TOOL).count(), 2);
    assert_eq!(
        remove_placement(&mut layout, second_id.as_str()),
        PlacementEdit::Changed
    );
    assert_eq!(layout.len(), 1);
    assert_eq!(layout[0].pin_id.as_str(), U1_TOOL);
}
use crate::toolbox_add_tool;
use upeg_core::{
    ALL_SURFACES, ColSpan, InputSpec, Invoker, PegboardUnits, PinKind, RowSpan, ToolId, ToolMeta,
};

const U1_TOOL: &str = "pegboard_test.u1";
const U2_TOOL: &str = "pegboard_test.u2";
const U1_B: &str = "pegboard_test.b";
const U1_C: &str = "pegboard_test.c";
const U1_D: &str = "pegboard_test.d";
const U1_E: &str = "pegboard_test.e";
const U1_F: &str = "pegboard_test.f";
const U1_G: &str = "pegboard_test.g";

fn local_id_for(id: &'static str) -> &'static str {
    ToolId::parse_canonical_in_toolkit(id, "pegboard_test")
        .expect("test ToolMeta id must be in canonical form")
        .local()
}

fn meta(id: &'static str, pegboard_units: PegboardUnits) -> ToolMeta {
    ToolMeta {
        id,
        toolkit: "pegboard_test",
        local_id: local_id_for(id),
        tags: &[],
        display_label: "Pegboard test tool",
        description: "",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units,
        invoker: Invoker::Function,
        surfaces: ALL_SURFACES,
        boards: &[],
    }
}

fn register_test_tools() {
    toolbox_add_tool(meta(U1_TOOL, PegboardUnits::U1));
    toolbox_add_tool(meta(U2_TOOL, PegboardUnits::U2));
    toolbox_add_tool(meta(U1_B, PegboardUnits::U1));
    toolbox_add_tool(meta(U1_C, PegboardUnits::U1));
    toolbox_add_tool(meta(U1_D, PegboardUnits::U1));
    toolbox_add_tool(meta(U1_E, PegboardUnits::U1));
    toolbox_add_tool(meta(U1_F, PegboardUnits::U1));
    toolbox_add_tool(meta(U1_G, PegboardUnits::U1));
}

fn ids_by_position(layout: &[Placement]) -> Vec<String> {
    let mut placements = layout.to_vec();
    placements.sort_by_key(|p| (p.y, p.x));
    placements.into_iter().map(|p| p.tool_id).collect()
}

fn has_no_overlap(layout: &[Placement]) -> bool {
    for (idx, left) in layout.iter().enumerate() {
        let left_rect = placement_rect(left);
        if left_rect.x.saturating_add(left_rect.w) > BOARD_COLS {
            return false;
        }
        for right in layout.iter().skip(idx + 1) {
            if rects_overlap(left_rect, placement_rect(right)) {
                return false;
            }
        }
    }
    true
}

fn position_of(layout: &[Placement], tool_id: &str) -> Option<(u16, u16)> {
    layout
        .iter()
        .find(|placement| placement.tool_id == tool_id)
        .map(|placement| (placement.x, placement.y))
}

#[test]
fn board_width_always_stays_six_columns() {
    register_test_tools();
    let board = vec![Placement::new(U1_TOOL, BOARD_COLS + 4, 0)];

    assert_eq!(fixed_board_cols(), u32::from(BOARD_COLS));
    assert!(!can_place_at(&board, None, BOARD_COLS + 4, 0, 1, 1));
}

#[test]
fn overlapped_pin_shifts_right_in_same_row_and_wraps_to_next_row_at_end() {
    register_test_tools();
    let mut layout = placements_from_ordered_ids([U1_TOOL, U1_B, U1_C, U1_D, U1_E, U1_F, U1_G]);

    assert_eq!(
        place_tool_with_push(&mut layout, U1_G, 2, 0),
        PlacementEdit::Changed
    );

    assert_eq!(position_of(&layout, U1_TOOL), Some((0, 0)));
    assert_eq!(position_of(&layout, U1_B), Some((1, 0)));
    assert_eq!(position_of(&layout, U1_G), Some((2, 0)));
    assert_eq!(position_of(&layout, U1_C), Some((3, 0)));
    assert_eq!(position_of(&layout, U1_D), Some((4, 0)));
    assert_eq!(position_of(&layout, U1_E), Some((5, 0)));
    assert_eq!(position_of(&layout, U1_F), Some((0, 1)));
    assert!(has_no_overlap(&layout));
}

#[test]
fn saved_placement_beyond_six_columns_is_reconciled_into_lower_row() {
    register_test_tools();
    let raw = vec![
        Placement::new(U1_TOOL, 0, 0),
        Placement::new(U1_B, 1, 0),
        Placement::new(U1_C, 2, 0),
        Placement::new(U1_D, 3, 0),
        Placement::new(U1_E, 4, 0),
        Placement::new(U1_F, 5, 0),
        Placement::new(U1_G, BOARD_COLS + 3, 0),
    ];

    let out = reconcile_placements(raw);

    assert_eq!(position_of(&out, U1_G), Some((0, 1)));
    assert!(has_no_overlap(&out));
}

#[test]
fn tool_wider_than_six_columns_is_not_placed() {
    let oversized_w = BOARD_COLS + 1;

    assert!(!can_place_at(&[], None, 0, 0, oversized_w, 1));
    assert_eq!(find_first_empty(&[], None, oversized_w, 1), (0, 0));
}

#[test]
fn rect_overlap_detects_partial_overlap_and_separation() {
    let a = PlacementRect {
        x: 0,
        y: 0,
        w: 2,
        h: 1,
    };
    let b = PlacementRect {
        x: 1,
        y: 0,
        w: 1,
        h: 1,
    };
    let c = PlacementRect {
        x: 2,
        y: 0,
        w: 1,
        h: 1,
    };
    assert!(rects_overlap(a, b));
    assert!(!rects_overlap(a, c));
}

#[test]
fn find_first_empty_fills_row_major() {
    let board = vec![Placement::new("num.hex_to_decimal", 0, 0)];
    let (x, y) = find_first_empty(&board, None, 1, 1);
    assert_eq!((x, y), (1, 0));
}

#[test]
fn can_place_at_rejects_overflow() {
    let board = Vec::new();
    assert!(!can_place_at(&board, None, BOARD_COLS, 0, 1, 1));
    assert!(can_place_at(&board, None, BOARD_COLS - 1, 0, 1, 1));
}

#[test]
fn reconcile_placements_reflows_collisions() {
    register_test_tools();
    let raw = vec![Placement::new(U2_TOOL, 0, 0), Placement::new(U1_TOOL, 0, 0)];
    let out = reconcile_placements(raw);
    assert_eq!(out.len(), 2);
    assert!(!rects_overlap(
        placement_rect(&out[0]),
        placement_rect(&out[1])
    ));
}

#[test]
fn swap_placement_pair_reconciles_mixed_sizes() {
    register_test_tools();
    let mut layout = vec![
        Placement::new(U1_TOOL, BOARD_COLS - 1, 0),
        Placement::new(U2_TOOL, 0, 0),
    ];

    assert_eq!(
        swap_placement_pair(&mut layout, U2_TOOL, U1_TOOL),
        PlacementEdit::Changed
    );

    assert!(
        layout
            .iter()
            .all(|placement| placement.x + placement_size(&placement.tool_id).0 <= BOARD_COLS)
    );
    assert_eq!(ids_by_position(&layout), vec![U1_TOOL, U2_TOOL]);
    assert!(!rects_overlap(
        placement_rect(&layout[0]),
        placement_rect(&layout[1])
    ));
}

#[test]
fn place_tool_with_push_moves_to_empty_cell_without_moving_other_tools() {
    register_test_tools();
    let mut layout = vec![
        Placement::new(U1_TOOL, 0, 0),
        Placement::new(U1_B, 1, 0),
        Placement::new(U1_C, 2, 0),
    ];

    assert_eq!(
        place_tool_with_push(&mut layout, U1_TOOL, 4, 0),
        PlacementEdit::Changed
    );

    assert_eq!(
        layout
            .iter()
            .find(|p| p.tool_id == U1_TOOL)
            .map(|p| (p.x, p.y)),
        Some((4, 0))
    );
    assert_eq!(
        layout
            .iter()
            .find(|p| p.tool_id == U1_B)
            .map(|p| (p.x, p.y)),
        Some((1, 0))
    );
    assert!(has_no_overlap(&layout));
}

#[test]
fn place_tool_with_push_pushes_colliding_tool_forward() {
    register_test_tools();
    let mut layout = vec![
        Placement::new(U1_TOOL, 0, 0),
        Placement::new(U1_B, 1, 0),
        Placement::new(U1_C, 2, 0),
        Placement::new(U1_D, 3, 0),
    ];

    assert_eq!(
        place_tool_with_push(&mut layout, U1_D, 1, 0),
        PlacementEdit::Changed
    );

    assert_eq!(
        layout
            .iter()
            .find(|p| p.tool_id == U1_D)
            .map(|p| (p.x, p.y)),
        Some((1, 0))
    );
    assert_eq!(
        layout
            .iter()
            .find(|p| p.tool_id == U1_B)
            .map(|p| (p.x, p.y)),
        Some((2, 0))
    );
    assert_eq!(
        layout
            .iter()
            .find(|p| p.tool_id == U1_C)
            .map(|p| (p.x, p.y)),
        Some((3, 0))
    );
    assert!(has_no_overlap(&layout));
}

#[test]
fn place_tool_with_push_spills_chained_moves_into_next_row() {
    register_test_tools();
    let mut layout = placements_from_ordered_ids([U1_TOOL, U1_B, U1_C, U1_D, U1_E, U1_F, U1_G]);

    assert_eq!(
        place_tool_with_push(&mut layout, U1_G, 0, 0),
        PlacementEdit::Changed
    );

    assert_eq!(ids_by_position(&layout)[0], U1_G);
    assert_eq!(
        layout
            .iter()
            .find(|p| p.tool_id == U1_TOOL)
            .map(|p| (p.x, p.y)),
        Some((1, 0))
    );
    assert!(
        layout
            .iter()
            .any(|p| p.tool_id == U1_F && p.x == 0 && p.y == 1),
        "the last pushed item must spill into the next row"
    );
    assert!(has_no_overlap(&layout));
}

#[test]
fn place_tool_with_push_fits_wide_tool_within_board_width() {
    register_test_tools();
    let mut layout = vec![Placement::new(U2_TOOL, 0, 0), Placement::new(U1_TOOL, 2, 0)];

    assert_eq!(
        place_tool_with_push(&mut layout, U2_TOOL, BOARD_COLS - 1, 2),
        PlacementEdit::Changed
    );

    assert_eq!(
        layout
            .iter()
            .find(|p| p.tool_id == U2_TOOL)
            .map(|p| (p.x, p.y)),
        Some((BOARD_COLS - 2, 2))
    );
    assert!(has_no_overlap(&layout));
}

#[test]
fn move_placement_swaps_with_row_major_neighbor() {
    register_test_tools();
    let mut layout = vec![Placement::new(U1_TOOL, 0, 0), Placement::new(U2_TOOL, 1, 0)];

    assert_eq!(
        move_placement(&mut layout, U2_TOOL, Direction::Prev),
        PlacementEdit::Changed
    );
    assert_eq!(ids_by_position(&layout), vec![U2_TOOL, U1_TOOL]);

    assert_eq!(
        move_placement(&mut layout, U2_TOOL, Direction::Prev),
        PlacementEdit::NoOp
    );
}

#[test]
fn slugify_normalizes_board_titles() {
    assert_eq!(slugify("Trading Desk"), "trading-desk");
    assert_eq!(slugify("  !!!  "), "board");
    assert_eq!(slugify("Dev / Ops"), "dev-ops");
}

fn span_of(cols: u16, rows: u16) -> PinSpan {
    PinSpan::new(
        ColSpan::new(cols).expect("test span cols"),
        RowSpan::new(rows).expect("test span rows"),
    )
}

#[test]
fn effective_size_prefers_span_override_over_manifest_default() {
    register_test_tools();
    let plain = Placement::new(U1_TOOL, 0, 0);

    assert_eq!(effective_size(&plain), placement_size(U1_TOOL));

    let overridden = plain.with_span(Some(span_of(2, 3)));
    assert_eq!(effective_size(&overridden), (2, 3));
}

#[test]
fn pin_with_span_uses_expanded_size_for_collision_checks() {
    register_test_tools();
    let board = vec![Placement::new(U1_TOOL, 0, 0).with_span(Some(span_of(2, 2)))];

    // The manifest says U1(1×1) but the 2×2 span occupies (1,0)/(0,1)/(1,1).
    assert!(!can_place_at(&board, None, 1, 0, 1, 1));
    assert!(!can_place_at(&board, None, 0, 1, 1, 1));
    assert!(!can_place_at(&board, None, 1, 1, 1, 1));
    assert!(can_place_at(&board, None, 2, 0, 1, 1));
}

#[test]
fn push_on_pin_with_span_keeps_expanded_size_and_span() {
    register_test_tools();
    let mut layout = vec![
        Placement::new(U1_TOOL, 0, 0).with_span(Some(span_of(2, 1))),
        Placement::new(U1_B, 2, 0),
    ];

    assert_eq!(
        place_tool_with_push(&mut layout, U1_B, 0, 0),
        PlacementEdit::Changed
    );

    assert_eq!(position_of(&layout, U1_B), Some((0, 0)));
    assert_eq!(position_of(&layout, U1_TOOL), Some((1, 0)));
    assert_eq!(
        layout
            .iter()
            .find(|p| p.tool_id == U1_TOOL)
            .and_then(|p| p.span),
        Some(span_of(2, 1)),
        "the span override must be preserved after being pushed"
    );
    assert!(has_no_overlap(&layout));
}

#[test]
fn drop_with_span_wider_than_board_normalizes_start_column() {
    register_test_tools();
    let mut layout = vec![Placement::new(U1_TOOL, 0, 0).with_span(Some(span_of(2, 1)))];

    assert_eq!(
        place_tool_with_push(&mut layout, U1_TOOL, BOARD_COLS - 1, 1),
        PlacementEdit::Changed
    );

    assert_eq!(position_of(&layout, U1_TOOL), Some((BOARD_COLS - 2, 1)));
    assert!(has_no_overlap(&layout));
}

#[test]
fn coordinate_placement_judges_placeability_by_span_size() {
    register_test_tools();
    let mut layout = vec![
        Placement::new(U1_TOOL, 0, 0).with_span(Some(span_of(2, 1))),
        Placement::new(U1_B, 3, 0),
    ];

    // Placing the 2-cell span tool at (2,0) collides with U1_B at (3,0).
    assert_eq!(
        place_tool_at(&mut layout, U1_TOOL, 2, 0),
        PlacementEdit::NoOp
    );
    assert_eq!(
        place_tool_at(&mut layout, U1_TOOL, 4, 0),
        PlacementEdit::Changed
    );
    assert_eq!(position_of(&layout, U1_TOOL), Some((4, 0)));
    assert!(has_no_overlap(&layout));
}

#[test]
fn reconcile_placements_preserves_span_and_args_preset_and_clears_collisions_by_expanded_size() {
    register_test_tools();
    let preset = upeg_core::ArgsPreset::parse(r#"{"unit":"c"}"#).expect("test preset");
    let raw = vec![
        Placement::new(U1_TOOL, 0, 0)
            .with_span(Some(span_of(2, 1)))
            .with_args_preset(Some(preset.clone())),
        Placement::new(U1_B, 1, 0),
    ];

    let out = reconcile_placements(raw);

    // U1_B's (1,0) is occupied by the span, so it is pushed to the next empty cell.
    assert_eq!(position_of(&out, U1_B), Some((2, 0)));
    let kept = out
        .iter()
        .find(|p| p.tool_id == U1_TOOL)
        .expect("span tool must remain");
    assert_eq!(kept.span, Some(span_of(2, 1)));
    assert_eq!(kept.args_preset, Some(preset));
    assert!(has_no_overlap(&out));
}
