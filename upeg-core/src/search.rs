//! Pure tool search contracts shared by every surface.

use std::collections::{HashMap, HashSet};

use crate::{Placement, Surface, ToolMeta};

/// UI-independent search request over tool metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchQuery<'a> {
    pub needle: &'a str,
    pub surface: Option<Surface>,
    pub limit: Option<usize>,
}

/// User-specific ranking signals supplied by a surface or profile store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchSignals {
    pub pinned: Vec<PinnedSignal>,
    pub recent: Vec<RecentSignal>,
}

impl SearchSignals {
    /// Build pinned ranking signals from existing pegboard placements.
    ///
    /// Placements are ranked in row-major order `(y, x, tool_id)`. If the same
    /// tool appears more than once, the best/lower rank is kept.
    #[must_use]
    pub fn from_pinned_placements(placements: &[Placement]) -> Self {
        Self {
            pinned: pinned_signals_from_placements(placements),
            recent: Vec::new(),
        }
    }
}

/// Pinned-tool rank. Lower ranks sort first; pegboards should use row-major
/// `(y, x)` ordering when converting placements into this signal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedSignal {
    pub tool_id: String,
    pub rank: usize,
}

/// Recent-tool rank. Lower ranks sort first; rank `0` means newest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentSignal {
    pub tool_id: String,
    pub rank: usize,
}

fn pinned_signals_from_placements(placements: &[Placement]) -> Vec<PinnedSignal> {
    let mut ranked: Vec<&Placement> = placements.iter().collect();
    ranked.sort_by(|left, right| {
        left.y
            .cmp(&right.y)
            .then(left.x.cmp(&right.x))
            .then(left.tool_id.cmp(&right.tool_id))
    });

    let mut best_ranks: HashMap<&str, usize> = HashMap::new();
    for (rank, placement) in ranked.into_iter().enumerate() {
        best_ranks.entry(&placement.tool_id).or_insert(rank);
    }

    let mut signals: Vec<PinnedSignal> = best_ranks
        .into_iter()
        .map(|(tool_id, rank)| PinnedSignal {
            tool_id: tool_id.to_string(),
            rank,
        })
        .collect();
    signals.sort_by(|left, right| {
        left.rank
            .cmp(&right.rank)
            .then(left.tool_id.cmp(&right.tool_id))
    });
    signals
}

/// Tool metadata fields searched by V1 core search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchField {
    DisplayLabel,
    Id,
    LocalId,
    Toolkit,
    Tags,
}

/// Best match quality for a result. The declaration order is the ranking order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SearchMatchTier {
    Exact,
    Prefix,
    Substring,
    EmptyQuery,
}

/// Search result containing the matched tool and V1 match metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult<'a> {
    pub tool: &'a ToolMeta,
    pub matched_fields: HashSet<SearchField>,
    pub match_tier: SearchMatchTier,
}

/// Search tools with deterministic V1 ranking.
///
/// The implementation is pure and deliberately small: it filters by surface,
/// trims the query, lowercases with Unicode [`str::to_lowercase`], searches only
/// `display_label`, `id`, `local_id`, `toolkit`, and `tags`, then sorts by:
/// `(not is_pinned, pinned_rank, not is_recent, recent_rank, match_tier, tool.id)`.
#[must_use]
pub fn search_tools<'a>(
    tools: &[&'a ToolMeta],
    query: SearchQuery<'_>,
    signals: SearchSignals,
) -> Vec<SearchResult<'a>> {
    let pinned_ranks = signal_ranks(
        signals
            .pinned
            .iter()
            .map(|signal| (signal.tool_id.as_str(), signal.rank)),
    );
    let recent_ranks = signal_ranks(
        signals
            .recent
            .iter()
            .map(|signal| (signal.tool_id.as_str(), signal.rank)),
    );
    let needle = query.needle.trim().to_lowercase();
    let mut seen_tool_ids = HashSet::new();

    let mut results: Vec<_> = tools
        .iter()
        .copied()
        .filter(|tool| {
            query
                .surface
                .is_none_or(|surface| tool.is_on_surface(surface))
        })
        .filter(|tool| seen_tool_ids.insert(tool.id))
        .filter_map(|tool| search_result_for_tool(tool, &needle))
        .collect();

    results.sort_by_key(|result| {
        let pinned_rank = pinned_ranks.get(result.tool.id).copied();
        let recent_rank = recent_ranks.get(result.tool.id).copied();
        (
            pinned_rank.is_none(),
            pinned_rank.unwrap_or(usize::MAX),
            recent_rank.is_none(),
            recent_rank.unwrap_or(usize::MAX),
            result.match_tier,
            result.tool.id,
        )
    });

    if let Some(limit) = query.limit {
        results.truncate(limit);
    }

    results
}

fn signal_ranks<'a>(signals: impl Iterator<Item = (&'a str, usize)>) -> HashMap<&'a str, usize> {
    let mut ranks: HashMap<&'a str, usize> = HashMap::new();
    for (tool_id, rank) in signals {
        ranks
            .entry(tool_id)
            .and_modify(|current| *current = (*current).min(rank))
            .or_insert(rank);
    }
    ranks
}

fn search_result_for_tool<'a>(tool: &'a ToolMeta, needle: &str) -> Option<SearchResult<'a>> {
    if needle.is_empty() {
        return Some(SearchResult {
            tool,
            matched_fields: HashSet::new(),
            match_tier: SearchMatchTier::EmptyQuery,
        });
    }

    let mut matched_fields = HashSet::new();
    let mut match_tier: Option<SearchMatchTier> = None;

    record_match(
        tool.display_label,
        needle,
        SearchField::DisplayLabel,
        &mut matched_fields,
        &mut match_tier,
    );
    record_match(
        tool.id,
        needle,
        SearchField::Id,
        &mut matched_fields,
        &mut match_tier,
    );
    record_match(
        tool.local_id,
        needle,
        SearchField::LocalId,
        &mut matched_fields,
        &mut match_tier,
    );
    record_match(
        tool.toolkit,
        needle,
        SearchField::Toolkit,
        &mut matched_fields,
        &mut match_tier,
    );
    for tag in tool.tags {
        record_match(
            tag,
            needle,
            SearchField::Tags,
            &mut matched_fields,
            &mut match_tier,
        );
    }

    match_tier.map(|tier| SearchResult {
        tool,
        matched_fields,
        match_tier: tier,
    })
}

fn record_match(
    haystack: &str,
    needle: &str,
    field: SearchField,
    matched_fields: &mut HashSet<SearchField>,
    match_tier: &mut Option<SearchMatchTier>,
) {
    let Some(tier) = field_match_tier(&haystack.to_lowercase(), needle) else {
        return;
    };

    matched_fields.insert(field);
    *match_tier = Some(match match_tier {
        Some(current) => (*current).min(tier),
        None => tier,
    });
}

fn field_match_tier(haystack: &str, needle: &str) -> Option<SearchMatchTier> {
    if haystack == needle {
        Some(SearchMatchTier::Exact)
    } else if haystack.starts_with(needle) {
        Some(SearchMatchTier::Prefix)
    } else if haystack.contains(needle) {
        Some(SearchMatchTier::Substring)
    } else {
        None
    }
}
