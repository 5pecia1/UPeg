//! Explicit desktop project-context API.
//!
//! The UI never changes process cwd. It validates and switches a directory
//! through `upeg_sources::project`, which atomically updates the runtime
//! registry and project board scope after active runs have drained.

use super::boot::FrbError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum ProjectToolChoiceDto {
    Global,
    Project,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ProjectToolkitDto {
    pub path: String,
    pub id: String,
    pub tool_ids: Vec<String>,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ProjectConflictDto {
    pub tool_id: String,
    pub global_source: String,
    pub project_source: String,
    pub choice: Option<ProjectToolChoiceDto>,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ProjectDefinitionDto {
    pub root: String,
    pub name: String,
    pub board_count: u32,
    pub toolkits: Vec<ProjectToolkitDto>,
    pub conflicts: Vec<ProjectConflictDto>,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ProjectActivationDto {
    pub root: String,
    pub name: String,
    pub loaded_tool_ids: Vec<String>,
    pub failed: Vec<String>,
    pub conflicts: Vec<ProjectConflictDto>,
}

#[cfg(not(target_arch = "wasm32"))]
fn project_error(error: upeg_sources::project::ProjectError) -> FrbError {
    FrbError::Validation {
        field: "project".to_string(),
        reason: error.to_string(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<upeg_core::ProjectToolChoice> for ProjectToolChoiceDto {
    fn from(value: upeg_core::ProjectToolChoice) -> Self {
        match value {
            upeg_core::ProjectToolChoice::Global => Self::Global,
            upeg_core::ProjectToolChoice::Project => Self::Project,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<ProjectToolChoiceDto> for upeg_core::ProjectToolChoice {
    fn from(value: ProjectToolChoiceDto) -> Self {
        match value {
            ProjectToolChoiceDto::Global => Self::Global,
            ProjectToolChoiceDto::Project => Self::Project,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<upeg_sources::project::ProjectConflict> for ProjectConflictDto {
    fn from(value: upeg_sources::project::ProjectConflict) -> Self {
        Self {
            tool_id: value.tool_id,
            global_source: value.global_source,
            project_source: value.project_source.display().to_string(),
            choice: value.choice.map(ProjectToolChoiceDto::from),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<upeg_sources::project::ProjectDefinition> for ProjectDefinitionDto {
    fn from(value: upeg_sources::project::ProjectDefinition) -> Self {
        Self {
            root: value.root.display().to_string(),
            name: value.name,
            board_count: value.boards.len() as u32,
            toolkits: value
                .toolkits
                .into_iter()
                .map(|toolkit| ProjectToolkitDto {
                    path: toolkit.path.display().to_string(),
                    id: toolkit.id,
                    tool_ids: toolkit.tool_ids,
                })
                .collect(),
            conflicts: value
                .conflicts
                .into_iter()
                .map(ProjectConflictDto::from)
                .collect(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<upeg_sources::project::ProjectActivation> for ProjectActivationDto {
    fn from(value: upeg_sources::project::ProjectActivation) -> Self {
        Self {
            root: value.root.display().to_string(),
            name: value.name,
            loaded_tool_ids: value
                .loaded_tool_ids
                .into_iter()
                .map(str::to_string)
                .collect(),
            failed: value
                .failed
                .into_iter()
                .map(|(path, reason)| format!("{}: {reason}", path.display()))
                .collect(),
            conflicts: value
                .conflicts
                .into_iter()
                .map(ProjectConflictDto::from)
                .collect(),
        }
    }
}

/// Current active project, if the desktop has explicitly opened one.
#[flutter_rust_bridge::frb(sync)]
pub fn current_project_definition() -> Option<ProjectDefinitionDto> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        return upeg_sources::project::current_project_definition().map(ProjectDefinitionDto::from);
    }
    #[cfg(target_arch = "wasm32")]
    None
}

/// Parse and validate a selected project without changing the active project.
#[flutter_rust_bridge::frb(sync)]
pub fn validate_project_root(root: String) -> Result<ProjectDefinitionDto, FrbError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        return upeg_sources::project::validate_project_root(std::path::Path::new(&root))
            .map(ProjectDefinitionDto::from)
            .map_err(project_error);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = root;
        Err(FrbError::Validation {
            field: "project".to_string(),
            reason: "project folders are unavailable in the browser".to_string(),
        })
    }
}

/// Activate a validated project. This does not change the process cwd.
#[flutter_rust_bridge::frb(sync)]
pub fn activate_project(root: String) -> Result<ProjectActivationDto, FrbError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        return upeg_sources::project::activate_project(std::path::Path::new(&root))
            .map(ProjectActivationDto::from)
            .map_err(project_error);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = root;
        Err(FrbError::Validation {
            field: "project".to_string(),
            reason: "project folders are unavailable in the browser".to_string(),
        })
    }
}

/// Persist one explicit source choice for a duplicate project tool id.
#[flutter_rust_bridge::frb(sync)]
pub fn set_project_tool_choice(
    root: String,
    tool_id: String,
    choice: ProjectToolChoiceDto,
) -> Result<ProjectActivationDto, FrbError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        return upeg_sources::project::set_project_tool_choice(
            std::path::Path::new(&root),
            &tool_id,
            choice.into(),
        )
        .map(ProjectActivationDto::from)
        .map_err(project_error);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (root, tool_id, choice);
        Err(FrbError::Validation {
            field: "project".to_string(),
            reason: "project folders are unavailable in the browser".to_string(),
        })
    }
}

/// Return the desktop to the global registry and board scope.
#[flutter_rust_bridge::frb(sync)]
pub fn close_project() -> Result<(), FrbError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        return upeg_sources::project::close_project().map_err(project_error);
    }
    #[cfg(target_arch = "wasm32")]
    Ok(())
}
