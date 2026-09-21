//! `pty` is an External-invoker field. Declaring it on another invoker
//! is a manifest bug; declaring it on a host with no pseudoterminal is
//! not — that costs the one tool and leaves the rest of the file alone.

use crate::dispatcher::external::pty::HOST_SUPPORTS_PTY;
use crate::parse::{HostCapabilities, LoweredToolkit, toolkit_to_meta_and_tools};
use crate::{LoadError, ToolkitToml};

use super::super::parse_single_tool;

/// The two answers a host can give, named so the assertions below read
/// as the contract rather than as two bare booleans.
const HOST_WITH_PTY: HostCapabilities = HostCapabilities::with_pty(true);
const HOST_WITHOUT_PTY: HostCapabilities = HostCapabilities::with_pty(false);

const TERMINAL_TOOL: &str = "y.terminal";
const PLAIN_TOOL: &str = "y.plain";

/// One manifest carrying both kinds of tool, so "the rest of the file
/// still loads" is something the assertions can actually observe rather
/// than something the prose claims.
fn two_tool_manifest(pty: &str) -> ToolkitToml {
    toml::from_str(&format!(
        r#"
id = "y"

[[tools]]
id = "terminal"
pegboard_units = "U1"
invoker = "External"
command = "git"
pty = {pty}

[[tools]]
id = "plain"
pegboard_units = "U1"
invoker = "External"
command = "git"
"#
    ))
    .expect("the fixed manifest parses")
}

fn loaded_ids(lowered: &LoweredToolkit) -> Vec<&'static str> {
    lowered.tools.iter().map(|(meta, _toml)| meta.id).collect()
}

fn lower(pty: &str, host: HostCapabilities) -> LoweredToolkit {
    toolkit_to_meta_and_tools(&two_tool_manifest(pty), host).expect("the manifest is valid")
}

#[test]
fn host_with_pty_accepts_declaration_verbatim() {
    // Given/When
    let lowered = lower("true", HOST_WITH_PTY);

    // Then
    assert_eq!(loaded_ids(&lowered), [TERMINAL_TOOL, PLAIN_TOOL]);
    assert!(lowered.skipped.is_empty(), "no reason to skip anything");
}

#[test]
fn host_without_pty_skips_only_that_tool() {
    // Not a file-level rejection: the manifest is correct, and the other
    // tools in it never asked for a terminal.
    let lowered = lower("true", HOST_WITHOUT_PTY);

    assert_eq!(
        loaded_ids(&lowered),
        [PLAIN_TOOL],
        "tools that never asked for a terminal still load"
    );
    let [skipped_tool] = lowered.skipped.as_slice() else {
        panic!("exactly one tool should be skipped: {:?}", lowered.skipped);
    };
    assert_eq!(skipped_tool.id, TERMINAL_TOOL);
    assert!(
        matches!(skipped_tool.reason, LoadError::PtyUnsupportedOnHost),
        "actual reason: {:?}",
        skipped_tool.reason
    );
}

#[test]
fn pty_false_is_not_skipped_on_any_host() {
    // `pty = false` says nothing the default did not already say, so no
    // host has anything to refuse.
    for host in [HOST_WITH_PTY, HOST_WITHOUT_PTY] {
        let lowered = lower("false", host);

        assert_eq!(loaded_ids(&lowered), [TERMINAL_TOOL, PLAIN_TOOL]);
        assert!(lowered.skipped.is_empty());
    }
}

#[test]
fn pty_skip_reason_reports_skip_and_alternative() {
    let message = LoadError::PtyUnsupportedOnHost.to_string();

    assert!(message.contains("pty = true"), "{message}");
    assert!(
        message.contains("skipped"),
        "a reason that does not say what vanished leaves only a hole: {message}"
    );
    assert!(
        message.contains("color = \"force\""),
        "a refusal without an alternative is a dead end: {message}"
    );
}

#[test]
fn pty_declaration_on_non_external_tool_is_rejected() {
    let error = parse_single_tool(
        r#"id = "y.x"
           toolkit = "y"
           invoker = "Http"
           url = "https://example.com"
           method = "GET"
           pty = true"#,
    )
    .expect_err("pty is an External-only field");

    assert!(
        matches!(
            error,
            LoadError::InvokerFieldConflict {
                field: "pty",
                expected_invoker: "External",
                ..
            }
        ),
        "actual error: {error:?}"
    );
}

#[cfg(unix)]
#[test]
fn pty_declaration_on_external_tool_loads_verbatim() {
    use super::super::parse_full_single_tool;

    let (_meta, toml) = parse_full_single_tool(
        r#"id = "y.x"
           toolkit = "y"
           invoker = "External"
           command = "sh"
           pty = true"#,
    )
    .expect("a pty declaration is valid on Unix hosts");

    assert_eq!(toml.pty, Some(true));
}

#[test]
fn public_parser_forwards_host_skip_verdicts() {
    // `parse_toolkit_full` discards `skipped`. Information lost there
    // cannot be revived by any surface, so a reporting caller needs at
    // least one channel that still carries it.
    let result = crate::parse_toolkit_with_skips(
        r#"
id = "y"

[[tools]]
id = "terminal"
pegboard_units = "U1"
invoker = "External"
command = "git"
pty = true
"#,
    )
    .expect("the manifest is valid");

    let ids: Vec<&'static str> = result.tools.iter().map(|(meta, _)| meta.id).collect();
    if HOST_SUPPORTS_PTY {
        assert_eq!(ids, [TERMINAL_TOOL]);
        assert!(result.skipped.is_empty());
    } else {
        assert!(ids.is_empty());
        let [skipped_tool] = result.skipped.as_slice() else {
            panic!("exactly one tool should be skipped: {:?}", result.skipped);
        };
        assert_eq!(skipped_tool.id, TERMINAL_TOOL);
        assert!(matches!(
            skipped_tool.reason,
            LoadError::PtyUnsupportedOnHost
        ));
    }
}
