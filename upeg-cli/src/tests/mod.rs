//! Cross-cutting CLI integration tests.
//!
//! Surface-specific tests live next to their surface (e.g.,
//! `surfaces/http/tests.rs`, `surfaces/tui/state_tests.rs`); these tests
//! exercise the broad CLI entry point and end-to-end behavior that
//! spans multiple modules. Helpers shared across files live in
//! [`common`].

mod common;
mod display_and_suggestions;

mod board_scope;
mod chain_approval;
mod cli_builtins;
mod cli_call;
mod cli_discovery;
mod dynamic_route_ux;
mod general;
mod late_tools;
