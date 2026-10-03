use crate::CliError;
use crate::surfaces::cli::StorageAction;
use upeg_sources::storage;

pub(super) fn run(action: StorageAction) -> Result<String, CliError> {
    let result = (|| -> Result<serde_json::Value, storage::StorageError> {
        match action {
            StorageAction::Status { .. } => storage::status(),
            StorageAction::Plan {
                source,
                target,
                ecosystem_prefix,
                output,
                ..
            } => {
                let plan = storage::plan(&source, &target, ecosystem_prefix.as_deref())?;
                if let Some(path) = output {
                    storage::write_plan(&path, &plan)?;
                }
                Ok(serde_json::to_value(plan)?)
            }
            StorageAction::Apply {
                plan,
                yes,
                quiesced,
                ..
            } => storage::apply(&storage::read_plan(&plan)?, yes, quiesced),
            StorageAction::Verify { target, .. } => storage::verify(&target),
            StorageAction::Rollback {
                target,
                yes,
                quiesced,
                ..
            } => storage::rollback(&target, yes, quiesced),
        }
    })()
    .map_err(|error| CliError::tool_failed(error.to_string()))?;
    serde_json::to_string_pretty(&result)
        .map(|output| format!("{output}\n"))
        .map_err(|error| CliError::tool_failed(error.to_string()))
}
