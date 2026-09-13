//! User-set pin span for pegboard placements.
//!
//! A [`PinSpan`] overrides the manifest footprint (`pegboard_units`) of a
//! pinned tool. [`ColSpan`]/[`RowSpan`] are validated newtypes (smart
//! constructors, same pattern as `PinColorHex`): the horizontal span can
//! never exceed the canonical board width ([`BOARD_COLS`]) and both axes
//! occupy at least one cell — corrupted data cannot pass the type, even
//! through deserialization (`try_from` pattern).

use crate::types::BOARD_COLS;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A pin always occupies at least one cell on each axis.
const MIN_SPAN_CELLS: u16 = 1;

/// Horizontal span of a pin in board columns. Valid range is
/// `MIN_SPAN_CELLS..=BOARD_COLS`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(try_from = "u16", into = "u16")
)]
pub struct ColSpan(u16);

impl ColSpan {
    pub const fn new(cols: u16) -> Result<Self, PinSpanError> {
        if cols >= MIN_SPAN_CELLS && cols <= BOARD_COLS {
            Ok(Self(cols))
        } else {
            Err(PinSpanError::ColsOutOfRange { actual: cols })
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for ColSpan {
    type Error = PinSpanError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ColSpan> for u16 {
    fn from(value: ColSpan) -> Self {
        value.0
    }
}

/// Vertical span of a pin in board rows. Rows are unbounded on the
/// canonical grid, so only the lower bound is enforced.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(try_from = "u16", into = "u16")
)]
pub struct RowSpan(u16);

impl RowSpan {
    pub const fn new(rows: u16) -> Result<Self, PinSpanError> {
        if rows >= MIN_SPAN_CELLS {
            Ok(Self(rows))
        } else {
            Err(PinSpanError::RowsOutOfRange { actual: rows })
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for RowSpan {
    type Error = PinSpanError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<RowSpan> for u16 {
    fn from(value: RowSpan) -> Self {
        value.0
    }
}

/// User-set footprint of a pinned tool on the canonical pegboard grid.
/// Fields are validated newtypes, so every constructed value is placeable
/// width-wise.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PinSpan {
    pub cols: ColSpan,
    pub rows: RowSpan,
}

impl PinSpan {
    pub const fn new(cols: ColSpan, rows: RowSpan) -> Self {
        Self { cols, rows }
    }

    /// `(w, h)` in board cells — same shape as
    /// [`crate::PegboardUnits::grid_span`].
    pub const fn grid_span(self) -> (u16, u16) {
        (self.cols.get(), self.rows.get())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum PinSpanError {
    #[error(
        "pin span cols must be within {min}..={max} board columns, got {actual}",
        min = MIN_SPAN_CELLS,
        max = BOARD_COLS
    )]
    ColsOutOfRange { actual: u16 },
    #[error("pin span rows must be at least {min}, got {actual}", min = MIN_SPAN_CELLS)]
    RowsOutOfRange { actual: u16 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn col_span은_한_칸부터_보드_열수까지_허용한다() {
        assert_eq!(ColSpan::new(MIN_SPAN_CELLS).map(ColSpan::get), Ok(1));
        assert_eq!(ColSpan::new(BOARD_COLS).map(ColSpan::get), Ok(BOARD_COLS));
    }

    #[test]
    fn col_span은_영과_보드_열수_초과를_거부한다() {
        assert_eq!(
            ColSpan::new(0),
            Err(PinSpanError::ColsOutOfRange { actual: 0 })
        );
        assert_eq!(
            ColSpan::new(BOARD_COLS + 1),
            Err(PinSpanError::ColsOutOfRange {
                actual: BOARD_COLS + 1
            })
        );
    }

    #[test]
    fn row_span은_영을_거부하고_1_이상을_허용한다() {
        assert_eq!(
            RowSpan::new(0),
            Err(PinSpanError::RowsOutOfRange { actual: 0 })
        );
        assert_eq!(RowSpan::new(1).map(RowSpan::get), Ok(1));
        assert_eq!(RowSpan::new(u16::MAX).map(RowSpan::get), Ok(u16::MAX));
    }

    #[test]
    fn pin_span_grid_span은_가로_세로_순서로_반환한다() {
        let span = PinSpan::new(
            ColSpan::new(2).expect("테스트 cols"),
            RowSpan::new(3).expect("테스트 rows"),
        );
        assert_eq!(span.grid_span(), (2, 3));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn pin_span은_숫자_필드_json으로_직렬화_왕복한다() {
        let span = PinSpan::new(
            ColSpan::new(2).expect("테스트 cols"),
            RowSpan::new(3).expect("테스트 rows"),
        );
        let json = serde_json::to_string(&span).expect("직렬화");
        assert_eq!(json, r#"{"cols":2,"rows":3}"#);
        let back: PinSpan = serde_json::from_str(&json).expect("역직렬화");
        assert_eq!(back, span);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn 손상된_span은_역직렬화를_통과하지_못한다() {
        assert!(serde_json::from_str::<PinSpan>(r#"{"cols":0,"rows":1}"#).is_err());
        assert!(serde_json::from_str::<PinSpan>(r#"{"cols":1,"rows":0}"#).is_err());
        let oversized = format!(r#"{{"cols":{},"rows":1}}"#, BOARD_COLS + 1);
        assert!(serde_json::from_str::<PinSpan>(&oversized).is_err());
    }
}
