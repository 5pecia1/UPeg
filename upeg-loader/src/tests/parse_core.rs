use super::{
    parse_full_single_tool as parse_full_fixture_tool, parse_single_tool as parse_fixture_tool,
};
use crate::{LoadError, parse_toolkit_full};
use upeg_core::{InputKind, Invoker, OutputKind, PinKind, Surface};

#[test]
fn flat_tool_toml_is_not_accepted_as_v2_1_manifest() {
    let flat = r#"
            id = "test.minimal"
            toolkit = "test"
        "#;
    match parse_toolkit_full(flat) {
        Err(LoadError::Toml(error)) => {
            let message = error.to_string();
            assert!(message.contains("unknown field"), "{message}");
            assert!(message.contains("toolkit"), "{message}");
        }
        other => panic!("flat tool TOML must be rejected as unknown root fields, got {other:?}"),
    }
}

#[test]
fn v2_1_manifest_merges_toolkit_and_tool_tags_deduped() {
    let s = r#"
            id = "tagkit"
            tags = ["pure", "text"]
            description = "Tag test toolkit"

            [[tools]]
            id = "echo"
            tags = ["text", "custom"]
            pegboard_units = "U1"
            invoker = "External"
            command = "echo"
        "#;
    let (toolkit, tools) = parse_toolkit_full(s).expect("parse toolkit");
    assert_eq!(toolkit.id, "tagkit");
    assert_eq!(toolkit.tags, &["pure", "text"]);
    assert_eq!(tools.len(), 1);
    let (meta, toml) = &tools[0];
    assert_eq!(meta.id, "tagkit.echo");
    assert_eq!(meta.tags, &["pure", "text", "custom"]);
    assert_eq!(
        toml.tags.as_deref(),
        Some(&["pure".to_string(), "text".to_string(), "custom".to_string()][..])
    );
}

#[test]
fn toolkit_manifest_allows_dotted_toolkit_and_local_names() {
    let s = r#"
            id = "github.com"
            description = "Dotted toolkit"

            [[tools]]
            id = "admin.tools.list"
            pegboard_units = "U1"
            invoker = "External"
            command = "echo"
    "#;
    let (toolkit, tools) = parse_toolkit_full(s).expect("parse dotted toolkit manifest");
    assert_eq!(toolkit.id, "github.com");
    assert_eq!(tools.len(), 1);
    let meta = &tools[0].0;
    assert_eq!(meta.id, "github.com.admin.tools.list");
    assert_eq!(meta.toolkit_id(), "github.com");
    assert_eq!(meta.tool_id(), "admin.tools.list");
}

#[test]
fn minimal_required_fields_parse() {
    let s = r#"
            id = "test.minimal"
            toolkit = "test"
            invoker = "External"
            command = "echo"
        "#;
    let m = parse_fixture_tool(s).expect("parse");
    assert_eq!(m.id, "test.minimal");
    assert_eq!(m.toolkit, "test");
    assert_eq!(m.pin, PinKind::Inline); // default
    assert_eq!(m.invoker, Invoker::External);
    assert_eq!(m.surfaces.len(), 7); // default = ALL
    assert!(m.boards.is_empty());
}

#[test]
fn full_shape_parses() {
    // Iter 242: External invoker now requires a `command` field —
    // included here so the test's intent (exercising the full
    // schema) survives the new validation.
    let s = r#"
            id = "convert.url_encode"
            toolkit = "convert"
            description = "URL-encode a string per RFC 3986."
            pin = "Launcher"
            invoker = "External"
            command = "echo"
            surfaces = ["cli", "tui", "http"]
            boards = ["dev", "trading"]
        "#;
    let m = parse_fixture_tool(s).expect("parse");
    assert_eq!(m.id, "convert.url_encode");
    assert_eq!(m.description, "URL-encode a string per RFC 3986.");
    assert_eq!(m.pin, PinKind::Launcher);
    assert_eq!(m.invoker, Invoker::External);
    assert_eq!(m.surfaces, &[Surface::Cli, Surface::Tui, Surface::Http]);
    assert_eq!(m.boards, &["dev", "trading"]);
}

#[test]
fn full_parse_returns_meta_and_declarative_fields() {
    let s = r#"
            id = "automation.demo"
            toolkit = "automation"
            invoker = "Chain"
            connections = [{ from = "first", to = "second" }]
            steps = [
                { id = "first", tool = "tool.first" },
                { id = "second", tool = "tool.second" },
            ]
        "#;
    let (meta, toml) = parse_full_fixture_tool(s).expect("parse_full");

    assert_eq!(meta.id, "automation.demo");
    assert_eq!(meta.toolkit, "automation");
    let step_tools: Vec<&str> = toml
        .steps
        .as_deref()
        .unwrap()
        .iter()
        .map(|step| step.tool.as_str())
        .collect();
    assert_eq!(step_tools, ["tool.first", "tool.second"]);
}

#[test]
fn minimal_description_defaults_to_empty() {
    let s = r#"
            id = "test.minimal_desc"
            toolkit = "test"
            invoker = "External"
            command = "echo"
        "#;
    let m = parse_fixture_tool(s).expect("parse");
    assert_eq!(m.description, "");
}

#[test]
fn parsing_rejects_unknown_pin_kind() {
    let s = r#"
            id = "y.x"
            toolkit = "y"
            pin = "Mauve"
            invoker = "External"
            command = "echo"
        "#;
    match parse_fixture_tool(s) {
        Err(LoadError::UnknownPinKind(k)) => assert_eq!(k, "Mauve"),
        other => panic!("expected UnknownPinKind, got {other:?}"),
    }
}

#[test]
fn parsing_rejects_unknown_invoker() {
    let s = r#"
            id = "y.x"
            toolkit = "y"
            invoker = "Telepathy"
        "#;
    assert!(matches!(
        parse_fixture_tool(s),
        Err(LoadError::UnknownInvoker(_))
    ));
}

#[test]
fn parsing_accepts_all_pin_kind_variants() {
    // Iter 188: companion to iter 187's invoker-variant coverage.
    // Pre-iter-188 the loader tests covered Inline (default),
    // Launcher (full-shape), Live (in a single mid-file test),
    // but never Action or Embed end-to-end through `parse_str`.
    // Embed flows via the examples test (examples/tools/embed-mdn.toml)
    // but Action had no production tool exercising it.
    for (toml_label, expected) in [
        ("Inline", PinKind::Inline),
        ("Launcher", PinKind::Launcher),
        ("Live", PinKind::Live),
        ("Action", PinKind::Action),
        ("Embed", PinKind::Embed),
        ("Chain", PinKind::Chain),
        ("Llm", PinKind::Llm),
    ] {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   pin = "{toml_label}"
                   invoker = "External"
                   command = "echo""#,
        );
        let m = parse_fixture_tool(&s).unwrap_or_else(|e| panic!("`{toml_label}`: {e}"));
        assert_eq!(
            m.pin, expected,
            "TOML `pin = \"{toml_label}\"` must parse to {expected:?}"
        );
    }
}

#[test]
fn parsing_accepts_all_surface_variants() {
    // Iter 188: end-to-end coverage of every Surface label through
    // the loader. Pre-iter-188 only Cli/Tui/Http were explicitly
    // exercised. Surfaces flow through `parse_surface` (loader-local
    // wrapper around iter-137's `Surface::parse`); a future
    // regression that drops a variant from the loader's chain would
    // not be caught by upeg-core's helper test alone.
    for (toml_label, expected) in [
        ("cli", Surface::Cli),
        ("tui", Surface::Tui),
        ("desktop", Surface::Desktop),
        ("pwa", Surface::Pwa),
        ("ext", Surface::Ext),
        ("mcp", Surface::Mcp),
        ("http", Surface::Http),
    ] {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   surfaces = ["{toml_label}"]
                   invoker = "External"
                   command = "echo""#,
        );
        let m = parse_fixture_tool(&s).unwrap_or_else(|e| panic!("`{toml_label}`: {e}"));
        assert_eq!(
            m.surfaces,
            &[expected],
            "TOML `surfaces = [\"{toml_label}\"]` must parse to [{expected:?}]"
        );
    }
}

#[test]
fn parsing_accepts_all_runtime_invoker_variants() {
    // Iter 165 added `Invoker::parse` and the loader migrated to use
    // it. Runtime TOML can only declare executable dynamic adapters:
    // `Function` belongs to link-time `#[tool]` inventory and is covered by
    // the rejection test below.
    for (toml_label, expected, extras) in [
        ("External", Invoker::External, "\ncommand = \"echo\""),
        ("Http", Invoker::Http, "\nurl = \"https://example.test\""),
        ("Embed", Invoker::Embed, ""),
        ("Llm", Invoker::Llm, "\nprompt = \"Summarize {{input}}\""),
        ("Wasm", Invoker::Wasm, "\nwasm_path = \"plugin.wasm\""),
        (
            "Chain",
            Invoker::Chain,
            "\nsteps = [{ tool = \"text.uppercase\" }]",
        ),
    ] {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   invoker = "{toml_label}"{extras}"#,
        );
        let m = parse_fixture_tool(&s).unwrap_or_else(|e| panic!("`{toml_label}`: {e}"));
        assert_eq!(
            m.invoker, expected,
            "TOML `invoker = \"{toml_label}\"` must parse to {expected:?}"
        );
    }
}

#[test]
fn parsing_rejects_function_invoker_in_runtime_toml() {
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "Function""#;
    assert!(matches!(
        parse_fixture_tool(s),
        Err(LoadError::UnsupportedRuntimeInvoker {
            invoker: "Function"
        })
    ));
}

#[test]
fn parsing_rejects_unknown_surface() {
    let s = r#"
            id = "y.x"
            toolkit = "y"
            surfaces = ["cli", "fax"]
            invoker = "External"
            command = "echo"
        "#;
    match parse_fixture_tool(s) {
        Err(LoadError::UnknownSurface(s)) => assert_eq!(s, "fax"),
        other => panic!("expected UnknownSurface, got {other:?}"),
    }
}

#[test]
fn parsing_rejects_invalid_toml() {
    assert!(matches!(
        parse_fixture_tool("totally :: not :: toml"),
        Err(LoadError::Toml(_))
    ));
}

#[test]
fn parsing_errors_on_missing_required_id_field() {
    let s = r#"toolkit = "x""#;
    // serde's `id: String` (non-Option) requirement is enforced at parse time.
    assert!(matches!(parse_fixture_tool(s), Err(LoadError::Toml(_))));
}

#[test]
fn parsing_errors_on_missing_required_pegboard_units() {
    let s = r#"
            id = "test"

            [[tools]]
            id = "minimal"
            invoker = "External"
            command = "echo"
        "#;
    assert!(matches!(
        parse_toolkit_full(s),
        Err(LoadError::MissingPegboardUnits)
    ));
}

#[test]
fn parsing_rejects_empty_or_unknown_pegboard_units() {
    for (toml_value, expected) in [
        (r#"pegboard_units = """#, "EmptyPegboardUnits"),
        (r#"pegboard_units = "  ""#, "EmptyPegboardUnits"),
        (r#"pegboard_units = "U3""#, "UnknownPegboardUnits"),
    ] {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   {toml_value}
                   invoker = "External"
                   command = "echo""#,
        );
        match (parse_fixture_tool(&s), expected) {
            (Err(LoadError::EmptyPegboardUnits), "EmptyPegboardUnits") => {}
            (Err(LoadError::UnknownPegboardUnits(units)), "UnknownPegboardUnits") => {
                assert_eq!(units, "U3");
            }
            (got, _) => panic!("{toml_value}: expected {expected}, got {got:?}"),
        }
    }
}

#[test]
fn parsing_rejects_empty_chain() {
    let s = r#"id = "y.x"
            toolkit = "y"
            steps = []"#;
    match parse_fixture_tool(s) {
        Err(LoadError::EmptyChain) => {}
        other => panic!("expected EmptyChain, got {other:?}"),
    }
}

#[test]
fn parsing_rejects_empty_string_chain_step() {
    // Iter 195: pre-iter-195 the loader accepted `steps = [{ tool = "" }]`
    // (and whitespace-only steps). The dispatcher would then fail
    // at first call with `step ``: tool not found in registry` —
    // a confusing message that points at the (referenced) tool
    // rather than the (empty) step entry. Three forms covered:
    // first-position empty, mid-position empty, whitespace-only.
    for (toml_steps, expected_pos) in [
        (r#"[{ tool = "" }, { tool = "hash.md5" }]"#, 0),
        (
            r#"[{ tool = "hash.md5" }, { tool = "" }, { tool = "text.upper" }]"#,
            1,
        ),
        (r#"[{ tool = "hash.md5" }, { tool = "   " }]"#, 1), // whitespace-only counts as empty (trim)
    ] {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   steps = {toml_steps}"#,
        );
        match parse_fixture_tool(&s) {
            Err(LoadError::EmptyChainStep { position }) => {
                assert_eq!(
                    position, expected_pos,
                    "steps {toml_steps} must report position {expected_pos}, got {position}"
                );
            }
            other => panic!("steps {toml_steps}: expected EmptyChainStep, got {other:?}"),
        }
    }
}

#[test]
fn parsing_rejects_padded_enum_values() {
    // Manifest enum values are canonical strings. Empty/all-whitespace
    // values get dedicated Empty* errors, but padded non-empty values
    // must not be silently normalized.
    let cases = [
        (
            r#"pin = "Inline "
                invoker = "External"
                command = "echo""#,
            "UnknownPinKind",
        ),
        (
            r#"pin = " Embed"
                invoker = "External"
                command = "echo""#,
            "UnknownPinKind",
        ),
        (
            r#"invoker = "External\n"
                command = "echo""#,
            "UnknownInvoker",
        ),
        (r#"invoker = "\tEmbed""#, "UnknownInvoker"),
        (
            r#"surfaces = ["cli ", "tui"]
                invoker = "External"
                command = "echo""#,
            "UnknownSurface",
        ),
        (
            r#"surfaces = ["cli", " mcp\t"]
                invoker = "External"
                command = "echo""#,
            "UnknownSurface",
        ),
    ];
    for (toml_value, expected) in cases {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   {toml_value}"#,
        );
        match (parse_fixture_tool(&s), expected) {
            (Err(LoadError::UnknownPinKind(_)), "UnknownPinKind") => {}
            (Err(LoadError::UnknownInvoker(_)), "UnknownInvoker") => {}
            (Err(LoadError::UnknownSurface(_)), "UnknownSurface") => {}
            (got, _) => panic!("{toml_value}: expected {expected}, got {got:?}"),
        }
    }
}

#[test]
fn parsing_rejects_empty_invoker_and_pin_kind() {
    // Iter 205: present-but-empty invoker / pin get
    // distinct error variants instead of falling through to
    // UnknownInvoker("  ") / UnknownPinKind("  ") with
    // empty-backtick messages. Invoker omission is no longer accepted
    // for runtime TOML; this test still pins PRESENT-and-empty.
    for (toml_value, field, expected) in [
        (r#"invoker = """#, "invoker", "EmptyInvoker"),
        (r#"invoker = "  ""#, "invoker", "EmptyInvoker"),
        (r#"pin = """#, "pin", "EmptyPinKind"),
        (r#"pin = "\t""#, "pin", "EmptyPinKind"),
    ] {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   {toml_value}"#,
        );
        match (parse_fixture_tool(&s), expected) {
            (Err(LoadError::EmptyInvoker), "EmptyInvoker") => {}
            (Err(LoadError::EmptyPinKind), "EmptyPinKind") => {}
            (got, _) => panic!("{field}={toml_value}: expected {expected}, got {got:?}"),
        }
    }
}

#[test]
fn parsing_rejects_empty_surface_entry() {
    // Iter 207: empty surface entries get the dedicated
    // EmptyInSurfaces error variant with position info, instead
    // of falling through to UnknownSurface("") with empty backticks.
    for (toml_value, expected_pos) in [
        (r#"["", "cli"]"#, 0),
        (r#"["cli", "", "http"]"#, 1),
        (r#"["cli", "  "]"#, 1), // whitespace counts as empty
    ] {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   surfaces = {toml_value}"#,
        );
        match parse_fixture_tool(&s) {
            Err(LoadError::EmptyInSurfaces { position }) => {
                assert_eq!(
                    position, expected_pos,
                    "surfaces {toml_value}: expected position {expected_pos}, got {position}"
                );
            }
            other => panic!("surfaces {toml_value}: expected EmptyInSurfaces, got {other:?}"),
        }
    }
}

#[test]
fn parsing_rejects_author_facing_input_schema_field() {
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   input_schema = "{\"type\":\"object\",\"properties\":{}}""#;
    match parse_fixture_tool(s) {
        Err(LoadError::Toml(error)) => {
            let message = error.to_string();
            assert!(message.contains("unknown field"), "{message}");
            assert!(message.contains("input_schema"), "{message}");
            assert!(message.contains("inputs"), "{message}");
        }
        other => panic!("input_schema must be rejected as an unknown field, got {other:?}"),
    }
}

#[test]
fn missing_or_empty_inputs_are_allowed() {
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo""#;
    let meta = parse_fixture_tool(s).expect("omitted inputs must be accepted");
    assert!(meta.input_spec.fields.is_empty());

    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   inputs = []"#;
    let meta = parse_fixture_tool(s).expect("empty inputs must be accepted");
    assert!(meta.input_spec.fields.is_empty());
}

#[test]
fn parsing_accepts_inputs_and_builds_input_spec() {
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   inputs = [
                     { name = "input", type = "string", label = "Input", description = "Text to echo", required = true },
                     { name = "count", type = "integer" },
                     { name = "mode", type = "options", required = true, options = [
                       { value = "fast", label = "Fast" },
                       { value = "safe", description = "Safe mode" },
                     ] },
                     { name = "flags", type = "multi_options", options = [
                       { value = "dry" },
                       { value = "verbose" },
                     ] },
                   ]"#;
    let meta = parse_fixture_tool(s).expect("typed inputs must load");
    assert_eq!(meta.input_spec.fields.len(), 4);
    assert_eq!(meta.input_spec.fields[0].name.as_str(), "input");
    assert_eq!(meta.input_spec.fields[0].label.as_deref(), Some("Input"));
    assert_eq!(
        meta.input_spec.fields[0].description.as_deref(),
        Some("Text to echo")
    );
    assert!(meta.input_spec.fields[0].required);
    assert_eq!(meta.input_spec.fields[1].kind.label(), "integer");
    match &meta.input_spec.fields[2].kind {
        InputKind::Options(choices) => {
            assert_eq!(choices.allowed_values(), vec!["fast", "safe"]);
            assert_eq!(choices.options[0].label.as_deref(), Some("Fast"));
            assert_eq!(choices.options[1].description.as_deref(), Some("Safe mode"));
        }
        other => panic!("expected options field, got {other:?}"),
    }
    match &meta.input_spec.fields[3].kind {
        InputKind::MultiOptions(choices) => {
            assert_eq!(choices.allowed_values(), vec!["dry", "verbose"]);
        }
        other => panic!("expected multi_options field, got {other:?}"),
    }

    let schema = meta.input_schema_value();
    assert_eq!(schema["properties"]["input"]["type"], "string");
    assert_eq!(schema["properties"]["count"]["type"], "integer");
    assert_eq!(schema["required"], serde_json::json!(["input", "mode"]));
}

#[test]
fn parsing_accepts_all_input_types() {
    for ty in [
        "string",
        "number",
        "integer",
        "boolean",
        "markdown",
        "json",
        "datetime",
        "file_path",
        "url",
        "file",
    ] {
        let s = format!(
            r#"id = "y.x"
               toolkit = "y"
               invoker = "External"
               command = "echo"
               inputs = [{{ name = "value", type = "{ty}" }}]"#,
        );
        let meta = parse_fixture_tool(&s).unwrap_or_else(|e| panic!("{ty}: {e}"));
        assert_eq!(meta.input_spec.fields[0].kind.label(), ty);
    }
}

#[test]
fn parsing_rejects_invalid_inputs() {
    let unknown_type = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   inputs = [{ name = "blob", type = "bytes" }]"#;
    match parse_fixture_tool(unknown_type) {
        Err(LoadError::UnknownInputType { name, kind, .. }) => {
            assert_eq!(name, "blob");
            assert_eq!(kind, "bytes");
        }
        other => panic!("unknown input type must fail, got {other:?}"),
    }

    let missing_choices = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   inputs = [{ name = "mode", type = "options" }]"#;
    match parse_fixture_tool(missing_choices) {
        Err(LoadError::InvalidInputSpec { detail }) => {
            assert!(detail.contains("choice inputs"), "{detail}");
        }
        other => panic!("options without choices must fail, got {other:?}"),
    }

    let scalar_choices = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   inputs = [{ name = "text", type = "string", options = [{ value = "x" }] }]"#;
    assert!(matches!(
        parse_fixture_tool(scalar_choices),
        Err(LoadError::UnexpectedInputOptions { .. })
    ));

    let duplicate = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   inputs = [
                     { name = "value", type = "string" },
                     { name = "value", type = "integer" },
                   ]"#;
    match parse_fixture_tool(duplicate) {
        Err(LoadError::InvalidInputSpec { detail }) => {
            assert!(detail.contains("appears more than once"), "{detail}");
        }
        other => panic!("duplicate inputs must fail, got {other:?}"),
    }
}

mod outputs;

#[test]
fn missing_invoker_is_rejected_for_runtime_tools() {
    // Runtime Toolkit TOML is dynamic: without an explicit executable adapter
    // there is no compile-time `#[tool]` dispatcher to call. Do not preserve the
    // old implicit Function default because it created visible-but-uncallable
    // tools.
    let s = r#"id = "y.x"
                   toolkit = "y""#;
    assert!(matches!(
        parse_fixture_tool(s),
        Err(LoadError::MissingInvoker)
    ));
}

#[test]
fn declared_invoker_keeps_missing_pin_kind_default() {
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo""#;
    let m = parse_fixture_tool(s).unwrap();
    assert_eq!(m.pin, PinKind::Inline, "omitted pin → Inline default");
}

#[test]
fn parsing_rejects_empty_id_and_toolkit() {
    // Iter 196: pre-iter-196 the loader accepted `id = ""` and
    // `toolkit = ""` because serde only enforced presence, not
    // content. Empty-id tools would register and silently shadow
    // other lookups; empty-toolkit tools would clump under a
    // meaningless "" group in `tool list`. Pin both rejections,
    // including the trim-whitespace case (matches the iter-195
    // chain-step trim).
    for (toml_id, toml_toolkit, expected) in [
        (r#""""#, r#""ok""#, "EmptyId"),
        (r#""   ""#, r#""ok""#, "EmptyId"),
        (r#""\t\n""#, r#""ok""#, "EmptyId"),
        (r#""ok.""#, r#""ok""#, "ToolIdContainsToolkit"),
        (r#""x""#, r#""""#, "EmptyToolkit"),
        (r#""x""#, r#""  ""#, "EmptyToolkit"),
    ] {
        let s = format!(
            "id = {toml_id}
                   toolkit = {toml_toolkit}",
        );
        match (parse_fixture_tool(&s), expected) {
            (Err(LoadError::EmptyId), "EmptyId") => {}
            (Err(LoadError::EmptyToolkit), "EmptyToolkit") => {}
            (Err(LoadError::ToolIdContainsToolkit { .. }), "ToolIdContainsToolkit") => {}
            (got, _) => {
                panic!("id={toml_id} toolkit={toml_toolkit}: expected {expected}, got {got:?}")
            }
        }
    }
}

#[test]
fn parsing_rejects_padded_manifest_identities() {
    let padded_toolkit = r#"
            id = " kit "

            [[tools]]
            id = "echo"
            invoker = "External"
            command = "echo"
        "#;
    assert!(
        matches!(
            parse_toolkit_full(padded_toolkit),
            Err(LoadError::NonCanonicalToolkit(_))
        ),
        "top-level Toolkit id must be canonical, not trimmed"
    );

    let padded_local_tool = r#"
            id = "kit"

            [[tools]]
            id = " echo "
            pegboard_units = "U1"
            invoker = "External"
            command = "echo"
        "#;
    assert!(
        matches!(
            parse_toolkit_full(padded_local_tool),
            Err(LoadError::NonCanonicalId(_))
        ),
        "[[tools]].id must be canonical, not trimmed"
    );
}

#[test]
fn empty_id_and_toolkit_messages_are_clear() {
    let id_msg = format!("{}", LoadError::EmptyId);
    let toolkit_msg = format!("{}", LoadError::EmptyToolkit);
    assert!(id_msg.contains("`id`") && id_msg.contains("non-empty"));
    assert!(toolkit_msg.contains("`toolkit`") && toolkit_msg.contains("non-empty"));
}

#[test]
fn id_shadows_builtin_message_explains_rename_path() {
    // Iter 249/256: parallel to upeg-wasm iter-252 + mcp_import
    // iter-250 message-format pins. Pre-iter-256 the loader's
    // IdShadowsBuiltIn Display message wasn't unit-tested in
    // upeg-loader (only via integration in upeg-cli/src/tests.rs).
    // A future drift in the loader's wording — say someone removes
    // the rename example or the pre-iter-249 explanation — would
    // ship without tripping any in-crate test. Pin the load-bearing
    // pieces here so the loader's user-facing message stays
    // discoverable from within its own crate.
    let err = LoadError::IdShadowsBuiltIn("num.hex_to_decimal".into());
    let msg = format!("{err}");
    assert!(
        msg.contains("num.hex_to_decimal"),
        "message must echo the colliding id; got `{msg}`"
    );
    assert!(
        msg.contains("shadows a built-in"),
        "message must explain the failure mode; got `{msg}`"
    );
    assert!(
        msg.contains("Rename"),
        "message must hint at the remedy; got `{msg}`"
    );
    // Loader-specific: includes a concrete rename example like
    // `my.num.hex_to_decimal` (parallel to mcp_import's
    // `my_<server>` example, but with the dotted-id convention).
    assert!(
        msg.contains("my.num.hex_to_decimal"),
        "loader message must give a concrete dotted-id rename example; got `{msg}`"
    );
}

#[test]
fn parsing_rejects_padded_id_toolkit_and_boards() {
    let s = r#"id = " iter241.padded_id "
                   toolkit = "  iter241  "
                   invoker = "External"
                   command = "echo"
                   boards = [" dev ", "trading\t"]"#;
    assert!(
        matches!(
            parse_fixture_tool(s),
            Err(LoadError::NonCanonicalToolkit(_) | LoadError::NonCanonicalId(_))
        ),
        "padded manifest identities must be rejected instead of normalized"
    );

    let s = r#"id = "iter241.board_trim"
                   toolkit = "iter241"
                   invoker = "External"
                   command = "echo"
                   boards = [" dev ", "trading\t"]"#;
    match parse_fixture_tool(s) {
        Err(LoadError::NonCanonicalBoard { position, board }) => {
            assert_eq!(position, 0);
            assert_eq!(board, " dev ");
        }
        other => panic!("padded boards must be rejected, got {other:?}"),
    }
}

#[test]
fn parsing_rejects_padded_tags() {
    let s = r#"id = "iter241.tags"
                   toolkit = "iter241"
                   tags = [" dev "]
                   invoker = "External"
                   command = "echo""#;
    match parse_fixture_tool(s) {
        Err(LoadError::NonCanonicalTag { position, tag }) => {
            assert_eq!(position, 0);
            assert_eq!(tag, " dev ");
        }
        other => panic!("padded tags must be rejected, got {other:?}"),
    }
}
