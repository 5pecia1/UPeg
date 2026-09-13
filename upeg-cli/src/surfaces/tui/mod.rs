//! TUI surface (PRD v2.1 §6.4 — ratatui + crossterm per STACK.md §1).
//!
//! `upeg` with no args drops you into a Board/Tag-filterable list of every
//! TUI-visible Tool in the toolbox (inventory + runtime). Internals follow
//! The Elm Architecture: `model` owns state, `msg` maps terminal input,
//! `update` performs pure state transitions, `view` renders pure Ratatui
//! frames, and `effects` owns terminal I/O + tool dispatch.

mod controls;
mod effects;
mod grid;
mod interface_inventory;
mod model;
mod msg;
mod scroll;
mod style;
mod update;
mod view;

pub use effects::Effect as Action;
pub use model::{State, View};
pub use msg::Key;
pub use update::{apply_outcome, handle_key};

#[cfg(test)]
pub(crate) use controls::{
    FilterBar, FilterBarClick, apply_filter_click, apply_filter_key, apply_filter_scroll,
    clamp_state_to_area,
};
#[cfg(not(test))]
pub(crate) use effects::serve_with_filters;
#[cfg(test)]
pub(crate) use effects::{Effect, serve_with_filters};
#[cfg(test)]
pub(crate) use effects::{list_tools, list_tools_for_board_and_tag};
pub(crate) use interface_inventory::interface_inventory_entries;
#[cfg(test)]
pub(crate) use model::{FocusArea, TuiFilters, TuiFormState};
#[cfg(test)]
pub(crate) use msg::{Mouse, MouseKind, Msg, PointerPhase, ScrollDelta};
#[cfg(test)]
pub(crate) use update::{handle_mouse, update};
#[cfg(test)]
pub(crate) use view::{render, render_with_filters, render_with_pin_colors};

#[cfg(test)]
pub(crate) use grid::{
    BOARD_FILTER_PREFIX, GridDirection, PlacementHint, PlacementHintsGuard, TAG_FILTER_PREFIX,
    filter_bar_max_scroll, filter_bar_scroll_step, filter_bar_scrollbar_area,
    filter_option_at_bar_cell, grid_content_area, grid_h_scrollbar_area, grid_visible_rect,
    move_cursor_in_grid, pegboard_cells, tool_index_at_grid_cell, tui_layout,
};
#[cfg(test)]
pub(crate) use scroll::ScrollOffset;

#[cfg(test)]
mod detail_tests;
#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod interface_tests;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod state_tests;
