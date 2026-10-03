//! Portable stdio launch configuration. The CLI itself binds its directory, so
//! clients do not need a vendor-specific `cwd` setting.

use std::path::PathBuf;

use serde_json::{Map, Value, json};
use upeg_core::BoardKey;

use super::{
    BoardAgentError, BoardConnectionPreview, BoardToolReadiness, BoardToolReadinessStatus,
    board_context,
};

const CLI_NAME: &str = "upeg";
const PROJECT_MANIFEST_ENV: &str = "UPEG_PROJECT_MANIFEST_PATH";
const PROJECT_MANIFEST_OFF: &str = "off";
pub fn board_connection_preview(
    board: &BoardKey,
) -> Result<BoardConnectionPreview, BoardAgentError> {
    let context = board_context(board)?;
    let binary = cli_binary();
    let mut readiness = BoardToolReadiness {
        status: if binary.is_some() {
            BoardToolReadinessStatus::Ready
        } else {
            BoardToolReadinessStatus::Unavailable
        },
        reasons: if binary.is_some() {
            Vec::new()
        } else {
            vec!["Install the upeg CLI on PATH before connecting an agent.".into()]
        },
    };
    if !context.unresolved_pins.is_empty() {
        if readiness.status == BoardToolReadinessStatus::Ready {
            readiness.status = BoardToolReadinessStatus::Unchecked;
        }
        readiness.reasons.push(format!(
            "Preview is incomplete: these pinned tools are not loaded in this process: {}. Check their sources; MCP imports may become available after connecting.",
            context.unresolved_pins.join(", "),
        ));
    }
    let mut env = Map::new();
    env.insert(
        PROJECT_MANIFEST_ENV.into(),
        context.project_manifest.as_ref().map_or_else(
            || PROJECT_MANIFEST_OFF.into(),
            |path| {
                let marker = if path.is_dir() {
                    path.as_path()
                } else {
                    path.parent().unwrap_or(path)
                };
                let root = marker.parent().unwrap_or(marker);
                Value::String(root.to_string_lossy().into_owned())
            },
        ),
    );
    if let Some(root) = upeg_core::paths::storage_root() {
        env.insert(
            "UPEG_HOME".into(),
            root.to_string_lossy().into_owned().into(),
        );
    }
    for name in ["UPEG_TOOLKITS_DIR", "UPEG_WASM_DIR", "UPEG_MCP_IMPORTS_DIR"] {
        if let Some(value) = std::env::var_os(name) {
            let path = PathBuf::from(value);
            let path = if path.is_absolute() {
                path
            } else {
                context.working_directory.join(path)
            };
            env.insert(name.into(), path.to_string_lossy().into_owned().into());
        }
    }
    let config = json!({"mcpServers": {
        format!("upeg-{}", board): {
            "command": binary.map_or_else(|| CLI_NAME.into(), |path| path.to_string_lossy().into_owned()),
            "args": ["--working-directory", context.working_directory, "mcp", "--board", board.as_str()],
            "env": env,
        }
    }});
    Ok(BoardConnectionPreview {
        context,
        config,
        readiness,
    })
}

fn cli_binary() -> Option<PathBuf> {
    let executable = if cfg!(windows) { "upeg.exe" } else { CLI_NAME };
    if let Ok(current) = std::env::current_exe() {
        if current.file_name().is_some_and(|name| name == executable) {
            return Some(current);
        }
        if let Some(parent) = current.parent() {
            let candidate = parent.join(executable);
            if upeg_runtime::readiness::find_executable(
                &candidate.to_string_lossy(),
                parent,
                None,
                None,
            )
            .is_some()
            {
                return Some(candidate);
            }
        }
    }
    let directory = std::env::current_dir().ok()?;
    upeg_runtime::readiness::find_executable(
        executable,
        &directory,
        None,
        std::env::var_os("PATH").as_deref(),
    )
}
