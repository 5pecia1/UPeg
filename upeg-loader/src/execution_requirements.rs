//! The single lowering of External command and directory requirements.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use upeg_runtime::execution_requirements::{
    CommandSearchPath, ToolExecutionRequirements, ToolSetupInstall, ToolSetupMetadata,
};

use crate::ToolToml;
use crate::manifest_origin::ManifestOrigin;

const EXTERNAL_INVOKER: &str = "External";
const SEARCH_PATH_ENV: &str = "PATH";

pub(crate) fn external_execution_requirements(
    parsed: &ToolToml,
    origin: Option<&ManifestOrigin>,
) -> Option<ToolExecutionRequirements> {
    if parsed.invoker.as_deref().map(str::trim) != Some(EXTERNAL_INVOKER) {
        return None;
    }
    let command = parsed.command.as_deref()?.trim();
    if command.is_empty() {
        return None;
    }
    Some(ToolExecutionRequirements {
        command: Some(command.to_string()),
        declared_working_directory: parsed
            .cwd
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(|path| absolutize(Path::new(path), origin)),
        project_root: origin
            .and_then(ManifestOrigin::implicit_working_directory)
            .map(Path::to_path_buf),
        search_path: declared_search_path(parsed),
        setup: parsed.setup.as_ref().map(|setup| ToolSetupMetadata {
            guide_url: setup
                .guide_url
                .as_ref()
                .map(|value| value.trim().to_string()),
            instructions: setup
                .instructions
                .as_ref()
                .map(|value| value.trim().to_string()),
            install: setup
                .install
                .as_ref()
                .map_or_else(ToolSetupInstall::default, |install| ToolSetupInstall {
                    linux: install
                        .linux
                        .iter()
                        .map(|value| value.trim().to_string())
                        .collect(),
                    macos: install
                        .macos
                        .iter()
                        .map(|value| value.trim().to_string())
                        .collect(),
                    windows: install
                        .windows
                        .iter()
                        .map(|value| value.trim().to_string())
                        .collect(),
                }),
        }),
    })
}

fn declared_search_path(parsed: &ToolToml) -> CommandSearchPath {
    let credential_sets_path = parsed
        .credentials
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|credential| {
            let target = credential
                .target
                .as_deref()
                .map(str::trim)
                .filter(|target| !target.is_empty())
                .unwrap_or_else(|| credential.name.trim());
            is_search_path(target)
        });
    if credential_sets_path {
        return CommandSearchPath::Credential;
    }
    parsed
        .env
        .as_deref()
        .unwrap_or_default()
        .iter()
        .rev()
        .find(|pair| is_search_path(pair.name.trim()))
        .map_or(CommandSearchPath::Inherit, |pair| {
            CommandSearchPath::Declared(OsString::from(&pair.value))
        })
}

fn is_search_path(name: &str) -> bool {
    if cfg!(windows) {
        name.eq_ignore_ascii_case(SEARCH_PATH_ENV)
    } else {
        name == SEARCH_PATH_ENV
    }
}

/// In-memory manifests keep relative paths relative to the process directory.
fn absolutize(path: &Path, origin: Option<&ManifestOrigin>) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    origin.map_or_else(
        || path.to_path_buf(),
        |origin| origin.base_directory().join(path),
    )
}
