#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::string_add,
    clippy::manual_let_else,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Pin every TOML in `examples/tools/` to the loader's contract.
//!
//! If a future change to `upeg_loader` breaks an example's parse — or
//! if someone copy-pastes a malformed example — this test catches it
//! at PR time. Examples are user-facing artifacts; they shouldn't rot.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `upeg-cli/`; walk up one to the repo root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("upeg-cli has a parent (the workspace root)")
        .to_path_buf()
}

fn examples_tools_dir() -> PathBuf {
    workspace_root().join("examples/tools")
}

fn examples_mcp_dir() -> PathBuf {
    workspace_root().join("examples/mcp-imports")
}

/// The repo's own dogfood Project Manifest (`/<repo>/upeg.toml`).
fn dogfood_project_manifest() -> PathBuf {
    workspace_root().join("upeg.toml")
}

#[test]
fn 커밋된_도구킷_schema는_생성된_것과_일치한다() {
    let path = workspace_root().join("fixtures/toolkit.schema.json");
    let committed = std::fs::read_to_string(&path).expect("read committed toolkit schema");
    let generated = upeg_loader::toolkit_schema_json().expect("schema generation serializes");
    assert_eq!(
        committed, generated,
        "fixtures/toolkit.schema.json drifted; run just toolkit-schema"
    );
}

#[test]
fn 커밋된_도구킷_manifest_문서_일치_생성된() {
    let path = workspace_root().join("docs/TOOL_MANIFEST.md");
    let committed = std::fs::read_to_string(&path).expect("read committed toolkit manifest docs");
    let generated =
        upeg_loader::toolkit_manifest_docs_markdown().expect("docs generation serializes");
    assert_eq!(
        committed, generated,
        "docs/TOOL_MANIFEST.md drifted; run just toolkit-schema"
    );
}

#[test]
fn 도구킷_schema는_도구_예제를_검증한다() {
    let schema_path = workspace_root().join("fixtures/toolkit.schema.json");
    let schema_raw = std::fs::read_to_string(&schema_path).expect("read committed toolkit schema");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_raw).expect("parse committed toolkit schema as JSON");
    let validator = jsonschema::validator_for(&schema).expect("compile committed toolkit schema");

    let dir = examples_tools_dir();
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("read examples/tools directory") {
        let path = entry.expect("read examples/tools directory entry").path();
        if path.extension().is_none_or(|x| x != "toml") {
            continue;
        }

        let raw = std::fs::read_to_string(&path).expect("read toolkit example TOML");
        let toml_value: toml::Value =
            toml::from_str(&raw).expect("parse toolkit example TOML before schema validation");
        let instance =
            serde_json::to_value(toml_value).expect("convert toolkit example TOML to JSON");
        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|e| e.to_string())
            .collect();
        if !errors.is_empty() {
            failures.push(format!("{}:\n{}", path.display(), errors.join("\n")));
        }
    }

    assert!(
        failures.is_empty(),
        "toolkit schema rejected examples:\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn manifest_문서_참조_기존_예제들() {
    let root = workspace_root();
    let docs_path = root.join("docs/TOOL_MANIFEST.md");
    let docs = std::fs::read_to_string(&docs_path).expect("read committed toolkit manifest docs");

    for relative_path in [
        "examples/tools/echo-bracketed.toml",
        "examples/tools/http-mock-echo.toml",
        "examples/tools/llm-echo.toml",
        "examples/tools/embed-mdn.toml",
        "examples/tools/chain-md5-then-uppercase.toml",
        "examples/plugins/greet/Cargo.toml",
        "examples/plugins/greet/src/lib.rs",
        "examples/mcp-imports/local.toml",
        "examples/mcp-imports/github.toml",
        "fixtures/toolkit.schema.json",
    ] {
        assert!(
            root.join(relative_path).exists(),
            "docs reference missing path `{relative_path}`"
        );
        assert!(
            docs.contains(relative_path),
            "docs/TOOL_MANIFEST.md should reference `{relative_path}`"
        );
    }
}

#[test]
fn 생성된_문서는_필수_manifest_필드를_포함한다() {
    let path = workspace_root().join("docs/TOOL_MANIFEST.md");
    let docs = std::fs::read_to_string(&path).expect("read committed toolkit manifest docs");

    assert!(
        docs.contains(
            "Generated from upeg-loader manifest documentation metadata. Do not edit by hand."
        ),
        "docs/TOOL_MANIFEST.md should carry the generated-file warning"
    );
    assert!(
        docs.contains("| Field | Required | Type | Description |"),
        "docs/TOOL_MANIFEST.md should contain the field reference header"
    );

    for field in [
        "`id`",
        "`tags`",
        "`display_label`",
        "`description`",
        "`tools`",
        "`inputs`",
        "`pin`",
        "`pegboard_units`",
        "`invoker`",
        "`surfaces`",
        "`boards`",
        "`command`",
        "`args_template`",
        "`steps`",
        "`connections`",
        "`output`",
        "`url`",
        "`method`",
        "`headers`",
        "`body`",
        "`prompt`",
        "`provider`",
        "`model`",
        "`credential`",
        "`credentials`",
        "`wasm_path`",
        "`triggers`",
        "`embed_url`",
        "`controlled_embed.bindings`",
    ] {
        assert!(
            docs.contains(field),
            "docs/TOOL_MANIFEST.md should document field {field}"
        );
    }
}

#[test]
fn 모든_예제_toml은_깔끔하게를_파싱한다() {
    let dir = examples_tools_dir();
    assert!(
        dir.is_dir(),
        "examples/tools/ should exist at {}",
        dir.display()
    );

    let mut count = 0;
    for entry in std::fs::read_dir(&dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|x| x != "toml") {
            continue;
        }
        count += 1;
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        upeg_loader::parse_toolkit_full(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
    assert!(
        count >= 3,
        "expected ≥3 example TOMLs, found {count} in {}",
        dir.display()
    );
}

#[test]
fn 모든_예제_mcp_설정은_깔끔하게를_파싱한다() {
    // examples/mcp-imports/*.toml must deserialize via the same schema the
    // MCP manager uses for `~/.upeg/mcp-imports/<name>.toml`. Catches
    // accidental field renames in `UpstreamMcpConfig`.
    let dir = examples_mcp_dir();
    assert!(
        dir.is_dir(),
        "examples/mcp-imports/ should exist at {}",
        dir.display()
    );

    let mut count = 0;
    for entry in std::fs::read_dir(&dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|x| x != "toml") {
            continue;
        }
        count += 1;
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let cfg: upeg_cli::UpstreamMcpConfig =
            toml::from_str(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            !cfg.command.is_empty(),
            "{}: empty `command`",
            path.display()
        );
    }
    assert!(count >= 2, "expected ≥2 mcp examples, found {count}");
}

#[test]
fn mcp_예제_파일_이름은_사용자_네임스페이스를_그대로_쓴다() {
    let dir = examples_mcp_dir();
    for expected in ["github.toml", "local.toml"] {
        assert!(
            dir.join(expected).is_file(),
            "examples/mcp-imports/{expected} should exist because the filename stem becomes the MCP namespace"
        );
    }
    for retired in ["github-server.toml", "local-self.toml"] {
        assert!(
            !dir.join(retired).exists(),
            "examples/mcp-imports/{retired} would create a surprising namespace"
        );
    }
}

#[test]
fn 플러그인_예제는_선언된_구조를_그대로_유지한다() {
    // Pin the file structure of `examples/plugins/greet/` so a future
    // change to the plugin contract (macro crate rename, attribute shape,
    // crate-type, etc.) fails loudly here instead of silently leaving
    // a broken example for users to copy-paste from.
    let plugin_dir = workspace_root().join("examples/plugins/greet");
    let cargo_toml =
        std::fs::read_to_string(plugin_dir.join("Cargo.toml")).expect("Cargo.toml exists");
    let lib_rs = std::fs::read_to_string(plugin_dir.join("src/lib.rs")).expect("src/lib.rs exists");

    // Cargo.toml requirements (every plugin author needs these).
    for tok in [
        "[workspace]",               // standalone — not the upeg ws
        "crate-type = [\"cdylib\"]", // wasm-buildable
        "extism-pdk",                // the SDK we ship against
        "upeg-plugin-api",           // shared manifest contract types
        "upeg-plugin-macros",        // #[tool] + upeg_plugin! macro crate
        "wasm32-unknown-unknown",    // build instructions in comments
    ] {
        assert!(
            cargo_toml.contains(tok),
            "examples/plugins/greet/Cargo.toml missing `{tok}`"
        );
    }

    // src/lib.rs requirements (the macro-based authoring contract).
    for tok in [
        "use upeg_plugin_macros::{tool, upeg_plugin};", // macro crate + attribute + aggregator import
        "#[tool(",                                      // attribute application
        "id = \"greet.hello\"",                         // tool id
        "toolkit = \"greet\"",                          // toolkit id
        "pegboard_units = U1",                          // required pegboard units
        "inputs = [",                                   // typed inputs DSL
        "pub fn greet_hello", // plain typed fn — no manual manifest/plugin_fn boilerplate
        "upeg_plugin! {",     // aggregator macro invocation
        "tools: [greet_hello]", // aggregator tool listing
    ] {
        assert!(
            lib_rs.contains(tok),
            "examples/plugins/greet/src/lib.rs missing `{tok}`"
        );
    }
    assert!(
        !lib_rs.contains("let m = r#"),
        "examples/plugins/greet should build its manifest from upeg's typed DTOs, not raw JSON"
    );
    assert!(
        !lib_rs.contains("PluginManifest::new"),
        "examples/plugins/greet should rely on upeg_plugin! for manifest construction, not hand-roll it"
    );
    assert!(
        !lib_rs.contains("#[plugin_fn]"),
        "examples/plugins/greet should not hand-write #[plugin_fn] exports; \
         the #[tool]/upeg_plugin! macros generate them"
    );
}

#[test]
fn 체인_예제는_대상으로_내장_단계들을_해결한다() {
    // The `chain-md5-then-uppercase.toml` example references
    // `hash.md5` + `text.uppercase`. Both are built-ins, so the chain
    // should --resolve-chain cleanly.
    //
    // Integration test binaries don't statically reference any
    // upeg-tools function, which on some linker configurations dead-
    // strips the `inventory` section before `toolbox_tool` can find
    // the entries. Touch a real symbol to defeat that — calling
    // `dispatch_tool` for one built-in fires `ensure_builtins_registered`
    // which references every tool fn directly.
    let _ = upeg_cli::dispatch_tool("hash.md5", &serde_json::json!({"input": "warmup"}));

    let dir = examples_tools_dir();
    let path = dir.join("chain-md5-then-uppercase.toml");
    let raw = std::fs::read_to_string(&path).expect("read chain example");
    let (_toolkit, tools) = upeg_loader::parse_toolkit_full(&raw).expect("parse");
    let (_meta, toml) = tools
        .into_iter()
        .find(|(meta, _)| meta.id == "demo.loud_md5")
        .expect("chain example tool exists");
    let steps = toml.steps.expect("example must declare steps");
    for step in &steps {
        assert!(
            upeg_runtime::toolbox_tool(step.tool.trim()).is_some(),
            "chain example references missing built-in `{}`",
            step.tool,
        );
    }
}

#[test]
fn embed_예제는_url_와_선택자_바인딩들_로_registries를_로드한다() {
    // Loading the Controlled Embed example populates both the
    // `embed_url_for(id)` registry (iter 87) and the
    // `selector_bindings_for(id)` registry (iter 92) end-to-end.
    let dir = examples_tools_dir();
    let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
    // All examples should load cleanly; failures here mean a
    // separate example regressed too.
    assert!(
        outcome.failed.is_empty(),
        "example dir must load cleanly — failed: {:?}",
        outcome.failed
    );

    let id = "embed.shout";
    let url = upeg_runtime::embed_url_for(id).expect("embed_url registered");
    assert!(
        url.starts_with("data:text/html"),
        "Controlled Embed example ships an inline `data:` URL to keep \
         the round-trip self-contained; got {url:?}"
    );

    // Bindings must be present and ordered as the TOML declared them.
    let bindings = upeg_runtime::selector_bindings_for(id);
    assert_eq!(
        bindings.len(),
        3,
        "embed example declares 3 selector bindings, registry shows {}",
        bindings.len()
    );
    // ControlledEmbed v2 requires input + trigger + output roles.
    assert_eq!(bindings[0].role, upeg_runtime::BindingRole::Input);
    assert_eq!(bindings[0].field, "text");
    assert_eq!(bindings[1].role, upeg_runtime::BindingRole::Trigger);
    assert_eq!(bindings[2].role, upeg_runtime::BindingRole::Output);
    assert_eq!(bindings[2].field, "loud");
    assert!(
        bindings[0].selector.starts_with('#'),
        "input binding selector should begin with `#` (got {:?})",
        bindings[0].selector
    );
}

#[test]
fn 생성된_문서는_x_문서_메타데이터를_포함한다() {
    // Ensures that field reference tables in the generated docs are
    // derived from schema x-doc-* metadata. If regression introduces
    // hard-coded facts, these assertions will fail.
    let path = workspace_root().join("docs/TOOL_MANIFEST.md");
    let docs = std::fs::read_to_string(&path).expect("read committed toolkit manifest docs");

    // Field tables must carry schema-derived descriptions, not blanks.
    for schema_name in &[
        "ToolkitToml",
        "ToolEntryToml",
        "ChainStepToml",
        "ChainConnectionToml",
        "KeyValueToml",
        "CredentialRefToml",
        "SelectorBindingToml",
        "TriggerToml",
    ] {
        assert!(
            docs.contains(&format!("### {schema_name}")),
            "generated docs should contain field table for {schema_name}"
        );
    }

    // Description cells should not be empty — x-doc-* metadata should
    // populate them per field.
    assert!(
        !docs.contains("|  |  |"),
        "generated docs should not have empty description cells"
    );
}

#[test]
fn 생성된_문서는_호출자_테이블_값을_포함한다() {
    // Ensures the invoker table in generated docs carries values sourced
    // from RUNTIME_INVOKERS, not hard-coded strings that could drift.
    let path = workspace_root().join("docs/TOOL_MANIFEST.md");
    let docs = std::fs::read_to_string(&path).expect("read committed toolkit manifest docs");

    for (name, purpose) in &[
        ("External", "spawn a local process"),
        ("Http", "call an HTTP endpoint"),
        (
            "Embed",
            "open a GUI sidecar runtime adapter in a WebView or iframe",
        ),
        (
            "Chain",
            "compose other tools with ordered steps and connections",
        ),
        ("Llm", "render a prompt into an LLM/provider adapter"),
        ("Wasm", "load a WASM module adapter"),
    ] {
        assert!(
            docs.contains(&format!("| `{name}` | {purpose}")),
            "generated docs should contain invoker `{name}` with purpose `{purpose}`"
        );
    }
}

#[test]
fn 저장소_dogfood_project_manifest는_로더_계약을_지킨다() {
    // The repo dogfoods itself: `/<repo>/upeg.toml` is a real Project
    // Manifest the maintainers call every day (README "사용자 tool 추가").
    // Pin it to the loader contract so a grammar change cannot rot the
    // one manifest we actually use.
    let path = dogfood_project_manifest();
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let (toolkit, tools) =
        upeg_loader::parse_toolkit_full(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    assert_eq!(toolkit.id, "dev", "dogfood manifest의 toolkit id는 `dev`다");
    assert!(
        tools.len() >= 10,
        "dogfood manifest는 실제로 쓰는 도구 모음이다: {} 개뿐",
        tools.len()
    );

    // Every Chain step must point at a tool the same manifest declares
    // (the `dev.*` namespace) — a dangling step would only surface at
    // dispatch time otherwise.
    let declared: Vec<&str> = tools.iter().map(|(meta, _)| meta.id).collect();
    for (meta, toml) in &tools {
        let Some(steps) = toml.steps.as_ref() else {
            continue;
        };
        for step in steps {
            let target = step.tool.trim();
            assert!(
                declared.contains(&target) || upeg_runtime::toolbox_tool(target).is_some(),
                "chain `{}` references unknown tool `{target}`",
                meta.id,
            );
        }
    }
}
