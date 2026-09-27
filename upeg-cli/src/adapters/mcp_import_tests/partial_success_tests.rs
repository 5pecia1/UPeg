//! Subprocess-driven partial-success tests for MCP import: mixed
//! convertible/unconvertible tool lists, the all-skipped server
//! error, reexport interplay, and the atomic conflict policy. Split
//! from the parent test module to keep both files inside the
//! 1000-line file-size budget.

use super::*;

/// Script server whose `tools/list` mixes convertible and
/// unconvertible tools. Shared by the partial-success and reexport
/// interplay tests below.
#[cfg(unix)]
fn write_partial_success_server(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    write_unix_script(
        name,
        r##"#!/bin/sh
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}'
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"good_echo","description":"convertible","inputSchema":{"type":"object","properties":{"input":{"type":"string"}}}},{"name":"bad_ref","description":"uses $ref","inputSchema":{"$ref":"#/defs/x"}}]}}'
IFS= read -r line
sleep 60
"##,
    )
}

#[cfg(unix)]
#[test]
fn server_registers_only_successes_and_reports_skips_on_partial_failure() {
    let (root, script) = write_partial_success_server("partial-success");
    let cfg = unix_script_config(&script);

    let outcome = register_server("partial_srv", &cfg).expect("partial success must register");

    assert_eq!(
        outcome.registered_ids(),
        &["partial_srv.good_echo"][..],
        "only the convertible tool registers"
    );
    assert_eq!(outcome.skipped.len(), 1);
    assert_eq!(outcome.skipped[0].id, "bad_ref");
    assert!(
        matches!(
            outcome.skipped[0].reason,
            SkipReason::UnsupportedInputSchema(_)
        ),
        "reason must be UnsupportedInputSchema; got {:?}",
        outcome.skipped[0].reason
    );
    assert!(
        outcome.skipped[0].reason.to_string().contains("$ref"),
        "reason must name the unsupported keyword; got `{}`",
        outcome.skipped[0].reason
    );
    assert!(upeg_runtime::toolbox_tool("partial_srv.good_echo").is_some());
    assert!(
        upeg_runtime::toolbox_tool("partial_srv.bad_ref").is_none(),
        "skipped tool must not appear in the toolbox"
    );

    drop(outcome);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn server_registration_fails_when_all_tools_fail_conversion() {
    let (root, script) = write_unix_script(
        "all-skipped",
        r##"#!/bin/sh
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}'
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"bad_ref","inputSchema":{"$ref":"#/defs/x"}},{"name":"bad_any_of","inputSchema":{"anyOf":[{"type":"string"}]}}]}}'
IFS= read -r line
sleep 60
"##,
    );
    let cfg = unix_script_config(&script);

    match register_server("all_skipped_srv", &cfg) {
        Err(ImportError::AllToolsSkipped { server, skipped }) => {
            assert_eq!(server, "all_skipped_srv");
            assert_eq!(skipped.len(), 2);
            let ids: Vec<&str> = skipped.iter().map(|tool| tool.id.as_str()).collect();
            assert_eq!(ids, ["bad_ref", "bad_any_of"]);
        }
        other => panic!("expected AllToolsSkipped, got {other:?}"),
    }
    assert!(upeg_runtime::toolbox_tool("all_skipped_srv.bad_ref").is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn skipped_tool_is_not_exposed_even_with_reexport_opt_in() {
    // reexport interplay: `reexport = true` widens the surfaces of
    // REGISTERED imports to include MCP; a skipped tool registers on
    // no surface at all, so the opt-in cannot resurrect it.
    let (root, script) = write_partial_success_server("reexport-skip");
    let mut cfg = unix_script_config(&script);
    cfg.reexport = true;

    let outcome = register_server("reexport_skip_srv", &cfg).expect("partial success");

    let good = upeg_runtime::toolbox_tool("reexport_skip_srv.good_echo")
        .expect("registered import present");
    assert!(
        good.surfaces.contains(&Surface::Mcp),
        "reexport opt-in must expose the registered tool over MCP"
    );
    assert!(
        upeg_runtime::toolbox_tool("reexport_skip_srv.bad_ref").is_none(),
        "skipped tool must not be reexported (it is not registered at all)"
    );
    assert_eq!(outcome.skipped.len(), 1);
    assert_eq!(outcome.skipped[0].id, "bad_ref");

    drop(outcome);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn namespace_conflict_rejects_whole_server_not_just_the_tool() {
    // Conflict policy stays atomic per-server (trust boundary): a
    // namespaced id shadowing a built-in rejects the WHOLE server —
    // including its otherwise-fine tools — instead of skipping just
    // the colliding tool. Server name `num` + upstream tool
    // `hex_to_decimal` namespaces as the built-in `num.hex_to_decimal`.
    let (root, script) = write_unix_script(
        "conflict-atomic",
        r#"#!/bin/sh
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}'
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"hex_to_decimal","inputSchema":{"type":"object","properties":{"input":{"type":"string"}}}},{"name":"fresh_conflict_probe","inputSchema":{"type":"object","properties":{"input":{"type":"string"}}}}]}}'
IFS= read -r line
sleep 60
"#,
    );
    let cfg = unix_script_config(&script);

    match register_server("num", &cfg) {
        Err(ImportError::IdShadowsBuiltIn { server, ns_id }) => {
            assert_eq!(server, "num");
            assert_eq!(ns_id, "num.hex_to_decimal");
        }
        other => panic!("expected IdShadowsBuiltIn, got {other:?}"),
    }
    assert!(
        upeg_runtime::toolbox_tool("num.fresh_conflict_probe").is_none(),
        "server-level rejection must not leave partial registrations behind"
    );
    let _ = std::fs::remove_dir_all(root);
}
