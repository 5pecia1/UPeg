//! One resolved Board view for people and MCP agents.
//!
//! Guidance comes from the same source view as the pins. Connection configuration
//! binds the current working directory and manifest before the child loads tools.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use serde::Serialize;
use serde_json::{Value, json};
use upeg_core::{BoardKey, Surface};
use upeg_runtime::ToolMetaRuntimeExt;
use upeg_sources::pegboard;

mod connection;
#[cfg(test)]
mod revision_tests;
#[cfg(test)]
mod source_tests;
mod tool;

pub use connection::board_connection_preview;
use tool::effective_tool_schema;

/// Reserved introspection name, present only on a Board-scoped MCP connection.
pub const BOARD_CONTEXT_TOOL: &str = "upeg.board_context";
/// The connection contract deliberately avoids silently changing agent context.
pub const BOARD_UPDATE_POLICY: &str = "Reconnect after changing board guidance, pins, presets or tool configuration. Layout-only changes do not require reconnecting.";

/// The checks performed before running a tool. Ready is a prerequisite check,
/// not a prediction that the requested operation will succeed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardToolReadinessStatus {
    Ready,
    Unavailable,
    Unchecked,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoardToolReadiness {
    pub status: BoardToolReadinessStatus,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoardAgentTool {
    pub id: String,
    pub description: String,
    pub input_schema: Value,
    pub defaults: Value,
    pub working_directory: Option<PathBuf>,
    pub readiness: BoardToolReadiness,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoardAgentContext {
    pub board: String,
    pub title: String,
    pub description: String,
    pub instructions: String,
    pub working_directory: PathBuf,
    pub project_manifest: Option<PathBuf>,
    /// Only present when this Board's guide is owned by a repository manifest.
    pub guidance_manifest: Option<PathBuf>,
    pub tools: Vec<BoardAgentTool>,
    /// Saved pins whose tool metadata has not loaded in this process.
    pub unresolved_pins: Vec<String>,
    pub revision: String,
    pub update_policy: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoardConnectionPreview {
    pub context: BoardAgentContext,
    pub config: Value,
    pub readiness: BoardToolReadiness,
}

#[derive(Debug, thiserror::Error)]
pub enum BoardAgentError {
    #[error("unknown board `{0}`")]
    UnknownBoard(String),
    #[error(
        "tool name `upeg.board_context` is reserved for Board connection guidance; rename this pinned tool"
    )]
    ReservedTool,
    #[error("board configuration store is unavailable")]
    StoreUnavailable,
    #[error("board configuration: {0}")]
    Store(#[from] pegboard::PegboardStateError),
    #[error("board execution environment: {0}")]
    Io(#[from] std::io::Error),
    #[error("board context serialization: {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "project manifest changed: {0}; restart UPeg or reconnect the MCP server to reload its guide and tools"
    )]
    ProjectChanged(PathBuf),
    #[error(transparent)]
    Sources(#[from] upeg_sources::LoadedSourcesError),
}

/// Inspect exactly the MCP-visible pinned tools for the selected Board.
pub fn board_context(board: &BoardKey) -> Result<BoardAgentContext, BoardAgentError> {
    Ok(board_snapshot(board)?.context)
}

/// A single persisted Board configuration shared by revision checks and calls.
pub(crate) struct BoardAgentSnapshot {
    pub context: BoardAgentContext,
    pub state: pegboard::PegboardState,
}

pub(crate) fn board_snapshot(board: &BoardKey) -> Result<BoardAgentSnapshot, BoardAgentError> {
    let path = pegboard::state_path_from_env().ok_or(BoardAgentError::StoreUnavailable)?;
    let visibility = pegboard::BoardVisibility::from_process();
    let mut state = upeg_sources::store::Store::open_at(&path)
        .and_then(|store| store.load_state(&visibility))
        .map_err(pegboard::PegboardStateError::from)?;
    if state.boards.is_empty() {
        state = pegboard::default_state();
    }
    pegboard::merge_project_boards(&mut state, &visibility);
    let context = context_from_snapshot(board, &state)?;
    Ok(BoardAgentSnapshot { context, state })
}

/// Reinspect deferred tool registrations without reading a different Board.
pub(crate) fn context_from_snapshot(
    board: &BoardKey,
    state: &pegboard::PegboardState,
) -> Result<BoardAgentContext, BoardAgentError> {
    upeg_sources::validate_loaded_sources()?;
    if let Some(scope) = upeg_runtime::pegboard_project::project_board_scope()
        && scope.source_changed()
    {
        return Err(BoardAgentError::ProjectChanged(
            scope.manifest_path().to_path_buf(),
        ));
    }
    let pins = state
        .layouts
        .get(board.as_str())
        .map_or(&[][..], Vec::as_slice);
    let mut unresolved_pins = pins
        .iter()
        .filter(|pin| upeg_runtime::toolbox_tool(&pin.tool_id).is_none())
        .map(|pin| pin.tool_id.clone())
        .collect::<Vec<_>>();
    unresolved_pins.sort();
    if state
        .layouts
        .get(board.as_str())
        .into_iter()
        .flatten()
        .any(|pin| pin.tool_id == BOARD_CONTEXT_TOOL)
    {
        return Err(BoardAgentError::ReservedTool);
    }
    let entry = state
        .boards
        .iter()
        .find(|entry| entry.key == board.as_str())
        .ok_or_else(|| BoardAgentError::UnknownBoard(board.to_string()))?;
    let working_directory = std::env::current_dir()?.canonicalize()?;
    let project_manifest = upeg_sources::project::current_project_root()
        .map(|root| root.join(upeg_core::PROJECT_MARKER_DIR))
        .or_else(upeg_sources::project::detect_project_manifest)
        .map(|marker| {
            let config = marker.join(upeg_core::PROJECT_CONFIG_FILE);
            if config.is_file() { config } else { marker }
        });
    let guidance_manifest = upeg_runtime::pegboard_project::project_board_scope()
        .filter(|scope| scope.declaration(board.as_str()).is_some())
        .map(|scope| scope.manifest_path().to_path_buf());
    let execution = upeg_runtime::board_context(board.as_str());
    let board_path = execution
        .env
        .iter()
        .find(|(key, _)| {
            if cfg!(windows) {
                key.eq_ignore_ascii_case("PATH")
            } else {
                key.as_str() == "PATH"
            }
        })
        .map(|(_, value)| value.as_str());
    let mut tools =
        pegboard::board_entries_on_surface_in(state, board.as_str(), None, Surface::Mcp)
            .into_iter()
            .filter(|(_, meta)| meta.id != BOARD_CONTEXT_TOOL)
            .map(|(placement, meta)| {
                tool::inspect(
                    meta,
                    placement.args_preset.as_ref(),
                    &working_directory,
                    board_path,
                )
            })
            .collect::<Vec<_>>();
    tools.sort_by(|left, right| left.id.cmp(&right.id));
    let mut context = BoardAgentContext {
        board: board.to_string(),
        title: entry.title.clone(),
        description: entry.guidance.description.clone(),
        instructions: entry.guidance.instructions.clone(),
        working_directory,
        project_manifest,
        guidance_manifest,
        tools,
        unresolved_pins,
        revision: String::new(),
        update_policy: BOARD_UPDATE_POLICY.into(),
    };
    context.revision = revision(&context, pins)?;
    Ok(context)
}

/// No layout coordinates or readiness probes in the revision: neither changes
/// the callable contract. This is an opaque local change token, not a signature.
fn revision(
    context: &BoardAgentContext,
    placements: &[upeg_core::Placement],
) -> Result<String, BoardAgentError> {
    let mut hash = DefaultHasher::new();
    context.board.hash(&mut hash);
    context.title.hash(&mut hash);
    context.description.hash(&mut hash);
    context.instructions.hash(&mut hash);
    context.working_directory.hash(&mut hash);
    // Registry availability can change when deferred MCP imports finish; the
    // list-changed notification handles that. Only persisted configuration binds
    // the session, so an imported pin cannot cause a permanent reconnect loop.
    let mut pins = placements.iter().collect::<Vec<_>>();
    pins.sort_by(|left, right| left.tool_id.cmp(&right.tool_id));
    for pin in pins {
        pin.tool_id.hash(&mut hash);
        serde_json::to_string(&pin.args_preset)?.hash(&mut hash);
    }
    if let Some(path) = &context.project_manifest {
        path.hash(&mut hash);
        if path.is_file() {
            std::fs::read(path)?.hash(&mut hash);
        }
    }
    for directory in [crate::toolkits_dir(), crate::mcp_import_dir()]
        .into_iter()
        .flatten()
    {
        if !directory.exists() {
            continue;
        }
        let mut files = std::fs::read_dir(directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        files.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        });
        files.sort();
        for path in files {
            path.hash(&mut hash);
            std::fs::read(path)?.hash(&mut hash);
        }
    }
    Ok(format!("{:016x}", hash.finish()))
}

/// Standard Tool representation with Board defaults applied.
pub(crate) fn board_tool_json(
    meta: &upeg_core::ToolMeta,
    preset: Option<&upeg_core::ArgsPreset>,
) -> Value {
    let mut value = meta.to_json_object("name");
    value["inputSchema"] = effective_tool_schema(meta, preset);
    value
}

/// A protocol introspection operation, never an executable Board pin.
pub(crate) fn context_tool_json(description: &str) -> Value {
    json!({
        "name": BOARD_CONTEXT_TOOL,
        "description": format!("Read this board's guidance and execution context before choosing its tools. {description}"),
        "inputSchema": {"type":"object", "properties":{}, "additionalProperties":false},
    })
}
