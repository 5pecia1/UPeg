//! Pegboard placement geometry shared by every surface.
//!
//! Pure helpers operate on `&[Placement]` plus the manifest-derived
//! `(w, h)` of each tool. Surfaces (Desktop/TUI/PWA) and native state
//! (upeg-sources) call into this module so the placement contract is
//! identical everywhere.

use std::collections::BTreeSet;

use upeg_core::{BOARD_COLS, PinColorHex, PinSpan, Placement};

use crate::toolbox::toolbox_tool;

/// Rectangle in the canonical pegboard grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacementRect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementEdit {
    Changed,
    NoOp,
}

impl PlacementEdit {
    pub const fn changed(self) -> bool {
        matches!(self, Self::Changed)
    }
}

/// Direction for an in-layout neighbor swap.
///
/// `Prev` moves the entry toward the start of the board's row-major tool list,
/// `Next` moves it toward the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Prev,
    Next,
}

/// URL-safe lowercase slug for a user-typed board title. ASCII alphanumerics
/// are preserved as lowercase; runs of any other character collapse to `-`.
#[must_use]
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_dash = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        return "board".to_string();
    }
    out
}

/// Size of a tool from its manifest. Falls back to `(1, 1)` for tools
/// missing from the toolbox — callers should normally drop unregistered
/// placements before reaching geometry checks.
pub fn placement_size(tool_id: &str) -> (u16, u16) {
    toolbox_tool(tool_id)
        .map(|tool| tool.pegboard_units.grid_span())
        .unwrap_or((1, 1))
}

/// Effective `(w, h)` of an existing placement: the user-set span
/// override wins; otherwise the tool's manifest footprint
/// ([`placement_size`]). Contexts without a `Placement` yet (new
/// placement candidates) keep using [`placement_size`].
pub fn effective_size(placement: &Placement) -> (u16, u16) {
    placement
        .span
        .map(PinSpan::grid_span)
        .unwrap_or_else(|| placement_size(&placement.tool_id))
}

pub fn placement_rect(p: &Placement) -> PlacementRect {
    let (w, h) = effective_size(p);
    PlacementRect {
        x: p.x,
        y: p.y,
        w,
        h,
    }
}

/// Fixed column count of the canonical pegboard grid.
pub fn fixed_board_cols() -> u32 {
    u32::from(BOARD_COLS)
}

pub const fn rects_overlap(a: PlacementRect, b: PlacementRect) -> bool {
    a.x < b.x.saturating_add(b.w)
        && b.x < a.x.saturating_add(a.w)
        && a.y < b.y.saturating_add(b.h)
        && b.y < a.y.saturating_add(a.h)
}

/// Can a tool of size `(w, h)` sit at `(x, y)` on `board` without
/// overflowing [`BOARD_COLS`] or overlapping any other placement? The
/// placement currently identified by `moving_tool_id` is excluded from
/// collision checks so a tool can be moved relative to itself.
pub fn can_place_at(
    board: &[Placement],
    moving_tool_id: Option<&str>,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
) -> bool {
    can_place_at_fixed_width(board, moving_tool_id, x, y, w, h)
}

fn is_placeable_span(w: u16, h: u16) -> bool {
    w != 0 && h != 0 && u32::from(w) <= fixed_board_cols()
}

fn can_place_at_fixed_width(
    board: &[Placement],
    moving_tool_id: Option<&str>,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
) -> bool {
    if !is_placeable_span(w, h) || u32::from(x).saturating_add(u32::from(w)) > fixed_board_cols() {
        return false;
    }
    let candidate = PlacementRect { x, y, w, h };
    !board.iter().any(|p| {
        Some(p.tool_id.as_str()) != moving_tool_id && rects_overlap(candidate, placement_rect(p))
    })
}

/// First `(x, y)` (row-major) that fits a tool of size `(w, h)`. Rows
/// are unbounded; the function always returns a valid slot.
pub fn find_first_empty(
    board: &[Placement],
    moving_tool_id: Option<&str>,
    w: u16,
    h: u16,
) -> (u16, u16) {
    find_first_empty_fixed_width(board, moving_tool_id, w, h)
}

fn max_start_x(w: u16) -> u16 {
    let max_start_x = fixed_board_cols().saturating_sub(u32::from(w));
    u16::try_from(max_start_x).unwrap_or(u16::MAX)
}

fn find_first_empty_fixed_width(
    board: &[Placement],
    moving_tool_id: Option<&str>,
    w: u16,
    h: u16,
) -> (u16, u16) {
    if !is_placeable_span(w, h) {
        return (0, 0);
    }
    let max_start_x = max_start_x(w);
    for y in 0..u16::MAX {
        for x in 0..=max_start_x {
            if can_place_at_fixed_width(board, moving_tool_id, x, y, w, h) {
                return (x, y);
            }
        }
    }
    (0, 0)
}

fn find_first_empty_from(
    board: &[Placement],
    moving_tool_id: Option<&str>,
    w: u16,
    h: u16,
    min_x: u16,
    min_y: u16,
) -> (u16, u16) {
    find_first_empty_from_fixed_width(board, moving_tool_id, w, h, min_x, min_y)
}

fn find_first_empty_from_fixed_width(
    board: &[Placement],
    moving_tool_id: Option<&str>,
    w: u16,
    h: u16,
    min_x: u16,
    min_y: u16,
) -> (u16, u16) {
    if !is_placeable_span(w, h) {
        return (0, 0);
    }
    let max_start_x = max_start_x(w);
    for y in min_y..u16::MAX {
        if y == min_y && min_x > max_start_x {
            continue;
        }
        let start_x = if y == min_y { min_x } else { 0 };
        for x in start_x..=max_start_x {
            if can_place_at_fixed_width(board, moving_tool_id, x, y, w, h) {
                return (x, y);
            }
        }
    }
    (0, 0)
}

fn normalize_start_x(x: u16, w: u16) -> u16 {
    if u32::from(w) >= fixed_board_cols() {
        0
    } else {
        let max_start_x = max_start_x(w);
        if x > max_start_x { max_start_x } else { x }
    }
}

const fn next_anchor_after(x: u16, y: u16) -> (u16, u16) {
    if x.saturating_add(1) < BOARD_COLS {
        (x + 1, y)
    } else {
        (0, y.saturating_add(1))
    }
}

/// Drop unknown/duplicate placements and reflow anything that collides. The
/// first valid occurrence of each tool keeps its coordinate whenever that
/// coordinate still satisfies the fixed-width geometry rules.
pub fn reconcile_placements<I>(raw: I) -> Vec<Placement>
where
    I: IntoIterator<Item = Placement>,
{
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for mut placement in raw {
        let id = placement.tool_id.trim().to_string();
        if id.is_empty() || toolbox_tool(&id).is_none() || !seen.insert(id.clone()) {
            continue;
        }
        placement.tool_id = id;
        let (w, h) = effective_size(&placement);
        if !is_placeable_span(w, h) {
            continue;
        }
        let (x, y) = if can_place_at(&out, None, placement.x, placement.y, w, h) {
            (placement.x, placement.y)
        } else {
            find_first_empty(&out, None, w, h)
        };
        placement.x = x;
        placement.y = y;
        out.push(placement);
    }
    out
}

/// Reconcile placements while preserving the incoming order in row-major anchor
/// order. Candidate coordinates are kept only when they remain valid and do not
/// sort before a placement that was already accepted.
pub fn reconcile_placements_in_order<I>(raw: I) -> Vec<Placement>
where
    I: IntoIterator<Item = Placement>,
{
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut min_anchor = (0, 0);
    for mut placement in raw {
        let id = placement.tool_id.trim().to_string();
        if id.is_empty() || toolbox_tool(&id).is_none() || !seen.insert(id.clone()) {
            continue;
        }
        placement.tool_id = id;
        let (w, h) = effective_size(&placement);
        if !is_placeable_span(w, h) {
            continue;
        }
        let candidate_key = (placement.y, placement.x);
        let min_key = (min_anchor.1, min_anchor.0);
        let (x, y) = if candidate_key >= min_key
            && can_place_at(&out, None, placement.x, placement.y, w, h)
        {
            (placement.x, placement.y)
        } else {
            find_first_empty_from(&out, None, w, h, min_anchor.0, min_anchor.1)
        };
        placement.x = x;
        placement.y = y;
        out.push(placement);
        min_anchor = next_anchor_after(x, y);
    }
    out
}

pub fn reconcile_layout(layout: &mut Vec<Placement>) {
    *layout = reconcile_placements(std::mem::take(layout));
}

pub fn pin_placement(layout: &mut Vec<Placement>, tool_id: &str) -> PlacementEdit {
    let id = tool_id.trim();
    if toolbox_tool(id).is_none() || layout.iter().any(|p| p.tool_id == id) {
        return PlacementEdit::NoOp;
    }
    let (w, h) = placement_size(id);
    if !is_placeable_span(w, h) {
        return PlacementEdit::NoOp;
    }
    let (x, y) = find_first_empty(layout, None, w, h);
    layout.push(Placement::new(id.to_string(), x, y));
    PlacementEdit::Changed
}

pub fn unpin_placement(layout: &mut Vec<Placement>, tool_id: &str) -> PlacementEdit {
    let before = layout.len();
    layout.retain(|p| p.tool_id != tool_id);
    if layout.len() < before {
        PlacementEdit::Changed
    } else {
        PlacementEdit::NoOp
    }
}

pub fn place_tool_at(layout: &mut [Placement], tool_id: &str, x: u16, y: u16) -> PlacementEdit {
    if toolbox_tool(tool_id).is_none() {
        return PlacementEdit::NoOp;
    }
    let Some(index) = layout.iter().position(|p| p.tool_id == tool_id) else {
        return PlacementEdit::NoOp;
    };
    let (w, h) = effective_size(&layout[index]);
    if !can_place_at(layout, Some(tool_id), x, y, w, h) {
        return PlacementEdit::NoOp;
    }
    layout[index].x = x;
    layout[index].y = y;
    PlacementEdit::Changed
}

pub fn set_pin_color(
    layout: &mut [Placement],
    tool_id: &str,
    color: Option<PinColorHex>,
) -> PlacementEdit {
    let Some(target) = layout.iter_mut().find(|p| p.tool_id == tool_id) else {
        return PlacementEdit::NoOp;
    };
    if target.color == color {
        return PlacementEdit::NoOp;
    }
    target.color = color;
    PlacementEdit::Changed
}

/// Move an existing tool to `(x, y)` and push colliding tools forward in
/// row-major order until the layout is non-overlapping again.
///
/// This is the coordinate-based equivalent of Android-style home-screen
/// placement: the dragged tool owns the target cell; existing tools keep
/// their current coordinates when possible and otherwise flow to the first
/// free slot at or after their previous anchor. Rows are unbounded while
/// columns remain fixed to [`BOARD_COLS`].
pub fn place_tool_with_push(
    layout: &mut Vec<Placement>,
    tool_id: &str,
    x: u16,
    y: u16,
) -> PlacementEdit {
    if toolbox_tool(tool_id).is_none() {
        return PlacementEdit::NoOp;
    }
    let Some(mut moving) = layout.iter().find(|p| p.tool_id == tool_id).cloned() else {
        return PlacementEdit::NoOp;
    };
    let (w, h) = effective_size(&moving);
    if !is_placeable_span(w, h) {
        return PlacementEdit::NoOp;
    }
    let before = layout.clone();
    moving.x = normalize_start_x(x, w);
    moving.y = y;

    let mut seen = BTreeSet::new();
    let mut existing: Vec<Placement> = layout
        .iter()
        .filter(|p| p.tool_id != tool_id)
        .filter(|p| toolbox_tool(&p.tool_id).is_some())
        .filter(|p| seen.insert(p.tool_id.clone()))
        .cloned()
        .collect();
    existing.sort_by(|a, b| (a.y, a.x, a.tool_id.as_str()).cmp(&(b.y, b.x, b.tool_id.as_str())));

    let mut next = vec![moving];
    for mut item in existing {
        let (item_w, item_h) = effective_size(&item);
        if !is_placeable_span(item_w, item_h) {
            continue;
        }
        let wanted_x = normalize_start_x(item.x, item_w);
        let wanted_y = item.y;
        let (placed_x, placed_y) = if can_place_at(&next, None, wanted_x, wanted_y, item_w, item_h)
        {
            (wanted_x, wanted_y)
        } else {
            find_first_empty_from(&next, None, item_w, item_h, wanted_x, wanted_y)
        };
        item.x = placed_x;
        item.y = placed_y;
        next.push(item);
    }
    next.sort_by(|a, b| (a.y, a.x, a.tool_id.as_str()).cmp(&(b.y, b.x, b.tool_id.as_str())));
    if next == before {
        PlacementEdit::NoOp
    } else {
        *layout = next;
        PlacementEdit::Changed
    }
}

pub fn swap_placement_pair(
    layout: &mut Vec<Placement>,
    source: &str,
    target: &str,
) -> PlacementEdit {
    if source == target {
        return PlacementEdit::NoOp;
    }
    let source_idx = layout.iter().position(|p| p.tool_id == source);
    let target_idx = layout.iter().position(|p| p.tool_id == target);
    let (Some(source_idx), Some(target_idx)) = (source_idx, target_idx) else {
        return PlacementEdit::NoOp;
    };
    let source_id = layout[source_idx].tool_id.clone();
    let target_id = layout[target_idx].tool_id.clone();
    let mut ordered = layout.clone();
    ordered.sort_by_key(|p| (p.y, p.x));
    let slots: Vec<(u16, u16)> = ordered.iter().map(|p| (p.x, p.y)).collect();
    let Some(source_pos) = ordered.iter().position(|p| p.tool_id == source_id) else {
        return PlacementEdit::NoOp;
    };
    let Some(target_pos) = ordered.iter().position(|p| p.tool_id == target_id) else {
        return PlacementEdit::NoOp;
    };
    ordered.swap(source_pos, target_pos);
    for (placement, (x, y)) in ordered.iter_mut().zip(slots) {
        placement.x = x;
        placement.y = y;
    }
    *layout = reconcile_placements_in_order(ordered);
    PlacementEdit::Changed
}

pub fn move_placement(
    layout: &mut Vec<Placement>,
    tool_id: &str,
    direction: Direction,
) -> PlacementEdit {
    let mut order: Vec<usize> = (0..layout.len()).collect();
    order.sort_by_key(|&idx| {
        let p = &layout[idx];
        (p.y, p.x)
    });
    let Some(pos) = order.iter().position(|&idx| layout[idx].tool_id == tool_id) else {
        return PlacementEdit::NoOp;
    };
    let target_pos = match direction {
        Direction::Prev if pos == 0 => return PlacementEdit::NoOp,
        Direction::Prev => pos - 1,
        Direction::Next if pos + 1 >= order.len() => return PlacementEdit::NoOp,
        Direction::Next => pos + 1,
    };
    let target_id = layout[order[target_pos]].tool_id.clone();
    swap_placement_pair(layout, tool_id, &target_id)
}

/// Build a coordinate-laid-out placement list from an ordered tool-id list.
/// Used for seeding default boards from toolbox order.
pub fn placements_from_ordered_ids<'a, I: IntoIterator<Item = &'a str>>(ids: I) -> Vec<Placement> {
    let mut out: Vec<Placement> = Vec::new();
    for id in ids {
        if toolbox_tool(id).is_none() {
            continue;
        }
        let (w, h) = placement_size(id);
        let (x, y) = find_first_empty(&out, None, w, h);
        out.push(Placement::new(id.to_string(), x, y));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbox_add_tool;
    use upeg_core::{
        ALL_SURFACES, ColSpan, InputSpec, Invoker, PegboardUnits, PinKind, RowSpan, ToolId,
        ToolMeta,
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
            .expect("테스트 ToolMeta id는 정규 형식이어야 한다")
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
    fn 보드_너비는_항상_6열로_유지된다() {
        register_test_tools();
        let board = vec![Placement::new(U1_TOOL, BOARD_COLS + 4, 0)];

        assert_eq!(fixed_board_cols(), u32::from(BOARD_COLS));
        assert!(!can_place_at(&board, None, BOARD_COLS + 4, 0, 1, 1));
    }

    #[test]
    fn 겹친_pin은_같은_행_오른쪽으로_밀리고_끝에서_다음_행으로_넘어간다() {
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
    fn 저장_배치가_6열_밖이면_아래쪽_행으로_정리된다() {
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
    fn 도구가_6열보다_넓으면_배치하지_않는다() {
        let oversized_w = BOARD_COLS + 1;

        assert!(!can_place_at(&[], None, 0, 0, oversized_w, 1));
        assert_eq!(find_first_empty(&[], None, oversized_w, 1), (0, 0));
    }

    #[test]
    fn 사각형_겹침은_부분_겹침과_분리를_감지한다() {
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
    fn 첫_빈칸_찾기는_행_우선으로_채운다() {
        let board = vec![Placement::new("num.hex_to_decimal", 0, 0)];
        let (x, y) = find_first_empty(&board, None, 1, 1);
        assert_eq!((x, y), (1, 0));
    }

    #[test]
    fn 배치_가능성은_넘침을_거부한다() {
        let board = Vec::new();
        assert!(!can_place_at(&board, None, BOARD_COLS, 0, 1, 1));
        assert!(can_place_at(&board, None, BOARD_COLS - 1, 0, 1, 1));
    }

    #[test]
    fn 배치_정리는_충돌을_다시_흘려보낸다() {
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
    fn 배치_쌍_교환은_섞인_크기를_다시_정리한다() {
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
    fn 밀어내기_도구_배치는_다른_도구를_옮기지_않고_빈칸으로_이동한다() {
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
    fn 밀어내기_도구_배치는_충돌한_도구를_앞으로_민다() {
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
    fn 밀어내기_도구_배치는_연쇄_이동을_다음_행으로_흘려보낸다() {
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
            "마지막으로 밀린 항목은 다음 행으로 흘러가야 한다"
        );
        assert!(has_no_overlap(&layout));
    }

    #[test]
    fn 밀어내기_도구_배치는_넓은_도구를_보드_폭에_맞춘다() {
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
    fn 배치_이동은_행_우선_이웃과_교환한다() {
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
    fn slugify는_보드_제목을_정규화한다() {
        assert_eq!(slugify("Trading Desk"), "trading-desk");
        assert_eq!(slugify("  !!!  "), "board");
        assert_eq!(slugify("Dev / Ops"), "dev-ops");
    }

    fn span_of(cols: u16, rows: u16) -> PinSpan {
        PinSpan::new(
            ColSpan::new(cols).expect("테스트 span cols"),
            RowSpan::new(rows).expect("테스트 span rows"),
        )
    }

    #[test]
    fn effective_size는_span_override를_manifest_기본보다_우선한다() {
        register_test_tools();
        let plain = Placement::new(U1_TOOL, 0, 0);

        assert_eq!(effective_size(&plain), placement_size(U1_TOOL));

        let overridden = plain.with_span(Some(span_of(2, 3)));
        assert_eq!(effective_size(&overridden), (2, 3));
    }

    #[test]
    fn span이_있는_pin은_충돌_판정에_확장_크기를_쓴다() {
        register_test_tools();
        let board = vec![Placement::new(U1_TOOL, 0, 0).with_span(Some(span_of(2, 2)))];

        // manifest는 U1(1×1)이지만 span 2×2가 (1,0)/(0,1)/(1,1)을 차지한다.
        assert!(!can_place_at(&board, None, 1, 0, 1, 1));
        assert!(!can_place_at(&board, None, 0, 1, 1, 1));
        assert!(!can_place_at(&board, None, 1, 1, 1, 1));
        assert!(can_place_at(&board, None, 2, 0, 1, 1));
    }

    #[test]
    fn span이_있는_pin_밀어내기는_확장_크기와_span을_유지한다() {
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
            "밀린 뒤에도 span override는 보존되어야 한다"
        );
        assert!(has_no_overlap(&layout));
    }

    #[test]
    fn span이_보드_폭을_넘는_드롭은_시작_열을_정규화한다() {
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
    fn 좌표_지정_배치는_span_크기로_배치_가능성을_판정한다() {
        register_test_tools();
        let mut layout = vec![
            Placement::new(U1_TOOL, 0, 0).with_span(Some(span_of(2, 1))),
            Placement::new(U1_B, 3, 0),
        ];

        // 2칸짜리 span 도구를 (2,0)에 두면 (3,0)의 U1_B와 겹친다.
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
    fn 배치_정리는_span과_args_preset을_보존하고_확장_크기로_충돌을_정리한다() {
        register_test_tools();
        let preset = upeg_core::ArgsPreset::parse(r#"{"unit":"c"}"#).expect("테스트 preset");
        let raw = vec![
            Placement::new(U1_TOOL, 0, 0)
                .with_span(Some(span_of(2, 1)))
                .with_args_preset(Some(preset.clone())),
            Placement::new(U1_B, 1, 0),
        ];

        let out = reconcile_placements(raw);

        // U1_B의 (1,0)은 span이 차지하므로 다음 빈칸으로 밀린다.
        assert_eq!(position_of(&out, U1_B), Some((2, 0)));
        let kept = out
            .iter()
            .find(|p| p.tool_id == U1_TOOL)
            .expect("span 도구 유지");
        assert_eq!(kept.span, Some(span_of(2, 1)));
        assert_eq!(kept.args_preset, Some(preset));
        assert!(has_no_overlap(&out));
    }
}
