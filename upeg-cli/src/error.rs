//! `CliError` — the public, transport-agnostic error type returned by
//! the upeg-cli library. Surfaces map it into their native error shape
//! (CLI: stderr + exit code; HTTP: JSON body + status; MCP: error
//! object), so the same enum drives every consumer.
//!
//! The "did you mean" hint suffix appended to `UnknownTool` is rendered
//! through [`crate::domain::tools`] so the algorithm stays uniform with
//! the other unknown-id surfaces.

use upeg_core::Surface;

use crate::domain::tools::display::display_id;
use crate::domain::tools::suggest::unknown_tool_hint;

#[derive(Debug, PartialEq, Eq)]
pub enum CliError {
    /// `upeg tool show <id>` was given an id we don't know.
    UnknownTool(String),
    /// A Tool returned `Err`. The owned `String` carries the tool's own message.
    ToolFailed(String),
    /// A failing command that must write its machine-readable payload to stdout.
    StdoutFailure { stdout: String },
}

impl CliError {
    pub fn tool_failed(msg: impl Into<String>) -> Self {
        Self::ToolFailed(msg.into())
    }

    pub fn stdout_failure(stdout: impl Into<String>) -> Self {
        Self::StdoutFailure {
            stdout: stdout.into(),
        }
    }

    pub fn stdout(&self) -> Option<&str> {
        match self {
            Self::StdoutFailure { stdout } => Some(stdout),
            Self::UnknownTool(_) | Self::ToolFailed(_) => None,
        }
    }

    pub const fn exit_code(&self) -> u8 {
        1
    }

    pub fn message(&self) -> String {
        match self {
            Self::UnknownTool(id) => {
                let mut base = format!("upeg: unknown tool `{}`", display_id(id));
                base.push_str(&unknown_tool_hint(id, Some(Surface::Cli)));
                base
            }
            Self::ToolFailed(msg) => format!("upeg: {msg}"),
            Self::StdoutFailure { .. } => String::new(),
        }
    }
}
