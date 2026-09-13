//! The set of surfaces one interface contract is advertised on.
//!
//! Schema v3 records a contract **once** with every surface it reaches,
//! instead of once per surface. [`SurfaceSet`] owns that invariant: every
//! construction path sorts by presentation rank and drops duplicates, so no
//! call site has to remember to do it.

use std::ops::Deref;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::Surface;

/// Presentation rank of a surface, lowest first.
///
/// Drives both inventory ordering and the label order inside a
/// [`SurfaceSet`], so `cli, http` never renders as `http, cli`.
const fn surface_rank(surface: Surface) -> u8 {
    match surface {
        Surface::Cli => 0,
        Surface::Tui => 1,
        Surface::Desktop => 2,
        Surface::Pwa => 3,
        Surface::Ext => 4,
        Surface::Mcp => 5,
        Surface::Http => 6,
    }
}

/// Rank given to a set that advertises no surface at all.
///
/// Keeps the inventory sort total and deterministic (surfaceless entries
/// sort last) without an unnamed literal; [`SurfaceSet::is_empty`] is what
/// validation actually rejects.
const EMPTY_SURFACE_SET_RANK: u8 = u8::MAX;

/// Separator between surface labels in single-line renderings.
const SURFACE_LABEL_SEPARATOR: &str = ", ";

/// Ordered, deduplicated set of the [`Surface`]s one interface contract is
/// advertised on.
///
/// Dereferences to `[Surface]`, so `set.contains(&Surface::Cli)`,
/// `set.iter()`, `set.first()`, and `set.is_empty()` all read as if it were
/// a slice — the newtype exists only to make the sorted/deduplicated
/// invariant unrepresentable to break.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "Vec<Surface>", into = "Vec<Surface>"))]
pub struct SurfaceSet(Vec<Surface>);

impl SurfaceSet {
    /// Build a surface set, sorting by rank and removing duplicates.
    #[must_use]
    pub fn new(surfaces: impl IntoIterator<Item = Surface>) -> Self {
        let mut surfaces = surfaces.into_iter().collect::<Vec<_>>();
        surfaces.sort_unstable_by_key(|surface| surface_rank(*surface));
        surfaces.dedup();
        Self(surfaces)
    }

    /// Build a surface set advertising exactly one surface.
    #[must_use]
    pub fn single(surface: Surface) -> Self {
        Self(vec![surface])
    }

    /// The surfaces as a rank-ordered slice.
    #[must_use]
    pub fn as_slice(&self) -> &[Surface] {
        &self.0
    }

    /// Rank of the lowest-ranked surface in the set, for deterministic sorts.
    #[must_use]
    pub fn rank(&self) -> u8 {
        self.0
            .first()
            .copied()
            .map_or(EMPTY_SURFACE_SET_RANK, surface_rank)
    }

    /// Surface labels in rank order.
    #[must_use]
    pub fn labels(&self) -> Vec<&'static str> {
        self.0.iter().copied().map(Surface::label).collect()
    }

    /// Surface labels joined for single-line rendering (`"cli, http, tui"`).
    #[must_use]
    pub fn joined_labels(&self) -> String {
        self.labels().join(SURFACE_LABEL_SEPARATOR)
    }
}

impl Deref for SurfaceSet {
    type Target = [Surface];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Surface> for SurfaceSet {
    fn from(surface: Surface) -> Self {
        Self::single(surface)
    }
}

impl From<Vec<Surface>> for SurfaceSet {
    fn from(surfaces: Vec<Surface>) -> Self {
        Self::new(surfaces)
    }
}

impl From<SurfaceSet> for Vec<Surface> {
    fn from(set: SurfaceSet) -> Self {
        set.0
    }
}

impl FromIterator<Surface> for SurfaceSet {
    fn from_iter<T: IntoIterator<Item = Surface>>(surfaces: T) -> Self {
        Self::new(surfaces)
    }
}

impl<'a> IntoIterator for &'a SurfaceSet {
    type Item = &'a Surface;
    type IntoIter = std::slice::Iter<'a, Surface>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ALL_SURFACES;

    #[test]
    fn surface_set은_rank_순서로_정렬하고_중복을_제거한다() {
        let set = SurfaceSet::new([
            Surface::Http,
            Surface::Cli,
            Surface::Http,
            Surface::Tui,
            Surface::Cli,
        ]);

        assert_eq!(&*set, &[Surface::Cli, Surface::Tui, Surface::Http]);
        assert_eq!(set.joined_labels(), "cli, tui, http");
    }

    #[test]
    fn surface_set의_rank는_가장_낮은_표면을_따른다() {
        assert_eq!(
            SurfaceSet::new([Surface::Http, Surface::Tui]).rank(),
            surface_rank(Surface::Tui)
        );
        assert_eq!(
            SurfaceSet::single(Surface::Cli).rank(),
            surface_rank(Surface::Cli)
        );
    }

    #[test]
    fn 빈_surface_set은_가장_뒤로_정렬된다() {
        let empty = SurfaceSet::default();

        assert!(empty.is_empty());
        assert_eq!(empty.rank(), EMPTY_SURFACE_SET_RANK);
        for surface in ALL_SURFACES {
            assert!(SurfaceSet::single(*surface).rank() < empty.rank());
        }
    }

    #[test]
    fn surface_rank는_모든_표면에_고유하다() {
        let mut ranks = ALL_SURFACES
            .iter()
            .copied()
            .map(surface_rank)
            .collect::<Vec<_>>();
        ranks.sort_unstable();
        ranks.dedup();

        assert_eq!(ranks.len(), ALL_SURFACES.len());
    }
}
