//! Declarative process requirements available to previews without dispatch.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// The manifest's command lookup policy, before execution-context overrides.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CommandSearchPath {
    #[default]
    Inherit,
    /// Plain `env` declaration. A caller's execution context may override it.
    Declared(OsString),
    /// A credential supplies PATH after all plain environment overrides.
    /// Previews must not resolve the credential or claim a PATH check passed.
    Credential,
}

/// External invocation metadata resolved exactly as the dispatcher resolves
/// the declaration. This records requirements and never runs the command.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolExecutionRequirements {
    pub command: Option<String>,
    pub declared_working_directory: Option<PathBuf>,
    pub project_root: Option<PathBuf>,
    pub search_path: CommandSearchPath,
}

fn requirements_lock() -> &'static Mutex<HashMap<String, ToolExecutionRequirements>> {
    static REQUIREMENTS: OnceLock<Mutex<HashMap<String, ToolExecutionRequirements>>> =
        OnceLock::new();
    REQUIREMENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Replace the registered requirements, or remove them for another invoker.
/// Loaders call this after registering the corresponding tool.
pub fn set_tool_execution_requirements(
    tool_id: &str,
    requirements: Option<ToolExecutionRequirements>,
) {
    let Ok(mut guard) = requirements_lock().lock() else {
        return;
    };
    match requirements {
        Some(requirements) => {
            guard.insert(tool_id.to_string(), requirements);
        }
        None => {
            guard.remove(tool_id);
        }
    }
}

/// The currently registered declaration, without executing or resolving secrets.
#[must_use]
pub fn tool_execution_requirements(tool_id: &str) -> Option<ToolExecutionRequirements> {
    requirements_lock()
        .lock()
        .ok()
        .and_then(|guard| guard.get(tool_id).cloned())
}
