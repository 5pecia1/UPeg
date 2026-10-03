use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value, json};
use upeg_core::{ProjectBoardNamespace, ProjectRoot, paths as core_paths};

use crate::CliError;
use crate::infrastructure::paths;

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(crate) fn format_paths(
    project: Option<(&ProjectRoot, &str)>,
    json_output: bool,
) -> Result<String, CliError> {
    let location = match core_paths::resolve_storage() {
        Ok(location) => Some(location),
        Err(core_paths::StoragePathError::Unavailable) => None,
        Err(error) => return Err(CliError::tool_failed(error.to_string())),
    };
    let user = location.as_ref().map(|location| &location.paths);
    let report = json!({
        "schema_version": 1,
        "layout": location.as_ref().map(|location| location.layout.as_str()),
        "user": {
            "config_dir": user.as_ref().map(|paths| path_string(&paths.config_dir)),
            "data_dir": user.as_ref().map(|paths| path_string(&paths.data_dir)),
            "state_dir": user.as_ref().map(|paths| path_string(&paths.state_dir)),
            "cache_dir": user.as_ref().map(|paths| path_string(&paths.cache_dir)),
            "runtime_dir": user.as_ref().map(|paths| path_string(&paths.runtime_dir)),
            "toolkits_dir": core_paths::toolkits_dir().as_deref().map(path_string),
            "wasm_dir": core_paths::wasm_dir().as_deref().map(path_string),
            "mcp_imports_dir": core_paths::mcp_import_dir().as_deref().map(path_string),
            "toolkit_packs_dir": core_paths::toolkit_packs_dir().as_deref().map(path_string),
            "store": core_paths::store_path().as_deref().map(path_string),
            "tweaks": core_paths::tweaks_path().as_deref().map(path_string),
            "desktop_lock": core_paths::desktop_lock_path().as_deref().map(path_string),
            "credentials": paths::credentials_path().as_deref().map(path_string),
            "execution_log": paths::execution_log_path().as_deref().map(path_string),
            "server_discovery": paths::server_json_path().as_deref().map(path_string),
            "http_log": paths::http_log_path().as_deref().map(path_string),
        },
        "project": project.map(|(root, origin)| json!({
            "root": path_string(root.as_path()),
            "marker": path_string(&root.marker_dir()),
            "config": path_string(&root.config_path()),
            "toolkits": path_string(&root.toolkits_dir()),
            "namespace": ProjectBoardNamespace::for_project_root(root.as_path()).as_str(),
            "origin": origin,
        })),
    });
    if json_output {
        let mut output = serde_json::to_string_pretty(&report)
            .map_err(|error| CliError::tool_failed(error.to_string()))?;
        output.push('\n');
        return Ok(output);
    }
    let mut output = String::new();
    format_fields(&mut output, "", &report);
    Ok(output)
}

fn format_fields(output: &mut String, prefix: &str, value: &Value) {
    if let Some(fields) = value.as_object() {
        for (name, value) in fields {
            let name = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            format_fields(output, &name, value);
        }
    } else if let Some(path) = value.as_str() {
        let _ = writeln!(output, "{prefix}\t{path}");
    } else {
        let _ = writeln!(output, "{prefix}\t{value}");
    }
}
