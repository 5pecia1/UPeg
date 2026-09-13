//! Domain-side helpers for tool identity: rendering ids for display
//! and suggesting near-matches when a caller asks for an unknown one.
//!
//! Transport-agnostic: every surface (CLI/HTTP/MCP/TUI) consumes the
//! same `display_id` truncation rule and the same "did you mean" hint
//! format so users see consistent diagnostics across protocols.

pub(crate) mod display;
pub(crate) mod suggest;
