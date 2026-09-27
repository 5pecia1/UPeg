//! Internal adapters for local process, filesystem, and runtime glue.

pub(crate) mod credentials;
pub(crate) mod diagnostics;
pub(crate) mod execution_log;
pub(crate) mod filesystem;
#[cfg(feature = "hotkey-trigger")]
pub(crate) mod hotkey;
pub(crate) mod mcp_import;
pub(crate) mod triggers;

#[cfg(test)]
mod mcp_import_tests;
