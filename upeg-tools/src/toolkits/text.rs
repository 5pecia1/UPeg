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
    fn 텍스트_차이는_동일한_입력에_대해_변경_없음을_보고한다() {
        let out = text_diff("alpha\nbeta\n", "alpha\nbeta\n");
        for line in out.lines() {
            assert!(
                line.starts_with(' '),
                "expected ` `-prefixed line for identical inputs, got: {line:?}"
            );
        }
    }

    #[test]
    fn 텍스트_차이는_추가된_줄을_보고한다() {
        let out = text_diff("a\nb\n", "a\nb\nc\n");
        assert!(out.contains("+c"), "expected `+c` line, got:\n{out}");
        assert!(out.contains(" a"));
        assert!(out.contains(" b"));
    }

    #[test]
    fn 텍스트_차이는_삭제된_줄을_보고한다() {
        let out = text_diff("a\nb\nc\n", "a\nc\n");
        assert!(out.contains("-b"), "expected `-b` line, got:\n{out}");
    }

    #[test]
    fn 텍스트_차이는_대체된_줄을_삭제와_삽입으로_보고한다() {
        let out = text_diff("a\nb\n", "a\nB\n");
        assert!(out.contains("-b"));
        assert!(out.contains("+B"));
    }

    #[test]
    fn 텍스트_차이는_빈에서_텍스트로_갈_때_삽입만_내보낸다() {
        let out = text_diff("", "hello\n");
        assert!(out.contains("+hello"));
        assert!(!out.contains('-'), "no removed lines from empty input");
    }

    // ─── regex_match ────────────────────────────────────────────

    #[test]
    fn regex_테스트_단일_일치를_검증한다() {
        let m = regex_match(r"\d+", "abc 42 def").expect("compile");
        assert_eq!(m, vec!["42"]);
    }

    #[test]
    fn regex_테스트_여러는_안_순서를_일치시킨다() {
        let m = regex_match("0x[0-9a-fA-F]+", "Tx 0xabc and 0xdeadbeef").expect("compile");
        assert_eq!(m, vec!["0xabc", "0xdeadbeef"]);
    }

    #[test]
    fn regex_테스트는_일치가_없으면_빈_벡터를_반환한다() {
        let m = regex_match(r"\d+", "no digits here").expect("compile");
        assert!(m.is_empty());
    }

    #[test]
    fn regex_테스트_빈_입력을_검증한다() {
        let m = regex_match(".+", "").expect("compile");
        assert!(m.is_empty());
    }

    #[test]
    fn regex_테스트_유효하지_않은_패턴은_오류를_반환한다() {
        assert_eq!(regex_match("(unclosed", "x"), Err("invalid regex pattern"));
        assert!(regex_match("[unclosed", "x").is_err());
        assert!(regex_match("*", "x").is_err());
    }

    #[test]
    fn regex_테스트는_앵커들_와_그룹들을_지원한다() {
        let m = regex_match(r"(?m)^line\d", "line1\nother\nline2").expect("compile");
        assert_eq!(m, vec!["line1", "line2"]);
    }

    #[test]
    fn regex_테스트_유니코드는_일치시킨다() {
        let m = regex_match(r"\p{Hangul}+", "hello 한글 world 안녕").expect("compile");
        assert_eq!(m, vec!["한글", "안녕"]);
    }

    #[test]
    fn regex_테스트는_인라인_플래그로_대소문자를_무시한다() {
        let m = regex_match("(?i)hello", "Hello HELLO hello").expect("compile");
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn regex_테스트_겹치는_일치는_겹치는이_아닌_기준_기본이다() {
        let m = regex_match("aba", "ababa").expect("compile");
        assert_eq!(m, vec!["aba"]);
    }

    // ─── lowercase / uppercase ─────────────────

    #[test]
    fn 텍스트_소문자는_아스키와_유니코드를_모두_처리한다() {
        assert_eq!(text_lowercase("Hello"), "hello");
        assert_eq!(
            text_lowercase("МИР"),
            "мир",
            "Cyrillic uppercase round-trip"
        );
        assert_eq!(text_lowercase(""), "");
    }

    #[test]
    fn 텍스트_대문자는_아스키와_유니코드를_모두_처리한다() {
        assert_eq!(text_uppercase("hello"), "HELLO");
        assert_eq!(text_uppercase("ß"), "SS", "German eszett uppercases to SS");
        assert_eq!(text_uppercase(""), "");
    }

    // ─── word/char/line counts ─────────────

    #[test]
    fn 단어_개수는_기본_텍스트와_공백_연속을_센다() {
        assert_eq!(text_word_count(""), 0);
        assert_eq!(text_word_count("hello"), 1);
        assert_eq!(text_word_count("hello world"), 2);
        assert_eq!(text_word_count("  hello   world  "), 2);
        assert_eq!(text_word_count("a\tb\nc"), 3);
    }

    #[test]
    fn 문자_개수는_문자와_바이트를_구별한다() {
        assert_eq!(text_char_count(""), 0);
        assert_eq!(text_char_count("abc"), 3);
        assert_eq!(text_char_count("한글"), 2);
        assert_eq!(text_char_count("😀"), 1);
    }

    #[test]
    fn 줄_개수는_끝의_개행과_빈_입력을_처리한다() {
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
    fn 텍스트는_기본_술어를_포함한다() {
        assert_eq!(text_contains("hello world", "world").unwrap(), "true");
        assert_eq!(text_contains("hello world", "Rust").unwrap(), "false");
        assert_eq!(text_contains("", "x").unwrap(), "false");
    }

    #[test]
    fn 텍스트는_유니코드_패턴을_포함한다() {
        assert_eq!(text_contains("한글 hello", "한글").unwrap(), "true");
    }

    #[test]
    fn 텍스트_포함_검사는_빈_패턴을_거부한다() {
        match text_contains("anything", "") {
            Err(msg) => assert!(msg.contains("non-empty")),
            Ok(_) => panic!("expected error for empty pattern"),
        }
    }

    // ─── replace ─────────────

    #[test]
    fn 텍스트_치환의_기본_동작을_확인한다() {
        assert_eq!(
            text_replace("hello world", "world", "Rust").unwrap(),
            "hello Rust"
        );
        assert_eq!(text_replace("aaa", "a", "b").unwrap(), "bbb");
        assert_eq!(text_replace("nope", "x", "y").unwrap(), "nope");
    }

    #[test]
    fn 텍스트_치환은_빈_문자열로_바꾸면_부분문자열을_제거한다() {
        assert_eq!(text_replace("abc abc", "b", "").unwrap(), "ac ac");
    }

    #[test]
    fn 텍스트_치환은_빈_검색어를_거부한다() {
        match text_replace("anything", "", "x") {
            Err(msg) => assert!(msg.contains("non-empty")),
            Ok(_) => panic!("expected error for empty `from`"),
        }
    }

    #[test]
    fn 텍스트_치환은_유니코드를_인식한다() {
        assert_eq!(
            text_replace("한글 hello 한글", "한글", "Korean").unwrap(),
            "Korean hello Korean"
        );
    }

    // ─── repeat ─────────────

    #[test]
    fn 텍스트_반복은_기본_동작과_경계를_처리한다() {
        assert_eq!(text_repeat("abc", 3).unwrap(), "abcabcabc");
        assert_eq!(text_repeat("abc", 1).unwrap(), "abc");
        assert_eq!(text_repeat("abc", 0).unwrap(), "");
        assert_eq!(text_repeat("", 5).unwrap(), "");
    }

    #[test]
    fn 텍스트_반복은_1024_횟수에_상한을_둔다() {
        assert!(text_repeat("a", 1024).is_ok());
        match text_repeat("a", 1025) {
            Err(msg) => assert!(msg.contains("count too large")),
            Ok(_) => panic!("expected error for count=1025"),
        }
    }

    // ─── split / join ─────────────

    #[test]
    fn 분할은_명시적_구분자를_사용한다() {
        assert_eq!(text_split("a,b,c", ","), r#"["a","b","c"]"#);
        assert_eq!(
            text_split("", ","),
            r#"[""]"#,
            "split of empty input → one empty token"
        );
    }

    #[test]
    fn 빈_구분자_분할은_연속_공백을_접어서_사용한다() {
        assert_eq!(
            text_split("  one   two\tthree\n", ""),
            r#"["one","two","three"]"#
        );
    }

    #[test]
    fn 결합의_기본_동작을_확인한다() {
        assert_eq!(text_join(r#"["a","b","c"]"#, ",").unwrap(), "a,b,c");
        assert_eq!(text_join(r#"["a","b","c"]"#, " - ").unwrap(), "a - b - c");
        assert_eq!(text_join("[]", ",").unwrap(), "");
    }

    #[test]
    fn 결합은_문자열이_아닌_요소를_거부한다() {
        match text_join(r#"["a", 1, "b"]"#, ",") {
            Err(msg) => assert!(msg.contains("strings")),
            Ok(_) => panic!("expected error for mixed-type array"),
        }
    }

    #[test]
    fn 결합은_배열이_아닌_것을_거부한다() {
        assert!(text_join(r#"{"k":"v"}"#, ",").is_err());
        assert!(text_join("not json", ",").is_err());
    }

    #[test]
    fn 같은_구분자로_분할과_결합을_왕복하면_무손실이다() {
        for s in ["a,b,c", "x,y", "single", ""] {
            let split = text_split(s, ",");
            let joined = text_join(&split, ",").unwrap();
            assert_eq!(joined, s, "round-trip diverged for {s:?}");
        }
    }

    // ─── trim ─────────────

    #[test]
    fn 텍스트_잘라냄은_기본과_유니코드_공백을_모두_처리한다() {
        assert_eq!(text_trim("  hello  "), "hello");
        assert_eq!(text_trim("\t\nfoo\r\n"), "foo");
        assert_eq!(text_trim(""), "");
        assert_eq!(text_trim("   "), "");
        assert_eq!(text_trim("\u{00A0}x\u{00A0}"), "x");
    }

    // ─── reverse ─────────────

    #[test]
    fn 뒤집기는_아스키와_유니코드_스칼라를_모두_처리한다() {
        assert_eq!(text_reverse(""), "");
        assert_eq!(text_reverse("Hello"), "olleH");
        assert_eq!(text_reverse("한글"), "글한");
        assert_eq!(text_reverse("a😀b"), "b😀a");
    }

    // ─── slugify ─────────────

    #[test]
    fn slugify_기본_구문들을_검증한다() {
        assert_eq!(text_slugify("Hello, World!"), "hello-world");
        assert_eq!(text_slugify("My First Post"), "my-first-post");
    }

    #[test]
    fn slugify는_연속된_구분자와_공백을_하나로_접는다() {
        assert_eq!(text_slugify("a__b---c"), "a-b-c");
        assert_eq!(
            text_slugify("  whitespace  in  middle  "),
            "whitespace-in-middle"
        );
    }

    #[test]
    fn slugify는_아스키가_아닌_것을_제거한다() {
        assert_eq!(text_slugify("한글hello세계"), "hello");
        assert_eq!(text_slugify("café"), "caf");
    }

    #[test]
    fn slugify는_빈_입력과_문장부호만_입력을_빈_문자열로_바꾼다() {
        assert_eq!(text_slugify(""), "");
        assert_eq!(text_slugify("!!!"), "");
        assert_eq!(text_slugify("---"), "");
    }

    #[test]
    fn slugify는_숫자들을_보존한다() {
        assert_eq!(text_slugify("Top 10 Reasons"), "top-10-reasons");
        assert_eq!(text_slugify("v2.0.1 release"), "v2-0-1-release");
    }
}
