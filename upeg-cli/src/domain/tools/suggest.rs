//! "Did you mean ..." suggestions for unknown tool ids and filter
//! values. Implements the two-tier ranking (substring win first, then
//! length-aware Levenshtein) used uniformly by every surface — CLI
//! stderr hint, HTTP error body, MCP `Method not found` message.

use upeg_core::Surface;
use upeg_runtime::toolbox_tools;

/// Levenshtein edit distance between two strings (chars, not bytes).
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr: Vec<usize> = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// rank a candidate against a query for "did you mean"
/// suggestions. Returns `Some((tier, distance))` for matches or `None`
/// for non-matches. Lower tier wins; tier 0 = substring, tier 1 =
/// Levenshtein-near.
///
/// The two-tier rule prevents the threshold-4 Levenshtein collapse
/// (which would return `color`/`id` for `conv`):
///   - Tier 0: query is a substring of candidate (case-insensitive)
///     OR vice versa. Catches truncations (`conv` → `convert`).
///   - Tier 1: Levenshtein distance ≤ `max(2, candidate.len() / 4)`.
///     Length-aware threshold so short candidates (`id`, `color`)
///     don't accept far-off queries while long candidates
///     (`num.hex_to_decimal`) still tolerate realistic typos.
///
/// cap query length at 256 chars to prevent O(query × tool_id)
/// matrix allocations on adversarial input.
pub(crate) fn match_score(query: &str, candidate: &str) -> Option<(u8, usize)> {
    if query.is_empty() {
        return None;
    }
    const MAX_QUERY_LEN: usize = 256;
    if query.len() > MAX_QUERY_LEN {
        return None;
    }
    let q_lower = query.to_ascii_lowercase();
    let c_lower = candidate.to_ascii_lowercase();
    let dist = levenshtein(query, candidate);
    if c_lower.contains(&q_lower) || q_lower.contains(&c_lower) {
        return Some((0, dist));
    }
    let len_aware_threshold = (candidate.len() / 4).max(2);
    if dist <= len_aware_threshold {
        return Some((1, dist));
    }
    None
}

/// Return up to `max` registered tool ids matching `query`, closest
/// first. Empty when the toolbox has no near-matches.
///
/// `on_surface` filters suggestions to tools the calling surface can
/// actually dispatch — an HTTP caller suggesting a CLI-only tool would
/// be misleading. Pass `None` to suggest across all surfaces.
///
/// The query itself is never suggested back. A tool can be registered
/// in this process and still produce a not-found outcome — a CLI that
/// auto-attached to a host which does not know the tool is the real
/// case (D-1) — and "unknown tool `dev.git_status` — did you mean
/// `dev.git_status`?" is pure noise.
pub fn suggest_tool_ids(query: &str, max: usize, on_surface: Option<Surface>) -> Vec<String> {
    let mut scored: Vec<(u8, usize, &'static str)> = toolbox_tools()
        .filter(|t| on_surface.is_none_or(|s| t.is_on_surface(s)))
        .filter(|t| t.id != query)
        .filter_map(|t| match_score(query, t.id).map(|(tier, d)| (tier, d, t.id)))
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));
    scored
        .into_iter()
        .take(max)
        .map(|(_, _, id)| id.to_string())
        .collect()
}

/// render the "did you mean" hint suffix for an unknown-tool
/// error message. Returns an empty string when there's no suggestion
/// (caller appends unconditionally; no-match → no noise). Format is
/// shared across CLI/HTTP/MCP so users see the same style everywhere.
pub fn unknown_tool_hint(query: &str, on_surface: Option<Surface>) -> String {
    let suggestions = suggest_tool_ids(query, 3, on_surface);
    if suggestions.is_empty() {
        return String::new();
    }
    let mut out = String::from(" — did you mean ");
    for (i, s) in suggestions.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push('`');
        out.push_str(s);
        out.push('`');
    }
    out.push('?');
    out
}

/// stderr hint for an unknown filter value (`--tag`, `--board`, …).
/// Reuses `match_score` so the suggestion algorithm is uniform with the
/// unknown-tool path. The deduped candidate set is computed by the caller
/// (only it knows whether to scope by surface, dedup, etc.).
pub(crate) fn warn_unknown_filter_value(
    label: &str,
    surface: Surface,
    user_input: &str,
    candidates: impl Iterator<Item = impl AsRef<str>>,
) {
    let mut deduped: Vec<String> = candidates.map(|c| c.as_ref().to_string()).collect();
    deduped.sort();
    deduped.dedup();
    let mut scored: Vec<(u8, usize, String)> = deduped
        .iter()
        .filter_map(|c| match_score(user_input, c).map(|(t, d)| (t, d, c.clone())))
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let suggestions: Vec<String> = scored.into_iter().take(3).map(|(_, _, c)| c).collect();
    let surface_label = surface.label();
    if suggestions.is_empty() {
        eprintln!("upeg: no tool with {label}=`{user_input}` on surface `{surface_label}`");
    } else {
        let hint = suggestions
            .iter()
            .map(|s| format!("`{s}`"))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "upeg: no tool with {label}=`{user_input}` on surface `{surface_label}` — did you mean {hint}?",
        );
    }
}
