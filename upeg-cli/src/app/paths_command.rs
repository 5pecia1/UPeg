use std::path::Path;

use upeg_core::ProjectRoot;
use upeg_sources::project::{ProjectManifestOrigin, detect_project_manifest_lookup};

use crate::CliError;
use crate::surfaces::cli::paths::format_paths;

pub(super) fn run(argument: Option<&Path>, json: bool) -> Result<String, CliError> {
    let project = if let Some(root) = argument {
        let project = ProjectRoot::new(root).ok_or_else(|| {
            CliError::tool_failed(format!(
                "--project: {} has no .upeg directory",
                root.display()
            ))
        })?;
        Some((project, "argument"))
    } else {
        detect_project_manifest_lookup().and_then(|lookup| {
            let root = ProjectRoot::new(lookup.path.parent()?)?;
            let origin = match lookup.origin {
                ProjectManifestOrigin::Detected => "detected",
                ProjectManifestOrigin::EnvOverride => "env_override",
            };
            Some((root, origin))
        })
    };
    format_paths(project.as_ref().map(|(root, origin)| (root, *origin)), json)
}
