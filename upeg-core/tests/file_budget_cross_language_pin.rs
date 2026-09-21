#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect and panic idiomatically"
)]

//! Cross-language pin for the canonical `File` value budget policy.
//!
//! `upeg-core/src/input/file_budget.rs` is the single source of truth for
//! every `File` resource limit. Dart (`flutter_app/lib/src/rust/...`,
//! `flutter_app/lib/src/platform/...`) and JS (`chrome-ext/wire.js`,
//! `chrome-ext/file_input.js`) declare the same numbers independently with
//! no shared build-time source, so nothing stops them from drifting apart
//! silently. This test reads each file by path, parses its constant
//! declarations with a tiny arithmetic evaluator (integer literals, `*`,
//! parentheses, identifiers resolved from the same file), and asserts the
//! evaluated value equals the corresponding Rust constant.
//!
//! Each parse is scoped to the exact `name = expr;` declaration text, never
//! to a whole-file substring search, so a stray same-looking literal
//! elsewhere in the file cannot satisfy the comparison — see the
//! `include_str!` source-pin meta-bug this project has been bitten by
//! before. The comparison target is always the Rust constant referenced by
//! path, never a literal restated in this test.

use upeg_core::{
    MAX_FILE_INPUT_COUNT, MAX_FILE_INPUT_METADATA_BYTES, MAX_FILE_INPUT_NODES,
    MAX_FILE_INPUT_RAW_BYTES, MAX_FILE_OUTPUT_METADATA_BYTES, MAX_FILE_OUTPUT_NESTING_DEPTH,
    MAX_FILE_OUTPUT_NODES, MAX_FILE_OUTPUT_RAW_BYTES,
};

const CANONICAL_FILE_VALUE_BUDGET_DART: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../flutter_app/lib/src/rust/canonical_file_value_budget.dart"
));
const FILE_INPUT_RESOURCE_LIMITS_DART: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../flutter_app/lib/src/platform/file_input_resource_limits.dart"
));
const BOUNDED_FILE_READER_DART: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../flutter_app/lib/src/platform/bounded_file_reader.dart"
));
const WIRE_JS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../chrome-ext/wire.js"
));
const FILE_INPUT_JS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../chrome-ext/file_input.js"
));

/// Minimal token set for the constant-expression grammar this test needs:
/// integer literals, `*`, parentheses, and bare identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Number(u64),
    Ident(String),
    Star,
    LParen,
    RParen,
}

fn tokenize(expr: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();
    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '*' => {
                chars.next();
                tokens.push(Token::Star);
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            '0'..='9' => {
                let mut digits = String::new();
                while let Some(&digit_or_sep) = chars.peek() {
                    if digit_or_sep.is_ascii_digit() {
                        digits.push(digit_or_sep);
                        chars.next();
                    } else if digit_or_sep == '_' {
                        // JS allows `1_000_000` numeric separators.
                        chars.next();
                    } else {
                        break;
                    }
                }
                let value = digits
                    .parse::<u64>()
                    .unwrap_or_else(|e| panic!("bad integer literal `{digits}` in `{expr}`: {e}"));
                tokens.push(Token::Number(value));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                while let Some(&letter_digit_or_underscore) = chars.peek() {
                    if letter_digit_or_underscore.is_ascii_alphanumeric()
                        || letter_digit_or_underscore == '_'
                    {
                        ident.push(letter_digit_or_underscore);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(ident));
            }
            other => panic!("unexpected character `{other}` while tokenizing `{expr}`"),
        }
    }
    tokens
}

/// Line openers that mark a line as commented out. A commented-out
/// look-alike (`// const int limit = 999;`, or the `*` continuation line
/// of a block comment) must never satisfy this pin: the point of the test
/// is that the LIVE declaration matches, and a bare substring search
/// would happily read a dead one.
const COMMENT_PREFIXES: [&str; 4] = ["//", "/*", "*", "#"];

/// Extracts the exact `keyword name = <rhs>;` declaration's right-hand
/// side.
///
/// Three guards make the match structural rather than textual:
///   1. the declaration must START its line (leading whitespace aside),
///      so `foo = bar(const int limit = 1)`-shaped text and any use site
///      mentioning the name cannot match;
///   2. commented-out lines ([`COMMENT_PREFIXES`]) are skipped; and
///   3. exactly one live declaration must remain — two would make the
///      pin depend on file order, which is how a pin quietly stops
///      testing anything.
fn declaration_rhs<'a>(source: &'a str, keyword: &str, name: &str) -> &'a str {
    let needle = format!("{keyword} {name} =");
    let mut rhs_start = None;
    let mut hits = 0_usize;
    let mut offset = 0_usize;
    for line in source.split_inclusive('\n') {
        if let Some(column) = declaration_column(line, &needle) {
            hits += 1;
            rhs_start.get_or_insert(offset + column + needle.len());
        }
        offset += line.len();
    }
    assert_eq!(
        hits, 1,
        "declaration `{needle}` must appear exactly once as a live (non-commented) \
         line-initial declaration; found {hits}"
    );
    let rest = &source[rhs_start.expect("exactly one hit was just asserted")..];
    let rhs_end = rest
        .find(';')
        .unwrap_or_else(|| panic!("declaration `{needle}` has no terminating `;`"));
    rest[..rhs_end].trim()
}

/// `Some(column)` when `line` is a live declaration starting with
/// `needle`; `None` for a comment line or any line that merely *mentions*
/// the declaration.
fn declaration_column(line: &str, needle: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if COMMENT_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    {
        return None;
    }
    trimmed
        .starts_with(needle)
        .then(|| line.len() - line.trim_start().len())
}

/// Evaluates a constant expression, resolving bare identifiers against
/// other `keyword name = expr;` declarations in the same `source`.
fn eval_declaration(source: &str, keyword: &str, name: &str) -> u64 {
    let rhs = declaration_rhs(source, keyword, name);
    let tokens = tokenize(rhs);
    let mut pos = 0;
    let value = eval_term(source, keyword, &tokens, &mut pos);
    assert_eq!(
        pos,
        tokens.len(),
        "trailing tokens after evaluating `{name} = {rhs}`"
    );
    value
}

// term := factor ('*' factor)*
fn eval_term(source: &str, keyword: &str, tokens: &[Token], pos: &mut usize) -> u64 {
    let mut value = eval_factor(source, keyword, tokens, pos);
    while matches!(tokens.get(*pos), Some(Token::Star)) {
        *pos += 1;
        let rhs = eval_factor(source, keyword, tokens, pos);
        value = value
            .checked_mul(rhs)
            .unwrap_or_else(|| panic!("overflow evaluating `{tokens:?}`"));
    }
    value
}

// factor := NUMBER | IDENT | '(' term ')'
fn eval_factor(source: &str, keyword: &str, tokens: &[Token], pos: &mut usize) -> u64 {
    match tokens.get(*pos) {
        Some(Token::Number(value)) => {
            *pos += 1;
            *value
        }
        Some(Token::Ident(ident)) => {
            *pos += 1;
            eval_declaration(source, keyword, ident)
        }
        Some(Token::LParen) => {
            *pos += 1;
            let value = eval_term(source, keyword, tokens, pos);
            assert_eq!(
                tokens.get(*pos),
                Some(&Token::RParen),
                "expected closing paren while evaluating `{tokens:?}`"
            );
            *pos += 1;
            value
        }
        other => panic!("unexpected token {other:?} while evaluating `{tokens:?}`"),
    }
}

fn dart_const(source: &str, name: &str) -> u64 {
    eval_declaration(source, "const int", name)
}

fn js_const(source: &str, name: &str) -> u64 {
    eval_declaration(source, "const", name)
}

#[test]
fn dart_output_raw_bytes_limit_matches_the_rust_constant() {
    assert_eq!(
        dart_const(
            CANONICAL_FILE_VALUE_BUDGET_DART,
            "canonicalFileMaximumRawBytes"
        ),
        MAX_FILE_OUTPUT_RAW_BYTES,
    );
}

#[test]
fn dart_output_node_limit_matches_the_rust_constant() {
    assert_eq!(
        dart_const(
            CANONICAL_FILE_VALUE_BUDGET_DART,
            "canonicalFileMaximumNodes"
        ),
        MAX_FILE_OUTPUT_NODES,
    );
}

#[test]
fn dart_output_metadata_limit_matches_the_rust_constant() {
    assert_eq!(
        dart_const(
            CANONICAL_FILE_VALUE_BUDGET_DART,
            "canonicalFileMaximumMetadataBytes"
        ),
        MAX_FILE_OUTPUT_METADATA_BYTES,
    );
}

#[test]
fn dart_output_nesting_depth_limit_matches_the_rust_constant() {
    let rust_depth = u64::try_from(MAX_FILE_OUTPUT_NESTING_DEPTH).expect("depth budget fits u64");
    assert_eq!(
        dart_const(
            CANONICAL_FILE_VALUE_BUDGET_DART,
            "canonicalFileMaximumNestingDepth"
        ),
        rust_depth,
    );
}

#[test]
fn dart_input_file_count_limit_matches_the_rust_constant() {
    let rust_count = u64::from(MAX_FILE_INPUT_COUNT);
    assert_eq!(
        dart_const(FILE_INPUT_RESOURCE_LIMITS_DART, "maxFileInputCount"),
        rust_count,
    );
}

#[test]
fn dart_input_node_limit_matches_the_rust_constant() {
    assert_eq!(
        dart_const(FILE_INPUT_RESOURCE_LIMITS_DART, "maxFileInputNodes"),
        MAX_FILE_INPUT_NODES,
    );
}

#[test]
fn dart_input_metadata_limit_matches_the_rust_constant() {
    assert_eq!(
        dart_const(FILE_INPUT_RESOURCE_LIMITS_DART, "maxFileInputMetadataBytes"),
        MAX_FILE_INPUT_METADATA_BYTES,
    );
}

#[test]
fn dart_input_raw_bytes_limit_matches_the_rust_constant() {
    assert_eq!(
        dart_const(BOUNDED_FILE_READER_DART, "maxFileInputTransportBytes"),
        MAX_FILE_INPUT_RAW_BYTES,
    );
}

#[test]
fn js_wire_output_raw_bytes_limit_matches_the_rust_constant() {
    assert_eq!(
        js_const(WIRE_JS, "MAX_FILE_OUTPUT_RAW_BYTES"),
        MAX_FILE_OUTPUT_RAW_BYTES,
    );
}

#[test]
fn js_file_input_node_limit_matches_the_rust_constant() {
    assert_eq!(
        js_const(FILE_INPUT_JS, "MAX_FILE_VALUE_NODE_COUNT"),
        MAX_FILE_OUTPUT_NODES,
    );
}

#[test]
fn js_file_input_metadata_limit_matches_the_rust_constant() {
    assert_eq!(
        js_const(FILE_INPUT_JS, "MAX_FILE_VALUE_METADATA_BYTES"),
        MAX_FILE_OUTPUT_METADATA_BYTES,
    );
}

#[test]
fn js_file_input_policy_count_limit_matches_the_rust_constant() {
    let rust_count = u64::from(MAX_FILE_INPUT_COUNT);
    assert_eq!(js_const(FILE_INPUT_JS, "MAX_FILE_POLICY_COUNT"), rust_count,);
}

#[test]
fn declaration_parser_ignores_commented_out_same_name_declarations() {
    let source = concat!(
        "// const int limit = 999;\n",
        "/* const int limit = 888;\n",
        " * const int limit = 777;\n",
        " */\n",
        "# const int limit = 666;\n",
        "const int limit = 7;\n",
    );

    assert_eq!(declaration_rhs(source, "const int", "limit"), "7");
}

#[test]
fn declaration_parser_finds_indented_declarations_and_ignores_use_sites() {
    // The chrome-ext JS constants are indented inside an IIFE. Use sites
    // (`Math.min(x, LIMIT)`) are not declarations and must not match.
    let source = concat!(
        "  const LIMIT = 3 * 4;\n",
        "  const other = Math.min(x, LIMIT);\n",
    );

    assert_eq!(declaration_rhs(source, "const", "LIMIT"), "3 * 4");
}

#[test]
#[should_panic(expected = "must appear exactly once")]
fn declaration_parser_rejects_live_duplicate_declarations() {
    let source = concat!("const int limit = 7;\n", "const int limit = 8;\n");

    let _ = declaration_rhs(source, "const int", "limit");
}

#[test]
#[should_panic(expected = "found 0")]
fn declaration_parser_fails_when_no_declaration_exists() {
    let _ = declaration_rhs("// const int limit = 7;\n", "const int", "limit");
}
