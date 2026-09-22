//! Non-executing External tool readiness for native Flutter surfaces.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum ExternalReadinessStatusDto {
    Ready,
    MissingExecutable,
    MissingWorkingDirectory,
    UncheckedCredentialPath,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ExternalReadinessDto {
    pub status: ExternalReadinessStatusDto,
    pub platform: String,
    pub command: Option<String>,
    pub working_directory: Option<String>,
    pub executable: Option<String>,
    pub guide_url: Option<String>,
    pub instructions: Option<String>,
    pub install_commands: Vec<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl From<upeg_runtime::readiness::ToolReadiness> for ExternalReadinessDto {
    fn from(value: upeg_runtime::readiness::ToolReadiness) -> Self {
        use upeg_runtime::readiness::{ToolPlatform, ToolReadinessStatus};
        let (guide_url, instructions, install_commands) = value.setup.map_or_else(
            || (None, None, Vec::new()),
            |setup| {
                (
                    setup.guide_url,
                    setup.instructions,
                    setup
                        .install
                        .map_or_else(Vec::new, |install| install.commands),
                )
            },
        );
        Self {
            status: match value.status {
                ToolReadinessStatus::Ready => ExternalReadinessStatusDto::Ready,
                ToolReadinessStatus::MissingExecutable => {
                    ExternalReadinessStatusDto::MissingExecutable
                }
                ToolReadinessStatus::MissingWorkingDirectory => {
                    ExternalReadinessStatusDto::MissingWorkingDirectory
                }
                ToolReadinessStatus::UncheckedCredentialPath => {
                    ExternalReadinessStatusDto::UncheckedCredentialPath
                }
            },
            platform: match value.platform {
                ToolPlatform::Linux => "linux",
                ToolPlatform::Macos => "macos",
                ToolPlatform::Windows => "windows",
            }
            .to_string(),
            command: value.command,
            working_directory: value
                .working_directory
                .map(|path| path.display().to_string()),
            executable: value.executable.map(|path| path.display().to_string()),
            guide_url,
            instructions,
            install_commands,
        }
    }
}

/// Inspect an External tool without running it. Native only: browser builds
/// have no trustworthy host filesystem or PATH to inspect.
pub fn inspect_tool_readiness(
    tool_id: String,
    board_key: Option<String>,
) -> Result<Option<ExternalReadinessDto>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        upeg_cli::inspect_local_tool_readiness(&tool_id, board_key.as_deref())
            .map(|readiness| readiness.map(ExternalReadinessDto::from))
            .map_err(|error| error.message())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (tool_id, board_key);
        Err("external readiness requires a native host".to_string())
    }
}
