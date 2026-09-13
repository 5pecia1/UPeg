//! Agent-facing Board context, MCP connection config, and personal guidance edits.

use std::fmt::Write as _;

use upeg_core::{BoardGuidance, BoardKey};
use upeg_sources::pegboard::{self, PegboardState};

use crate::board_agent::{
    BoardAgentContext, BoardToolReadinessStatus, board_connection_preview, board_context,
};
use crate::error::CliError;

const EMPTY_VALUE: &str = "(none)";

pub(super) fn run_context(board: &BoardKey, json: bool) -> Result<String, CliError> {
    let context = board_context(board).map_err(board_agent_error)?;
    if json {
        return pretty_json(&context);
    }
    Ok(render_context(&context))
}

pub(super) fn run_connect(board: &BoardKey) -> Result<String, CliError> {
    let preview = board_connection_preview(board).map_err(board_agent_error)?;
    pretty_json(&preview.config)
}

#[allow(
    clippy::fn_params_excessive_bools,
    reason = "the booleans preserve clap's explicit clear flags across the command boundary"
)]
pub(super) fn run_describe(
    state: &PegboardState,
    board: &BoardKey,
    description: Option<String>,
    clear_description: bool,
    instructions: Option<String>,
    clear_instructions: bool,
    json: bool,
) -> Result<String, CliError> {
    if description.is_none() && !clear_description && instructions.is_none() && !clear_instructions
    {
        return Err(CliError::tool_failed(
            "board describe: nothing to update; pass --description, --instructions, or a --clear-* flag",
        ));
    }

    let current = pegboard::board_guidance_in(state, board.as_str())
        .cloned()
        .ok_or_else(|| CliError::tool_failed(format!("unknown board `{board}`")))?;
    let guidance = BoardGuidance {
        description: patched_value(current.description, description, clear_description),
        instructions: patched_value(current.instructions, instructions, clear_instructions),
    };
    pegboard::set_board_guidance(board.as_str(), guidance.clone())
        .map_err(|error| CliError::tool_failed(format!("board describe: {error}")))?;

    if json {
        return pretty_json(&serde_json::json!({
            "board": board.as_str(),
            "description": guidance.description,
            "instructions": guidance.instructions,
        }));
    }
    Ok(format!(
        "updated board `{board}` guidance\nDescription: {}\nInstructions:\n{}\n",
        display_or_empty(&guidance.description),
        display_or_empty(&guidance.instructions),
    ))
}

fn patched_value(current: String, replacement: Option<String>, clear: bool) -> String {
    if clear {
        String::new()
    } else {
        replacement.unwrap_or(current)
    }
}

fn board_agent_error(error: impl std::fmt::Display) -> CliError {
    CliError::tool_failed(format!("board context: {error}"))
}

fn pretty_json(value: &impl serde::Serialize) -> Result<String, CliError> {
    let mut output = serde_json::to_string_pretty(value)
        .map_err(|error| CliError::tool_failed(format!("board context: {error}")))?;
    output.push('\n');
    Ok(output)
}

fn render_context(context: &BoardAgentContext) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Board: {} ({})", context.title, context.board);
    let _ = writeln!(
        output,
        "Description: {}",
        display_or_empty(&context.description)
    );
    let _ = writeln!(output, "Instructions:");
    let _ = writeln!(output, "{}", display_or_empty(&context.instructions));
    let _ = writeln!(
        output,
        "Working directory: {}",
        context.working_directory.display()
    );
    let _ = writeln!(
        output,
        "Project manifest: {}",
        display_path(context.project_manifest.as_deref())
    );
    let _ = writeln!(
        output,
        "Guidance manifest: {}",
        display_path(context.guidance_manifest.as_deref())
    );
    let _ = writeln!(output, "Revision: {}", context.revision);
    let _ = writeln!(output, "Update policy: {}", context.update_policy);
    if !context.unresolved_pins.is_empty() {
        let _ = writeln!(
            output,
            "Preview incomplete — pinned tools not loaded: {}. Check sources; MCP imports may load after connecting.",
            context.unresolved_pins.join(", "),
        );
    }
    let _ = writeln!(output, "Tools ({}):", context.tools.len());
    for tool in &context.tools {
        let _ = writeln!(
            output,
            "- {} [{}] {}",
            tool.id,
            readiness_label(tool.readiness.status),
            tool.description
        );
        let _ = writeln!(output, "  defaults: {}", tool.defaults);
        if let Some(directory) = &tool.working_directory {
            let _ = writeln!(output, "  working directory: {}", directory.display());
        }
        if !tool.readiness.reasons.is_empty() {
            let _ = writeln!(output, "  readiness: {}", tool.readiness.reasons.join("; "));
        }
    }
    output
}

fn display_or_empty(value: &str) -> &str {
    if value.is_empty() { EMPTY_VALUE } else { value }
}

fn display_path(path: Option<&std::path::Path>) -> String {
    path.map_or_else(
        || EMPTY_VALUE.to_string(),
        |path| path.display().to_string(),
    )
}

const fn readiness_label(status: BoardToolReadinessStatus) -> &'static str {
    match status {
        BoardToolReadinessStatus::Ready => "ready",
        BoardToolReadinessStatus::Unavailable => "unavailable",
        BoardToolReadinessStatus::Unchecked => "unchecked",
    }
}
