//! Source-located unit tests for `mcp_import`. Tests live in this
//! sibling file to keep the production module under the ~1000 `LoC`
//! complexity budget. Config parsing / pure conversion / script-server
//! tests only — tests that need the real `upeg` binary live in
//! `tests/mcp_import_e2e.rs` (integration target so
//! `CARGO_BIN_EXE_upeg` is set). Subprocess-driven partial-success /
//! reexport / conflict-policy tests live in the `partial_success_tests`
//! submodule, split for the same file-size budget.

mod partial_success_tests;

use crate::adapters::mcp_import::*;
use upeg_core::{InputSpec, Invoker, OutputKind, OutputSpec, PinKind, Surface, ToolMeta};

fn short_timeouts() -> McpTimeouts {
    McpTimeouts {
        initialize: std::time::Duration::from_millis(200),
        tools_list: std::time::Duration::from_millis(200),
        tools_call: std::time::Duration::from_millis(200),
    }
}

#[cfg(unix)]
fn write_unix_script(name: &str, body: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!(
        "upeg-mcp-manager-{name}-{}-{}",
        std::process::id(),
        line!()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let script = root.join("server.sh");
    std::fs::write(&script, body).unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();
    (root, script)
}

/// Overrides the interpreter used to run fixture scripts.
#[cfg(unix)]
const TEST_SHELL_ENV: &str = "UPEG_TEST_SHELL";

/// POSIX shell every supported unix target ships.
#[cfg(unix)]
const DEFAULT_TEST_SHELL: &str = "/bin/sh";

#[cfg(unix)]
fn unix_script_config(script: &std::path::Path) -> UpstreamConfig {
    // Run fixtures through a shell interpreter instead of direct exec.
    // This avoids intermittent ETXTBSY on some Linux/overlayfs setups.
    let shell = std::env::var(TEST_SHELL_ENV).unwrap_or_else(|_| DEFAULT_TEST_SHELL.to_string());
    UpstreamConfig {
        command: shell,
        args: vec![script.to_str().unwrap().to_string()],
        reexport: false,
    }
}

#[test]
fn 서버_설정은_최소_toml을_파싱한다() {
    let raw = r#"command = "echo""#;
    let cfg: UpstreamConfig = toml::from_str(raw).unwrap();
    assert_eq!(cfg.command, "echo");
    assert!(cfg.args.is_empty());
}

#[test]
fn 서버_설정은_인자가_있는_경우도_파싱한다() {
    let raw = r#"
        command = "npx"
        args = ["-y", "@modelcontextprotocol/server-github"]
    "#;
    let cfg: UpstreamConfig = toml::from_str(raw).unwrap();
    assert_eq!(cfg.command, "npx");
    assert_eq!(cfg.args, vec!["-y", "@modelcontextprotocol/server-github"]);
}

#[test]
fn 선언에서_메타로의_변환은_id에_네임스페이스를_붙인다() {
    let decl = RemoteToolDecl {
        id: "num.hex_to_decimal".into(),
        description: "from upstream".into(),
        input_spec: InputSpec::empty(),
        output_spec: OutputSpec::empty(),
    };
    let meta = decl_to_meta("github", &decl, McpReexport::Blocked).unwrap();
    assert_eq!(meta.id, "github.num.hex_to_decimal");
    assert_eq!(meta.toolkit, "github");
    assert_eq!(meta.description, "from upstream");
    assert_eq!(meta.invoker, Invoker::External);
    // 재노출 기본 차단: ALL_SURFACES에서 Mcp만 빠진다.
    assert_eq!(meta.surfaces, upeg_core::ALL_SURFACES_EXCEPT_MCP);
    assert!(!meta.surfaces.contains(&Surface::Mcp));
}

#[test]
fn reexport_opt_in이면_임포트_도구가_mcp_표면에도_노출된다() {
    let decl = RemoteToolDecl {
        id: "echo".into(),
        description: "from upstream".into(),
        input_spec: InputSpec::empty(),
        output_spec: OutputSpec::empty(),
    };
    let meta = decl_to_meta("reexport_srv", &decl, McpReexport::OptedIn).unwrap();
    assert_eq!(meta.surfaces, upeg_core::ALL_SURFACES);
    assert!(meta.surfaces.contains(&Surface::Mcp));
}

#[test]
fn upstream_config의_reexport_기본값은_차단이다() {
    let cfg: UpstreamConfig = toml::from_str(r#"command = "echo""#).expect("최소 config");
    assert_eq!(cfg.reexport_policy(), McpReexport::Blocked);

    let cfg: UpstreamConfig =
        toml::from_str("command = \"echo\"\nreexport = true").expect("opt-in config");
    assert_eq!(cfg.reexport_policy(), McpReexport::OptedIn);
}

#[test]
fn 선언에서_메타로의_변환은_점이_포함된_서버_네임스페이스와_도구_이름을_보존한다() {
    let decl = RemoteToolDecl {
        id: "admin.tools.list".into(),
        description: "from upstream".into(),
        input_spec: InputSpec::empty(),
        output_spec: OutputSpec::empty(),
    };
    let meta = decl_to_meta("github.com", &decl, McpReexport::Blocked).unwrap();
    assert_eq!(meta.id, "github.com.admin.tools.list");
    assert_eq!(meta.toolkit_id(), "github.com");
    assert_eq!(meta.tool_id(), "admin.tools.list");
}

#[test]
fn 도구_목록_항목_파싱은_빈_이름을_스킵_사유와_함께_건너뛴다() {
    // A buggy upstream MCP server emitting `{"name": ""}` would
    // namespace as `<server>.` (server prefix + dot + empty) —
    // unidentifiable for CLI dispatch. Skip so other well-formed
    // tools from the same server still register, and record the
    // structured reason so callers can report the drop. Pin three
    // sub-cases: empty, whitespace, mixed with valid.
    use serde_json::json;
    let raw = vec![
        json!({"name": "good_one", "description": "ok"}),
        json!({"name": "",         "description": "should drop"}),
        json!({"name": "   ",      "description": "should drop too"}),
        json!({"name": "another_good", "description": "also ok"}),
    ];
    let out = parse_tools_list_entries(&raw, "test_server").unwrap();
    assert_eq!(
        out.decls.len(),
        2,
        "exactly the 2 non-empty entries should survive; got {} entries",
        out.decls.len()
    );
    assert_eq!(out.decls[0].id, "good_one");
    assert_eq!(out.decls[1].id, "another_good");
    assert_eq!(out.skipped.len(), 2, "both empty names should be recorded");
    for skipped in &out.skipped {
        assert_eq!(skipped.reason, SkipReason::EmptyName);
    }
}

#[test]
fn 도구_목록_항목_파싱은_패딩된_이름을_거부한다() {
    // Upstream MCP names become upeg Tool ids. They must be canonical
    // instead of being silently normalized.
    use serde_json::json;
    for bad_name in [" my_tool ", "\tspaced\n", "  prefix.foo "] {
        let raw = vec![json!({"name": bad_name, "description": "padded"})];
        match parse_tools_list_entries(&raw, "test_server") {
            Err(ImportError::Protocol(msg)) => {
                assert!(msg.contains("canonical and unpadded"), "got {msg}");
            }
            other => panic!("name={bad_name:?}: expected Protocol error, got {other:?}"),
        }
    }
}

#[test]
fn 도구_목록_항목_파싱은_변환_불가_입력_schema를_사유와_함께_스킵한다() {
    // Partial success: an inputSchema upeg's typed inputs cannot
    // express skips the ONE tool with a structured reason instead of
    // failing the whole server listing.
    use serde_json::json;
    for input_schema in [
        json!("not an object"),
        json!(42),
        json!(null),
        json!([]),
        json!({"type": "array", "items": {"type": "string"}}),
        json!({"type": "object", "properties": []}),
    ] {
        let raw = vec![json!({"name": "bad_schema", "inputSchema": input_schema})];
        let out = parse_tools_list_entries(&raw, "test_server")
            .expect("conversion failure must not fail the listing");
        assert!(out.decls.is_empty(), "bad schema must not produce a decl");
        assert_eq!(out.skipped.len(), 1);
        assert_eq!(out.skipped[0].id, "bad_schema");
        assert!(
            matches!(out.skipped[0].reason, SkipReason::UnsupportedInputSchema(_)),
            "reason must be UnsupportedInputSchema; got {:?}",
            out.skipped[0].reason
        );
    }
}

#[test]
fn 일부_tool의_schema_변환이_실패하면_성공분과_스킵을_함께_반환한다() {
    // The headline partial-success contract at the pure level: one
    // $ref tool + one good tool → good tool parses, $ref tool lands
    // in `skipped` with the unsupported keyword named in the reason.
    use serde_json::json;
    let raw = vec![
        json!({"name": "good_echo", "inputSchema": {"type": "object", "properties": {"input": {"type": "string"}}}}),
        json!({"name": "bad_ref", "inputSchema": {"$ref": "#/components/schemas/Input"}}),
        json!({"name": "bad_one_of", "inputSchema": {"type": "object", "properties": {"mode": {"oneOf": [{"type": "string"}]}}}}),
    ];
    let out = parse_tools_list_entries(&raw, "test_server").expect("partial success");
    assert_eq!(out.decls.len(), 1);
    assert_eq!(out.decls[0].id, "good_echo");
    assert_eq!(out.skipped.len(), 2);
    assert_eq!(out.skipped[0].id, "bad_ref");
    assert!(
        out.skipped[0].reason.to_string().contains("$ref"),
        "skip reason must name the unsupported keyword; got `{}`",
        out.skipped[0].reason
    );
    assert_eq!(out.skipped[1].id, "bad_one_of");
    assert!(
        out.skipped[1].reason.to_string().contains("oneOf"),
        "skip reason must name the unsupported keyword; got `{}`",
        out.skipped[1].reason
    );
}

#[test]
fn 도구_목록_항목_파싱은_객체_입력_schema를_허용한다() {
    use serde_json::json;
    let raw = vec![json!({
        "name": "ok_schema",
        "inputSchema": {"type": "object", "properties": {"x": {"type": "string"}}, "required": ["x"]}
    })];
    let out = parse_tools_list_entries(&raw, "test_server").expect("input schema imported");
    assert_eq!(out.decls.len(), 1);
    assert!(out.skipped.is_empty());
    assert_eq!(out.decls[0].input_spec.fields.len(), 1);
    assert_eq!(out.decls[0].input_spec.fields[0].name.as_str(), "x");
    assert!(out.decls[0].input_spec.fields[0].required);
}

#[test]
fn 도구_목록_항목_파싱은_output_schema를_출력_명세로_가져온다() {
    use serde_json::json;
    let raw = vec![json!({
        "name": "ok_output",
        "outputSchema": {
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "title": "Summary",
                    "description": "short output"
                },
                "count": { "type": "integer" }
            },
            "required": ["summary"],
            "additionalProperties": false
        }
    })];

    let out = parse_tools_list_entries(&raw, "test_server").expect("output schema imported");

    let summary = out.decls[0]
        .output_spec
        .fields
        .iter()
        .find(|field| field.name == "summary")
        .expect("summary output should exist");
    assert_eq!(summary.label.as_deref(), Some("Summary"));
    assert_eq!(summary.description.as_deref(), Some("short output"));
    assert!(matches!(&summary.kind, OutputKind::String));

    let count = out.decls[0]
        .output_spec
        .fields
        .iter()
        .find(|field| field.name == "count")
        .expect("count output should exist");
    assert!(matches!(&count.kind, OutputKind::Integer));
}

#[test]
fn 선언에서_메타로의_변환은_output_schema를_보존한다() {
    use serde_json::json;
    let raw = vec![json!({
        "name": "render_result",
        "outputSchema": {
            "type": "object",
            "properties": {
                "body": {
                    "type": "string",
                    "format": "markdown",
                    "x-upeg-kind": "markdown",
                    "description": "rendered body"
                }
            }
        }
    })];
    let decls = parse_tools_list_entries(&raw, "test_server")
        .expect("output schema imported")
        .decls;

    let meta =
        decl_to_meta("remote", &decls[0], McpReexport::Blocked).expect("decl should lower to meta");

    assert_eq!(meta.output_spec.fields.len(), 1);
    assert_eq!(meta.output_spec.fields[0].name, "body");
    assert_eq!(
        meta.output_spec.fields[0].description.as_deref(),
        Some("rendered body")
    );
    assert!(matches!(
        &meta.output_spec.fields[0].kind,
        OutputKind::Markdown
    ));
}

#[test]
fn 도구_목록_항목_파싱은_x_upeg_kind_출력을_보존한다() {
    use serde_json::json;
    let raw = vec![json!({
        "name": "render_result",
        "outputSchema": {
            "type": "object",
            "properties": {
                "body": {
                    "type": "string",
                    "format": "markdown",
                    "x-upeg-kind": "markdown"
                },
                "view": {
                    "type": "string",
                    "format": "uri",
                    "x-upeg-kind": "embedded_view",
                    "x-upeg-url": "https://example.com/"
                }
            }
        }
    })];

    let decls = parse_tools_list_entries(&raw, "test_server")
        .expect("output schema imported")
        .decls;

    let body = decls[0]
        .output_spec
        .fields
        .iter()
        .find(|field| field.name == "body")
        .expect("body output should exist");
    assert!(matches!(&body.kind, OutputKind::Markdown));

    let view = decls[0]
        .output_spec
        .fields
        .iter()
        .find(|field| field.name == "view")
        .expect("view output should exist");
    assert!(matches!(
        &view.kind,
        OutputKind::EmbeddedView { url } if url == "https://example.com/"
    ));
}

#[test]
fn 도구_목록_항목_파싱은_url이_없는_임베디드뷰_출력을_스킵한다() {
    use serde_json::json;
    let raw = vec![json!({
        "name": "render_result",
        "outputSchema": {
            "type": "object",
            "properties": {
                "view": {
                    "type": "string",
                    "format": "uri",
                    "x-upeg-kind": "embedded_view"
                }
            }
        }
    })];

    let out = parse_tools_list_entries(&raw, "test_server")
        .expect("conversion failure must not fail the listing");
    assert!(out.decls.is_empty());
    assert_eq!(out.skipped.len(), 1);
    assert_eq!(out.skipped[0].id, "render_result");
    let reason = out.skipped[0].reason.to_string();
    assert!(
        matches!(
            out.skipped[0].reason,
            SkipReason::UnsupportedOutputSchema(_)
        ),
        "reason must be UnsupportedOutputSchema; got {:?}",
        out.skipped[0].reason
    );
    assert!(
        reason.contains("x-upeg-url"),
        "missing embedded view URL reason must name x-upeg-url; got `{reason}`"
    );
}

#[test]
fn 도구_목록_항목_파싱은_변환_불가_output_schema를_사유와_함께_스킵한다() {
    use serde_json::json;
    for output_schema in [
        json!("not an object"),
        json!(42),
        json!(null),
        json!([]),
        json!({"type": "array", "items": {"type": "string"}}),
        json!({"type": "object", "properties": []}),
    ] {
        let raw = vec![json!({"name": "bad_schema", "outputSchema": output_schema})];
        let out = parse_tools_list_entries(&raw, "test_server")
            .expect("conversion failure must not fail the listing");
        assert!(out.decls.is_empty(), "bad schema must not produce a decl");
        assert_eq!(out.skipped.len(), 1);
        assert_eq!(out.skipped[0].id, "bad_schema");
        assert!(
            matches!(
                out.skipped[0].reason,
                SkipReason::UnsupportedOutputSchema(_)
            ),
            "reason must be UnsupportedOutputSchema; got {:?}",
            out.skipped[0].reason
        );
    }
}

#[test]
fn 도구_목록_항목_파싱은_이름이_누락되면_오류를_낸다() {
    // Distinguish missing vs empty. Missing `name` is a spec
    // violation → loud-fail Protocol error (the upstream is broken).
    // Empty is just bad data → silently skip.
    use serde_json::json;
    let raw = vec![json!({"description": "no name field"})];
    match parse_tools_list_entries(&raw, "test_server") {
        Err(ImportError::Protocol(msg)) => {
            assert!(
                msg.contains("missing `name`"),
                "missing-name error must mention the field; got `{msg}`"
            );
        }
        other => panic!("expected Protocol error, got {other:?}"),
    }
}

#[test]
fn 등록_서버는_빈_이름을_거부한다() {
    // Validate at the namespace-prefix boundary. An empty name
    // would namespace tools as `.<original_id>` (leading dot) which
    // is usable but confusing. Direct callers of register_server
    // (e.g., tests, future programmatic users) must hit this guard
    // even when bypassing `register_dir`'s `unwrap_or("server")`
    // fallback for non-UTF8 paths.
    let cfg = UpstreamConfig {
        command: "echo".into(),
        args: vec![],
        reexport: false,
    };
    for bad_name in ["", "  ", "\t\n"] {
        match register_server(bad_name, &cfg) {
            Err(ImportError::EmptyServerName) => {}
            other => panic!("name=`{bad_name}`: expected EmptyServerName, got {other:?}"),
        }
    }
}

#[test]
fn 등록_서버는_패딩된_이름을_거부한다() {
    let cfg = UpstreamConfig {
        command: "echo".into(),
        args: vec![],
        reexport: false,
    };
    for bad_name in [" myserver ", "server\t", "server. name"] {
        match register_server(bad_name, &cfg) {
            Err(ImportError::NonCanonicalServerName(name)) => assert_eq!(name, bad_name),
            other => panic!("name=`{bad_name}`: expected NonCanonicalServerName, got {other:?}"),
        }
    }
}

#[test]
fn 인벤토리가_없는_가림_확인은_충돌을_감지한다() {
    // The extracted check is directly unit-testable. Pin the
    // contract: a decl whose namespaced id matches a built-in
    // inventory id surfaces as `IdShadowsBuiltIn` with both server
    // name and ns_id echoed. `num.hex_to_decimal` is a stable
    // upeg-tools built-in; pairing server `num` with tool
    // `hex_to_decimal` namespaces as exactly that id.
    let decls = vec![RemoteToolDecl {
        id: "hex_to_decimal".into(),
        description: "would shadow built-in".into(),
        input_spec: InputSpec::empty(),
        output_spec: OutputSpec::empty(),
    }];
    match check_no_inventory_shadows("num", &decls) {
        Err(ImportError::IdShadowsBuiltIn { server, ns_id }) => {
            assert_eq!(server, "num");
            assert_eq!(ns_id, "num.hex_to_decimal");
        }
        other => panic!("expected IdShadowsBuiltIn, got {other:?}"),
    }
}

#[test]
fn 충돌이_없으면_인벤토리가_없는_가림_확인은_통과한다() {
    // Sanity: a non-colliding namespace passes the check.
    let decls = vec![
        RemoteToolDecl {
            id: "hex_to_dec".into(),
            description: String::new(),
            input_spec: InputSpec::empty(),
            output_spec: OutputSpec::empty(),
        },
        RemoteToolDecl {
            id: "uuid_v7".into(),
            description: String::new(),
            input_spec: InputSpec::empty(),
            output_spec: OutputSpec::empty(),
        },
    ];
    // `iter262unique` server prefix can't collide with any built-in.
    check_no_inventory_shadows("iter262unique", &decls).expect("non-colliding namespace must pass");
}

#[test]
fn 인벤토리가_없는_가림_확인은_runtime_도구킷_충돌을_거부한다() {
    let id = "github.com.iter262.runtime_conflict";
    upeg_runtime::toolbox_add_tool(ToolMeta {
        id,
        toolkit: "github.com",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "github.com")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "already registered",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::External,
        surfaces: &[Surface::Mcp],
        boards: &[],
    });

    let decls = vec![RemoteToolDecl {
        id: "com.iter262.runtime_conflict".into(),
        description: "from upstream".into(),
        input_spec: InputSpec::empty(),
        output_spec: OutputSpec::empty(),
    }];

    match check_no_inventory_shadows("github", &decls) {
        Err(ImportError::IdConflictsWithRegisteredTool {
            server,
            ns_id,
            existing_toolkit,
        }) => {
            assert_eq!(server, "github");
            assert_eq!(ns_id, id);
            assert_eq!(existing_toolkit, "github.com");
        }
        other => panic!("expected IdConflictsWithRegisteredTool, got {other:?}"),
    }

    let meta = upeg_runtime::toolbox_tool(id).expect("original tool remains registered");
    assert_eq!(meta.toolkit_id(), "github.com");
}

#[test]
fn 인벤토리가_없는_가림_확인은_첫번째_충돌에서_중단한다() {
    // Atomic-per-server semantics: if any decl collides, the check
    // returns Err on the FIRST one. Subsequent decls aren't
    // examined (we'd return the first error). Pin this so a
    // future change to "collect all collisions" is a deliberate
    // diff, not silent.
    let decls = vec![
        RemoteToolDecl {
            id: "uuid_v7".into(), // would collide as `id.uuid_v7`
            description: String::new(),
            input_spec: InputSpec::empty(),
            output_spec: OutputSpec::empty(),
        },
        RemoteToolDecl {
            id: "hex_to_dec".into(), // would collide as `id.hex_to_dec` — no, wait
            description: String::new(),
            input_spec: InputSpec::empty(),
            output_spec: OutputSpec::empty(),
        },
    ];
    // UpstreamServer `id` namespaces tools as `id.uuid_v7` (collides) and
    // `id.hex_to_dec` (no built-in by that name). First-collision
    // aborts on uuid_v7.
    match check_no_inventory_shadows("id", &decls) {
        Err(ImportError::IdShadowsBuiltIn { ns_id, .. }) => {
            assert_eq!(
                ns_id, "id.uuid_v7",
                "first-found collision must abort the check"
            );
        }
        other => panic!("expected IdShadowsBuiltIn for first-found collision, got {other:?}"),
    }
}

#[test]
fn 인벤토리가_없는_확인은_빈_선언을_가린다() {
    // Empty decls list — degenerate case, must pass cleanly.
    let decls: Vec<RemoteToolDecl> = vec![];
    check_no_inventory_shadows("any_server_name", &decls).expect("empty decls must always pass");
}

#[test]
fn id가_내장을_가리면_메시지는_이름변경_경로를_설명한다() {
    // error message must name both the offending server    // and the colliding ns_id, and hint at the rename remedy.
    let err = ImportError::IdShadowsBuiltIn {
        server: "convert".into(),
        ns_id: "num.hex_to_decimal".into(),
    };
    let msg = format!("{err}");
    assert!(
        msg.contains("`convert`"),
        "message must name the server; got `{msg}`"
    );
    assert!(
        msg.contains("`num.hex_to_decimal`"),
        "message must name the colliding ns_id; got `{msg}`"
    );
    assert!(
        msg.contains("rename"),
        "message must hint at the remedy; got `{msg}`"
    );
    assert!(
        msg.contains("my_convert"),
        "message must give a concrete rename example; got `{msg}`"
    );
}

#[test]
fn 빈_서버_이름_메시지는_네임스페이스의_역할을_설명한다() {
    let msg = format!("{}", ImportError::EmptyServerName);
    assert!(
        msg.contains("non-empty"),
        "message must say `non-empty`; got `{msg}`"
    );
    assert!(
        msg.contains("namespace prefix"),
        "message should explain WHY the name matters; got `{msg}`"
    );
}

#[test]
fn 표시_값은_큰_json을_줄인다() {
    // A pathological upstream entry (e.g., a multi-MB inputSchema)
    // must not bloat the error response. Pin both behaviours: small
    // values pass through, large values get truncated with marker +
    // length suffix.
    use serde_json::json;
    let small = json!({"name": "ok", "description": "short"});
    let displayed = display_value(&small);
    assert!(
        displayed.contains("\"name\""),
        "small entry passes through with content; got `{displayed}`"
    );

    // Large value: 10 KB description.
    let big_str = "x".repeat(10_000);
    let big = json!({"description": big_str});
    let displayed = display_value(&big);
    assert!(
        displayed.contains("…"),
        "10 KB entry must trigger truncation; got prefix `{}`",
        &displayed[..50.min(displayed.len())]
    );
    assert!(
        displayed.contains("chars total"),
        "must report total length"
    );
    assert!(
        displayed.len() < 500,
        "10 KB input must produce bounded output; got {} chars",
        displayed.len()
    );
}

#[test]
fn 표시_값은_짧은_멀티바이트를_잘라낼_때_손상시키지_않는다() {
    // Parallel multibyte safety to display_id. The byte-cheap
    // pre-filter must not over-trigger on multibyte input (e.g.,
    // 100 emojis = 400 bytes / 100 chars: byte check fires at
    // MAX=200 bytes but `chars().take(200)` returns all 100 chars).
    // Emitting "…(truncated, 100 chars total)" here would misleadingly
    // suggest data was elided. Only emit the truncation marker when
    // the prefix is actually shorter.
    use serde_json::json;
    // 100 emojis (each 4 UTF-8 bytes = 400 bytes; 100 chars).
    // JSON-stringified: `"<emojis>"` = 402 bytes / 102 chars.
    // Byte check (>200) fires; char check (≤200) saves it.
    let emojis: String = "🦀".repeat(100);
    let v = json!(emojis);
    let displayed = display_value(&v);
    assert!(
        !displayed.contains("…"),
        "102-char emoji string must NOT report truncation (byte=402 > MAX=200, but chars=102 ≤ MAX); got `{}`",
        &displayed[..displayed
            .char_indices()
            .nth(40)
            .map_or(displayed.len(), |(i, _)| i)]
    );
    assert!(
        !displayed.contains("truncated"),
        "char-fitting input must not include the truncation suffix; got `{displayed:?}`"
    );
}

#[test]
fn 도구_배열_추출은_누락된_경우와_잘못된_타입을_처리한다() {
    // array-level missing-vs-wrong-type disambiguation.
    // Missing → empty Vec (legitimate). Wrong type → Protocol error.
    use serde_json::json;

    // Missing tools field → empty Vec (server has no tools).
    let r = extract_tools_array(&json!({})).unwrap();
    assert!(r.is_empty(), "missing `tools` field should yield empty Vec");

    // Empty array → empty Vec (server explicitly has no tools).
    let r = extract_tools_array(&json!({"tools": []})).unwrap();
    assert!(r.is_empty(), "empty `tools` array should yield empty Vec");

    // Valid array passes through.
    let r = extract_tools_array(&json!({"tools": [{"name": "x"}]})).unwrap();
    assert_eq!(r.len(), 1);

    // Wrong types → Protocol error.
    for bad in [json!("string"), json!(42), json!(null), json!({"obj": 1})] {
        match extract_tools_array(&json!({"tools": bad.clone()})) {
            Err(ImportError::Protocol(msg)) => {
                assert!(
                    msg.contains("must be an array"),
                    "wrong-type `tools={bad:?}`: error must say `must be an array`; got `{msg}`"
                );
            }
            other => panic!("`tools={bad:?}`: expected Protocol error, got {other:?}"),
        }
    }
}

#[test]
fn 도구_목록_항목_파싱은_잘못된_타입의_이름에_오류를_낸다() {
    // Missing-vs-wrong-type disambiguation: a `name: 42` entry
    // must not return "missing `name`" (that would mislead the
    // upstream server author). Present-but-wrong-type returns a
    // distinct message.
    use serde_json::json;
    for bad_name in [json!(42), json!(null), json!(["array"]), json!({"obj": 1})] {
        let raw = vec![json!({"name": bad_name})];
        match parse_tools_list_entries(&raw, "test_server") {
            Err(ImportError::Protocol(msg)) => {
                assert!(
                    msg.contains("must be a string"),
                    "wrong-type error must say `must be a string`; got `{msg}`"
                );
                // Ensures the missing-vs-wrong-type disambiguation
                // is real: a wrong-type error must NOT use the
                // "missing" phrasing the missing-case test pinned,
                // otherwise the two error paths would collide.
                assert!(
                    !msg.contains("missing `name`"),
                    "wrong-type must not pretend the field is missing; got `{msg}`"
                );
            }
            other => panic!("name={bad_name:?}: expected Protocol error, got {other:?}"),
        }
    }
}

#[test]
fn 스폰_알수없는_명령은_깨끗한_오류를_반환한다() {
    let cfg = UpstreamConfig {
        command: "definitely_not_a_real_program_iter50".into(),
        args: vec![],
        reexport: false,
    };
    let result = UpstreamServer::spawn("noexec", &cfg);
    match result {
        Err(ImportError::Spawn { command, .. }) => {
            assert!(command.contains("definitely_not_a_real"));
        }
        Err(other) => panic!("expected Spawn, got: {other}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[cfg(unix)]
#[test]
fn 응답_없는_서버_스폰은_initialize_시간초과를_반환한다() {
    let cfg = UpstreamConfig {
        command: "sleep".into(),
        args: vec!["60".into()],
        reexport: false,
    };
    let started = std::time::Instant::now();

    let result = UpstreamServer::spawn_with_timeouts("silent", &cfg, short_timeouts());

    match result {
        Err(ImportError::Timeout { method, .. }) => {
            assert_eq!(method, "initialize");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(2),
                "initialize timeout should be bounded"
            );
        }
        Err(other) => panic!("expected initialize timeout, got: {other}"),
        Ok(_) => panic!("expected timeout, got Ok"),
    }
}

#[cfg(unix)]
#[test]
fn 응답_없는_서버의_도구_목록_호출은_시간초과를_반환한다() {
    let (root, script) = write_unix_script(
        "tools-list-timeout",
        r#"#!/bin/sh
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}'
IFS= read -r line
sleep 60
"#,
    );
    let cfg = unix_script_config(&script);
    let mut server = UpstreamServer::spawn_with_timeouts("list_timeout", &cfg, short_timeouts())
        .expect("initialize should succeed");
    let started = std::time::Instant::now();

    let result = server.tools_list();

    match result {
        Err(ImportError::Timeout { method, .. }) => {
            assert_eq!(method, "tools/list");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(2),
                "tools/list timeout should be bounded"
            );
        }
        Err(other) => panic!("expected tools/list timeout, got: {other}"),
        Ok(_) => panic!("expected timeout, got Ok"),
    }
    drop(server);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn 응답_없는_서버의_도구_호출은_시간초과를_반환한다() {
    use serde_json::json;

    let (root, script) = write_unix_script(
        "tools-call-timeout",
        r#"#!/bin/sh
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}'
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"hang","description":"fixture","inputSchema":{"type":"object","properties":{}}}]}}'
IFS= read -r line
sleep 60
"#,
    );
    let cfg = unix_script_config(&script);
    let mut server = UpstreamServer::spawn_with_timeouts("call_timeout", &cfg, short_timeouts())
        .expect("initialize should succeed");
    let tools = server.tools_list().expect("tools/list should succeed");
    assert_eq!(tools.decls.len(), 1);
    let started = std::time::Instant::now();

    let result = server.call("hang", &json!({}));

    match result {
        Err(ImportError::Timeout { method, .. }) => {
            assert_eq!(method, "tools/call");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(2),
                "tools/call timeout should be bounded"
            );
        }
        Err(other) => panic!("expected tools/call timeout, got: {other}"),
        Ok(_) => panic!("expected timeout, got Ok"),
    }
    drop(server);
    let _ = std::fs::remove_dir_all(root);
}
