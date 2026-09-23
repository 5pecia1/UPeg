//! Declarative process requirements available to previews without dispatch.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::readiness::{ToolInstallInstructions, ToolPlatform, ToolSetup};

/// Per-platform display-only commands supplied by an External manifest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolSetupInstall {
    pub linux: Vec<String>,
    pub macos: Vec<String>,
    pub windows: Vec<String>,
}

/// Non-secret operator guidance supplied by an External manifest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolSetupMetadata {
    pub guide_url: Option<String>,
    pub instructions: Option<String>,
    pub install: ToolSetupInstall,
}

impl ToolSetupMetadata {
    #[must_use]
    pub fn selected(&self, platform: ToolPlatform) -> ToolSetup {
        let commands = match platform {
            ToolPlatform::Linux => &self.install.linux,
            ToolPlatform::Macos => &self.install.macos,
            ToolPlatform::Windows => &self.install.windows,
        };
        ToolSetup {
            guide_url: self.guide_url.clone(),
            instructions: self.instructions.clone(),
            install: (!commands.is_empty()).then(|| ToolInstallInstructions {
                platform,
                commands: commands.clone(),
            }),
        }
    }
}

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
    pub setup: Option<ToolSetupMetadata>,
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
