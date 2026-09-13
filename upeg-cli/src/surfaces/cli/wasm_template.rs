//! Starter source printed by `upeg wasm template`. Mirrors the
//! contract enforced by `upeg-wasm`:
//!
//!   - The plugin must export `manifest` returning JSON `{id, tools: [...]}`.
//!   - Each tool's `id` must use the canonical `{toolkit}.{tool}` form;
//!     the `export` field names the wasm export (a Rust identifier).
//!   - Each tool's exported function takes the JSON-stringified args
//!     and returns a string.
//!
//! Feature-gated together with the WASM plugin host itself.

/// Static starter source for an upeg WASM plugin. Printed by
/// `upeg wasm template` so users can pipe it directly into a new
/// project and edit.
#[cfg(feature = "wasm-plugin")]
pub(crate) fn wasm_plugin_template() -> String {
    String::from(
        r#"//! upeg WASM plugin template — starter for `cargo build --target wasm32-unknown-unknown --release`.
//!
//! Cargo.toml prerequisites (sketch):
//!
//!     # Empty `[workspace]` so cargo doesn't try to fold this into the
//!     # parent (only relevant if you `cargo new` inside an existing
//!     # workspace; harmless otherwise).
//!     [workspace]
//!
//!     [package]
//!     name = "my_upeg_plugin"
//!     version = "0.1.0"
//!     edition = "2021"
//!
//!     [lib]
//!     crate-type = ["cdylib"]
//!
//!     [dependencies]
//!     extism-pdk = "1"
//!     serde_json = "1"
//!     upeg-plugin-api = "0.1"
//!
//!     [profile.release]
//!     lto = true
//!     opt-level = "s"
//!
//! Drop the resulting `target/wasm32-unknown-unknown/release/<name>.wasm`
//! into `~/.upeg/wasm/` (or `$UPEG_WASM_DIR`) — `upeg` (built
//! with `--features wasm-plugin`) auto-loads it on startup.

use extism_pdk::*;
use upeg_plugin_api::{
    PluginInputField, PluginInputKind, PluginInputSpec, PluginManifest, PluginToolDecl,
};

/// Required: `manifest` export tells upeg which tools this plugin offers.
/// Top-level `id` is the Toolkit id. Each tool `id` must use the canonical
/// `{toolkit}.{tool}` shape; `export` must be the literal Rust function name
/// (wasm export name).
#[plugin_fn]
pub fn manifest(_: ()) -> FnResult<String> {
    let manifest = PluginManifest::new("myplugin")
        .with_tags(["wasm", "example"])
        .with_tool(
            PluginToolDecl::new("myplugin", "myplugin.greet", "myplugin_greet")
                .with_tags(["example"])
                .with_description("Greet someone by name.")
                .with_input_spec(myplugin_greet_input_spec()),
        );

    Ok(manifest.to_json()?)
}

/// Each declared tool gets one exported function. The host calls it
/// with the JSON-stringified args object and expects a string back.
#[plugin_fn]
pub fn myplugin_greet(input: String) -> FnResult<String> {
    let v: serde_json::Value = serde_json::from_str(&input)
        .unwrap_or(serde_json::Value::Null);
    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("world");
    Ok(format!("Hello, {name}!"))
}

fn myplugin_greet_input_spec() -> PluginInputSpec {
    PluginInputSpec::new([PluginInputField::required("name", PluginInputKind::String)])
}
"#,
    )
}
