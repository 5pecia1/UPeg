//! Domain core — transport-agnostic business logic.
//!
//! Hexagonal architecture: this module hosts the rules that depend on
//! neither a specific surface (CLI/HTTP/MCP/TUI under [`crate::surfaces`])
//! nor a specific outbound adapter (filesystem/credentials/project under
//! [`crate::adapters`]).
//!
//! Surfaces translate inbound protocol payloads into domain calls;
//! adapters implement the ports the domain depends on. Migration is
//! incremental — see the in-tree refactor plan.

pub(crate) mod execution;
pub(crate) mod plugin;
pub(crate) mod tools;
