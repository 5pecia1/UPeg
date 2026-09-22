//! Non-executing readiness inspection for declarative External tools.

use std::ffi::OsString;
use std::path::PathBuf;

use serde::Serialize;

#[cfg(not(target_arch = "wasm32"))]
use crate::execution_requirements::CommandSearchPath;
use crate::execution_requirements::{ToolExecutionRequirements, tool_execution_requirements};

/// Native operating system selected for readiness guidance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPlatform {
    Linux,
    Macos,
    Windows,
}

/// Caller-provided execution context used for a readiness inspection.
#[derive(Clone, Debug, Default)]
pub struct ToolReadinessContext {
    pub working_directory: PathBuf,
    /// A board or transport PATH override. Its value is never exposed in a result.
    pub search_path: Option<OsString>,
}

/// A non-secret setup instruction selected for the current operating system.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ToolSetup {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guide_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install: Option<ToolInstallInstructions>,
}

/// Display-only install commands for one operating system.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ToolInstallInstructions {
    pub platform: ToolPlatform,
    pub commands: Vec<String>,
}

/// The result of checking a tool without executing it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ToolReadiness {
    pub status: ToolReadinessStatus,
    pub platform: ToolPlatform,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<ToolSetup>,
}

/// A readiness state that does not predict dispatch success.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolReadinessStatus {
    Ready,
    MissingExecutable,
    MissingWorkingDirectory,
    UncheckedCredentialPath,
}

/// Inspect a registered External tool without spawning a process or resolving a secret.
#[must_use]
pub fn inspect_tool_readiness(
    tool_id: &str,
    context: &ToolReadinessContext,
) -> Option<ToolReadiness> {
    let requirements = tool_execution_requirements(tool_id)?;
    inspect_requirements(&requirements, context)
}

#[cfg(not(target_arch = "wasm32"))]
fn inspect_requirements(
    requirements: &ToolExecutionRequirements,
    context: &ToolReadinessContext,
) -> Option<ToolReadiness> {
    let platform = current_tool_platform();
    let directory = effective_working_directory(requirements, &context.working_directory);
    let setup = requirements
        .setup
        .as_ref()
        .map(|setup| setup.selected(platform));
    if !directory.is_dir() {
        return Some(ToolReadiness {
            status: ToolReadinessStatus::MissingWorkingDirectory,
            platform,
            command: requirements.command.clone(),
            working_directory: Some(directory),
            executable: None,
            setup,
        });
    }
    let command = requirements.command.clone();
    let inherited_path = std::env::var_os("PATH");
    if requirements.search_path == CommandSearchPath::Credential {
        return Some(ToolReadiness {
            status: ToolReadinessStatus::UncheckedCredentialPath,
            platform,
            command,
            working_directory: Some(directory),
            executable: None,
            setup,
        });
    }
    let executable = command.as_deref().and_then(|command| {
        // `Command::env("PATH", ...)` searches that child value first, then
        // follows the platform's normal parent-path fallback. An inherited
        // PATH is not a child override, so on Windows the application and
        // system directories retain their std::process precedence.
        let child_path = context
            .search_path
            .as_deref()
            .or(match &requirements.search_path {
                CommandSearchPath::Declared(path) => Some(path.as_os_str()),
                CommandSearchPath::Inherit | CommandSearchPath::Credential => None,
            });
        find_executable(command, &directory, child_path, inherited_path.as_deref())
    });
    Some(ToolReadiness {
        status: if executable.is_some() {
            ToolReadinessStatus::Ready
        } else {
            ToolReadinessStatus::MissingExecutable
        },
        platform,
        command,
        working_directory: Some(directory),
        executable,
        setup,
    })
}

#[cfg(target_arch = "wasm32")]
fn inspect_requirements(
    _requirements: &ToolExecutionRequirements,
    _context: &ToolReadinessContext,
) -> Option<ToolReadiness> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn effective_working_directory(
    requirements: &ToolExecutionRequirements,
    caller: &std::path::Path,
) -> PathBuf {
    requirements
        .declared_working_directory
        .clone()
        .unwrap_or_else(|| {
            requirements
                .project_root
                .as_deref()
                .filter(|root| !within_project(caller, root))
                .map_or_else(|| caller.to_path_buf(), PathBuf::from)
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn within_project(directory: &std::path::Path, root: &std::path::Path) -> bool {
    match (directory.canonicalize(), root.canonicalize()) {
        (Ok(directory), Ok(root)) => directory.starts_with(root),
        _ => false,
    }
}

/// Find an executable using External's effective working directory and PATH semantics.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn find_executable(
    command: &str,
    directory: &std::path::Path,
    child_path: Option<&std::ffi::OsStr>,
    parent_path: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let _ = directory;
        return find_windows_executable(command, child_path, parent_path);
    }

    #[cfg(not(windows))]
    {
        let path = std::path::Path::new(command);
        if path.is_absolute() || path.components().count() > 1 {
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                directory.join(path)
            };
            return executable_candidate(path);
        }
        child_path.or(parent_path).and_then(|paths| {
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
}

/// Windows' `std::process::Command` resolver only supplies an implicit
/// `.exe` suffix. It searches a child PATH override, then the running
/// executable's directory, common system directories, and finally the parent
/// PATH; it does not apply `PATHEXT`.
#[cfg(windows)]
fn find_windows_executable(
    command: &str,
    child_path: Option<&std::ffi::OsStr>,
    parent_path: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let application_directory = std::env::current_exe().ok().map(|mut path| {
        path.pop();
        path
    });
    find_windows_executable_in(
        command,
        child_path,
        parent_path,
        application_directory,
        &windows_system_directories(),
    )
}

#[cfg(any(windows, test))]
fn find_windows_executable_in(
    command: &str,
    child_path: Option<&std::ffi::OsStr>,
    parent_path: Option<&std::ffi::OsStr>,
    application_directory: Option<PathBuf>,
    system_directories: &[PathBuf],
) -> Option<PathBuf> {
    let command_path = std::path::Path::new(command);
    if !windows_bare_file_name(command_path) {
        return windows_explicit_candidate(command_path.to_path_buf());
    }

    let mut directories = Vec::new();
    extend_windows_paths(&mut directories, child_path);
    if let Some(application_directory) = application_directory {
        directories.push(application_directory);
    }
    directories.extend(system_directories.iter().cloned());
    extend_windows_paths(&mut directories, parent_path);

    directories.into_iter().find_map(|directory| {
        let candidate = directory.join(command_path);
        windows_bare_candidate(candidate)
    })
}

#[cfg(any(windows, test))]
fn windows_bare_file_name(path: &std::path::Path) -> bool {
    path.file_name() == Some(path.as_os_str())
}

#[cfg(any(windows, test))]
fn extend_windows_paths(directories: &mut Vec<PathBuf>, paths: Option<&std::ffi::OsStr>) {
    let parent_directory = std::env::current_dir().ok();
    for path in paths.into_iter().flat_map(std::env::split_paths) {
        if path.as_os_str().is_empty() {
            continue;
        }
        directories.push(if path.is_absolute() {
            path
        } else if let Some(parent_directory) = parent_directory.as_ref() {
            parent_directory.join(path)
        } else {
            path
        });
    }
}

#[cfg(windows)]
fn windows_system_directories() -> Vec<PathBuf> {
    // `GetSystemDirectoryW`/`GetWindowsDirectoryW` would be exact, but require
    // unsafe FFI. These standard environment roots cover normal Windows hosts
    // without claiming a missing result is authoritative on an unusual host.
    let mut directories = Vec::new();
    for root in [std::env::var_os("SystemRoot"), std::env::var_os("WINDIR")]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
    {
        directories.push(root.join("System32"));
        directories.push(root);
    }
    directories
}

#[cfg(any(windows, test))]
fn windows_bare_candidate(path: PathBuf) -> Option<PathBuf> {
    if !path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().contains('.'))
    {
        let executable = path.with_extension("exe");
        return executable_file(&executable).then_some(executable);
    }
    executable_file(&path).then_some(path)
}

#[cfg(any(windows, test))]
fn windows_explicit_candidate(path: PathBuf) -> Option<PathBuf> {
    if has_windows_exe_suffix(&path) {
        return executable_file(&path).then_some(path);
    }
    let mut suffixed = path.as_os_str().to_os_string();
    suffixed.push(".exe");
    let suffixed = PathBuf::from(suffixed);
    executable_file(&suffixed)
        .then_some(suffixed)
        .or_else(|| executable_file(&path).then_some(path))
}

#[cfg(any(windows, test))]
fn has_windows_exe_suffix(path: &std::path::Path) -> bool {
    let path = path.as_os_str().to_string_lossy();
    path.get(path.len().saturating_sub(4)..)
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".exe"))
}

#[cfg(all(not(target_arch = "wasm32"), not(windows)))]
fn executable_candidate(path: PathBuf) -> Option<PathBuf> {
    if executable_file(&path) {
        return Some(path);
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn executable_file(path: &std::path::Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// The operating system for which setup guidance is selected.
#[must_use]
pub const fn current_tool_platform() -> ToolPlatform {
    #[cfg(target_os = "windows")]
    {
        ToolPlatform::Windows
    }
    #[cfg(target_os = "macos")]
    {
        ToolPlatform::Macos
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        ToolPlatform::Linux
    }
}

#[cfg(test)]
mod tests;
