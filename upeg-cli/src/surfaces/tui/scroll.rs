//! Axis-tagged scroll offsets.
//!
//! The TUI surface has five independent scroll axes (grid V/H, right
//! pane V, two filter bars H). Each one had been a bare `u16`, which
//! let a refactor accidentally assign the horizontal grid scroll to a
//! vertical clamp helper (or worse, swap them). `ScrollOffset<Axis>`
//! makes the axis a compile-time tag so the borrow checker rejects
//! such mixes at the call site.
//!
//! The `Axis` parameter is purely a phantom — `ScrollOffset<V>` and
//! `ScrollOffset<H>` are layout-identical to `u16`, so this costs
//! nothing at runtime but everything to a future axis-mix bug.

use std::marker::PhantomData;

/// Marker for vertically-scrolled surfaces (grid rows, right pane).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Vertical;

/// Marker for horizontally-scrolled surfaces (grid columns, filter bars).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Horizontal;

/// A scroll position along one axis.
///
/// `PhantomData<fn() -> A>` rather than `PhantomData<A>` so the type
/// stays `Send + Sync` regardless of `A` (the marker types are zero-
/// sized; we never actually own one) and so `A` is not subject to
/// variance over `&'_`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScrollOffset<A> {
    value: u16,
    _axis: PhantomData<fn() -> A>,
}

impl<A> ScrollOffset<A> {
    /// Origin — equivalent to `0` on this axis.
    pub const ZERO: Self = Self {
        value: 0,
        _axis: PhantomData,
    };

    pub const fn new(value: u16) -> Self {
        Self {
            value,
            _axis: PhantomData,
        }
    }

    pub const fn get(self) -> u16 {
        self.value
    }

    /// Cap this offset at `max`. Used by per-frame clamping when the
    /// viewport shrank since the last frame.
    pub const fn clamp_to(self, max: u16) -> Self {
        let value = if self.value > max { max } else { self.value };
        Self::new(value)
    }

    /// Move forward by `delta`, saturating then clamping to `max`.
    /// Used by wheel/Right/Down handlers.
    pub const fn add_clamped(self, delta: u16, max: u16) -> Self {
        Self::new(self.value.saturating_add(delta)).clamp_to(max)
    }

    /// Move back by `delta`, saturating at zero. Used by wheel/Left/Up
    /// handlers — no `max` parameter because `0` is the only bound.
    pub const fn sub_saturating(self, delta: u16) -> Self {
        Self::new(self.value.saturating_sub(delta))
    }
}

impl<A> Default for ScrollOffset<A> {
    fn default() -> Self {
        Self::ZERO
    }
}

impl<A> std::fmt::Debug for ScrollOffset<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ScrollOffset({})", self.value)
    }
}

// Cross-type comparisons with bare `u16` are dimensionless (a number,
// not another axis), so allow them. This keeps test assertions like
// `state.grid_scroll > 0` readable while still forbidding cross-axis
// comparisons (`grid_scroll > grid_h_scroll`) because two different
// `ScrollOffset<A>` don't share an `Ord` instance.
impl<A> PartialEq<u16> for ScrollOffset<A> {
    fn eq(&self, other: &u16) -> bool {
        self.value == *other
    }
}

impl<A> PartialOrd<u16> for ScrollOffset<A> {
    fn partial_cmp(&self, other: &u16) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_clamped_never_exceeds_max() {
        let off: ScrollOffset<Vertical> = ScrollOffset::new(10);
        assert_eq!(off.add_clamped(50, 30).get(), 30);
        assert_eq!(off.add_clamped(5, 30).get(), 15);
        assert_eq!(off.add_clamped(u16::MAX, 30).get(), 30);
    }

    #[test]
    fn sub_saturating_floors_at_zero() {
        let off: ScrollOffset<Horizontal> = ScrollOffset::new(10);
        assert_eq!(off.sub_saturating(3).get(), 7);
        assert_eq!(off.sub_saturating(99).get(), 0);
    }

    #[test]
    fn clamp_to_leaves_values_at_or_below_max() {
        let off: ScrollOffset<Vertical> = ScrollOffset::new(20);
        assert_eq!(off.clamp_to(30).get(), 20);
        assert_eq!(off.clamp_to(10).get(), 10);
    }

    #[test]
    fn default_equals_zero() {
        let v: ScrollOffset<Vertical> = ScrollOffset::default();
        let h: ScrollOffset<Horizontal> = ScrollOffset::default();
        assert_eq!(v, ScrollOffset::<Vertical>::ZERO);
        assert_eq!(h, ScrollOffset::<Horizontal>::ZERO);
    }
}
