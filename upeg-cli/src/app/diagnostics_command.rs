//! CLI presentation for retained local diagnostics.

use crate::error::CliError;
use crate::surfaces::cli::DiagnosticAction;
use std::fmt::Write as _;

pub(super) fn run_diagnostic_action(action: DiagnosticAction) -> Result<String, CliError> {
    let store = upeg_sources::store::Store::open()
        .map_err(|error| CliError::tool_failed(format!("open diagnostics: {error}")))?;
    match action {
        DiagnosticAction::List { limit, json } => {
            let records = store
                .read_diagnostics(Some(limit))
                .map_err(|error| CliError::tool_failed(format!("read diagnostics: {error}")))?;
            if json {
                return serde_json::to_string_pretty(&records)
                    .map(|value| format!("{value}\n"))
                    .map_err(|error| {
                        CliError::tool_failed(format!("serialize diagnostics: {error}"))
                    });
            }
            let mut output = String::new();
            for record in records {
                let _ = writeln!(
                    output,
                    "{}\t{}\t{}\t{}",
                    record.id,
                    record.occurred_at_ms,
                    record.tool_id.as_deref().unwrap_or("-"),
                    record.error_message
                );
            }
            Ok(output)
        }
        DiagnosticAction::Show { id, json } => {
            let record = store
                .diagnostic(&id)
                .map_err(|error| CliError::tool_failed(format!("read diagnostic: {error}")))?
                .ok_or_else(|| CliError::tool_failed(format!("diagnostic `{id}` was not found")))?;
            if json {
                return serde_json::to_string_pretty(&record)
                    .map(|value| format!("{value}\n"))
                    .map_err(|error| {
                        CliError::tool_failed(format!("serialize diagnostic: {error}"))
                    });
            }
            Ok(format_diagnostic(&record, true))
        }
        DiagnosticAction::Export { id, debug } => {
            let mut record = store
                .diagnostic(&id)
                .map_err(|error| CliError::tool_failed(format!("read diagnostic: {error}")))?
                .ok_or_else(|| CliError::tool_failed(format!("diagnostic `{id}` was not found")))?;
            if !debug {
                record.debug_context = None;
            }
            serde_json::to_string_pretty(&record)
                .map(|value| format!("{value}\n"))
                .map_err(|error| {
                    CliError::tool_failed(format!("serialize diagnostic export: {error}"))
                })
        }
    }
}

fn format_diagnostic(
    record: &upeg_sources::store::DiagnosticRecord,
    include_debug: bool,
) -> String {
    let mut text = format!(
        "id: {}\nrun: {}\ntime: {}\ntool: {}\nsource: {}\nstatus: {}\nerror: {}: {}\n",
        record.id,
        record.run_id,
        record.occurred_at_ms,
        record.tool_id.as_deref().unwrap_or("-"),
        record.source,
        record.status,
        record.error_code,
        record.error_message,
    );
    if !record.stderr.is_empty() {
        text.push_str(&format!("stderr:\n{}\n", record.stderr));
    }
    if !record.stdout.is_empty() {
        text.push_str(&format!("stdout:\n{}\n", record.stdout));
    }
    if include_debug && let Some(context) = &record.debug_context {
        text.push_str(&format!("debug_context:\n{context}\n"));
    }
    text
}
