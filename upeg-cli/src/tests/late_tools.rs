//! Late CLI tool-surface tests split from `tests_c.rs` to keep modules below the line budget.

use super::common::parse;
use crate::*;

#[test]
fn 사용자지정_길이의_랜덤_hex_바이트는_호출을_통해_dispatch된다() {
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
fn 보안_비밀번호_도구는_call_하위명령으로_dispatch된다() {
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
fn 텍스트_뒤집기_하위명령은_문자열을_역순으로_출력한다() {
    let out = run(parse(&["upeg", "text", "reverse", "Hello"])).unwrap();
    assert_eq!(out, "olleH\n");
}

#[test]
fn html_인코딩과_디코딩_하위명령은_왕복이_보존된다() {
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
fn html_디코딩_유효하지_않은_숫자는_도구_실패를_반환한다() {
    let r = run(parse(&["upeg", "convert", "html-decode", "&#xZZ;"]));
    assert!(matches!(r, Err(CliError::ToolFailed(_))));
}

#[test]
fn 늦게_추가된_텍스트_및_html_도구는_목록과_dispatch_모두에_나타난다() {
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
