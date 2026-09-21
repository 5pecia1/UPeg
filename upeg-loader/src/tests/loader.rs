use super::{parse_single_tool as parse_fixture_tool, runtime_success_text, single_tool_toml_str};
use crate::{
    LoadError, load_and_register_dir, load_and_register_dir_verbose,
    load_and_register_file_verbose, load_dir,
};
use std::path::Path;
use upeg_core::PinKind;
use upeg_runtime::ToolMetaRuntimeExt;

#[test]
fn loaded_and_registered_tool_is_visible_in_registry() {
    // Use a unique id so test ordering doesn't matter — a previous test in
    // the same binary may have already populated the runtime registry.
    let id = "test.runtime_register_unique_xyz";
    let s = format!(
        r#"
            id = "{id}"
            toolkit = "test"
            pin = "Live"
            invoker = "External"
            command = "echo"
            "#
    );
    let meta = parse_fixture_tool(&s).expect("parse");
    upeg_runtime::toolbox_add_tool(meta);

    let found = upeg_runtime::toolbox_tool(id).expect("registered");
    assert_eq!(found.toolkit, "test");
    assert_eq!(found.pin, PinKind::Live);
}

#[test]
fn missing_dir_load_dir_returns_error() {
    let result = load_dir(Path::new("/no/such/dir/upeg/test"));
    assert!(result.is_err());
}

#[test]
fn missing_dir_load_and_register_dir_returns_zero() {
    let (loaded, failed) = load_and_register_dir(Path::new("/no/such/dir/upeg/test"));
    assert_eq!((loaded, failed), (0, 0));
}

#[test]
fn missing_dir_verbose_outcome_is_empty() {
    let out = load_and_register_dir_verbose(Path::new("/no/such/dir/upeg/test"));
    assert!(out.loaded.is_empty());
    assert!(out.failed.is_empty());
}

#[test]
fn verbose_outcome_reports_error_per_bad_file() {
    let dir = std::env::temp_dir().join("upeg_loader_verbose_errs");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(
        dir.join("good.toml"),
        single_tool_toml_str(
            r#"id = "loader.verbose.ok"
	toolkit = "loader"
	invoker = "External"
	command = "echo""#,
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("bad_pin.toml"),
        single_tool_toml_str(
            r#"id = "loader.verbose.bad_pin"
	toolkit = "loader"
	pin = "Mauve"
	invoker = "External"
	command = "echo""#,
        ),
    )
    .unwrap();
    std::fs::write(dir.join("not_toml.toml"), "this :: is :: not :: toml").unwrap();
    std::fs::write(
        dir.join("empty_chain.toml"),
        single_tool_toml_str(
            r#"id = "loader.verbose.empty_chain"
toolkit = "loader"
steps = []"#,
        ),
    )
    .unwrap();

    let out = load_and_register_dir_verbose(&dir);
    assert_eq!(out.loaded.len(), 1, "exactly the good.toml should register");
    assert_eq!(out.loaded[0], "loader.verbose.ok");
    assert_eq!(out.failed.len(), 3, "three failures expected");

    // Each failure remembers its path so the user can fix the right file.
    let bad_paths: Vec<&str> = out
        .failed
        .iter()
        .map(|(p, _)| p.file_name().and_then(|s| s.to_str()).unwrap())
        .collect();
    assert!(bad_paths.contains(&"bad_pin.toml"));
    assert!(bad_paths.contains(&"not_toml.toml"));
    assert!(bad_paths.contains(&"empty_chain.toml"));

    // Spot-check error variants.
    let by_name = |n: &str| -> &LoadError {
        out.failed
            .iter()
            .find(|(p, _)| p.file_name().and_then(|s| s.to_str()) == Some(n))
            .map(|(_, e)| e)
            .unwrap()
    };
    assert!(matches!(
        by_name("bad_pin.toml"),
        LoadError::UnknownPinKind(_)
    ));
    assert!(matches!(by_name("not_toml.toml"), LoadError::Toml(_)));
    assert!(matches!(by_name("empty_chain.toml"), LoadError::EmptyChain));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_invoker_registers_no_metadata_only_tool() {
    // Dynamic Toolkit TOML has no compile-time `#[tool]` function behind it.
    // A missing invoker must fail load-time validation and must not leave a
    // visible ToolMeta without a dispatcher in the global registry.
    let dir = std::env::temp_dir().join("upeg_loader_missing_invoker_no_ghost");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "loader.no_ghost_missing_invoker";
    std::fs::write(
        dir.join("bad.toml"),
        single_tool_toml_str(&format!(
            r#"id = "{id}"
toolkit = "loader""#
        )),
    )
    .unwrap();

    let out = load_and_register_dir_verbose(&dir);
    assert!(out.loaded.is_empty(), "must not load metadata-only tools");
    assert_eq!(out.failed.len(), 1, "{:?}", out.failed);
    assert!(matches!(out.failed[0].1, LoadError::MissingInvoker));
    assert!(
        upeg_runtime::toolbox_tool(id).is_none(),
        "failed TOML must not become visible in the registry"
    );
    assert!(
        upeg_runtime::try_runtime_dispatch(id, &serde_json::json!({})).is_none(),
        "failed TOML must not install a dispatcher either"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_verbose_load_and_register_merges_one_project_manifest() {
    let dir = std::env::temp_dir().join("upeg_loader_project_file");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("upeg.toml");
    std::fs::write(
        &path,
        r#"id = "projectfile"
tags = ["project"]

[[tools]]
id = "echo"
tags = ["dev"]
pegboard_units = "U1"
invoker = "External"
command = "echo"
"#,
    )
    .unwrap();

    let out = load_and_register_file_verbose(&path);
    assert!(
        out.failed.is_empty(),
        "project manifest failed: {:?}",
        out.failed
    );
    assert_eq!(out.loaded, vec!["projectfile.echo"]);
    let meta = upeg_runtime::toolbox_tool("projectfile.echo").expect("registered");
    assert_eq!(meta.toolkit, "projectfile");
    assert!(meta.has_tag("project"));
    assert!(meta.has_tag("dev"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_registers_trigger_bindings_for_discovery_surfaces() {
    let dir = std::env::temp_dir().join("upeg_loader_trigger_bindings");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("upeg.toml");
    std::fs::write(
        &path,
        r#"id = "triggerfile"

[[tools]]
id = "webhook_echo"
pegboard_units = "U1"
invoker = "External"
command = "echo"
triggers = [
  { source = "webhook" },
  { source = "schedule", condition = "every:1h" },
]
"#,
    )
    .unwrap();

    let out = load_and_register_file_verbose(&path);
    assert!(
        out.failed.is_empty(),
        "trigger manifest failed: {:?}",
        out.failed
    );
    let triggers = upeg_runtime::trigger_bindings_for("triggerfile.webhook_echo");
    assert_eq!(triggers.len(), 2);
    assert_eq!(triggers[0].source, "webhook");
    // `webhook` carries no condition; the loader rejects one.
    assert_eq!(triggers[0].condition, None);
    assert_eq!(triggers[1].source, "schedule");
    assert_eq!(triggers[1].condition.as_deref(), Some("every:1h"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_trims_selector_binding_fields_and_selectors() {
    // Iter 243: parallel to iter-241 trim work, applied to
    // selector_bindings entries. `field` must match typed input
    // property names exactly per SelectorBinding's doc-comment, so
    // a TOML with `{field = " input ", ...}` would silently fail
    // to match anything in the schema. iter-197 EmptySelectorBinding
    // catches all-whitespace; this trim handles surrounding-
    // whitespace.
    let dir = std::env::temp_dir().join("upeg_loader_iter243_selectors");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("padded.toml"),
        single_tool_toml_str(
            r##"id = "iter243.padded_bindings"
toolkit = "iter243"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.test/iter243"
controlled_embed = { bindings = [
  { role = "input",   field = " input ",     selector = " #search-box " },
  { role = "trigger", field = "",            selector = "button" },
  { role = "output",  field = "\toutput\n",  selector = "#result" },
] }"##,
        ),
    )
    .unwrap();
    let outcome = load_and_register_dir_verbose(&dir);
    assert!(
        outcome.failed.is_empty(),
        "padded selector_bindings must load: {:?}",
        outcome.failed
    );

    let bindings = upeg_runtime::selector_bindings_for("iter243.padded_bindings");
    assert_eq!(bindings.len(), 3, "all bindings must register");
    assert_eq!(
        bindings[0].field, "input",
        "field must be trimmed at storage time; got {:?}",
        bindings[0].field
    );
    assert_eq!(
        bindings[0].selector, "#search-box",
        "selector must be trimmed; got {:?}",
        bindings[0].selector
    );
    assert_eq!(
        bindings[2].field, "output",
        "tab/newline whitespace must also trim; got {:?}",
        bindings[2].field
    );
    assert_eq!(bindings[2].selector, "#result");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn registered_external_tool_is_callable_via_runtime_dispatcher() {
    // End-to-end PoC: TOML on disk → registry → callable closure that
    // shells out. This is the declarative-loader closure.
    let dir = std::env::temp_dir().join("upeg_loader_external_e2e");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "test.external.echo_e2e";
    std::fs::write(
        dir.join("echo.toml"),
        single_tool_toml_str(&format!(
            r#"id = "{id}"
toolkit = "test"
invoker = "External"
command = "echo"
args_template = ["from-toml"]"#,
        )),
    )
    .unwrap();

    let (loaded, failed) = load_and_register_dir(&dir);
    assert_eq!((loaded, failed), (1, 0));

    let r = upeg_runtime::try_runtime_dispatch(id, &serde_json::json!({}));
    let out = runtime_success_text(r);
    assert!(out.contains("from-toml"), "got: {out:?}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_dir_walks_toml_and_skips_other_extensions() {
    let dir = std::env::temp_dir().join("upeg_loader_test_walk");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(
        dir.join("good.toml"),
        single_tool_toml_str(
            r#"id = "loader.walk_good"
	toolkit = "loader"
	invoker = "External"
	command = "echo""#,
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("bad.toml"),
        single_tool_toml_str(
            r#"id = "loader.walk_bad"
	toolkit = "loader"
	pin = "Mauve"
	invoker = "External"
	command = "echo""#,
        ),
    )
    .unwrap();
    std::fs::write(dir.join("ignored.txt"), "should not be picked up").unwrap();

    let results = load_dir(&dir).expect("read dir");
    let oks = results.iter().filter(|r| r.is_ok()).count();
    let errs = results.iter().filter(|r| r.is_err()).count();
    assert_eq!(oks, 1, "expected 1 good toml");
    assert_eq!(errs, 1, "expected 1 bad toml");

    let _ = std::fs::remove_dir_all(&dir);
}

// ─── embed_url field (iter 87) ──────────────────────────

#[test]
fn load_and_register_dir_populates_embed_url_registry() {
    // End-to-end: drop a TOML with `embed_url` into a temp dir,
    // run the loader, verify `embed_url_for(id)` returns Some(url).
    let dir = std::env::temp_dir().join("upeg_loader_embed_url_iter87");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "iter87.embed.demo";
    std::fs::write(
        dir.join("demo.toml"),
        single_tool_toml_str(&format!(
            r#"id = "{id}"
toolkit = "iter87"
pin = "Embed"
invoker = "Static"
embed_url = "https://example.com/iter87-loader""#,
        )),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert_eq!(outcome.loaded.len(), 1, "{:?}", outcome.failed);
    assert_eq!(
        upeg_runtime::embed_url_for(id),
        Some("https://example.com/iter87-loader"),
        "TOML loader must propagate embed_url into the sidecar registry",
    );
    let dispatch = upeg_runtime::try_runtime_dispatch(id, &serde_json::json!({}))
        .map(|result| match result {
            upeg_core::ToolResult::Failure(failure) => failure.error.message,
            upeg_core::ToolResult::Success(success) => {
                panic!("Passive Embed has no headless invocation, got success: {success:?}")
            }
        })
        .expect("Passive Embed loader must register a headless contract dispatcher");
    assert!(
        dispatch.contains("Passive Embed"),
        "Passive Embed dispatcher must explain the kind, got: {dispatch}"
    );
    assert!(
        dispatch.contains("GUI surface") || dispatch.contains("Desktop"),
        "Passive Embed dispatcher must point at the executable surface, got: {dispatch}"
    );
    assert!(
        !dispatch.contains("dispatch not implemented"),
        "Passive Embed dispatcher must not fall through to generic missing-dispatch errors"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_without_embed_url_registers_nothing() {
    // Negative path: a regular (non-Embed) TOML must not pollute
    // the embed_url registry.
    let dir = std::env::temp_dir().join("upeg_loader_no_embed_url_iter87");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "iter87.no_embed_url";
    std::fs::write(
        dir.join("plain.toml"),
        single_tool_toml_str(&format!(
            r#"id = "{id}"
	toolkit = "iter87"
	invoker = "External"
	command = "echo""#,
        )),
    )
    .unwrap();
    let outcome = load_and_register_dir_verbose(&dir);
    assert_eq!(outcome.loaded.len(), 1);
    assert_eq!(
        upeg_runtime::embed_url_for(id),
        None,
        "tool without embed_url must not appear in the registry"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ─── selector_bindings TOML field (iter 92) ──────────────

#[test]
fn load_and_register_dir_populates_selector_binding_registry() {
    let dir = std::env::temp_dir().join("upeg_loader_sel_bindings_iter92");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "iter92.embed.with_bindings";
    std::fs::write(
        dir.join("demo.toml"),
        single_tool_toml_str(&format!(
            // Double-hash raw string — the TOML body contains `"#`
            // (the hash-selector value) which would close the
            // single-hash raw string early.
            r##"id = "{id}"
toolkit = "iter92"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/"
controlled_embed = {{ bindings = [
  {{ role = "input",   field = "input",  selector = ".q" }},
  {{ role = "trigger", field = "",       selector = "button" }},
  {{ role = "output",  field = "output", selector = "#r" }},
] }}"##,
        )),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert_eq!(outcome.loaded.len(), 1, "{:?}", outcome.failed);

    let bindings = upeg_runtime::selector_bindings_for(id);
    assert_eq!(bindings.len(), 3);
    assert_eq!(bindings[0].role, upeg_runtime::BindingRole::Input);
    assert_eq!(bindings[0].field, "input");
    assert_eq!(bindings[0].selector, ".q");
    assert_eq!(bindings[1].role, upeg_runtime::BindingRole::Trigger);
    assert_eq!(bindings[1].selector, "button");
    assert_eq!(bindings[2].role, upeg_runtime::BindingRole::Output);
    assert_eq!(bindings[2].field, "output");
    assert_eq!(bindings[2].selector, "#r");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_without_selector_bindings_registers_nothing() {
    let dir = std::env::temp_dir().join("upeg_loader_no_sel_bindings_iter92");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "iter92.no_bindings";
    std::fs::write(
        dir.join("plain.toml"),
        single_tool_toml_str(&format!(
            r#"id = "{id}"
	toolkit = "iter92"
	invoker = "External"
	command = "echo""#
        )),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert_eq!(outcome.loaded.len(), 1);
    assert!(
        upeg_runtime::selector_bindings_for(id).is_empty(),
        "tool without selector_bindings must not pollute the registry"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reloading_tool_without_sidecars_clears_prior_runtime_sidecars() {
    let dir = std::env::temp_dir().join("upeg_loader_clear_sidecars");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("demo.toml");
    let id = "iter_clear.sidecars";
    std::fs::write(
        &path,
        single_tool_toml_str(&format!(
            r##"id = "{id}"
toolkit = "iter_clear"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.test/old"
controlled_embed = {{ bindings = [
  {{ role = "input",   field = "input",  selector = "#q" }},
  {{ role = "trigger", field = "",       selector = "button" }},
  {{ role = "output",  field = "result", selector = "#r" }},
] }}
triggers = [{{ source = "webhook" }}]"##,
        )),
    )
    .unwrap();
    assert_eq!(load_and_register_file_verbose(&path).failed.len(), 0);
    assert_eq!(
        upeg_runtime::embed_url_for(id),
        Some("https://example.test/old")
    );
    assert_eq!(upeg_runtime::selector_bindings_for(id).len(), 3);
    assert_eq!(upeg_runtime::trigger_bindings_for(id).len(), 1);

    // Re-load with a tool kind that has no sidecars (Inline/External
    // — anything not Embed-family). All previously registered
    // sidecars must be cleared.
    std::fs::write(
        &path,
        single_tool_toml_str(&format!(
            r#"id = "{id}"
toolkit = "iter_clear"
invoker = "External"
command = "echo""#,
        )),
    )
    .unwrap();
    assert_eq!(load_and_register_file_verbose(&path).failed.len(), 0);
    assert_eq!(upeg_runtime::embed_url_for(id), None);
    assert!(upeg_runtime::selector_bindings_for(id).is_empty());
    assert!(upeg_runtime::trigger_bindings_for(id).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
