//! Tool-level partial-success types for MCP import.
//!
//! A tool whose upstream declaration fails conversion to upeg typed
//! I/O is skipped with a structured [`SkipReason`] while the rest of
//! the same server registers. These types are the contract callers
//! (startup log, MCP manager surface, tests) consume to report which
//! tool was dropped and why.

use upeg_core::InputAdapterError;

use super::{OutputSchemaError, Registration, RemoteToolDecl};

/// Cap for the skipped-tools summary embedded in
/// [`super::ImportError::AllToolsSkipped`] so a pathological server
/// (hundreds of unconvertible tools) cannot bloat the error message.
const SKIPPED_SUMMARY_MAX_CHARS: usize = 500;

/// Why a single upstream tool was skipped while the rest of the same
/// server imported (tool-level partial success). Every variant is a
/// deterministic conversion failure between the upstream declaration
/// and upeg's typed I/O — retrying without changing the upstream
/// schema cannot succeed, so the tool is dropped instead of failing
/// the server.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SkipReason {
    /// The upstream `name` was empty or whitespace-only; the
    /// namespaced id `<server>.` would be unidentifiable downstream.
    #[error("tool entry `name` is empty or whitespace-only")]
    EmptyName,
    /// The upstream `inputSchema` uses a construct upeg's typed input
    /// subset cannot express. The wrapped error names the unsupported
    /// keyword or shape (e.g. `$ref`, `oneOf`, nested object).
    #[error("`inputSchema` is not importable as upeg typed inputs: {0}")]
    UnsupportedInputSchema(InputAdapterError),
    /// The upstream `outputSchema` failed conversion to a typed
    /// output spec, including malformed `x-upeg-*` extension metadata.
    #[error("`outputSchema` is not importable as upeg typed outputs: {0}")]
    UnsupportedOutputSchema(OutputSchemaError),
}

/// One upstream tool that was not imported, plus the structured reason.
#[derive(Debug, Clone, PartialEq)]
pub struct SkippedTool {
    /// The upstream server's own tool id (unnamespaced).
    pub id: String,
    pub reason: SkipReason,
}

/// Bounded `id: reason` listing for
/// [`super::ImportError::AllToolsSkipped`].
pub(super) fn skipped_summary(skipped: &[SkippedTool]) -> String {
    let joined = skipped
        .iter()
        .map(|tool| format!("`{}`: {}", tool.id, tool.reason))
        .collect::<Vec<_>>()
        .join("; ");
    crate::truncate_for_display(&joined, SKIPPED_SUMMARY_MAX_CHARS)
}

/// A parsed `tools/list` response: importable declarations plus the
/// entries that were skipped with their reasons.
#[derive(Debug, Clone, Default)]
pub struct ParsedToolsList {
    pub decls: Vec<RemoteToolDecl>,
    pub skipped: Vec<SkippedTool>,
}

/// Result of a successful per-server import: what registered plus what
/// was skipped (tool-level partial success).
#[derive(Debug)]
pub struct ImportOutcome {
    /// Owning handle for the registered dispatchers; drop it to
    /// deregister the imported tools.
    pub registration: Registration,
    /// Tools that failed conversion, with structured reasons. A
    /// skipped tool is registered on no surface at all, so the
    /// `reexport` opt-in never applies to it.
    pub skipped: Vec<SkippedTool>,
}

impl ImportOutcome {
    /// Namespaced ids of the tools that actually registered.
    pub fn registered_ids(&self) -> &[&'static str] {
        self.registration.ids()
    }
}
