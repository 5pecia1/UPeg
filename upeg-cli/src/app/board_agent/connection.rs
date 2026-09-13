//! Portable stdio launch configuration. The CLI itself binds its directory, so
//! clients do not need a vendor-specific `cwd` setting.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use upeg_core::BoardKey;

use super::{
    BoardAgentError, BoardConnectionPreview, BoardToolReadiness, BoardToolReadinessStatus,
    board_context,
};

const CLI_NAME: &str = "upeg";
const PROJECT_MANIFEST_ENV: &str = "UPEG_PROJECT_MANIFEST_PATH";
const PROJECT_MANIFEST_OFF: &str = "off";
#[cfg(unix)]
const EXECUTABLE_PERMISSION_BITS: u32 = 0o111;

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
            |path| Value::String(path.to_string_lossy().into_owned()),
        ),
    );
    if let Some(root) = upeg_core::paths::config_root() {
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
            if executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    find_executable(executable, &std::env::current_dir().ok()?)
}

pub(super) fn find_executable(command: &str, directory: &Path) -> Option<PathBuf> {
    find_executable_on_path(command, directory, std::env::var_os("PATH").as_deref())
}

pub(super) fn find_executable_on_path(
    command: &str,
    directory: &Path,
    search_path: Option<&OsStr>,
) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.is_absolute() || path.components().count() > 1 {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            directory.join(path)
        };
        return executable_file(&path).then_some(path);
    }
    search_path.and_then(|paths| {
        std::env::split_paths(paths)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    directory.join(path)
                }
            })
            .map(|path| path.join(command))
            .find_map(executable_candidate)
    })
}

fn executable_candidate(path: PathBuf) -> Option<PathBuf> {
    if executable_file(&path) {
        return Some(path);
    }
    #[cfg(windows)]
    if path.extension().is_none() {
        for extension in ["exe", "cmd", "bat", "com"] {
            let candidate = path.with_extension(extension);
            if executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & EXECUTABLE_PERMISSION_BITS != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
