//! Dynamic `{toolkit} {tool}` route UX: `--help` interception, output
//! flag parity with `upeg call`, and `--local` attach opt-out.

use super::common::parse;
use crate::*;

// ─── --help interception ─────────────────────────────────

#[test]
fn toolkit_help_shows_tools_with_one_line_descriptions() {
    for argv in [&["upeg", "num", "--help"][..], &["upeg", "num", "-h"][..]] {
        let out = run(parse(argv)).expect("toolkit help must not dispatch");
        assert!(
            out.contains("usage: upeg num <tool> [args...]"),
            "missing usage line for {argv:?}:\n{out}"
        );
        assert!(
            out.contains("hex-to-decimal"),
            "tool listing must use kebab spelling:\n{out}"
        );
        assert!(
            out.contains("Parse a hex string"),
            "each tool row must carry its one-line description:\n{out}"
        );
    }
}

#[test]
fn tool_help_shows_schema_based_usage_and_both_invocation_examples() {
    let out = run(parse(&["upeg", "num", "hex-to-decimal", "--help"]))
        .expect("tool help must not dispatch");
    // Reuses the `tool show` block: schema inputs with type/required.
    assert!(
        out.contains("id            num.hex_to_decimal"),
        "tool help must reuse the tool show block:\n{out}"
    );
    assert!(
        out.contains("(string, required)"),
        "inputs must carry type + required marker:\n{out}"
    );
    // Both invocation forms, ready to copy-paste.
    assert!(
        out.contains("upeg num hex-to-decimal <input>"),
        "missing dynamic route example:\n{out}"
    );
    assert!(
        out.contains("upeg call num.hex_to_decimal -a input=<string>"),
        "missing call example:\n{out}"
    );
}

#[test]
fn help_for_an_unknown_toolkit_returns_an_error() {
    let result = run(parse(&["upeg", "no_such_toolkit_zzz", "--help"]));
    match result {
        Err(CliError::ToolFailed(msg)) => {
            assert!(msg.contains("unknown toolkit"), "got: {msg}");
        }
        other => panic!("expected unknown toolkit error, got {other:?}"),
    }
}

#[test]
fn help_for_an_unknown_tool_returns_an_unknown_tool_error() {
    let result = run(parse(&["upeg", "num", "no-such-tool", "--help"]));
    assert!(
        matches!(result, Err(CliError::UnknownTool(_))),
        "got {result:?}"
    );
}

// ─── output flag parity with `upeg call` ─────────────────

#[test]
fn dynamic_route_json_returns_the_same_envelope_as_call() {
    let direct =
        run(parse(&["upeg", "num", "hex-to-decimal", "0xff", "--json"])).expect("dynamic --json");
    let via_call = run(parse(&[
        "upeg",
        "call",
        "num.hex_to_decimal",
        "-a",
        "input=0xff",
        "--json",
    ]))
    .expect("call --json");
    assert_eq!(direct, via_call, "canonical envelope must match call");

    let value: serde_json::Value = serde_json::from_str(&direct).expect("canonical JSON");
    assert_eq!(value["ok"], true);
    assert_eq!(value["outputs"][0]["value"], 255);
}

#[test]
fn dynamic_route_field_and_pretty_flags_match_call_semantics() {
    let field = run(parse(&[
        "upeg",
        "num",
        "hex-to-decimal",
        "0xff",
        "--field",
        "result",
    ]))
    .expect("dynamic --field");
    assert_eq!(field, "255\n");

    let direct_pretty = run(parse(&[
        "upeg",
        "num",
        "hex-to-decimal",
        "0xff",
        "--pretty",
    ]))
    .expect("dynamic --pretty");
    let call_pretty = run(parse(&[
        "upeg",
        "call",
        "num.hex_to_decimal",
        "-a",
        "input=0xff",
        "--pretty",
    ]))
    .expect("call --pretty");
    assert_eq!(direct_pretty, call_pretty);
}

#[test]
fn dynamic_route_json_returns_tool_errors_as_stdout_failure_envelopes() {
    let result = run(parse(&["upeg", "num", "hex-to-decimal", "0xZZ", "--json"]));
    match result {
        Err(CliError::StdoutFailure { stdout }) => {
            let value: serde_json::Value = serde_json::from_str(&stdout).expect("failure JSON");
            assert_eq!(value["ok"], false);
            assert!(
                value["error"]["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("invalid hex")
            );
        }
        other => panic!("expected stdout failure JSON, got {other:?}"),
    }
}

#[test]
fn dynamic_route_rejects_conflicting_output_flags() {
    let result = run(parse(&[
        "upeg",
        "num",
        "hex-to-decimal",
        "0xff",
        "--json",
        "--pretty",
    ]));
    match result {
        Err(CliError::ToolFailed(msg)) => {
            assert!(msg.contains("cannot be combined"), "got: {msg}");
        }
        other => panic!("expected output-mode conflict error, got {other:?}"),
    }
}

#[test]
fn flag_like_tokens_after_double_dash_are_passed_as_input_values() {
    // `--` ends option scanning: `--json` here is slugify's INPUT, not
    // an output flag.
    let out = run(parse(&["upeg", "text", "slugify", "--", "--json"]))
        .expect("literal positional after --");
    assert_eq!(out, "json\n");
}

// ─── --local attach opt-out ──────────────────────────────

#[test]
fn local_flag_dispatches_in_process_for_dynamic_routes_and_call() {
    let direct =
        run(parse(&["upeg", "num", "hex-to-decimal", "0xff", "--local"])).expect("dynamic --local");
    assert_eq!(direct, "255\n");

    let via_call = run(parse(&[
        "upeg",
        "call",
        "num.hex_to_decimal",
        "-a",
        "input=0xff",
        "--local",
    ]))
    .expect("call --local");
    assert_eq!(via_call, "255\n");
}

// ─── tool show invocation examples ───────────────────────

#[test]
fn tool_show_ends_with_examples_of_both_invocation_forms() {
    let out = run(parse(&["upeg", "tool", "show", "num.hex_to_decimal"])).expect("tool show");
    let invoke_index = out.find("invoke\n").expect("invoke section present");
    let tail = &out[invoke_index..];
    assert!(
        tail.contains("  upeg num hex-to-decimal <input>"),
        "missing dynamic route example:\n{out}"
    );
    assert!(
        tail.contains("  upeg call num.hex_to_decimal -a input=<string>"),
        "missing call example:\n{out}"
    );
}

// ─── completions carry tool ids ──────────────────────────

#[test]
fn completion_scripts_include_dynamic_tool_id_candidates() {
    let out = run(parse(&["upeg", "completions", "bash"])).expect("bash completions");
    assert!(
        out.contains("num.hex_to_decimal"),
        "`upeg call <TAB>` candidates must include canonical ids:\n(script elided)"
    );
    assert!(
        out.contains("hex-to-decimal"),
        "`upeg num <TAB>` candidates must include kebab tool names:\n(script elided)"
    );

    let zsh = run(parse(&["upeg", "completions", "zsh"])).expect("zsh completions");
    assert!(zsh.contains("num.hex_to_decimal"));
}
