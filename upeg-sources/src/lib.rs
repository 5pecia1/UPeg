//! Runtime source discovery and registration.
//!
//! This crate is the application boundary for loading Tool sources before a
//! surface renders or dispatches them. CLI/TUI, native Desktop, and future host
//! surfaces call this crate instead of carrying their own path/env/manifest
//! policy.

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

pub mod diagnostics;
pub mod mcp_import;
pub mod memos;
pub mod pegboard;
pub mod project;
mod protocol;
mod source_freshness;
mod sources;
#[cfg(not(target_arch = "wasm32"))]
pub mod storage;
pub mod store;

pub use source_freshness::{LoadedSourcesError, validate_loaded_sources};

pub use sources::{
    DirectoryStatus, McpImportLoad, ProjectManifestLoad, RuntimeSourceConfig, RuntimeSourceDirs,
    RuntimeSourceReport, load_local_runtime_sources, load_mcp_imports_for_host,
    mcp_import_reexport_policy, survey_runtime_source_dirs,
};

pub type McpImportRegistration = (
    String,
    Result<mcp_import::ImportOutcome, mcp_import::ImportError>,
);
pub type McpImportRegistrations = Vec<McpImportRegistration>;

fn truncate_for_display(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }
    let total_chars = s.chars().count();
    if total_chars <= max_chars {
        return s.to_string();
    }
    let prefix: String = s.chars().take(max_chars).collect();
    format!("{prefix}…(truncated, {total_chars} chars total)")
}
