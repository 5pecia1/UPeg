use super::common::parse;
use crate::*;

#[test]
fn tool_show_returns_an_error_for_an_unknown_id() {
    let result = run(parse(&["upeg", "tool", "show", "no.such.tool"]));
    assert_eq!(result, Err(CliError::UnknownTool("no.such.tool".into())));
}

#[test]
fn unknown_tool_error_suggests_a_similar_id() {
    // a typo like `num.hex_to_decimai` should produce a "did
    // you mean num.hex_to_decimal?" hint via Levenshtein. The error
    // type itself is unchanged; the message rendering enriches it.
    let err = CliError::UnknownTool("num.hex_to_decimai".into());
    let msg = err.message();
    assert!(
        msg.contains("did you mean"),
        "typo must trigger a suggestion; got {msg:?}"
    );
    assert!(
        msg.contains("num.hex_to_decimal"),
        "the close match should be suggested; got {msg:?}"
    );
}

#[test]
fn unknown_tool_message_omits_suggestions_for_distant_input() {
    // A wildly-different input shouldn't get noise. Threshold 4 means
    // a long random string finds no close-enough match → no hint.
    let err = CliError::UnknownTool("xqzaaapoiuyt.nonexistent_far_off".into());
    let msg = err.message();
    assert!(
        !msg.contains("did you mean"),
        "far-off input must not produce a suggestion; got {msg:?}"
    );
}

#[test]
fn unknown_tool_message_lists_similar_matches() {
    // For inputs that are equidistant from several real ids, list up
    // to 3 — sorted by distance then by id.
    let err = CliError::UnknownTool("hash.sha".into());
    let msg = err.message();
    // hash.sha1, hash.sha256, hash.sha512 are all 1-3 edits away.
    let suggestion_count = ["hash.sha1", "hash.sha256", "hash.sha512"]
        .iter()
        .filter(|id| msg.contains(*id))
        .count();
    assert!(
        suggestion_count >= 1,
        "near-matches in `hash.sha*` family must surface; got {msg:?}"
    );
}

#[test]
fn display_id_truncates_long_ids() {
    // Pin both behaviours: short id passes through,
    // long id gets truncated with marker + length suffix.
    assert_eq!(
        crate::display_id("num.hex_to_decimal"),
        "num.hex_to_decimal",
        "short id passes through unchanged"
    );
    assert_eq!(
        crate::display_id(""),
        "",
        "empty id passes through (boundary case)"
    );

    // 64 chars: at the limit, still passes through.
    let at_limit = "a".repeat(64);
    assert_eq!(
        crate::display_id(&at_limit),
        at_limit,
        "id at MAX boundary passes through"
    );

    // 65 chars: just past the limit. Truncation kicks in but the
    // marker + length suffix overhead makes the displayed string
    // briefly longer than the input. Acceptable — the win is for
    // pathological inputs (multi-KB), not for slightly-over cases.
    let just_over = "b".repeat(65);
    let displayed = crate::display_id(&just_over);
    assert!(
        displayed.contains("…"),
        "over-limit id must contain truncation marker; got `{displayed}`"
    );
    assert!(
        displayed.contains("65 chars total"),
        "over-limit id must report total length; got `{displayed}`"
    );

    // Pathological case: 10 KB id. Output stays bounded.
    let huge = "c".repeat(10_000);
    let displayed = crate::display_id(&huge);
    assert!(
        displayed.len() < 200,
        "10 KB input must produce bounded output; got {} chars",
        displayed.len()
    );
    assert!(
        displayed.contains("10000 chars total"),
        "huge input must still report the full length; got `{displayed}`"
    );
}

#[test]
fn display_id_handles_multibyte_characters() {
    // `display_id` is char-boundary-safe: `&id[..MAX]` byte-slicing
    // would panic when MAX (=64 bytes) falls in the middle of a
    // multibyte UTF-8 char. Worst case: id containing emojis (4 bytes
    // each), where any non-multiple-of-4 boundary panics.
    //
    // Crab emoji is 4 bytes; 17 of them = 68 bytes (just past MAX=64).
    // Byte 64 falls between emoji bytes 1 and 2 of the 17th crab.
    let crabs = "🦀".repeat(17);
    assert_eq!(crabs.len(), 68, "test fixture: 17 crabs = 68 bytes");
    // 17-crab input is 68 bytes (over byte threshold) but
    // only 17 chars (under char threshold). Only show the truncation
    // marker when actual truncation happened — 17 crabs passes through
    // verbatim.
    let displayed = crate::display_id(&crabs);
    assert!(
        !displayed.contains("…"),
        "17 chars (under MAX=64) must pass through without false truncation marker; got `{displayed}`"
    );
    assert_eq!(
        displayed, crabs,
        "17-char multibyte input must round-trip exactly"
    );

    // Also test a mix: ASCII + emoji at exactly the boundary.
    let mixed = format!("{}🦀", "a".repeat(63)); // 63 ASCII bytes + 4 emoji bytes = 67 bytes / 64 chars
    assert_eq!(mixed.len(), 67);
    let _ = crate::display_id(&mixed); // must not panic

    // actual multibyte truncation — 65 crabs (260 bytes /
    // 65 chars) DOES exceed both byte and char thresholds. Verify
    // the truncation marker fires and the count reflects char count.
    let many_crabs = "🦀".repeat(65);
    let displayed = crate::display_id(&many_crabs);
    assert!(
        displayed.contains("…"),
        "65 chars (over MAX=64) must show truncation marker; got `{displayed}`"
    );
    assert!(
        displayed.contains("65 chars total"),
        "suffix must report char count (65); got `{displayed}`"
    );
}

#[test]
fn truncate_for_display_accepts_arbitrary_character_limits() {
    // the helper is now parameterised over `max_chars` so
    // both display_id (MAX=64) and display_value (MAX=200) share one
    // implementation. Pin the arbitrary-max case with values neither
    // caller exercises so a future shape regression (e.g., off-by-one
    // in the gate, byte/char comparison flip) fails here.

    // max=10 with under-the-limit input → passes through.
    assert_eq!(crate::truncate_for_display("hello", 10), "hello");
    assert_eq!(
        crate::truncate_for_display("", 10),
        "",
        "empty input is valid, passes through"
    );

    // max=10 boundary (exactly 10 chars) → passes through.
    assert_eq!(
        crate::truncate_for_display("abcdefghij", 10),
        "abcdefghij",
        "input at max boundary must pass through (gate is `<=`)"
    );

    // max=10 just past boundary (11 chars) → truncated.
    let displayed = crate::truncate_for_display("abcdefghijk", 10);
    assert!(
        displayed.starts_with("abcdefghij"),
        "truncated output must start with the prefix; got `{displayed}`"
    );
    assert!(
        displayed.contains("…(truncated, 11 chars total)"),
        "truncated output must report char count; got `{displayed}`"
    );

    // max=0 (degenerate) — every non-empty string is truncated.
    let displayed = crate::truncate_for_display("a", 0);
    assert!(
        displayed.contains("…(truncated, 1 chars total)"),
        "max=0 must truncate any non-empty string; got `{displayed}`"
    );

    // max=0 with empty input → empty (passes through, gate is `<=`).
    assert_eq!(
        crate::truncate_for_display("", 0),
        "",
        "max=0 + empty input passes through (0 <= 0)"
    );

    // Multibyte parity: 5 emojis = 20 bytes / 5 chars. With max=10:
    //   - byte gate fires (20 > 10)
    //   - char gate skips (5 <= 10)
    //   → must pass through unchanged (multibyte safety).
    let emojis = "🦀".repeat(5);
    assert_eq!(
        crate::truncate_for_display(&emojis, 10),
        emojis,
        "multibyte input that fits in chars must pass through even when bytes exceed max"
    );
}

#[test]
fn tool_id_suggestions_enforce_the_query_length_limit() {
    // A megabyte-long query would trigger O(query_len × tool_id_len)
    // Levenshtein matrices for every registered tool — a DoS surface
    // on the unknown-tool hint path. Cap at MAX_QUERY_LEN (256 chars);
    // beyond that, return no suggestions silently. Pin the exact
    // boundary contract: "256 → process, 257 → skip".

    // Pathological case (10 KB) → empty.
    let huge = "x".repeat(10_000);
    assert!(
        crate::suggest_tool_ids(&huge, 3, None).is_empty(),
        "10000-char query must short-circuit to empty"
    );

    // Just past the cap (257 chars of "x") → empty due to cap.
    // 257 "x" wouldn't match anything substring-wise either, but the
    // cap MUST short-circuit so the levenshtein cost isn't paid.
    let just_over = "x".repeat(257);
    assert!(
        crate::suggest_tool_ids(&just_over, 3, None).is_empty(),
        "257-char query (just past cap) must short-circuit to empty"
    );

    // At cap (256 chars) → not skipped (cap is `>`, not `>=`).
    // Still likely empty since 256 x's match nothing, but the cap
    // doesn't fire — match_score gets called for each tool.
    let at_limit = "x".repeat(256);
    let _ = crate::suggest_tool_ids(&at_limit, 3, None); // must not panic, may return empty

    // Just under the cap → fully processes; if the query happens to
    // be a substring of a real tool id, we'd get a match. Use a
    // 250-char query that includes a real tool id substring.
    let containing_real = format!("{}num.hex_to_decimal{}", "z".repeat(116), "z".repeat(116));
    assert_eq!(
        containing_real.len(),
        250,
        "test fixture should be 250 bytes"
    );
    let suggestions = crate::suggest_tool_ids(&containing_real, 3, None);
    assert!(
        suggestions.iter().any(|s| s == "num.hex_to_decimal"),
        "250-char query containing `num.hex_to_decimal` substring must surface it; got {suggestions:?}"
    );
}

#[test]
fn tool_id_suggestions_return_empty_when_nothing_matches() {
    // Direct test of the helper — far input means empty Vec.
    let suggestions = crate::suggest_tool_ids("zzzzzzzzz_no_match_zzzzzz", 3, None);
    assert!(
        suggestions.is_empty(),
        "no matches within threshold → empty Vec; got {suggestions:?}"
    );
}

#[test]
fn tool_id_suggestions_exclude_the_exact_query_id() {
    // A locally registered project-manifest tool can still return `NotFound`
    // when the CLI attaches to a host that never loaded it. The hint must not
    // suggest the same ID that the host just rejected.
    let exact = "num.hex_to_decimal";
    let suggestions = crate::suggest_tool_ids(exact, 5, None);
    assert!(
        !suggestions.iter().any(|s| s == exact),
        "the exact query id must be excluded from suggestions; got {suggestions:?}"
    );
    assert!(
        !crate::unknown_tool_hint(exact, None).contains(exact),
        "the hint string must not suggest the query id itself"
    );
}

#[test]
fn tool_id_suggestions_respect_the_maximum_result_count() {
    // A short query like "a" might match many short ids. The cap
    // prevents the error message from listing every tool. Pin the
    // cap at the API boundary.
    let suggestions = crate::suggest_tool_ids("a", 2, None);
    assert!(
        suggestions.len() <= 2,
        "max=2 must be honoured even with many candidates; got {suggestions:?}"
    );
}

#[test]
fn tool_id_suggestions_rank_a_unique_substring_match_ahead_of_levenshtein_matches() {
    // substring matches must surface before Levenshtein-only    // matches. Use a query unique to one tool id to make ranking
    // deterministic — `nanoid` is a substring of `id.nanoid` only.
    let suggestions = crate::suggest_tool_ids("nanoid", 5, None);
    assert!(!suggestions.is_empty(), "substring should produce ≥1 hit");
    // First match is the substring-containing id (tier 0).
    assert_eq!(
        suggestions[0], "id.nanoid",
        "id.nanoid should rank first as the unique substring match; got {suggestions:?}"
    );
}

#[test]
fn tool_id_suggestions_rank_all_substring_matches_ahead_of_levenshtein_only_matches() {
    // When multiple ids share the same substring (e.g., "hex_to"    // is in both `num.hex_to_decimal` and `color.hex_to_rgb`),
    // both should appear at tier 0; tier-0 entries must beat any
    // tier-1 (Levenshtein-only) candidates. Pin the property without
    // locking the order between the two co-tier-0 winners (their
    // Levenshtein distance to the query may not match user intent
    // in either direction).
    let suggestions = crate::suggest_tool_ids("hex_to", 5, None);
    let substring_hits: Vec<&str> = suggestions
        .iter()
        .filter(|s| s.contains("hex_to"))
        .map(std::string::String::as_str)
        .collect();
    // All substring matches come before any non-substring match.
    let first_non_substring = suggestions.iter().position(|s| !s.contains("hex_to"));
    if let Some(pos) = first_non_substring {
        assert!(
            pos >= substring_hits.len(),
            "substring-tier matches must precede tier-1 entries; got {suggestions:?}"
        );
    }
    assert!(
        substring_hits.contains(&"num.hex_to_decimal"),
        "num.hex_to_decimal must appear; got {suggestions:?}"
    );
    assert!(
        substring_hits.contains(&"color.hex_to_rgb"),
        "color.hex_to_rgb must appear; got {suggestions:?}"
    );
}

#[test]
fn tool_id_suggestions_return_empty_for_an_empty_query() {
    // empty query is meaningless — without an explicit guard the
    // tier-0 substring rule (`c.contains("")` is always true) would
    // make every candidate a tier-0 match. The guard returns no
    // suggestions.
    let suggestions = crate::suggest_tool_ids("", 3, None);
    assert!(
        suggestions.is_empty(),
        "empty query must produce no suggestions; got {suggestions:?}"
    );
}

#[test]
fn unknown_tool_message_has_no_hint_for_an_empty_id() {
    // Verify the hint suffix is suppressed for an empty tool id —    // otherwise the user would see misleading garbage like
    // "upeg: unknown tool `` — did you mean `hash.md5`, ...".
    let err = CliError::UnknownTool(String::new());
    let msg = err.message();
    assert!(
        !msg.contains("did you mean"),
        "empty tool id must not produce a suggestion suffix; got {msg:?}"
    );
}

#[test]
fn tool_id_suggestions_do_not_overmatch_short_candidates() {
    // The tier-1 threshold scales with candidate length so short
    // names (`id`, `xyz`) need closer matches. (A fixed threshold
    // of 4 would let a query like `encod` match the short Toolkit
    // tag `id`.)
    //
    // Register a short canonical tool id to exercise the short-candidate path.
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: "y.z",
        toolkit: "y",
        local_id: "z",
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    // `random_query_far_off` has Levenshtein distance > 2 from `y.z`.
    // length-aware threshold for a short candidate remains tight.
    let suggestions = crate::suggest_tool_ids("random_query_far_off", 10, None);
    assert!(
        !suggestions.iter().any(|s| s == "y.z"),
        "short-candidate `y.z` must not over-match a far query; got {suggestions:?}"
    );
}

#[test]
fn tool_id_suggestions_filter_candidates_by_surface() {
    // an HTTP caller suggesting a CLI-only tool would
    // mislead. Pin that the surface filter actually narrows the
    // candidate set. Register an off-surface tool and confirm it
    // doesn't appear in suggestions for a different surface.
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: "test.iter132.cli_only_pin",
        toolkit: "test",
        local_id: "iter132.cli_only_pin",
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        // Only on the CLI surface.
        surfaces: &[upeg_core::Surface::Cli],
        boards: &[],
    });

    // Querying with surface=Http: must NOT find the CLI-only tool.
    let http_suggestions = crate::suggest_tool_ids(
        "test.iter132.cli_only_widge",
        5,
        Some(upeg_core::Surface::Http),
    );
    assert!(
        !http_suggestions
            .iter()
            .any(|s| s == "test.iter132.cli_only_pin"),
        "CLI-only tool must NOT surface as a suggestion to HTTP caller; got {http_suggestions:?}"
    );

    // Querying with surface=Cli: SHOULD find it.
    let cli_suggestions = crate::suggest_tool_ids(
        "test.iter132.cli_only_widge",
        5,
        Some(upeg_core::Surface::Cli),
    );
    assert!(
        cli_suggestions
            .iter()
            .any(|s| s == "test.iter132.cli_only_pin"),
        "CLI suggestions must include CLI-surface tools; got {cli_suggestions:?}"
    );
}
