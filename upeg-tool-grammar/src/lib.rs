//! Shared, drift-prone grammar for upeg's `#[tool]`-style attribute macros.
//!
//! The built-in `upeg-macros` and WASM-guest `upeg-plugin-macros` crates
//! parse the same `inputs = [...]` / `outputs = [...]` DSL, validate the
//! same `{toolkit}.{tool}` identities, and check the same closed
//! `pin`/`pegboard_units`/`invoker`/`surfaces` enum-ident sets. Proc-macro
//! crates cannot export plain `pub` items, so this crate replaces their
//! hand-kept grammar copies.
//!
//! This is a **plain library crate** (no `proc-macro = true`), so it can
//! export ordinary types, consts, and functions for both macro crates. It
//! depends only on `syn`/`proc-macro2`/`quote`, not on `upeg-core`,
//! `upeg-runtime`, or either macro crate.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::get_unwrap,
        clippy::tests_outside_test_module,
        clippy::print_stdout,
        clippy::unreachable,
        clippy::string_add,
        clippy::manual_let_else,
        reason = "tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
    )
)]

mod display_label;
mod enums;
mod file_policy;
mod identity;
mod parse;

pub use display_label::{display_label_from_slug, rustdoc_first_line};
pub use enums::{
    ALLOWED_INVOKERS, ALLOWED_PEGBOARD_UNITS, ALLOWED_PIN_KINDS, ALLOWED_SURFACES,
    validate_enum_ident,
};
pub use file_policy::reject_file_policy_on_output;
pub use identity::validate_tool_identity;
pub use parse::{
    InputRequirement, KindParams, SUPPORTED_INPUT_TYPES, ToolInput, ToolOutput, input_type_label,
};

#[cfg(test)]
mod file_policy_tests;
#[cfg(test)]
mod tests;
