//! Late CLI tool-surface tests split from `tests_c.rs` to keep modules below the line budget.

use super::common::parse;
use crate::*;

#[test]
fn call_dispatches_random_hex_bytes_with_a_custom_length() {
    // `upeg call` route accepts the `count` arg as a JSON integer.
    let out = run(parse(&[
        "upeg",
        "call",
        "security.bytes_generate",
        "-a",
        "count=4",
    ]))
    .unwrap();
    assert_eq!(out.trim_end_matches('\n').len(), 8);
}

#[test]
fn call_dispatches_password_generation_and_strength_estimation() {
    let generated = run(parse(&[
        "upeg",
        "call",
        "security.password_generate",
        "-a",
        "count=12",
        "-a",
        "include_symbols=false",
    ]))
    .unwrap();
    let password = generated.trim_end_matches('\n');
    assert_eq!(password.len(), 12);
    assert!(password.chars().all(|c| c.is_ascii_alphanumeric()));

    let strength = run(parse(&[
        "upeg",
        "call",
        "security.password_estimate",
        "-a",
        "input=password123",
    ]))
    .unwrap();
    assert!(strength.contains("\"common_pattern\""));
}

// ─── text.reverse + html_encode/decode ────────────

#[test]
fn text_reverse_subcommand_outputs_the_reversed_string() {
    let out = run(parse(&["upeg", "text", "reverse", "Hello"])).unwrap();
    assert_eq!(out, "olleH\n");
}

#[test]
fn html_encode_and_decode_subcommands_round_trip() {
    let enc = run(parse(&[
        "upeg",
        "convert",
        "html-encode",
        "<p>hi & bye</p>",
    ]))
    .unwrap();
    assert_eq!(enc, "&lt;p&gt;hi &amp; bye&lt;/p&gt;\n");
    let dec = run(parse(&[
        "upeg",
        "convert",
        "html-decode",
        "&lt;p&gt;hi &amp; bye&lt;/p&gt;",
    ]))
    .unwrap();
    assert_eq!(dec, "<p>hi & bye</p>\n");
}

#[test]
fn html_decode_returns_tool_failure_for_invalid_numeric_entities() {
    let r = run(parse(&["upeg", "convert", "html-decode", "&#xZZ;"]));
    assert!(matches!(r, Err(CliError::ToolFailed(_))));
}

#[test]
fn text_reverse_and_html_tools_are_listed_and_html_encode_has_call_parity() {
    let listed = run(parse(&["upeg", "tool", "list"])).unwrap();
    for expected in ["text.reverse", "convert.html_encode", "convert.html_decode"] {
        assert!(
            listed.contains(expected),
            "missing `{expected}` in:\n{listed}"
        );
    }
    // Direct vs `upeg call` parity.
    let direct = run(parse(&["upeg", "convert", "html-encode", "&"])).unwrap();
    let viacall = run(parse(&[
        "upeg",
        "call",
        "convert.html_encode",
        "-a",
        "input=&",
    ]))
    .unwrap();
    assert_eq!(direct, viacall);
}
