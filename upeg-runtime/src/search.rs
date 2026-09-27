//! Runtime search over the live registered Toolbox.

use upeg_core::{
    ToolMeta,
    search::{SearchQuery, SearchResult, SearchSignals},
};

use crate::toolbox_tools;

/// Search the current runtime-visible Toolbox.
///
/// Callers provide any user-specific ranking signals explicitly; this wrapper
/// only snapshots [`toolbox_tools`] and delegates to the pure core search
/// implementation.
#[must_use]
pub fn search_toolbox_tools(
    query: SearchQuery<'_>,
    signals: SearchSignals,
) -> Vec<SearchResult<'static>> {
    let _catalog = crate::project_scope::catalog_read_guard();
    let tools: Vec<&'static ToolMeta> = toolbox_tools().collect();
    upeg_core::search::search_tools(&tools, query, signals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbox_add_tool_managed;
    use upeg_core::{
        InputSpec, Invoker, PegboardUnits, PinKind, PinnedSignal, Surface, ToolId, ToolMeta,
    };

    const SEARCH_SURFACES: &[Surface] = &[Surface::Cli];
    const SEARCH_TAGS: &[&str] = &["runtime-search-wrapper-fixture"];

    fn local_id_for(id: &'static str, toolkit: &'static str) -> &'static str {
        ToolId::parse_canonical_in_toolkit(id, toolkit)
            .expect("test ToolMeta ids must be canonical")
            .local()
    }

    fn meta(id: &'static str, display_label: &'static str) -> ToolMeta {
        ToolMeta {
            id,
            toolkit: "test",
            local_id: local_id_for(id, "test"),
            tags: SEARCH_TAGS,
            display_label,
            description: "runtime search wrapper fixture",
            input_spec: InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            effect: upeg_core::ToolEffect::Unknown,
            presentation: None,
            source: upeg_core::Source::UserInput,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: SEARCH_SURFACES,
            boards: &[],
        }
    }

    #[test]
    fn search_toolbox_tools_uses_current_runtime_toolbox_and_signals() {
        let first_id = "test.runtime_search_first";
        let second_id = "test.runtime_search_second";
        let _first = toolbox_add_tool_managed(meta(first_id, "Runtime search first"));
        let _second = toolbox_add_tool_managed(meta(second_id, "Runtime search second"));

        let results = search_toolbox_tools(
            SearchQuery {
                needle: "runtime-search-wrapper-fixture",
                surface: Some(Surface::Cli),
                limit: Some(2),
            },
            SearchSignals {
                pinned: vec![
                    PinnedSignal {
                        tool_id: second_id.to_string(),
                        rank: 0,
                    },
                    PinnedSignal {
                        tool_id: first_id.to_string(),
                        rank: 1,
                    },
                ],
                recent: Vec::new(),
            },
        );

        let ids: Vec<_> = results.iter().map(|result| result.tool.id).collect();
        assert_eq!(ids, vec![second_id, first_id]);
    }
}
