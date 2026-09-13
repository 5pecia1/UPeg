//! CLI adapter facade for upstream MCP server registration.
//!
//! `upeg-sources` owns the shared implementation so CLI and Desktop load the
//! same upstream MCP source model. This module preserves the CLI's internal
//! adapter path used by inventory code and tests.

pub use upeg_sources::mcp_import::*;
