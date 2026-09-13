#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::manual_let_else,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Sync check between the `#[tool]` macros' enum-ident allow-lists (in
//! `upeg-tool-grammar/src/enums.rs`) and the real `upeg_core` enums they
//! mirror.
//!
//! Neither macro crate (`upeg-macros`, the built-in one, nor the WASM-guest
//! `upeg-plugin-macros`) can depend on `upeg-core` (the dependency runs the
//! other way — `upeg-core` depends on `upeg-macros` for the `#[tool]`
//! attribute), so `upeg-tool-grammar` — a plain library crate shared by
//! both macro crates — keeps a hand-written copy of each enum's variant
//! names for its friendly "unknown pin `Foo`; expected one of: ..." error
//! messages.
//!
//! `upeg-tool-grammar` is a normal lib (not a proc-macro crate), so its
//! `ALLOWED_*` consts are `pub` and could in principle be imported here as
//! Rust values directly. This test instead compares them against the real
//! `upeg_core` enum variants by reading both source files as plain text at
//! compile time (`include_str!`) and independently parsing out (a) each
//! enum's variant names from `upeg-core/src/types.rs`, and (b) each allow
//! list's string literals from `upeg-tool-grammar/src/enums.rs`, then
//! asserting the two lists are equal (same variants, same order) — this
//! keeps the check working purely from source text, with no dependency on
//! `upeg_tool_grammar`'s compiled surface. If either file drifts — a new
//! `PinKind` variant added without updating the grammar crate's allow list,
//! or vice versa — this test fails with a readable diff.

const CORE_TYPES_SRC: &str = include_str!("../../upeg-core/src/types.rs");
const GRAMMAR_ENUMS_SRC: &str = include_str!("../../upeg-tool-grammar/src/enums.rs");

/// Extracts the variant names of `pub enum {enum_name} { ... }` from a
/// `upeg-core/src/types.rs`-style source file. Handles the plain fieldless
/// enums used for `PinKind`/`PegboardUnits`/`Invoker`/`Surface`: one variant
/// per line, optional leading `///` doc-comment lines (dropped), optional
/// trailing `// ...` line comments (dropped), trailing `,` stripped.
fn enum_variant_names(source: &str, enum_name: &str) -> Vec<String> {
    let needle = format!("pub enum {enum_name} {{");
    let start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("`{needle}` not found in upeg-core/src/types.rs"));
    let block = brace_delimited_block(&source[start..]);

    block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("//") && !line.starts_with("#["))
        .map(|line| {
            line.split("//")
                .next()
                .unwrap_or_default()
                .trim()
                .trim_end_matches(',')
                .to_string()
        })
        .filter(|variant| !variant.is_empty())
        .collect()
}

/// Extracts the string literals of `pub const {const_name}: &[&str] =
/// &[ "..", ".." ];` from `upeg-tool-grammar/src/enums.rs`.
fn allowed_list(source: &str, const_name: &str) -> Vec<String> {
    let needle = format!("const {const_name}");
    let start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("`{needle}` not found in upeg-tool-grammar/src/enums.rs"));
    let after_name = &source[start..];
    let eq_idx = after_name
        .find('=')
        .unwrap_or_else(|| panic!("no `=` found after `{needle}`"));
    let block = brace_or_bracket_delimited_block(&after_name[eq_idx..], '[', ']');

    block
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// Returns the content strictly between the first `{` in `text` and its
/// matching `}` (brace-depth aware, so nested braces don't confuse it).
fn brace_delimited_block(text: &str) -> &str {
    brace_or_bracket_delimited_block(text, '{', '}')
}

fn brace_or_bracket_delimited_block(text: &str, open: char, close: char) -> &str {
    let open_idx = text
        .find(open)
        .unwrap_or_else(|| panic!("no `{open}` found"));
    let mut depth = 0_i32;
    let mut end_idx = None;
    for (offset, ch) in text[open_idx..].char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                end_idx = Some(open_idx + offset);
                break;
            }
        }
    }
    let end_idx = end_idx.unwrap_or_else(|| panic!("no matching `{close}` found"));
    &text[open_idx + 1..end_idx]
}

fn assert_variant_lists_match(enum_name: &str, const_name: &str) {
    let core_variants = enum_variant_names(CORE_TYPES_SRC, enum_name);
    let macro_allowed = allowed_list(GRAMMAR_ENUMS_SRC, const_name);
    assert_eq!(
        macro_allowed, core_variants,
        "upeg-tool-grammar/src/enums.rs's `{const_name}` has drifted from \
         `upeg_core::{enum_name}`'s real variants (upeg-core/src/types.rs). \
         macro list: {macro_allowed:?}, core variants: {core_variants:?}"
    );
}

#[test]
fn 매크로_허용_variant_목록은_core_enum과_일치한다() {
    assert_variant_lists_match("PinKind", "ALLOWED_PIN_KINDS");
    assert_variant_lists_match("PegboardUnits", "ALLOWED_PEGBOARD_UNITS");
    assert_variant_lists_match("Invoker", "ALLOWED_INVOKERS");
    assert_variant_lists_match("Surface", "ALLOWED_SURFACES");
}
