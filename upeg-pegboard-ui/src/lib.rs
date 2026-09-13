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
//! Pure Rust UI state for the upeg pegboard.
//!
//! Holds pegboard features (boards/layouts/tweaks/memos/backup), the
//! deep-link resolver, pin chrome (colors/icons), the i18n catalog, and
//! platform storage helpers as UI-framework-agnostic Rust. Exposed to
//! the Flutter UI via `upeg-frb` (flutter_rust_bridge). Grid placement
//! (the non-overlapping push/reflow algorithm) lives in `upeg-runtime`;
//! grid geometry (cell sizing, anchor-from-pointer) is Dart-side in the
//! Flutter shell. This crate carries no desktop-specific interaction
//! state of its own.

pub mod deep_link;
pub mod features;
pub mod i18n;
pub mod pin_chrome;
pub mod platform;
pub mod tool_meta;
