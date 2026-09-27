//! Closed allow-lists mirroring the `upeg_core` enums a tool declaration may
//! reference (`pin`, `pegboard_units`, `invoker`, `surfaces`), plus the
//! shared ident validator.
//!
//! Neither macro crate can depend on `upeg_core`: it depends on
//! `upeg-macros` for `#[tool]`/`#[toolkit]`. These lists therefore copy the
//! real enum variants. `upeg-tools/tests/macro_variant_sync.rs` compares
//! this source with `upeg-core/src/types.rs` to catch drift.
//!
//! Keep each list's order and spelling identical to the corresponding
//! enum variants so that the sync test's diff stays readable.

use syn::{Ident, Result};

/// Mirrors `upeg_core::PinKind` (upeg-core/src/types.rs).
pub const ALLOWED_PIN_KINDS: &[&str] = &[
    "Inline",
    "Launcher",
    "Live",
    "Action",
    "Embed",
    "ControlledEmbed",
    "Chain",
    "Llm",
];

/// Mirrors `upeg_core::PegboardUnits` (upeg-core/src/types.rs).
pub const ALLOWED_PEGBOARD_UNITS: &[&str] = &["U1", "U2", "U2T"];

/// Mirrors `upeg_core::Invoker` (upeg-core/src/types.rs).
pub const ALLOWED_INVOKERS: &[&str] = &[
    "Function", "External", "Http", "Static", "Embed", "Chain", "Llm", "Wasm",
];

/// Mirrors `upeg_core::Surface` (upeg-core/src/types.rs).
pub const ALLOWED_SURFACES: &[&str] = &["Cli", "Tui", "Desktop", "Pwa", "Ext", "Mcp", "Http"];

/// Validates that `ident` names a known variant of the `upeg_core` enum
/// backing `field`: `pin`, `pegboard_units`, `invoker`, or `surfaces`.
///
/// Pasting an unchecked ident straight into `::upeg_core::SomeEnum::#ident`
/// (as macro codegen does) gives an unhelpful rustc "no variant named" error
/// inside macro expansion. Calling this first reports the field and valid
/// options, for example for `pegboard_units = U3`.
pub fn validate_enum_ident(ident: &Ident, field: &str, allowed: &[&str]) -> Result<()> {
    let value = ident.to_string();
    if allowed.contains(&value.as_str()) {
        return Ok(());
    }
    Err(syn::Error::new(
        ident.span(),
        format!(
            "unknown {field} `{value}`; expected one of: {}",
            allowed.join(", ")
        ),
    ))
}
