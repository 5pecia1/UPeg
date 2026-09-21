use super::common::parse;
use crate::*;

// ─── --board filter + text.slugify ──────────────

#[test]
fn tool_list_board_filter_keeps_only_matching_tools() {
    // Built-ins like `convert.base64_encode` declare
    // `boards = ["dev"]`; nothing pins to a made-up "iter56_zzz".
    let dev_view = run(parse(&["upeg", "tool", "list", "--board", "dev"])).unwrap();
    // There IS at least one tool pinned to "dev" — base64_encode .
    assert!(
        dev_view.contains("convert.base64_encode"),
        "expected `convert.base64_encode` (pinned to dev), got:\n{dev_view}"
    );
    // hash.sha256 has no boards in its #[tool] annotation.
    assert!(!dev_view.contains("hash.sha256\t"));

    let no_match = run(parse(&[
        "upeg",
        "tool",
        "list",
        "--board",
        "no_such_board_iter56_zzz",
    ]))
    .unwrap();
    assert!(
        no_match.is_empty(),
        "unknown board should yield empty: {no_match:?}"
    );
}

#[test]
fn tool_list_combines_tag_and_board_filters() {
    // --tag AND --board compose. `convert.base64_encode` inherits the
    // `convert` Toolkit tag and is pinned to "dev" — it should appear under both.
    let combo = run(parse(&[
        "upeg", "tool", "list", "--tag", "convert", "--board", "dev",
    ]))
    .unwrap();
    assert!(combo.contains("convert.base64_encode"));
    for line in combo.lines() {
        let toolkit = line.split('\t').nth(1).unwrap_or("");
        assert_eq!(toolkit, "convert", "non-convert leaked: {line}");
    }
}

#[test]
fn text_slugify_subcommand_handles_ascii_and_unicode_input() {
    let s = run(parse(&["upeg", "text", "slugify", "Hello, World!"])).unwrap();
    assert_eq!(s, "hello-world\n");
    // Non-ASCII drops gracefully (no transliteration; it's documented).
    let s2 = run(parse(&["upeg", "text", "slugify", "한글 hello"])).unwrap();
    assert_eq!(s2, "hello\n");
}

#[test]
fn slugify_is_listed_under_the_text_tag_and_dispatches_through_call() {
    let listed = run(parse(&["upeg", "tool", "list", "--tag", "text"])).unwrap();
    assert!(listed.contains("text.slugify"));
    let direct = run(parse(&["upeg", "text", "slugify", "Foo Bar"])).unwrap();
    let via_call = run(parse(&[
        "upeg",
        "call",
        "text.slugify",
        "-a",
        "input=Foo Bar",
    ]))
    .unwrap();
    assert_eq!(direct, via_call);
}

// ─── --pin filter + embed_url on tool show ──────

#[test]
fn tool_list_pin_filter_keeps_only_matching_kinds() {
    // Register a Launcher-only tool to verify negative filtering.
    let id = "test.iter91.launcher_only";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Launcher,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });

    // --pin Inline: a registered Launcher must NOT appear.
    let inline_view = run(parse(&["upeg", "tool", "list", "--pin", "Inline"])).unwrap();
    assert!(
        !inline_view.contains(id),
        "Launcher-only tool leaked into --pin Inline view"
    );
    // Most built-ins are Inline — at least one must surface.
    assert!(
        inline_view.contains("num.hex_to_decimal"),
        "expected `num.hex_to_decimal` in --pin Inline list"
    );

    // --pin Launcher: the registered tool MUST appear.
    let launcher_view = run(parse(&["upeg", "tool", "list", "--pin", "Launcher"])).unwrap();
    assert!(
        launcher_view.contains(id),
        "expected `{id}` in --pin Launcher list:\n{launcher_view}"
    );
}

#[test]
fn tool_show_rejects_padded_ids() {
    let text = run(parse(&["upeg", "tool", "show", " num.hex_to_decimal "]));
    assert!(
        matches!(text, Err(CliError::UnknownTool(_))),
        "padded id text-form must not resolve; got {text:?}"
    );

    let json = run(parse(&[
        "upeg",
        "tool",
        "show",
        "\tnum.hex_to_decimal\n",
        "--json",
    ]));
    assert!(
        matches!(json, Err(CliError::UnknownTool(_))),
        "padded id JSON-form must not resolve; got {json:?}"
    );
}

#[test]
fn call_rejects_padded_tool_ids() {
    let result = run(parse(&[
        "upeg",
        "call",
        " num.hex_to_decimal ",
        "-a",
        "input=0xff",
    ]));
    assert!(
        matches!(result, Err(CliError::UnknownTool(_))),
        "padded tool_id must not dispatch; got {result:?}"
    );

    let r = run(parse(&[
        "upeg",
        "call",
        "convert . hex_to_decimal",
        "-a",
        "input=0xff",
    ]));
    assert!(
        matches!(r, Err(CliError::UnknownTool(_))),
        "internal whitespace must not be removed; got {r:?}"
    );
}

#[test]
fn tool_list_trims_whitespace_in_surface_and_pin_filters() {
    // parallel to upeg-loader / upeg-wasm whitespace trimming.
    // Shell paste with trailing space (`--surface "cli "`) must not
    // produce an "unknown surface" error; same for `--pin "Inline "`.
    // Trim forgivingly. Pin both filters work for leading and
    // trailing whitespace.

    // Surface filter — trimmed values should validate cleanly.
    for s in ["cli ", " mcp", "http\t"] {
        let r = run(parse(&["upeg", "tool", "list", "--surface", s]));
        assert!(
            r.is_ok(),
            "--surface `{s}`: trimmed value should validate; got {r:?}"
        );
    }

    // Pin filter — trimmed values should both validate AND match
    // tools (Inline is the most common pin kind, hex_to_decimal uses it).
    let out = run(parse(&["upeg", "tool", "list", "--pin", "Inline "]))
        .expect("trimmed pin filter should validate");
    assert!(
        out.contains("num.hex_to_decimal"),
        "trimmed pin filter should match the same tools as the canonical form; got:\n{out}"
    );
}

#[test]
fn tool_list_trims_whitespace_in_tag_and_board_filters() {
    // parallel to the pin-filter trim. A padded `--tag " convert"`
    // (shell paste) must trim before comparison against registered
    // Toolkit tags; loader-side trim guarantees no registered tag carries
    // whitespace, so trimming the filter is symmetric and safe.
    //
    // Tag filter — trimmed value should match the same tools as the
    // canonical form.
    let canonical = run(parse(&["upeg", "tool", "list", "--tag", "convert"]))
        .expect("canonical --tag should work");
    for padded in [" convert", "convert ", "  convert  ", "\tconvert\n"] {
        let out = run(parse(&["upeg", "tool", "list", "--tag", padded]))
            .expect("trimmed --tag should match");
        assert_eq!(
            out, canonical,
            "--tag `{padded:?}` must produce the same listing as --tag `convert`"
        );
    }

    // Pinned filter — same contract. `dev` is the most-used board key
    // (every convert tool pins to it).
    let canonical = run(parse(&["upeg", "tool", "list", "--board", "dev"]))
        .expect("canonical --board should work");
    for padded in [" dev", "dev ", "\tdev\t"] {
        let out = run(parse(&["upeg", "tool", "list", "--board", padded]))
            .expect("trimmed --board should match");
        assert_eq!(
            out, canonical,
            "--board `{padded:?}` must produce the same listing as --board `dev`"
        );
    }
}

#[test]
fn tool_list_pin_filter_returns_a_clear_error_for_unknown_kinds() {
    let r = run(parse(&["upeg", "tool", "list", "--pin", "Mauve"]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(msg.contains("unknown pin kind"));
            assert!(msg.contains("Mauve"));
        }
        other => panic!("expected ToolFailed, got {other:?}"),
    }
}

#[test]
fn tool_list_pin_filter_accepts_all_known_variants() {
    // cli pin validation now delegates to
    // `PinKind::parse`. Pin the round-trip — every label produced
    // by `PinKind::label()` must be accepted by `--pin`. If a
    // new variant is added to upeg-core but this test is not
    // refreshed, the test stays green (parse handles it
    // automatically). If cli is reverted to a hardcoded list that
    // drops a variant, the test fails on that variant immediately.
    // Same audit pattern as the centralised Surface parse — adding
    // a Surface ripples to every consumer.
    for kind in [
        upeg_core::PinKind::Inline,
        upeg_core::PinKind::Launcher,
        upeg_core::PinKind::Live,
        upeg_core::PinKind::Action,
        upeg_core::PinKind::Embed,
    ] {
        let label = kind.label();
        let r = run(parse(&["upeg", "tool", "list", "--pin", label]));
        // A successful filter call is Ok(_) regardless of how many
        // tools match — the contract is "valid pin label is
        // accepted", not "at least one tool exists for it".
        assert!(
            r.is_ok(),
            "PinKind::{kind:?} label `{label}` should be accepted by --pin; got {r:?}"
        );
    }
}

#[test]
fn tool_list_combines_pin_and_tag_filters() {
    // --pin AND --tag compose. convert.base64_encode is Inline plus the convert Toolkit tag.
    let combo = run(parse(&[
        "upeg", "tool", "list", "--tag", "convert", "--pin", "Inline",
    ]))
    .unwrap();
    assert!(combo.contains("convert.base64_encode"));
    for line in combo.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols[1], "convert", "non-convert leaked: {line}");
        assert_eq!(cols[2], "Inline", "non-Inline leaked: {line}");
    }
}

#[test]
fn tool_list_json_includes_the_embed_url_field() {
    // every JSON entry must have an `embedUrl` field (string
    // when registered, null otherwise) so consumers can rely on shape.
    let id = "test.iter91.embed_url_in_list";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Embed,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Static,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    upeg_runtime::register_embed_url(id, "https://example.com/iter91");

    let out = run(parse(&["upeg", "tool", "list", "--json", "--pin", "Embed"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let entry = v
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id)
        .expect("registered Embed tool missing from --json --pin Embed");
    assert_eq!(entry["embedUrl"], "https://example.com/iter91");
}

#[test]
fn tool_show_displays_a_registered_embed_url() {
    let id = "test.iter91.show_embed_url";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Embed,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Static,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    upeg_runtime::register_embed_url(id, "https://show.example/iter91");

    // Tabular shows on its own row.
    let out = run(parse(&["upeg", "tool", "show", id])).unwrap();
    assert!(
        out.contains("embed_url     https://show.example/iter91"),
        "tabular tool show must surface embed_url, got:\n{out}"
    );

    // JSON includes `embedUrl`.
    let out_json = run(parse(&["upeg", "tool", "show", id, "--json"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out_json).unwrap();
    assert_eq!(v["embedUrl"], "https://show.example/iter91");
}

#[test]
fn tool_show_omits_the_embed_url_row_when_none_is_registered() {
    // Tabular: no row. JSON: field is `null` (always present).
    let out = run(parse(&["upeg", "tool", "show", "num.hex_to_decimal"])).unwrap();
    assert!(
        !out.contains("embed_url"),
        "non-Embed tool should not show an embed_url row"
    );

    let out_json = run(parse(&[
        "upeg",
        "tool",
        "show",
        "num.hex_to_decimal",
        "--json",
    ]))
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out_json).unwrap();
    assert!(
        v["embedUrl"].is_null(),
        "JSON shape: embedUrl must be `null` for tools without a registered URL"
    );
}

// ─── surface selector_bindings via CLI ─────────────

#[test]
fn tool_show_lists_registered_selector_bindings() {
    let id = "test.iter93.show_bindings";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::ControlledEmbed,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Embed,
        surfaces: upeg_core::EMBED_SURFACES,
        boards: &[],
    });
    upeg_runtime::set_selector_bindings(
        id,
        vec![
            upeg_runtime::SelectorBinding {
                role: upeg_runtime::BindingRole::Input,
                field: "input".into(),
                selector: ".q".into(),
                trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
                wait: None,
            },
            upeg_runtime::SelectorBinding {
                role: upeg_runtime::BindingRole::Input,
                field: "output".into(),
                selector: "#r".into(),
                trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
                wait: None,
            },
        ],
    );

    let out = run(parse(&["upeg", "tool", "show", id])).unwrap();
    // Header line + indented binding rows.
    assert!(
        out.contains("selector_bindings (2)"),
        "tabular tool show must announce binding count, got:\n{out}"
    );
    assert!(out.contains("input"), "missing `input` binding row:\n{out}");
    assert!(out.contains(".q"), "missing `.q` selector row:\n{out}");
    assert!(
        out.contains("output"),
        "missing `output` binding row:\n{out}"
    );
    assert!(out.contains("#r"), "missing `#r` selector row:\n{out}");
}

#[test]
fn tool_show_json_includes_the_selector_bindings_array() {
    let id = "test.iter93.show_bindings_json";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::ControlledEmbed,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Embed,
        surfaces: upeg_core::EMBED_SURFACES,
        boards: &[],
    });
    upeg_runtime::set_selector_bindings(
        id,
        vec![upeg_runtime::SelectorBinding {
            role: upeg_runtime::BindingRole::Input,
            field: "submit_btn".into(),
            selector: "button[type=submit]".into(),
            trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
            wait: None,
        }],
    );

    let out = run(parse(&["upeg", "tool", "show", id, "--json"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let arr = v["selectorBindings"]
        .as_array()
        .expect("selectorBindings must be an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["field"], "submit_btn");
    assert_eq!(arr[0]["selector"], "button[type=submit]");
}

#[test]
fn tool_show_json_uses_an_empty_array_when_no_selector_bindings_exist() {
    // Shape stability: the field is always an array, never `null` or
    // missing. Empty array when none registered.
    let out = run(parse(&[
        "upeg",
        "tool",
        "show",
        "num.hex_to_decimal",
        "--json",
    ]))
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["selectorBindings"].is_array());
    assert_eq!(v["selectorBindings"].as_array().unwrap().len(), 0);
}

#[test]
fn tool_show_omits_the_selector_bindings_section_when_none_are_registered() {
    // Tabular: no rows. The header line shouldn't appear either —
    // empty registry means the section is fully suppressed.
    let out = run(parse(&["upeg", "tool", "show", "num.hex_to_decimal"])).unwrap();
    assert!(
        !out.contains("selector_bindings"),
        "non-Embed tool should not show a selector_bindings section"
    );
}

#[test]
fn tool_list_json_includes_the_selector_bindings_field() {
    // `tool list --json` carries the `selectorBindings` field on
    // every entry for shape parity with `tool show --json`. The
    // CLI surface only lists tools exposed on CLI, so a built-in
    // (which has no selector bindings) is sufficient to pin the
    // empty-array shape contract. ControlledEmbed-with-bindings
    // testing lives in `tool_show_json_includes_the_selector_bindings_array`,
    // which uses `tool show` (no surface filter).
    let out = run(parse(&["upeg", "tool", "list", "--json"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let arr = v.as_array().expect("tool list --json must be an array");
    assert!(!arr.is_empty(), "expected at least one CLI-exposed tool");
    // Shape contract: every entry has a `selectorBindings` array, even
    // when empty. Content is exercised in `tool show` tests instead;
    // CLI surface filters out Controlled Embed (GUI-only), so list
    // entries are limited to passive/non-embed tools.
    for entry in arr {
        assert!(
            entry["selectorBindings"].is_array(),
            "selectorBindings must be an array on every entry, got {:?} on `{}`",
            entry["selectorBindings"],
            entry["id"]
        );
    }
}

// ─── color tools ────────────────────────────────────

#[test]
fn color_subcommands_round_trip_between_hex_and_rgb() {
    let rgb = run(parse(&["upeg", "color", "hex-to-rgb", "#ff8800"])).unwrap();
    assert_eq!(rgb, "255,136,0\n");
    let hex = run(parse(&["upeg", "color", "rgb-to-hex", "255,136,0"])).unwrap();
    assert_eq!(hex, "#ff8800\n");
}

#[test]
fn color_subcommands_propagate_errors_to_cli() {
    let r = run(parse(&["upeg", "color", "hex-to-rgb", "#zz0000"]));
    assert!(matches!(r, Err(CliError::ToolFailed(_))));
    let r2 = run(parse(&["upeg", "color", "rgb-to-hex", "256,0,0"]));
    assert!(matches!(r2, Err(CliError::ToolFailed(_))));
}

#[test]
fn color_conversion_tools_appear_in_the_color_tag_list_and_dispatch() {
    let listed = run(parse(&["upeg", "tool", "list", "--tag", "color"])).unwrap();
    for expected in ["color.hex_to_rgb", "color.rgb_to_hex"] {
        assert!(
            listed.contains(expected),
            "missing `{expected}` in:\n{listed}"
        );
    }
    // `upeg call` path must agree with direct subcommand.
    let direct = run(parse(&["upeg", "color", "hex-to-rgb", "#ff8800"])).unwrap();
    let via_call = run(parse(&[
        "upeg",
        "call",
        "color.hex_to_rgb",
        "-a",
        "input=#ff8800",
    ]))
    .unwrap();
    assert_eq!(direct, via_call);
}

// ─── url/csv tools ──────────────────────────────────────────

#[test]
fn text_count_subcommands_match_call_dispatch_byte_for_byte() {
    // Direct subcommand and `upeg call` must produce identical bytes —
    // proves the runtime dispatcher and the hardcoded subcommand path
    // are in sync. (Same invariant as the hash.md5 parity test.)
    for (sub, id) in [
        (vec!["text", "word-count", "hello world"], "text.word_count"),
        (vec!["text", "char-count", "한글"], "text.char_count"),
        (vec!["text", "line-count", "a\nb\nc"], "text.line_count"),
    ] {
        let direct_argv: Vec<&str> = std::iter::once("upeg").chain(sub.iter().copied()).collect();
        let direct = run(parse(&direct_argv)).unwrap();
        let arg_input = sub.last().unwrap().to_string();
        let via_call = run(parse(&[
            "upeg",
            "call",
            id,
            "-a",
            &format!("input={arg_input}"),
        ]))
        .unwrap();
        assert_eq!(
            direct, via_call,
            "subcommand and call disagreed for `{id}`: direct={direct:?} call={via_call:?}"
        );
    }
}

#[test]
fn text_counting_tools_appear_in_the_text_tag_list() {
    let listed = run(parse(&["upeg", "tool", "list", "--tag", "text"])).unwrap();
    for expected in ["text.word_count", "text.char_count", "text.line_count"] {
        assert!(
            listed.contains(expected),
            "missing `{expected}` in:\n{listed}"
        );
    }
}

// ─── hash tools ──────────────────────────────────────────

#[test]
fn hash_md5_subcommand_matches_a_known_vector() {
    let out = run(parse(&["upeg", "hash", "md5", "abc"])).unwrap();
    assert_eq!(out, "900150983cd24fb0d6963f7d28e17f72\n");
}

#[test]
fn convert_base32_round_trips_through_subcommands() {
    let enc = run(parse(&["upeg", "convert", "base32-encode", "foo"])).unwrap();
    assert_eq!(enc, "MZXW6===\n");
    let dec = run(parse(&["upeg", "convert", "base32-decode", "MZXW6==="])).unwrap();
    assert_eq!(dec, "foo\n");
}

#[test]
fn convert_base32_decode_returns_tool_failure_for_invalid_input() {
    let r = run(parse(&["upeg", "convert", "base32-decode", "not-valid!"]));
    assert!(matches!(r, Err(CliError::ToolFailed(_))));
}

// ─── completion + tool list filter ───────────────

// ─── upeg doctor  ────────────────────────────────

#[test]
fn doctor_prints_the_required_sections_and_counts() {
    let out = run(parse(&["upeg", "doctor"])).expect("doctor");
    // Section headers users rely on.
    assert!(out.contains("=== upeg doctor ==="));
    assert!(out.contains("binary:"));
    assert!(out.contains("version:"));
    assert!(out.contains("features:"));
    assert!(out.contains("runtime sources:"));
    assert!(out.contains("toolbox:"));

    // The version line must exactly match the crate version.
    let v = env!("CARGO_PKG_VERSION");
    assert!(
        out.contains(&format!("version:  {v}")),
        "expected `version:  {v}` line, got:\n{out}"
    );

    // All three runtime-source dirs are listed by label.
    for label in ["toolkits dir", "wasm dir", "mcp-imports dir"] {
        assert!(out.contains(label), "missing `{label}` row:\n{out}");
    }

    // Toolbox counts must be present and add up.
    let total_line = out.lines().find(|l| l.contains("total:")).unwrap();
    let total: usize = total_line
        .split_whitespace()
        .last()
        .unwrap()
        .parse()
        .unwrap();
    let builtins_line = out.lines().find(|l| l.contains("built-ins:")).unwrap();
    let builtins: usize = builtins_line
        .split_whitespace()
        .last()
        .unwrap()
        .parse()
        .unwrap();
    let runtime_line = out.lines().find(|l| l.contains("runtime:")).unwrap();
    let runtime: usize = runtime_line
        .split_whitespace()
        .last()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        builtins + runtime,
        total,
        "built-ins ({builtins}) + runtime ({runtime}) must equal total ({total})"
    );
    assert!(
        builtins >= 20,
        "expected at least 20 built-ins, got {builtins}"
    );
}

#[test]
fn doctor_json_emits_valid_json_with_the_required_fields() {
    let out = run(parse(&["upeg", "doctor", "--json"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("--json must emit valid JSON: {e}\n{out}"));
    for required in ["binary", "version", "features", "runtimeSources", "toolbox"] {
        assert!(v.get(required).is_some(), "missing `{required}`: {v}");
    }
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    // runtimeSources is an array of 3 entries.
    let sources = v["runtimeSources"].as_array().unwrap();
    assert_eq!(sources.len(), 3);
    for src in sources {
        for k in ["label", "path", "present", "count"] {
            assert!(src.get(k).is_some(), "source entry missing `{k}`: {src}");
        }
    }
    // Same arithmetic invariant as the human-readable form.
    let r = &v["toolbox"];
    assert_eq!(
        r["builtins"].as_u64().unwrap() + r["runtime"].as_u64().unwrap(),
        r["total"].as_u64().unwrap(),
    );
}

#[test]
fn doctor_json_shapes_the_surface_diagnosis_section() {
    let out = run(parse(&["upeg", "doctor", "--json"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let diagnosis = v
        .get("surfaceDiagnosis")
        .expect("doctor --json must include `surfaceDiagnosis`");

    let by_surface = diagnosis["bySurface"]
        .as_array()
        .expect("`bySurface` must be an array");
    assert_eq!(
        by_surface.len(),
        7,
        "one row per PRD surface (cli/tui/desktop/pwa/ext/mcp/http); got {by_surface:?}"
    );
    let surfaces: std::collections::HashSet<&str> = by_surface
        .iter()
        .map(|row| row["surface"].as_str().unwrap())
        .collect();
    for expected in ["cli", "tui", "desktop", "pwa", "ext", "mcp", "http"] {
        assert!(
            surfaces.contains(expected),
            "missing surface `{expected}` row: {by_surface:?}"
        );
    }
    for row in by_surface {
        for k in ["surface", "visible", "hidden"] {
            assert!(row.get(k).is_some(), "row missing `{k}`: {row}");
        }
    }

    let why = &diagnosis["whyClasses"];
    assert!(why["wasmFeatureStubbed"].is_u64());
    assert!(why["nativeOnlyExcluded"].is_u64());
    // This binary is always compiled native — the wasm32-exclusion class
    // is structurally zero here (see `doctor_surfaces` module doc).
    assert_eq!(why["nativeOnlyExcluded"], 0);
}

#[test]
fn doctor_text_includes_the_surface_diagnosis_section() {
    let out = run(parse(&["upeg", "doctor"])).unwrap();
    assert!(out.contains("surface diagnosis:"), "got:\n{out}");
    for label in ["cli", "tui", "desktop", "pwa", "ext", "mcp", "http"] {
        assert!(
            out.contains(&format!("  {label}")),
            "missing `{label}` row in surface diagnosis:\n{out}"
        );
    }
    assert!(out.contains("wasm-plugin feature:"), "got:\n{out}");
    assert!(out.contains("native-only"), "got:\n{out}");
}

#[cfg(feature = "wasm-plugin")]
#[test]
fn doctor_json_reports_the_wasm_plugin_feature_as_true() {
    let out = run(parse(&["upeg", "doctor", "--json"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["features"]["wasm-plugin"], true);
}

#[cfg(not(feature = "wasm-plugin"))]
#[test]
fn doctor_json_reports_the_wasm_plugin_feature_as_false() {
    let out = run(parse(&["upeg", "doctor", "--json"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["features"]["wasm-plugin"], false);
}

#[cfg(feature = "wasm-plugin")]
#[test]
fn doctor_reports_wasm_plugin_enabled_when_the_feature_is_built() {
    let out = run(parse(&["upeg", "doctor"])).unwrap();
    assert!(
        out.contains("wasm-plugin (enabled)"),
        "expected `wasm-plugin (enabled)`, got:\n{out}"
    );
}

#[cfg(not(feature = "wasm-plugin"))]
#[test]
fn doctor_reports_wasm_plugin_disabled_when_the_feature_is_not_built() {
    let out = run(parse(&["upeg", "doctor"])).unwrap();
    assert!(
        out.contains("wasm-plugin (disabled"),
        "expected `wasm-plugin (disabled`, got:\n{out}"
    );
}

#[test]
fn completion_bash_emits_a_non_empty_script() {
    let out = run(parse(&["upeg", "completion", "bash"])).unwrap();
    assert!(!out.is_empty());
    // `complete -F` is the bash completion declaration line clap_complete emits.
    assert!(
        out.contains("complete -F"),
        "expected bash complete declaration"
    );
    // Script should reference the binary name.
    assert!(
        out.contains("upeg"),
        "completion script should mention `upeg`"
    );
}

#[test]
fn completion_zsh_emits_a_non_empty_script() {
    let out = run(parse(&["upeg", "completion", "zsh"])).unwrap();
    assert!(!out.is_empty());
    // zsh completion uses #compdef.
    assert!(out.contains("#compdef"), "expected zsh #compdef directive");
}

#[test]
fn completion_fish_emits_a_non_empty_script() {
    let out = run(parse(&["upeg", "completion", "fish"])).unwrap();
    assert!(!out.is_empty());
    // fish completion uses `complete -c <bin>`.
    assert!(out.contains("complete -c upeg"));
}

#[test]
fn md5_and_base32_tools_support_tool_list_and_call_dispatch() {
    let listed = run(parse(&["upeg", "tool", "list"])).unwrap();
    for expected in ["hash.md5", "convert.base32_encode", "convert.base32_decode"] {
        assert!(
            listed.contains(expected),
            "missing `{expected}` in:\n{listed}"
        );
    }
    // Dispatch path for md5 (proves the runtime dispatcher matches the
    // direct subcommand byte-for-byte).
    let direct = run(parse(&["upeg", "hash", "md5", "abc"])).unwrap();
    let via_call = run(parse(&["upeg", "call", "hash.md5", "-a", "input=abc"])).unwrap();
    assert_eq!(direct, via_call);
}

#[test]
fn new_tools_are_reachable_through_call_dispatch() {
    // `upeg call` exercises the runtime dispatcher path — must agree
    // with the hardcoded `upeg text lowercase ...` route.
    let lc_call = run(parse(&[
        "upeg",
        "call",
        "text.lowercase",
        "-a",
        "input=Foo",
    ]))
    .unwrap();
    assert_eq!(lc_call, "foo\n");

    let sha = run(parse(&["upeg", "call", "hash.sha256", "-a", "input=abc"])).unwrap();
    assert!(sha.starts_with("ba7816bf"));
}

// With the daemon and the `--daemon` flag removed (PRD §11), the
// former `call_via_daemon_skips_local_cli_surface_check` test no
// longer has anything to assert. Negative-parse coverage lives in
// tests_b.rs::call_no_longer_accepts_daemon_flag.
