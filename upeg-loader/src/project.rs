//! The small project declaration kept apart from Toolkit manifests.

use std::collections::BTreeMap;

use serde::Deserialize;
use upeg_core::{ProjectToolChoice, ToolId};
use upeg_runtime::pegboard_project::ProjectBoardDecl;

use crate::model::BoardEntryToml;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectToml {
    schema_version: u32,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    boards: Vec<BoardEntryToml>,
    #[serde(default)]
    tool_choices: BTreeMap<String, ProjectToolChoice>,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectConfig {
    pub name: Option<String>,
    pub boards: Vec<ProjectBoardDecl>,
    pub tool_choices: BTreeMap<String, ProjectToolChoice>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectConfigError {
    #[error("invalid project.toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unsupported project.toml schema_version {0}; expected 1")]
    SchemaVersion(u32),
    #[error("project.toml name must not be blank")]
    BlankName,
    #[error("invalid tool_choices id `{0}`; expected a canonical toolkit.tool id")]
    ToolChoiceId(String),
    #[error("invalid project board: {0}")]
    Board(#[from] crate::LoadError),
}

pub fn parse_project_config(input: &str) -> Result<ProjectConfig, ProjectConfigError> {
    let parsed: ProjectToml = toml::from_str(input)?;
    if parsed.schema_version != 1 {
        return Err(ProjectConfigError::SchemaVersion(parsed.schema_version));
    }
    let name = parsed.name.map(|name| name.trim().to_string());
    if name.as_deref() == Some("") {
        return Err(ProjectConfigError::BlankName);
    }
    for id in parsed.tool_choices.keys() {
        ToolId::parse_canonical(id).map_err(|_| ProjectConfigError::ToolChoiceId(id.clone()))?;
    }
    Ok(ProjectConfig {
        name,
        boards: crate::parse::boards::lower_board_entries(&parsed.boards)?,
        tool_choices: parsed.tool_choices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_marker_needs_no_config_file() {
        let config = ProjectConfig::default();
        assert!(config.boards.is_empty());
        assert!(config.tool_choices.is_empty());
    }

    #[test]
    fn v1_config_parses_boards_and_explicit_choices() {
        let config = parse_project_config(
            "schema_version = 1\nname = 'Example'\n[[boards]]\nid = 'work'\n[tool_choices]\n'dev.check' = 'project'\n",
        )
        .expect("valid config");
        assert_eq!(config.name.as_deref(), Some("Example"));
        assert_eq!(config.boards[0].id.as_str(), "work");
        assert_eq!(config.tool_choices["dev.check"], ProjectToolChoice::Project);
    }

    #[test]
    fn project_config_rejects_unsupported_version() {
        assert!(matches!(
            parse_project_config("schema_version = 2"),
            Err(ProjectConfigError::SchemaVersion(2))
        ));
    }
}
