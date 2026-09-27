use std::path::{Path, PathBuf};

use upeg_sources::project::{ProjectDefinition, ProjectError};

use crate::error::CliError;
use crate::surfaces::cli::ProjectAction;

fn target_root(root: Option<PathBuf>) -> Result<PathBuf, CliError> {
    if let Some(root) = root {
        return Ok(root);
    }
    if let Some(root) = upeg_sources::project::current_project_root() {
        return Ok(root);
    }
    if let Some(marker) = upeg_sources::project::detect_project_manifest()
        && let Some(root) = marker.parent()
    {
        return Ok(root.to_path_buf());
    }
    std::env::current_dir().map_err(|error| CliError::tool_failed(error.to_string()))
}

fn format_definition(definition: &ProjectDefinition, json: bool) -> String {
    if json {
        let toolkits: Vec<_> = definition.toolkits.iter().map(|toolkit| {
            serde_json::json!({ "id": toolkit.id, "path": toolkit.path, "tools": toolkit.tool_ids })
        }).collect();
        let conflicts: Vec<_> = definition.conflicts.iter().map(|conflict| {
            serde_json::json!({
                "tool_id": conflict.tool_id,
                "global_source": conflict.global_source,
                "project_source": conflict.project_source,
                "choice": conflict.choice.map(|choice| match choice { upeg_core::ProjectToolChoice::Global => "global", upeg_core::ProjectToolChoice::Project => "project" }),
                "blocked": conflict.choice.is_none(),
            })
        }).collect();
        return format!(
            "{}\n",
            serde_json::json!({
                "root": definition.root, "name": definition.name,
                "boards": definition.boards.iter().map(|board| board.id.as_str()).collect::<Vec<_>>(),
                "toolkits": toolkits, "conflicts": conflicts,
            })
        );
    }
    let mut lines = vec![format!(
        "{}\t{}",
        definition.root.display(),
        definition.name
    )];
    for toolkit in &definition.toolkits {
        lines.push(format!(
            "toolkit\t{}\t{} tool(s)",
            toolkit.id,
            toolkit.tool_ids.len()
        ));
    }
    for conflict in &definition.conflicts {
        let choice = match conflict.choice {
            Some(upeg_core::ProjectToolChoice::Global) => "global",
            Some(upeg_core::ProjectToolChoice::Project) => "project",
            None => "blocked: choose global or project",
        };
        lines.push(format!("conflict\t{}\t{choice}", conflict.tool_id));
    }
    format!("{}\n", lines.join("\n"))
}

fn project_error(error: ProjectError) -> CliError {
    CliError::tool_failed(error.to_string())
}

fn init(root: &Path, name: Option<String>) -> Result<String, CliError> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        CliError::tool_failed(format!("project root {}: {error}", root.display()))
    })?;
    let marker = root.join(upeg_core::PROJECT_MARKER_DIR);
    std::fs::create_dir_all(&marker)
        .map_err(|error| CliError::tool_failed(format!("{}: {error}", marker.display())))?;
    let toolkits = marker.join(upeg_core::PROJECT_TOOLKITS_DIR);
    std::fs::create_dir_all(&toolkits)
        .map_err(|error| CliError::tool_failed(format!("{}: {error}", toolkits.display())))?;
    let config_path = marker.join(upeg_core::PROJECT_CONFIG_FILE);
    if config_path.exists() {
        upeg_sources::project::validate_project_root(&root).map_err(project_error)?;
        return Ok(format!("project already initialized: {}\n", root.display()));
    }
    let name = name.map(|name| name.trim().to_string());
    if name.as_deref() == Some("") {
        return Err(CliError::tool_failed("project name must not be blank"));
    }
    let mut config = toml::map::Map::new();
    config.insert("schema_version".into(), toml::Value::Integer(1));
    if let Some(name) = name {
        config.insert("name".into(), toml::Value::String(name));
    }
    let body = toml::to_string_pretty(&config)
        .map_err(|error| CliError::tool_failed(error.to_string()))?;
    std::fs::write(&config_path, body)
        .map_err(|error| CliError::tool_failed(format!("{}: {error}", config_path.display())))?;
    Ok(format!("initialized project: {}\n", root.display()))
}

pub(super) fn run_project_action(action: ProjectAction) -> Result<String, CliError> {
    match action {
        ProjectAction::Show { json, root } => {
            let root = target_root(root)?;
            let definition =
                upeg_sources::project::validate_project_root(&root).map_err(project_error)?;
            Ok(format_definition(&definition, json))
        }
        ProjectAction::Validate { json, root } => {
            let root = target_root(root)?;
            let definition =
                upeg_sources::project::validate_project_root(&root).map_err(project_error)?;
            let output = format_definition(&definition, json);
            if definition
                .conflicts
                .iter()
                .any(|conflict| conflict.choice.is_none())
            {
                Err(CliError::stdout_failure(output))
            } else {
                Ok(output)
            }
        }
        ProjectAction::Init { root, name } => {
            init(&root.unwrap_or_else(|| PathBuf::from(".")), name)
        }
        ProjectAction::Choose {
            tool_id,
            choice,
            root,
        } => {
            let root = target_root(root)?;
            let activation =
                upeg_sources::project::set_project_tool_choice(&root, &tool_id, choice.into())
                    .map_err(project_error)?;
            Ok(format!("{}\t{}\n", activation.root.display(), tool_id))
        }
    }
}
