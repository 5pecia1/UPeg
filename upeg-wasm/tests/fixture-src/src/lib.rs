//! Test fixture: a minimal extism plugin used by upeg-wasm integration tests.
//!
//! Tool ids follow upeg's `{toolkit}.{tool}` convention; the wasm export
//! name (an `__upeg_export_*` symbol emitted by `#[tool]`) is declared
//! separately via the manifest's `export` field.
use upeg_plugin_macros::{tool, upeg_plugin};

/// Echo input back, prefixed.
#[tool(
    id = "test.wasm.echo",
    toolkit = "test",
    pegboard_units = U1,
    inputs = [ optional input: String = "Text to echo" ],
)]
pub fn echo(input: Option<&str>) -> String {
    format!("echoed: {}", input.unwrap_or(""))
}

/// Uppercase the input.
#[tool(
    id = "test.wasm.shout",
    toolkit = "test",
    pegboard_units = U1,
    inputs = [ required input: String = "Text to uppercase" ],
)]
pub fn shout(input: &str) -> String {
    input.to_uppercase()
}

upeg_plugin! {
    toolkit: "test",
    tags: ["wasm", "test"],
    tools: [echo, shout],
}
