//! Pegboard placement geometry shared by every surface.
//!
//! Pure helpers operate on `&[Placement]` plus the manifest-derived
//! `(w, h)` of each tool. Surfaces (Desktop/TUI/PWA) and native state
//! (upeg-sources) call into this module so the placement contract is
//! identical everywhere.

use std::collections::BTreeSet;

use upeg_core::{BOARD_COLS, PinColorHex, PinId, PinSpan, Placement};

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
/// placement currently identified by `moving_pin_id` is excluded from
/// collision checks so a tool can be moved relative to itself.
pub fn can_place_at(
    board: &[Placement],
    moving_pin_id: Option<&str>,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
) -> bool {
    can_place_at_fixed_width(board, moving_pin_id, x, y, w, h)
}

fn is_placeable_span(w: u16, h: u16) -> bool {
    w != 0 && h != 0 && u32::from(w) <= fixed_board_cols()
}

fn can_place_at_fixed_width(
    board: &[Placement],
    moving_pin_id: Option<&str>,
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
        Some(p.pin_id.as_str()) != moving_pin_id && rects_overlap(candidate, placement_rect(p))
    })
}

/// First `(x, y)` (row-major) that fits a tool of size `(w, h)`. Rows
/// are unbounded; the function always returns a valid slot.
pub fn find_first_empty(
    board: &[Placement],
    moving_pin_id: Option<&str>,
    w: u16,
    h: u16,
) -> (u16, u16) {
    find_first_empty_fixed_width(board, moving_pin_id, w, h)
}

fn max_start_x(w: u16) -> u16 {
    let max_start_x = fixed_board_cols().saturating_sub(u32::from(w));
    u16::try_from(max_start_x).unwrap_or(u16::MAX)
}

fn find_first_empty_fixed_width(
    board: &[Placement],
    moving_pin_id: Option<&str>,
    w: u16,
    h: u16,
) -> (u16, u16) {
    if !is_placeable_span(w, h) {
        return (0, 0);
    }
    let max_start_x = max_start_x(w);
    for y in 0..u16::MAX {
        for x in 0..=max_start_x {
            if can_place_at_fixed_width(board, moving_pin_id, x, y, w, h) {
                return (x, y);
            }
        }
    }
    (0, 0)
}

fn find_first_empty_from(
    board: &[Placement],
    moving_pin_id: Option<&str>,
    w: u16,
    h: u16,
    min_x: u16,
    min_y: u16,
) -> (u16, u16) {
    find_first_empty_from_fixed_width(board, moving_pin_id, w, h, min_x, min_y)
}

fn find_first_empty_from_fixed_width(
    board: &[Placement],
    moving_pin_id: Option<&str>,
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
            if can_place_at_fixed_width(board, moving_pin_id, x, y, w, h) {
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

/// Drop unknown tools and duplicate pin ids, and reflow collisions. The
/// first valid occurrence of each pin keeps its coordinate whenever that
/// coordinate still satisfies the fixed-width geometry rules.
pub fn reconcile_placements<I>(raw: I) -> Vec<Placement>
where
    I: IntoIterator<Item = Placement>,
{
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for mut placement in raw {
        let id = placement.tool_id.trim().to_string();
        if id.is_empty() || toolbox_tool(&id).is_none() || !seen.insert(placement.pin_id.clone()) {
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
        if id.is_empty() || toolbox_tool(&id).is_none() || !seen.insert(placement.pin_id.clone()) {
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

/// Append an additional instance of a registered tool. The caller supplies
/// an opaque id so retries can be made idempotent without conflating tools.
pub fn add_placement(layout: &mut Vec<Placement>, tool_id: &str, pin_id: PinId) -> PlacementEdit {
    let tool_id = tool_id.trim();
    if toolbox_tool(tool_id).is_none() || layout.iter().any(|placement| placement.pin_id == pin_id)
    {
        return PlacementEdit::NoOp;
    }
    let (w, h) = placement_size(tool_id);
    if !is_placeable_span(w, h) {
        return PlacementEdit::NoOp;
    }
    let (x, y) = find_first_empty(layout, None, w, h);
    layout.push(Placement::new(tool_id, x, y).with_pin_id(pin_id));
    PlacementEdit::Changed
}

/// Remove exactly one pin, leaving other instances of its tool intact.
pub fn remove_placement(layout: &mut Vec<Placement>, pin_id: &str) -> PlacementEdit {
    let before = layout.len();
    layout.retain(|placement| placement.pin_id.as_str() != pin_id);
    if layout.len() < before {
        PlacementEdit::Changed
    } else {
        PlacementEdit::NoOp
    }
}

/// Existing tool-oriented callers select the first matching pin; pin-aware
/// callers select their exact instance before this legacy fallback.
fn placement_index(layout: &[Placement], id: &str) -> Option<usize> {
    layout
        .iter()
        .position(|placement| placement.pin_id.as_str() == id)
        .or_else(|| layout.iter().position(|placement| placement.tool_id == id))
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
    let Some(index) = placement_index(layout, tool_id) else {
        return PlacementEdit::NoOp;
    };
    if toolbox_tool(&layout[index].tool_id).is_none() {
        return PlacementEdit::NoOp;
    }
    let (w, h) = effective_size(&layout[index]);
    if !can_place_at(layout, Some(layout[index].pin_id.as_str()), x, y, w, h) {
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
    let Some(index) = placement_index(layout, tool_id) else {
        return PlacementEdit::NoOp;
    };
    let target = &mut layout[index];
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
    let Some(index) = placement_index(layout, tool_id) else {
        return PlacementEdit::NoOp;
    };
    if toolbox_tool(&layout[index].tool_id).is_none() {
        return PlacementEdit::NoOp;
    }
    let mut moving = layout[index].clone();
    let moving_pin_id = moving.pin_id.clone();
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
        .filter(|p| p.pin_id != moving_pin_id)
        .filter(|p| toolbox_tool(&p.tool_id).is_some())
        .filter(|p| seen.insert(p.pin_id.clone()))
        .cloned()
        .collect();
    existing.sort_by(|a, b| (a.y, a.x, a.pin_id.as_str()).cmp(&(b.y, b.x, b.pin_id.as_str())));

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
    next.sort_by(|a, b| (a.y, a.x, a.pin_id.as_str()).cmp(&(b.y, b.x, b.pin_id.as_str())));
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
    let source_idx = placement_index(layout, source);
    let target_idx = placement_index(layout, target);
    let (Some(source_idx), Some(target_idx)) = (source_idx, target_idx) else {
        return PlacementEdit::NoOp;
    };
    let source_id = layout[source_idx].pin_id.clone();
    let target_id = layout[target_idx].pin_id.clone();
    let mut ordered = layout.clone();
    ordered.sort_by_key(|p| (p.y, p.x));
    let slots: Vec<(u16, u16)> = ordered.iter().map(|p| (p.x, p.y)).collect();
    let Some(source_pos) = ordered.iter().position(|p| p.pin_id == source_id) else {
        return PlacementEdit::NoOp;
    };
    let Some(target_pos) = ordered.iter().position(|p| p.pin_id == target_id) else {
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
    let Some(selected_index) = placement_index(layout, tool_id) else {
        return PlacementEdit::NoOp;
    };
    let Some(pos) = order.iter().position(|&idx| idx == selected_index) else {
        return PlacementEdit::NoOp;
    };
    let target_pos = match direction {
        Direction::Prev if pos == 0 => return PlacementEdit::NoOp,
        Direction::Prev => pos - 1,
        Direction::Next if pos + 1 >= order.len() => return PlacementEdit::NoOp,
        Direction::Next => pos + 1,
    };
    let target_id = layout[order[target_pos]].pin_id.clone();
    let source_id = layout[selected_index].pin_id.clone();
    swap_placement_pair(layout, source_id.as_str(), target_id.as_str())
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
#[path = "pegboard_tests.rs"]
mod tests;
