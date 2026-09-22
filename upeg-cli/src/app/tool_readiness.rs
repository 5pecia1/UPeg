//! Shared native readiness context for non-executing tool inspection.

use std::ffi::OsString;

use upeg_core::{BoardKey, Surface};
use upeg_runtime::readiness::{ToolReadiness, ToolReadinessContext, inspect_tool_readiness};

use crate::error::CliError;

/// Inspect one tool in this process without starting its command.
///
/// A Board supplies the same PATH override its scoped MCP server receives;
/// the current directory remains the process directory, as it does for a
/// Board connection. Callers enforce their own surface and Board pin gates
/// before using this helper.
pub(crate) fn inspect_local_tool_readiness(
    tool_id: &str,
    board: Option<&BoardKey>,
) -> Result<Option<ToolReadiness>, CliError> {
    let working_directory = std::env::current_dir()
        .and_then(|directory| directory.canonicalize())
        .map_err(|error| CliError::tool_failed(format!("readiness: working directory: {error}")))?;
    inspect_tool_readiness_at(tool_id, board, working_directory)
}

/// Same inspection with the caller's already-resolved execution directory.
pub(crate) fn inspect_tool_readiness_at(
    tool_id: &str,
    board: Option<&BoardKey>,
    working_directory: std::path::PathBuf,
) -> Result<Option<ToolReadiness>, CliError> {
    let search_path = board.and_then(board_search_path);
    Ok(inspect_tool_readiness(
        tool_id,
        &ToolReadinessContext {
            working_directory,
            search_path,
        },
    ))
}

fn board_search_path(board: &BoardKey) -> Option<OsString> {
    upeg_runtime::board_context(board.as_str())
        .env
        .into_iter()
        .find_map(|(key, value)| {
            let is_path = if cfg!(windows) {
                key.eq_ignore_ascii_case("PATH")
            } else {
                key == "PATH"
            };
            is_path.then(|| OsString::from(value))
        })
}

/// Parse an optional Board only once at the public command boundary.
pub(crate) fn parse_board_key(board: Option<&str>) -> Result<Option<BoardKey>, CliError> {
    board
        .map(BoardKey::parse)
        .transpose()
        .map_err(|error| CliError::tool_failed(format!("readiness: board: {error}")))
}

/// Does the tool exist on the requested surface? Readiness must not disclose
/// tools that the caller could not otherwise list or invoke.
pub(crate) fn ensure_visible_on_surface(tool_id: &str, surface: Surface) -> Result<(), CliError> {
    match upeg_runtime::toolbox_tool(tool_id) {
        Some(meta) if meta.is_on_surface(surface) => Ok(()),
        _ => Err(CliError::tool_failed(format!(
            "unknown tool `{}`",
            crate::display_id(tool_id)
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use upeg_runtime::{
        execution_requirements::{
            CommandSearchPath, ToolExecutionRequirements, set_tool_execution_requirements,
        },
        readiness::ToolReadinessStatus,
    };

    #[test]
    fn unknown_tool_is_not_reported_as_external_readiness() {
        assert_eq!(
            inspect_local_tool_readiness("test.readiness.unknown", None).expect("inspection"),
            None
        );
    }

    #[test]
    fn inspection_reports_a_missing_command_without_executing_it() {
        let id = "test.readiness.missing_command";
        set_tool_execution_requirements(
            id,
            Some(ToolExecutionRequirements {
                command: Some("upeg-command-that-does-not-exist".to_string()),
                search_path: CommandSearchPath::Inherit,
                ..ToolExecutionRequirements::default()
            }),
        );

        let readiness = inspect_local_tool_readiness(id, None)
            .expect("inspect requirements")
            .expect("external requirements");

        assert_eq!(readiness.status, ToolReadinessStatus::MissingExecutable);
        set_tool_execution_requirements(id, None);
    }
}
