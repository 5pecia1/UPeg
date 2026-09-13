//! Closed allow-lists mirroring the `upeg_core` enums a tool declaration may
//! reference (`pin`, `pegboard_units`, `invoker`, `surfaces`), plus the
//! shared ident validator.
//!
//! Neither macro crate can depend on `upeg_core` directly — `upeg_core`
//! depends on `upeg-macros` for `#[tool]`/`#[toolkit]`, so the dependency
//! can only run one way — so these lists are hand-kept copies of the real
//! enum variants rather than a shared import. Drift between this file and
//! the real enums is caught by a sync test
//! (`upeg-tools/tests/macro_variant_sync.rs`), which reads this file's
//! source and `upeg-core/src/types.rs` and compares the lists.
//!
//! Keep each list's order and spelling identical to the corresponding
//! enum's variants so that sync test's diff output stays readable.

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
/// backing `field` (one of `pin`, `pegboard_units`, `invoker`, `surfaces`).
///
/// Pasting an unchecked ident straight into `::upeg_core::SomeEnum::#ident`
/// (as the macros' codegen does) produces a raw, unhelpful rustc "no variant
/// named" error deep inside macro expansion. Callers run this first so a
/// typo like `pegboard_units = U3` gets a message naming the field and
/// listing the real options.
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
