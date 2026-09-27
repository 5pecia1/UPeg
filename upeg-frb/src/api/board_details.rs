//! Human-facing Board guidance and the exact native MCP connection preview.

use super::boot::FrbError;

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct BoardDetailsDto {
    pub board_key: String,
    pub title: String,
    pub description: String,
    pub instructions: String,
    pub project_manifest_path: Option<String>,
    pub execution_directory: Option<String>,
    pub native_connection_supported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum BoardToolReadinessDto {
    Ready,
    Unavailable,
    Unchecked,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct BoardConnectionToolDto {
    pub tool_id: String,
    pub description: String,
    pub defaults_json: String,
    pub input_schema_json: String,
    pub execution_directory: Option<String>,
    pub readiness: BoardToolReadinessDto,
    pub readiness_reasons: Vec<String>,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct BoardConnectionPreviewDto {
    pub board_key: String,
    pub execution_directory: String,
    pub project_manifest_path: Option<String>,
    pub tools: Vec<BoardConnectionToolDto>,
    pub config_json: String,
    pub readiness: BoardToolReadinessDto,
    pub readiness_reasons: Vec<String>,
}

#[cfg(not(target_arch = "wasm32"))]
fn readiness_dto(value: upeg_cli::board_agent::BoardToolReadinessStatus) -> BoardToolReadinessDto {
    use upeg_cli::board_agent::BoardToolReadinessStatus;
    match value {
        BoardToolReadinessStatus::Ready => BoardToolReadinessDto::Ready,
        BoardToolReadinessStatus::Unavailable => BoardToolReadinessDto::Unavailable,
        BoardToolReadinessStatus::Unchecked => BoardToolReadinessDto::Unchecked,
    }
}

/// Read guidance without implying the browser has access to native project files.
pub fn load_board_details(board_key: String) -> Result<BoardDetailsDto, FrbError> {
    let _catalog_read = upeg_runtime::project_scope::catalog_read_guard();
    use upeg_pegboard_ui::features::boards::{default_boards, load_boards};

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(scope) = upeg_runtime::pegboard_project::project_board_scope()
        && scope.source_changed()
    {
        return Err(FrbError::ProjectManifestChanged {
            path: scope.manifest_path().display().to_string(),
        });
    }

    let boards = load_boards().unwrap_or_else(default_boards);
    let board = boards
        .iter()
        .find(|board| board.key == board_key)
        .ok_or_else(|| FrbError::Validation {
            field: "board_key".to_string(),
            reason: format!("unknown board `{board_key}`"),
        })?;
    #[cfg(not(target_arch = "wasm32"))]
    let (project_manifest_path, execution_directory) = {
        let visibility = upeg_sources::pegboard::BoardVisibility::from_process();
        let manifest = visibility
            .project()
            .filter(|scope| scope.declaration(&board_key).is_some())
            .map(|scope| scope.manifest_path().display().to_string());
        let directory = std::env::current_dir().map_err(|err| FrbError::Io {
            message: err.to_string(),
        })?;
        (manifest, Some(directory.display().to_string()))
    };
    #[cfg(target_arch = "wasm32")]
    let (project_manifest_path, execution_directory) = (None, None);

    Ok(BoardDetailsDto {
        board_key,
        title: board.title.to_string(),
        description: board.guidance.description.clone(),
        instructions: board.guidance.instructions.clone(),
        project_manifest_path,
        execution_directory,
        native_connection_supported: !cfg!(target_arch = "wasm32"),
    })
}

/// Save personal guidance through the shared store; project manifests own theirs.
pub fn save_board_guidance(
    board_key: String,
    description: String,
    instructions: String,
) -> Result<(), FrbError> {
    let guidance = upeg_core::BoardGuidance {
        description,
        instructions,
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        use upeg_sources::pegboard::BoardGuidanceEditError;
        upeg_sources::pegboard::set_board_guidance(&board_key, guidance).map_err(|err| match err {
            BoardGuidanceEditError::Persistence(error) => FrbError::Io {
                message: error.to_string(),
            },
            error => FrbError::Validation {
                field: "board_guidance".to_string(),
                reason: error.to_string(),
            },
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        use upeg_pegboard_ui::features::boards::{default_boards, load_boards, serialize_boards};
        use upeg_pegboard_ui::platform::storage;
        let mut boards = load_boards().unwrap_or_else(default_boards);
        let board = boards
            .iter_mut()
            .find(|board| board.key == board_key)
            .ok_or_else(|| FrbError::Validation {
                field: "board_key".to_string(),
                reason: format!("unknown board `{board_key}`"),
            })?;
        board.guidance = guidance;
        storage::set_item(storage::BOARDS_KEY, &serialize_boards(&boards)).map_err(|()| {
            FrbError::Io {
                message: "save board guidance".to_string(),
            }
        })
    }
}

/// Preview the same scoped tools and defaults the agent receives on connection.
/// The directory is fixed to the current process so source discovery agrees.
pub fn preview_board_connection(board_key: String) -> Result<BoardConnectionPreviewDto, FrbError> {
    let _catalog_read = upeg_runtime::project_scope::catalog_read_guard();
    #[cfg(not(target_arch = "wasm32"))]
    {
        use upeg_cli::board_agent::board_connection_preview;
        let key = upeg_core::BoardKey::parse(&board_key).map_err(|err| FrbError::Validation {
            field: "board_key".to_string(),
            reason: err.to_string(),
        })?;
        let preview = board_connection_preview(&key).map_err(|err| FrbError::Validation {
            field: "board_connection".to_string(),
            reason: err.to_string(),
        })?;
        let config_json =
            serde_json::to_string_pretty(&preview.config).map_err(|err| FrbError::Internal {
                message: err.to_string(),
            })?;
        Ok(BoardConnectionPreviewDto {
            board_key,
            execution_directory: preview.context.working_directory.display().to_string(),
            project_manifest_path: preview
                .context
                .project_manifest
                .map(|path| path.display().to_string()),
            tools: preview
                .context
                .tools
                .into_iter()
                .map(|tool| BoardConnectionToolDto {
                    tool_id: tool.id,
                    description: tool.description,
                    defaults_json: tool.defaults.to_string(),
                    input_schema_json: tool.input_schema.to_string(),
                    execution_directory: tool
                        .working_directory
                        .map(|path| path.display().to_string()),
                    readiness: readiness_dto(tool.readiness.status),
                    readiness_reasons: tool.readiness.reasons,
                })
                .collect(),
            config_json,
            readiness: readiness_dto(preview.readiness.status),
            readiness_reasons: preview.readiness.reasons,
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = board_key;
        Err(FrbError::HostUnavailable)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::api::pegboard::project_scope_test_support::ScopedProjectBoard;
    use crate::api::test_support::run_with_storage_backup;

    #[test]
    fn changed_project_source_does_not_load_previous_guidance() {
        run_with_storage_backup(|| {
            use upeg_runtime::pegboard_project::{
                ProjectBoardDecl, ProjectBoardScope, set_project_board_scope,
            };
            let _scope = ScopedProjectBoard::declare("stale-guide", "Stale guide");
            let path =
                std::env::temp_dir().join(format!("upeg-frb-guide-{}.toml", std::process::id()));
            let original = "# original guide";
            std::fs::write(&path, original).expect("original source");
            set_project_board_scope(
                ProjectBoardScope::for_manifest(
                    &path,
                    vec![ProjectBoardDecl::new(
                        upeg_core::BoardKey::parse("stale-guide").expect("key"),
                        "Stale guide".into(),
                    )],
                )
                .with_loaded_content(original.into()),
            );
            std::fs::write(&path, "# changed guide").expect("edit source");

            let details = load_board_details("stale-guide".into());

            std::fs::remove_file(&path).expect("remove source fixture");
            assert!(
                matches!(details, Err(FrbError::ProjectManifestChanged { .. })),
                "must not show the previous guidance of a changed source"
            );
        });
    }

    #[test]
    fn personal_board_guidance_loads_saved_markdown_verbatim() {
        run_with_storage_backup(|| {
            let key = crate::api::pegboard::create_board("Guidance round trip".to_string())
                .expect("board");
            let markdown = "# 시작\n\n1. 상태를 먼저 확인한다.\n";
            save_board_guidance(key.clone(), "개인 작업".to_string(), markdown.to_string())
                .expect("save");
            let details = load_board_details(key).expect("details");
            assert_eq!(details.description, "개인 작업");
            assert_eq!(details.instructions, markdown);
            assert!(details.project_manifest_path.is_none());
        });
    }

    #[test]
    fn project_board_shows_source_path_and_rejects_personal_save() {
        run_with_storage_backup(|| {
            let _scope = ScopedProjectBoard::declare("guide-project", "Project guidance");
            let details = load_board_details("guide-project".to_string()).expect("details");
            assert!(
                details
                    .project_manifest_path
                    .is_some_and(|path| path.ends_with("upeg.toml"))
            );
            assert!(
                save_board_guidance(
                    "guide-project".to_string(),
                    "override".to_string(),
                    String::new()
                )
                .is_err()
            );
            assert_eq!(
                load_board_details("guide-project".to_string())
                    .expect("unchanged")
                    .description,
                ""
            );
        });
    }
}
