//! Command-palette tool search exposed to the Flutter UI.
//!
//! Search delegates to the shared runtime/core search implementation so empty
//! query, field matching, and surface filtering stay aligned with non-Flutter
//! surfaces.

use upeg_core::{SearchQuery, SearchSignals, Surface};
use upeg_runtime::search_toolbox_tools;

use super::tools::PinKindDto;

/// One palette match returned by [`search_tools`].
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PaletteHit {
    /// Canonical tool id (`toolkit.tool` form).
    pub id: String,
    /// User-facing label.
    pub label: String,
    /// One-line description.
    pub description: String,
    /// Higher = better match. Heuristic only — not stable across
    /// versions.
    pub score: f64,
    /// PinKind of the source tool — drives the per-row `KindBadge`
    /// (H11). Sealed enum mirror lives in `api::tools::PinKindDto`.
    pub pin_kind: PinKindDto,
}

/// Search over registered Desktop-surface toolbox tools.
///
/// Empty query returns browseable tools rather than an empty list, so the
/// Flutter palette can pin without requiring the user to type first.
#[flutter_rust_bridge::frb(sync)]
pub fn search_tools(query: String) -> Vec<PaletteHit> {
    if let Err(error) = super::tools::register_toolkit_runtime() {
        tracing::error!(%error, "upeg toolkit runtime registration failed");
    }
    let results = search_toolbox_tools(
        SearchQuery {
            needle: &query,
            surface: Some(Surface::Desktop),
            limit: None,
        },
        SearchSignals::default(),
    );
    let total = results.len();
    results
        .into_iter()
        .enumerate()
        .map(|(rank, result)| {
            let tool = result.tool;
            PaletteHit {
                id: tool.id.to_string(),
                label: tool.display_label.to_string(),
                description: tool.description.to_string(),
                score: (total.saturating_sub(rank)) as f64,
                pin_kind: PinKindDto::from(tool.pin),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_tools_exposes_pin_kind_on_each_hit() {
        super::super::tools::register_toolkit_runtime().expect("register toolkits");
        let hits = search_tools("hex_to_decimal".to_string());
        let hex = hits
            .iter()
            .find(|h| h.id == "num.hex_to_decimal")
            .expect("hex_to_decimal should be registered");
        // hex_to_decimal is an Inline tool — assert the variant lines up.
        assert_eq!(hex.pin_kind, PinKindDto::Inline);
    }

    #[test]
    fn search_tools_returns_browsable_tools_for_empty_query() {
        super::super::tools::register_toolkit_runtime().expect("register toolkits");
        let hits = search_tools(String::new());

        assert!(
            hits.iter().any(|hit| hit.id == "num.hex_to_decimal"),
            "an empty query must return a browsable tool list for the pre-search browse state"
        );
    }
}
