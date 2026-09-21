#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Red-phase tests for controlled_embed.bindings nesting refactoring.
//!
//! These tests verify the new TOML structure where `selector_bindings`
//! are nested inside `controlled_embed` as `bindings`.
//!
//! ## Current State (flat structure - will FAIL)
//!
//! ```toml
//! [[tools]]
//! controlled_embed = { user_agent = "mobile_safari" }
//! selector_bindings = [...]
//! ```
//!
//! ## Target State (nested structure - should PASS after Tasks 2-4)
//!
//! ```toml
//! [[tools]]
//! controlled_embed = { user_agent = "mobile_safari", bindings = [...] }
//! ```
//!
//! Run with: `cargo test --package upeg-loader --test controlled_embed_nesting`

use upeg_loader::ToolEntryToml;

/// Helper to parse TOML into ToolEntryToml
fn parse_tool_entry(toml_str: &str) -> Result<ToolEntryToml, toml::de::Error> {
    toml::from_str(toml_str)
}

/// Test: nested controlled-embed bindings parse
///
/// Verifies that bindings nested inside controlled_embed are correctly parsed.
///
/// CURRENT: This test will FAIL because `bindings` field doesn't exist in ControlledEmbedToml
/// AFTER REFACTORING: This test will PASS
#[test]
fn nested_controlled_embed_bindings_parse() {
    let toml_str = r#"
id = "test_tool"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com"

[controlled_embed.browser]
user_agent = "mobile_safari"
viewport = "mobile"

[[controlled_embed.bindings]]
role = "input"
field = "query"
selector = '''#search-input'''

[[controlled_embed.bindings]]
role = "trigger"
field = ""
selector = '''#search-button'''
action = "click"

[[controlled_embed.bindings]]
role = "output"
field = "results"
selector = '''#results'''
"#;

    let tool: ToolEntryToml = parse_tool_entry(toml_str).expect("TOML should parse");

    // Verify controlled_embed exists
    let ce = tool
        .controlled_embed
        .expect("controlled_embed should be present");

    // Verify nested bindings exist
    let bindings = ce
        .bindings
        .expect("bindings should be nested inside controlled_embed");
    assert_eq!(bindings.len(), 3, "should have 3 bindings");

    // Verify first binding
    assert_eq!(bindings[0].role, "input");
    assert_eq!(bindings[0].field, "query");
    assert_eq!(bindings[0].selector, "#search-input");

    // Verify second binding (trigger)
    assert_eq!(bindings[1].role, "trigger");
    assert_eq!(bindings[1].selector, "#search-button");
    assert_eq!(bindings[1].action.as_deref(), Some("click"));

    // Verify third binding (output)
    assert_eq!(bindings[2].role, "output");
    assert_eq!(bindings[2].field, "results");
    assert_eq!(bindings[2].selector, "#results");
}

/// Test: top-level selector bindings are rejected
///
/// Verifies that top-level selector_bindings (flat structure) are rejected
/// after the refactoring.
///
#[test]
fn top_level_selector_bindings_are_rejected() {
    // Use inline table format (similar to working tests in loader.rs)
    // Note: Use ''' for raw strings to avoid # being interpreted as prefix
    let toolkit_str = r#"id = "test_tool"

[[tools]]
id = "test"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com"
selector_bindings = [
  { role = "input", field = "query", selector = '#search-input' },
]"#;

    // Full toolkit parsing includes validation
    let result = upeg_loader::parse_toolkit_full(toolkit_str);

    assert!(
        result.is_err(),
        "top-level selector_bindings should be rejected"
    );
    let err = result.unwrap_err();
    match err {
        upeg_loader::LoadError::Toml(error) => {
            let message = error.to_string();
            assert!(message.contains("unknown field"), "{message}");
            assert!(message.contains("selector_bindings"), "{message}");
        }
        other => panic!("expected TOML unknown-field error, got: {other}"),
    }
}

/// Test: empty bindings are treated as optional
///
/// Verifies that omitting bindings (controlled_embed without bindings) is valid.
#[test]
fn empty_bindings_are_treated_as_optional() {
    let toml_str = r#"
id = "test_tool"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com"

[controlled_embed.browser]
user_agent = "mobile_safari"
viewport = "mobile"
"#;

    let tool: ToolEntryToml = parse_tool_entry(toml_str).expect("TOML should parse");

    let ce = tool
        .controlled_embed
        .expect("controlled_embed should be present");

    // bindings should be optional (None is valid)
    assert!(
        ce.bindings.is_none(),
        "bindings should be optional - None is valid"
    );

    // Other fields should still work
    assert_eq!(ce.user_agent.as_deref(), Some("mobile_safari"));
    assert_eq!(ce.viewport.as_deref(), Some("mobile"));
}

/// Test: minimal bindings alone are valid
///
/// Verifies that only selector field is required, other fields have defaults.
#[test]
fn minimal_bindings_alone_are_valid() {
    let toml_str = r#"
id = "test_tool"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com"

[controlled_embed.browser]
user_agent = "default"

[[controlled_embed.bindings]]
selector = '''#input-field'''
"#;

    let tool: ToolEntryToml = parse_tool_entry(toml_str).expect("TOML should parse");

    let ce = tool
        .controlled_embed
        .expect("controlled_embed should be present");

    let bindings = ce.bindings.expect("bindings should be present");
    assert_eq!(bindings.len(), 1);

    // Default role should be "input"
    assert_eq!(bindings[0].role, "input", "default role should be 'input'");

    // Default field should be empty string
    assert_eq!(bindings[0].field, "", "default field should be empty");

    // selector should be as specified
    assert_eq!(bindings[0].selector, "#input-field");
}
