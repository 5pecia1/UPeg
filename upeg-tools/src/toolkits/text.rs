//! `text` toolkit — case, count, search, replace, repeat, split/join, trim,
//! reverse, slugify, regex, diff.

use upeg_core::tool;

/// `text.regex_match` — find every match of `pattern` in `input`. Returns
/// the matched substrings in order. Returns `Err` if `pattern` is not a
/// valid regular expression.
#[tool(
    id = "text.regex_match",
    display_label = "Regex match",
    toolkit = "text",
    description = "Find every match of a regular expression in an input string.",
    inputs = [
        required pattern: String = "Rust regex syntax",
        required input: String = "Text to search",
    ],
    outputs = [
        result: Json = "Matched substrings in order",
    ],
    pin = Inline,
    pegboard_units = U2,
    invoker = Function,
    boards = ["dev"],
)]
pub fn regex_match(pattern: &str, input: &str) -> Result<Vec<String>, &'static str> {
    let re = regex::Regex::new(pattern).map_err(|_| "invalid regex pattern")?;
    Ok(re
        .find_iter(input)
        .map(|m| m.as_str().to_string())
        .collect())
}

/// `text.diff` — unified-style diff between two text inputs (line-level).
/// Output uses `+` / `-` / ` ` line prefixes, similar to `diff -u`.
#[tool(
    id = "text.diff",
    display_label = "Text diff",
    toolkit = "text",
    description = "Diff two text inputs line by line (similar to `diff`).",
    inputs = [
        required left: String = "First text",
        required right: String = "Second text",
    ],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
    boards = [],
)]
pub fn text_diff(left: &str, right: &str) -> String {
    use similar::{ChangeTag, TextDiff};
    let diff = TextDiff::from_lines(left, right);
    let mut out = String::new();
    for change in diff.iter_all_changes() {
        let prefix = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        out.push_str(prefix);
        out.push_str(change.value());
    }
    out
}

/// `text.lowercase` — lowercase a string. Unicode-aware via `str::to_lowercase`.
#[tool(
    id = "text.lowercase",
    display_label = "Lowercase",
    toolkit = "text",
    description = "Lowercase a string (Unicode-aware).",
    inputs = [
        required input: String = "Text to lowercase",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_lowercase(input: &str) -> String {
    input.to_lowercase()
}

/// `text.uppercase` — uppercase a string. Unicode-aware via `str::to_uppercase`.
#[tool(
    id = "text.uppercase",
    display_label = "Uppercase",
    toolkit = "text",
    description = "Uppercase a string (Unicode-aware).",
    inputs = [
        required input: String = "Text to uppercase",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_uppercase(input: &str) -> String {
    input.to_uppercase()
}

/// `text.word_count` — number of whitespace-separated words.
///
/// `str::split_whitespace` handles all Unicode whitespace categories, so
/// "hi\tworld" → 2 not 1, and runs of whitespace don't produce empty
/// tokens.
#[tool(
    id = "text.word_count",
    display_label = "Word count",
    toolkit = "text",
    description = "Count whitespace-separated words in a string.",
    inputs = [
        required input: String = "Text to count words in",
    ],
    outputs = [
        result: Number = "Word count",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_word_count(input: &str) -> usize {
    input.split_whitespace().count()
}

/// `text.char_count` — number of Unicode scalar values (NOT bytes).
///
/// `"한글".len()` is 6 (UTF-8 bytes), `"한글".chars().count()` is 2.
/// We want the latter for human-meaningful character counting.
#[tool(
    id = "text.char_count",
    display_label = "Char count",
    toolkit = "text",
    description = "Count Unicode scalar values (chars), not bytes.",
    inputs = [
        required input: String = "Text to count chars in",
    ],
    outputs = [
        result: Number = "Character count",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_char_count(input: &str) -> usize {
    input.chars().count()
}

/// `text.line_count` — number of lines (count of `\n`-terminated runs;
/// trailing content without a `\n` still counts as one line).
///
/// Empty input → `0`. `"a"` → 1. `"a\n"` → 1. `"a\nb"` → 2. `"a\nb\n"` → 2.
#[tool(
    id = "text.line_count",
    display_label = "Line count",
    toolkit = "text",
    description = "Count lines in a string. Empty input is 0; trailing newline doesn't add a line.",
    inputs = [
        required input: String = "Text to count lines in",
    ],
    outputs = [
        result: Number = "Line count",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_line_count(input: &str) -> usize {
    if input.is_empty() {
        return 0;
    }
    input.lines().count()
}

/// `text.contains` — predicate: does `input` contain `pattern`?
///
/// Returns `"true"` or `"false"` as a string (consistent with the
/// stringly-typed dispatch contract). Empty `pattern` is rejected —
/// `str::contains("")` is always `true` which is rarely useful and
/// likely indicates a programming error.
#[tool(
    id = "text.contains",
    display_label = "Contains",
    toolkit = "text",
    description = "Test whether `input` contains `pattern`, returning `true` or `false`.",
    inputs = [
        required input: String = "Text to search",
        required pattern: String = "Substring to look for",
    ],
    outputs = [
        result: Boolean = "Whether `pattern` occurs in `input`",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_contains(input: &str, pattern: &str) -> Result<String, &'static str> {
    if pattern.is_empty() {
        return Err("pattern must be non-empty");
    }
    Ok(if input.contains(pattern) {
        "true".into()
    } else {
        "false".into()
    })
}

/// `text.replace` — replace all occurrences of `from` with `to` in `input`.
/// Empty `from` returns `input` unchanged (matches `str::replace` semantics
/// for our purposes — `str::replace("", from, to)` is well-defined but
/// `replace(input, "", to)` would insert `to` between every char which
/// is rarely what users want; we explicitly reject it).
#[tool(
    id = "text.replace",
    display_label = "Replace",
    toolkit = "text",
    description = "Replace all occurrences of `from` with `to` in `input`.",
    inputs = [
        required input: String = "Text to search",
        required from: String = "Substring to replace",
        required to: String = "Replacement text",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_replace(input: &str, from: &str, to: &str) -> Result<String, &'static str> {
    if from.is_empty() {
        return Err("from must be non-empty");
    }
    Ok(input.replace(from, to))
}

/// `text.repeat` — concatenate `input` with itself `count` times.
///
/// Capped at 1024 repeats so the response payload stays bounded. Useful
/// for padding (`text.repeat " " 4`), separators (`text.repeat "-" 80`),
/// and similar formatting. `count=0` returns the empty string; `count=1`
/// returns `input` unchanged.
#[tool(
    id = "text.repeat",
    display_label = "Repeat",
    toolkit = "text",
    description = "Repeat a string `count` times (default 1, max 1024).",
    inputs = [
        required input: String = "Text to repeat",
        optional count: Integer = "Repeat count (default 1, max 1024)",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_repeat(input: &str, count: usize) -> Result<String, &'static str> {
    if count > 1024 {
        return Err("count too large (max 1024 repeats)");
    }
    Ok(input.repeat(count))
}

/// `text.split` — split a string on `separator` and return a JSON array
/// of strings. Empty `separator` falls back to `split_whitespace`
/// (multiple runs of whitespace produce no empty tokens).
#[tool(
    id = "text.split",
    display_label = "Split",
    toolkit = "text",
    description = "Split text on `separator` (default: whitespace) and return a JSON array.",
    inputs = [
        required input: String = "Text to split",
        optional separator: String = "Separator; empty/omitted = whitespace",
    ],
    outputs = [
        result: Json = "Split parts as a JSON array of strings",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_split(input: &str, separator: &str) -> String {
    let parts: Vec<&str> = if separator.is_empty() {
        input.split_whitespace().collect()
    } else {
        input.split(separator).collect()
    };
    serde_json::to_string(&parts).unwrap_or_else(|_| "[]".to_string())
}

/// `text.join` — join a JSON array of strings with `delim`. Errors if
/// the input isn't a JSON array of strings.
#[tool(
    id = "text.join",
    display_label = "Join",
    toolkit = "text",
    description = "Join a JSON array of strings with `separator` (default: \" \").",
    inputs = [
        required input: String = "JSON array of strings",
        optional separator: String = "Separator (default: space)",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_join(input: &str, separator: &str) -> Result<String, &'static str> {
    let v: serde_json::Value =
        serde_json::from_str(input).map_err(|_| "input must be a JSON array of strings")?;
    let arr = v.as_array().ok_or("input must be a JSON array")?;
    let mut strs = Vec::with_capacity(arr.len());
    for item in arr {
        let s = item.as_str().ok_or("array elements must all be strings")?;
        strs.push(s.to_string());
    }
    Ok(strs.join(separator))
}

/// `text.trim` — strip leading and trailing whitespace (Unicode-aware).
#[tool(
    id = "text.trim",
    display_label = "Trim",
    toolkit = "text",
    description = "Strip leading and trailing whitespace (Unicode-aware).",
    inputs = [
        required input: String = "Text to trim",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_trim(input: &str) -> String {
    input.trim().to_string()
}

/// `text.reverse` — reverse a string by Unicode scalar values.
///
/// "Hello" → "olleH". `s.chars().rev().collect()` is the standard
/// approach; it reverses code points, NOT graphemes. Combining marks
/// and ZWJ-glued emoji sequences (`👨‍👩‍👧`) reverse component-by-
/// component, which can produce visually surprising output. Document
/// the contract; users wanting grapheme-correct reversal should
/// pre-normalize.
#[tool(
    id = "text.reverse",
    display_label = "Reverse text",
    toolkit = "text",
    description = "Reverse a string by Unicode scalar values (NOT graphemes — combining marks reverse separately).",
    inputs = [
        required input: String = "Text to reverse",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_reverse(input: &str) -> String {
    input.chars().rev().collect()
}

/// `text.slugify` — convert a string to a URL-safe lowercase slug.
///
/// Rule: lowercase, treat any run of non-`[a-z0-9]` (after lowercase) as
/// a single hyphen, then trim leading/trailing hyphens. ASCII-only —
/// non-ASCII chars are dropped (they're typically not URL-safe and
/// transliteration is its own can of worms).
#[tool(
    id = "text.slugify",
    display_label = "Slugify",
    toolkit = "text",
    description = "Convert text to a URL-safe lowercase slug ([a-z0-9] runs joined by hyphens).",
    inputs = [
        required input: String = "Text to slugify",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn text_slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_sep = true;
    for ch in input.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('-');
            last_was_sep = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── text_diff ──────────────────────────────────────────────

    #[test]
    fn text_diff_reports_no_changes_for_identical_inputs() {
        let out = text_diff("alpha\nbeta\n", "alpha\nbeta\n");
        for line in out.lines() {
            assert!(
                line.starts_with(' '),
                "expected ` `-prefixed line for identical inputs, got: {line:?}"
            );
        }
    }

    #[test]
    fn text_diff_reports_added_lines() {
        let out = text_diff("a\nb\n", "a\nb\nc\n");
        assert!(out.contains("+c"), "expected `+c` line, got:\n{out}");
        assert!(out.contains(" a"));
        assert!(out.contains(" b"));
    }

    #[test]
    fn text_diff_reports_removed_lines() {
        let out = text_diff("a\nb\nc\n", "a\nc\n");
        assert!(out.contains("-b"), "expected `-b` line, got:\n{out}");
    }

    #[test]
    fn text_diff_reports_replaced_lines_as_delete_and_insert() {
        let out = text_diff("a\nb\n", "a\nB\n");
        assert!(out.contains("-b"));
        assert!(out.contains("+B"));
    }

    #[test]
    fn text_diff_emits_only_inserts_when_going_from_empty_to_text() {
        let out = text_diff("", "hello\n");
        assert!(out.contains("+hello"));
        assert!(!out.contains('-'), "no removed lines from empty input");
    }

    // ─── regex_match ────────────────────────────────────────────

    #[test]
    fn regex_match_finds_single_match() {
        let m = regex_match(r"\d+", "abc 42 def").expect("compile");
        assert_eq!(m, vec!["42"]);
    }

    #[test]
    fn regex_match_returns_multiple_matches_in_order() {
        let m = regex_match("0x[0-9a-fA-F]+", "Tx 0xabc and 0xdeadbeef").expect("compile");
        assert_eq!(m, vec!["0xabc", "0xdeadbeef"]);
    }

    #[test]
    fn regex_match_returns_empty_vec_when_no_match() {
        let m = regex_match(r"\d+", "no digits here").expect("compile");
        assert!(m.is_empty());
    }

    #[test]
    fn regex_match_handles_empty_input() {
        let m = regex_match(".+", "").expect("compile");
        assert!(m.is_empty());
    }

    #[test]
    fn regex_match_errors_on_invalid_pattern() {
        assert_eq!(regex_match("(unclosed", "x"), Err("invalid regex pattern"));
        assert!(regex_match("[unclosed", "x").is_err());
        assert!(regex_match("*", "x").is_err());
    }

    #[test]
    fn regex_match_supports_anchors_and_groups() {
        let m = regex_match(r"(?m)^line\d", "line1\nother\nline2").expect("compile");
        assert_eq!(m, vec!["line1", "line2"]);
    }

    #[test]
    fn regex_match_matches_unicode() {
        let m = regex_match(r"\p{Hangul}+", "hello 한글 world 안녕").expect("compile");
        assert_eq!(m, vec!["한글", "안녕"]);
    }

    #[test]
    fn regex_match_ignores_case_with_inline_flags() {
        let m = regex_match("(?i)hello", "Hello HELLO hello").expect("compile");
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn regex_match_defaults_to_non_overlapping_matches() {
        let m = regex_match("aba", "ababa").expect("compile");
        assert_eq!(m, vec!["aba"]);
    }

    // ─── lowercase / uppercase ─────────────────

    #[test]
    fn text_lowercase_handles_ascii_and_unicode() {
        assert_eq!(text_lowercase("Hello"), "hello");
        assert_eq!(
            text_lowercase("МИР"),
            "мир",
            "Cyrillic uppercase round-trip"
        );
        assert_eq!(text_lowercase(""), "");
    }

    #[test]
    fn text_uppercase_handles_ascii_and_unicode() {
        assert_eq!(text_uppercase("hello"), "HELLO");
        assert_eq!(text_uppercase("ß"), "SS", "German eszett uppercases to SS");
        assert_eq!(text_uppercase(""), "");
    }

    // ─── word/char/line counts ─────────────

    #[test]
    fn word_count_counts_basic_text_and_whitespace_runs() {
        assert_eq!(text_word_count(""), 0);
        assert_eq!(text_word_count("hello"), 1);
        assert_eq!(text_word_count("hello world"), 2);
        assert_eq!(text_word_count("  hello   world  "), 2);
        assert_eq!(text_word_count("a\tb\nc"), 3);
    }

    #[test]
    fn char_count_distinguishes_chars_from_bytes() {
        assert_eq!(text_char_count(""), 0);
        assert_eq!(text_char_count("abc"), 3);
        assert_eq!(text_char_count("한글"), 2);
        assert_eq!(text_char_count("😀"), 1);
    }

    #[test]
    fn line_count_handles_trailing_newline_and_empty_input() {
        assert_eq!(text_line_count(""), 0);
        assert_eq!(text_line_count("a"), 1);
        assert_eq!(
            text_line_count("a\n"),
            1,
            "trailing newline doesn't add a line"
        );
        assert_eq!(text_line_count("a\nb"), 2);
        assert_eq!(text_line_count("a\nb\n"), 2);
        assert_eq!(text_line_count("\n"), 1, "single newline = one empty line");
    }

    // ─── contains ─────────────

    #[test]
    fn text_contains_matches_basic_predicate() {
        assert_eq!(text_contains("hello world", "world").unwrap(), "true");
        assert_eq!(text_contains("hello world", "Rust").unwrap(), "false");
        assert_eq!(text_contains("", "x").unwrap(), "false");
    }

    #[test]
    fn text_contains_matches_unicode_pattern() {
        assert_eq!(text_contains("한글 hello", "한글").unwrap(), "true");
    }

    #[test]
    fn text_contains_rejects_empty_pattern() {
        match text_contains("anything", "") {
            Err(msg) => assert!(msg.contains("non-empty")),
            Ok(_) => panic!("expected error for empty pattern"),
        }
    }

    // ─── replace ─────────────

    #[test]
    fn text_replace_covers_basic_behavior() {
        assert_eq!(
            text_replace("hello world", "world", "Rust").unwrap(),
            "hello Rust"
        );
        assert_eq!(text_replace("aaa", "a", "b").unwrap(), "bbb");
        assert_eq!(text_replace("nope", "x", "y").unwrap(), "nope");
    }

    #[test]
    fn text_replace_with_empty_string_removes_substring() {
        assert_eq!(text_replace("abc abc", "b", "").unwrap(), "ac ac");
    }

    #[test]
    fn text_replace_rejects_empty_search_term() {
        match text_replace("anything", "", "x") {
            Err(msg) => assert!(msg.contains("non-empty")),
            Ok(_) => panic!("expected error for empty `from`"),
        }
    }

    #[test]
    fn text_replace_is_unicode_aware() {
        assert_eq!(
            text_replace("한글 hello 한글", "한글", "Korean").unwrap(),
            "Korean hello Korean"
        );
    }

    // ─── repeat ─────────────

    #[test]
    fn text_repeat_handles_basic_behavior_and_boundaries() {
        assert_eq!(text_repeat("abc", 3).unwrap(), "abcabcabc");
        assert_eq!(text_repeat("abc", 1).unwrap(), "abc");
        assert_eq!(text_repeat("abc", 0).unwrap(), "");
        assert_eq!(text_repeat("", 5).unwrap(), "");
    }

    #[test]
    fn text_repeat_caps_count_at_1024() {
        assert!(text_repeat("a", 1024).is_ok());
        match text_repeat("a", 1025) {
            Err(msg) => assert!(msg.contains("count too large")),
            Ok(_) => panic!("expected error for count=1025"),
        }
    }

    // ─── split / join ─────────────

    #[test]
    fn text_split_uses_explicit_delimiter() {
        assert_eq!(text_split("a,b,c", ","), r#"["a","b","c"]"#);
        assert_eq!(
            text_split("", ","),
            r#"[""]"#,
            "split of empty input → one empty token"
        );
    }

    #[test]
    fn text_split_with_empty_delimiter_folds_whitespace_runs() {
        assert_eq!(
            text_split("  one   two\tthree\n", ""),
            r#"["one","two","three"]"#
        );
    }

    #[test]
    fn text_join_covers_basic_behavior() {
        assert_eq!(text_join(r#"["a","b","c"]"#, ",").unwrap(), "a,b,c");
        assert_eq!(text_join(r#"["a","b","c"]"#, " - ").unwrap(), "a - b - c");
        assert_eq!(text_join("[]", ",").unwrap(), "");
    }

    #[test]
    fn text_join_rejects_non_string_elements() {
        match text_join(r#"["a", 1, "b"]"#, ",") {
            Err(msg) => assert!(msg.contains("strings")),
            Ok(_) => panic!("expected error for mixed-type array"),
        }
    }

    #[test]
    fn text_join_rejects_non_array_input() {
        assert!(text_join(r#"{"k":"v"}"#, ",").is_err());
        assert!(text_join("not json", ",").is_err());
    }

    #[test]
    fn split_join_roundtrip_with_same_delimiter_is_lossless() {
        for s in ["a,b,c", "x,y", "single", ""] {
            let split = text_split(s, ",");
            let joined = text_join(&split, ",").unwrap();
            assert_eq!(joined, s, "round-trip diverged for {s:?}");
        }
    }

    // ─── trim ─────────────

    #[test]
    fn text_trim_handles_basic_and_unicode_whitespace() {
        assert_eq!(text_trim("  hello  "), "hello");
        assert_eq!(text_trim("\t\nfoo\r\n"), "foo");
        assert_eq!(text_trim(""), "");
        assert_eq!(text_trim("   "), "");
        assert_eq!(text_trim("\u{00A0}x\u{00A0}"), "x");
    }

    // ─── reverse ─────────────

    #[test]
    fn text_reverse_handles_ascii_and_unicode_scalars() {
        assert_eq!(text_reverse(""), "");
        assert_eq!(text_reverse("Hello"), "olleH");
        assert_eq!(text_reverse("한글"), "글한");
        assert_eq!(text_reverse("a😀b"), "b😀a");
    }

    // ─── slugify ─────────────

    #[test]
    fn slugify_handles_basic_phrases() {
        assert_eq!(text_slugify("Hello, World!"), "hello-world");
        assert_eq!(text_slugify("My First Post"), "my-first-post");
    }

    #[test]
    fn slugify_folds_consecutive_delimiters_and_spaces() {
        assert_eq!(text_slugify("a__b---c"), "a-b-c");
        assert_eq!(
            text_slugify("  whitespace  in  middle  "),
            "whitespace-in-middle"
        );
    }

    #[test]
    fn slugify_strips_non_ascii() {
        assert_eq!(text_slugify("한글hello세계"), "hello");
        assert_eq!(text_slugify("café"), "caf");
    }

    #[test]
    fn slugify_maps_empty_and_punctuation_only_input_to_empty() {
        assert_eq!(text_slugify(""), "");
        assert_eq!(text_slugify("!!!"), "");
        assert_eq!(text_slugify("---"), "");
    }

    #[test]
    fn slugify_preserves_digits() {
        assert_eq!(text_slugify("Top 10 Reasons"), "top-10-reasons");
        assert_eq!(text_slugify("v2.0.1 release"), "v2-0-1-release");
    }
}
