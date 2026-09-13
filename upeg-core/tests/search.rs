#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::print_stdout,
    clippy::string_add,
    clippy::manual_let_else,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

use std::collections::HashSet;

use upeg_core::{
    ALL_SURFACES, InputSpec, Invoker, PegboardUnits, PinKind, PinnedSignal, Placement,
    RecentSignal, SearchField, SearchMatchTier, SearchQuery, SearchSignals, Surface, ToolMeta,
    search_tools,
};

fn test_tool(
    id: &'static str,
    toolkit: &'static str,
    local_id: &'static str,
    display_label: &'static str,
    tags: &'static [&'static str],
    surfaces: &'static [Surface],
) -> ToolMeta {
    ToolMeta {
        id,
        toolkit,
        local_id,
        tags,
        display_label,
        description: "description is intentionally not searched",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces,
        boards: &[],
    }
}

fn refs(tools: &[ToolMeta]) -> Vec<&ToolMeta> {
    tools.iter().collect()
}

fn query(needle: &str) -> SearchQuery<'_> {
    SearchQuery {
        needle,
        surface: None,
        limit: None,
    }
}

fn result_ids(results: &[upeg_core::SearchResult<'_>]) -> Vec<&'static str> {
    results.iter().map(|result| result.tool.id).collect()
}

#[test]
fn search_빈_query는_scope_안의_모든_도구를_포함한다() {
    let tools = vec![
        test_tool("alpha.one", "alpha", "one", "Alpha One", &[], ALL_SURFACES),
        test_tool("beta.two", "beta", "two", "Beta Two", &[], ALL_SURFACES),
    ];
    let tool_refs = refs(&tools);

    let results = search_tools(&tool_refs, query("   "), SearchSignals::default());

    assert_eq!(result_ids(&results), vec!["alpha.one", "beta.two"]);
    assert!(
        results
            .iter()
            .all(|result| result.matched_fields.is_empty())
    );
    assert!(
        results
            .iter()
            .all(|result| result.match_tier == SearchMatchTier::EmptyQuery)
    );
}

#[test]
fn search_query는_trim_후_unicode_case_insensitive로_비교한다() {
    let tools = vec![test_tool(
        "unicode.cafe",
        "unicode",
        "cafe",
        "Café Converter",
        &[],
        ALL_SURFACES,
    )];
    let tool_refs = refs(&tools);

    let results = search_tools(&tool_refs, query("  café  "), SearchSignals::default());

    assert_eq!(result_ids(&results), vec!["unicode.cafe"]);
    assert_eq!(results[0].match_tier, SearchMatchTier::Prefix);
}

#[test]
fn search_surface_filter는_검색_전에_적용된다() {
    let tools = vec![
        test_tool(
            "desktop.only",
            "desktop",
            "only",
            "Shared Label",
            &[],
            &[Surface::Desktop],
        ),
        test_tool(
            "cli.only",
            "cli",
            "only",
            "Shared Label",
            &[],
            &[Surface::Cli],
        ),
    ];
    let tool_refs = refs(&tools);

    let results = search_tools(
        &tool_refs,
        SearchQuery {
            needle: "shared",
            surface: Some(Surface::Desktop),
            limit: None,
        },
        SearchSignals::default(),
    );

    assert_eq!(result_ids(&results), vec!["desktop.only"]);
}

#[test]
fn search는_허용된_필드만_매칭하고_필드는_중복제거된다() {
    let tools = vec![
        test_tool(
            "alpha.alpha",
            "alpha",
            "alpha",
            "Alpha",
            &["alpha", "alpha-extra"],
            ALL_SURFACES,
        ),
        test_tool(
            "description.only",
            "description",
            "only",
            "Other",
            &[],
            ALL_SURFACES,
        ),
    ];
    let tool_refs = refs(&tools);

    let results = search_tools(&tool_refs, query("alpha"), SearchSignals::default());

    assert_eq!(result_ids(&results), vec!["alpha.alpha"]);
    assert_eq!(
        results[0].matched_fields,
        HashSet::from([
            SearchField::DisplayLabel,
            SearchField::Id,
            SearchField::LocalId,
            SearchField::Toolkit,
            SearchField::Tags,
        ])
    );
    assert_eq!(results[0].match_tier, SearchMatchTier::Exact);
}

#[test]
fn search_non_empty_query는_match가_없으면_빈_결과를_반환한다() {
    let tools = vec![test_tool(
        "alpha.one",
        "alpha",
        "one",
        "Alpha One",
        &["text"],
        ALL_SURFACES,
    )];
    let tool_refs = refs(&tools);

    let results = search_tools(&tool_refs, query("missing"), SearchSignals::default());

    assert!(results.is_empty());
}

#[test]
fn search_pinned와_recent는_정해진_순서로_랭킹된다() {
    let tools = vec![
        test_tool("plain.tool", "plain", "tool", "Common", &[], ALL_SURFACES),
        test_tool("recent.tool", "recent", "tool", "Common", &[], ALL_SURFACES),
        test_tool("pinned.two", "pinned", "two", "Common", &[], ALL_SURFACES),
        test_tool("pinned.one", "pinned", "one", "Common", &[], ALL_SURFACES),
    ];
    let tool_refs = refs(&tools);

    let results = search_tools(
        &tool_refs,
        query("common"),
        SearchSignals {
            pinned: vec![
                PinnedSignal {
                    tool_id: "pinned.two".to_string(),
                    rank: 2,
                },
                PinnedSignal {
                    tool_id: "pinned.one".to_string(),
                    rank: 1,
                },
            ],
            recent: vec![RecentSignal {
                tool_id: "recent.tool".to_string(),
                rank: 0,
            }],
        },
    );

    assert_eq!(
        result_ids(&results),
        vec!["pinned.one", "pinned.two", "recent.tool", "plain.tool"]
    );
}

#[test]
fn search_pinned_placements는_row_major와_중복제거를_적용한다() {
    let signals = SearchSignals::from_pinned_placements(&[
        Placement::new("beta.tool", 1, 0),
        Placement::new("duplicate.tool", 3, 1),
        Placement::new("alpha.tool", 1, 0),
        Placement::new("duplicate.tool", 0, 0),
        Placement::new("gamma.tool", 0, 1),
    ]);

    assert_eq!(
        signals.pinned,
        vec![
            PinnedSignal {
                tool_id: "duplicate.tool".to_string(),
                rank: 0,
            },
            PinnedSignal {
                tool_id: "alpha.tool".to_string(),
                rank: 1,
            },
            PinnedSignal {
                tool_id: "beta.tool".to_string(),
                rank: 2,
            },
            PinnedSignal {
                tool_id: "gamma.tool".to_string(),
                rank: 3,
            },
        ]
    );
    assert!(signals.recent.is_empty());
}

#[test]
fn search_pinned와_recent_중복_signal과_도구_중복은_하나의_결과로_합쳐진다() {
    let duplicated = test_tool(
        "duplicate.tool",
        "duplicate",
        "tool",
        "Common",
        &[],
        ALL_SURFACES,
    );
    let other = test_tool("other.tool", "other", "tool", "Common", &[], ALL_SURFACES);
    let tool_refs = vec![&other, &duplicated, &duplicated];

    let results = search_tools(
        &tool_refs,
        query("common"),
        SearchSignals {
            pinned: vec![
                PinnedSignal {
                    tool_id: "duplicate.tool".to_string(),
                    rank: 5,
                },
                PinnedSignal {
                    tool_id: "duplicate.tool".to_string(),
                    rank: 0,
                },
            ],
            recent: vec![RecentSignal {
                tool_id: "duplicate.tool".to_string(),
                rank: 0,
            }],
        },
    );

    assert_eq!(result_ids(&results), vec!["duplicate.tool", "other.tool"]);
}

#[test]
fn search_limit는_sort_후에_적용되고_tool_id가_tie_breaker이다() {
    let tools = vec![
        test_tool("zeta.tool", "zeta", "tool", "Tool", &[], ALL_SURFACES),
        test_tool("alpha.tool", "alpha", "tool", "Tool", &[], ALL_SURFACES),
        test_tool("beta.tool", "beta", "tool", "Tool", &[], ALL_SURFACES),
    ];
    let tool_refs = refs(&tools);

    let results = search_tools(
        &tool_refs,
        SearchQuery {
            needle: "tool",
            surface: None,
            limit: Some(2),
        },
        SearchSignals::default(),
    );

    assert_eq!(result_ids(&results), vec!["alpha.tool", "beta.tool"]);
}

#[test]
fn search_match_tier는_exact_prefix_substring_순서로_정렬된다() {
    let tools = vec![
        test_tool(
            "rank.substring",
            "rank",
            "substring",
            "Best Tool",
            &[],
            ALL_SURFACES,
        ),
        test_tool(
            "rank.prefix",
            "rank",
            "prefix",
            "Toolbox",
            &[],
            ALL_SURFACES,
        ),
        test_tool("rank.exact", "rank", "exact", "Tool", &[], ALL_SURFACES),
    ];
    let tool_refs = refs(&tools);

    let results = search_tools(&tool_refs, query("tool"), SearchSignals::default());

    assert_eq!(
        result_ids(&results),
        vec!["rank.exact", "rank.prefix", "rank.substring"]
    );
    assert_eq!(results[0].match_tier, SearchMatchTier::Exact);
    assert_eq!(results[1].match_tier, SearchMatchTier::Prefix);
    assert_eq!(results[2].match_tier, SearchMatchTier::Substring);
}
