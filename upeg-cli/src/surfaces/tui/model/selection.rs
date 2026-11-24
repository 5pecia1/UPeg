use upeg_core::{PinId, Placement};

use super::State;

#[derive(Clone, Debug, PartialEq, Eq)]
/// In-progress coordinate move operation. `base` is the committed board layout
/// at move start; `preview` is recomputed on every arrow key with the shared
/// push-placement primitive and becomes the committed layout only on Enter.
pub struct MoveMode {
    pub board: String,
    pub tool_id: &'static str,
    pub pin_id: PinId,
    pub target_x: u16,
    pub target_y: u16,
    pub base: Vec<Placement>,
    pub preview: Vec<Placement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// In-progress pin-resize operation — the size twin of [`MoveMode`].
/// `cols`/`rows` are the currently previewed effective span; `base` is
/// the committed board layout at resize start and `preview` is
/// recomputed on every span change with the shared push-placement
/// primitive. Only Enter turns the preview into the committed layout.
pub struct ResizeMode {
    pub board: String,
    pub tool_id: &'static str,
    pub pin_id: PinId,
    /// Currently previewed span width in board columns.
    pub cols: u16,
    /// Currently previewed span height in board rows.
    pub rows: u16,
    pub base: Vec<Placement>,
    pub preview: Vec<Placement>,
}

impl State {
    /// Resolve the index against the same filtered order the grid renders.
    pub(crate) fn selected_board_pin(&self) -> Option<Placement> {
        self.filters.board.as_deref()?;
        self.visible_placements()
            .into_iter()
            .nth(self.cursor)
            .map(|(pin, _)| pin)
    }

    pub(crate) fn focus_pin(&mut self, pin_id: &PinId) {
        if let Some(index) = self
            .visible_placements()
            .iter()
            .position(|(placement, _)| placement.pin_id == *pin_id)
        {
            self.cursor = index;
        }
    }

    pub(crate) fn cancel_pin_preview(&mut self) {
        let moved = self.move_mode.take().map(|mode| mode.pin_id);
        let resized = self.resize_mode.take().map(|mode| mode.pin_id);
        if let Some(id) = moved.or(resized) {
            self.focus_pin(&id);
        }
    }
}
