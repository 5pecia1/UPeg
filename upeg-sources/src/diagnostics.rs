//! Failure capture for runtime-source loading.
//!
//! Source discovery is not a tool dispatch, but a malformed local Toolkit or
//! project manifest should still leave a report users can export. This module
//! intentionally records only path/error metadata, never manifest contents.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::RuntimeSourceReport;
use crate::store::Store;

/// Persist each runtime-source failure best-effort. Callers keep their normal
/// loader report and stderr behavior even when diagnostics storage is absent.
pub fn record_runtime_source_failures(report: &RuntimeSourceReport) {
    let Ok(mut store) = Store::open() else {
        return;
    };
    for (path, error) in &report.toolkits_outcome.failed {
        append(
            &mut store,
            "toolkits",
            path.display().to_string(),
            error.to_string(),
        );
    }
    if let Some(project) = &report.project_manifest {
        for (path, error) in &project.outcome.failed {
            append(
                &mut store,
                "project_manifest",
                path.display().to_string(),
                error.to_string(),
            );
        }
    }
    for (path, error) in &report.wasm_failures {
        append(
            &mut store,
            "wasm",
            path.display().to_string(),
            error.clone(),
        );
    }
}

fn append(store: &mut Store, source: &str, path: String, message: String) {
    let now = now_ms();
    let draft = upeg_core::diagnostics::DiagnosticDraft {
        run_id: format!("source-loader-{}-{now}", std::process::id()),
        occurred_at_ms: now,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        tool_id: None,
        source: source.to_string(),
        cwd: std::env::current_dir()
            .ok()
            .map(|cwd| cwd.display().to_string()),
        project: (source == "project_manifest").then_some(path.clone()),
        error_code: "runtime_source_load_failed".to_string(),
        error_message: message.clone(),
        status: "source_error".to_string(),
        stdout: String::new(),
        stderr: message,
        debug_context: Some(serde_json::json!({ "path": path })),
    };
    let _ = store.append_diagnostic(draft);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
