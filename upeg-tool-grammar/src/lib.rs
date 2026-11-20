//! Shared duration and tool-declaration grammar.
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
//! The default `syntax` feature provides the macro DSL using
//! `syn`/`proc-macro2`/`quote`. Without it, only the standard-library duration
//! grammar is compiled, so runtime/wasm consumers do not acquire syntax crates.
//! This crate never depends on `upeg-core`, `upeg-runtime`, or either macro crate.

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

#[cfg(feature = "syntax")]
mod display_label;
mod duration;
#[cfg(feature = "syntax")]
mod enums;
#[cfg(feature = "syntax")]
mod file_policy;
#[cfg(feature = "syntax")]
mod identity;
#[cfg(feature = "syntax")]
mod parse;

#[cfg(feature = "syntax")]
pub use display_label::{display_label_from_slug, rustdoc_first_line};
pub use duration::{DurationParseError, FormattedDuration, format_duration, parse_duration};
#[cfg(feature = "syntax")]
pub use enums::{
    ALLOWED_INVOKERS, ALLOWED_PEGBOARD_UNITS, ALLOWED_PIN_KINDS, ALLOWED_SURFACES,
    validate_enum_ident,
};
#[cfg(feature = "syntax")]
pub use file_policy::reject_file_policy_on_output;
#[cfg(feature = "syntax")]
pub use identity::validate_tool_identity;
#[cfg(feature = "syntax")]
pub use parse::{
    InputRequirement, KindParams, SUPPORTED_INPUT_TYPES, ToolInput, ToolOutput, input_type_label,
};

#[cfg(test)]
mod duration_tests;
#[cfg(all(test, feature = "syntax"))]
mod file_policy_tests;
#[cfg(all(test, feature = "syntax"))]
mod tests;
