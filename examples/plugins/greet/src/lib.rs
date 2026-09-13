//! `greet` — a minimal upeg WASM plugin example using the new macro system.
//!
//! Exports a single tool `greet.hello` that takes `{"name": "<str>"}`
//! and returns `"Hello, <name>!"`.
//!
//! The `#[tool]` attribute generates the export wrapper and declaration accessor.
//! The `upeg_plugin!` macro generates the manifest export and auto-manages the export symbol.

use upeg_plugin_macros::{tool, upeg_plugin};

/// Greet a person by name.
#[tool(
    id = "greet.hello",
    toolkit = "greet",
    pegboard_units = U1,
    inputs = [ required name: String = "Person to greet" ],
)]
pub fn greet_hello(name: &str) -> String {
    format!("Hello, {name}!")
}

upeg_plugin! {
    toolkit: "greet",
    tools: [greet_hello],
}
