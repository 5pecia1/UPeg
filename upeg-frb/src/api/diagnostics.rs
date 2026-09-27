//! Local failure-diagnostic API for Flutter.
//!
//! Reports outlive a result modal and are stored separately from the
//! metadata-only execution log. Every payload returned here was redacted and
//! bounded before persistence; normal export omits debug context.

use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};

use super::tools::CanonicalToolResult;

/// Compact row for a recent-failures panel.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct DiagnosticSummaryDto {
    pub id: String,
    pub run_id: String,
    pub occurred_at_ms: u64,
    pub tool_id: Option<String>,
    pub source: String,
    pub error_code: String,
    pub error_message: String,
    pub status: String,
}

/// Full report for an explicit detail view or export preview.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct DiagnosticReportDto {
    pub id: String,
    pub run_id: String,
    pub occurred_at_ms: u64,
    pub app_version: String,
    pub os: String,
    pub tool_id: Option<String>,
    pub source: String,
    pub cwd: Option<String>,
    pub project: Option<String>,
    pub error_code: String,
    pub error_message: String,
    pub status: String,
    pub stdout: String,
    pub stderr: String,
    /// Available only from `show` and debug export; already redacted.
    pub debug_context: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl From<upeg_sources::store::DiagnosticRecord> for DiagnosticReportDto {
    fn from(value: upeg_sources::store::DiagnosticRecord) -> Self {
        Self {
            id: value.id,
            run_id: value.run_id,
            occurred_at_ms: value.occurred_at_ms,
            app_version: value.app_version,
            os: value.os,
            tool_id: value.tool_id,
            source: value.source,
            cwd: value.cwd,
            project: value.project,
            error_code: value.error_code,
            error_message: value.error_message,
            status: value.status,
            stdout: value.stdout,
            stderr: value.stderr,
            debug_context: value.debug_context,
        }
    }
}

impl From<&DiagnosticReportDto> for DiagnosticSummaryDto {
    fn from(value: &DiagnosticReportDto) -> Self {
        Self {
            id: value.id.clone(),
            run_id: value.run_id.clone(),
            occurred_at_ms: value.occurred_at_ms,
            tool_id: value.tool_id.clone(),
            source: value.source.clone(),
            error_code: value.error_code.clone(),
            error_message: value.error_message.clone(),
            status: value.status.clone(),
        }
    }
}

/// List retained reports, newest first. A zero limit means the store default.
#[flutter_rust_bridge::frb(sync)]
pub fn list_diagnostics(limit: u32) -> Vec<DiagnosticSummaryDto> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Ok(store) = upeg_sources::store::Store::open() else {
            return Vec::new();
        };
        return store
            .read_diagnostics((limit != 0).then_some(limit as usize))
            .map(|records| {
                records
                    .into_iter()
                    .map(DiagnosticReportDto::from)
                    .map(|report| DiagnosticSummaryDto::from(&report))
                    .collect()
            })
            .unwrap_or_default();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = limit;
        Vec::new()
    }
}

/// Load a report that a recent-failures panel selected.
#[flutter_rust_bridge::frb(sync)]
pub fn show_diagnostic(id: String) -> Option<DiagnosticReportDto> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let store = upeg_sources::store::Store::open().ok()?;
        return store
            .diagnostic(&id)
            .ok()
            .flatten()
            .map(DiagnosticReportDto::from);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = id;
        None
    }
}

/// Serialize a safe support bundle preview. `debug` includes the stored
/// structured details; normal export keeps its useful summary and streams.
#[flutter_rust_bridge::frb]
pub fn export_diagnostic(id: String, debug: bool) -> Result<String, super::boot::FrbError> {
    let Some(mut report) = show_diagnostic(id.clone()) else {
        return Err(super::boot::FrbError::Validation {
            field: "id".to_string(),
            reason: format!("diagnostic `{id}` was not found"),
        });
    };
    if !debug {
        report.debug_context = None;
    }
    serde_json::to_string_pretty(&report_to_value(report)).map_err(|error| {
        super::boot::FrbError::Internal {
            message: format!("serialize diagnostic export: {error}"),
        }
    })
}

/// Persist an uncaught Flutter-side failure through the same bounded,
/// redacting diagnostic store as native dispatch failures. The UI deliberately
/// sends only its error and stack text; no widget state or form values cross
/// this boundary.
#[flutter_rust_bridge::frb(sync)]
pub fn record_flutter_error(message: String, stack: String) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let draft = upeg_core::diagnostics::DiagnosticDraft {
            run_id: generated_run_id(),
            occurred_at_ms: now_ms(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS.to_string(),
            tool_id: None,
            source: "flutter".to_string(),
            cwd: std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string()),
            project: upeg_sources::project::current_project_root()
                .map(|path| path.display().to_string()),
            error_code: "FLUTTER_INTERNAL_ERROR".to_string(),
            error_message: message,
            status: "internal_error".to_string(),
            stdout: String::new(),
            stderr: stack.clone(),
            debug_context: Some(serde_json::json!({ "stack": stack })),
        };
        if let Ok(mut store) = upeg_sources::store::Store::open() {
            let _ = store.append_diagnostic(draft);
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (message, stack);
    }
}

fn report_to_value(report: DiagnosticReportDto) -> Value {
    serde_json::json!({
        "id": report.id,
        "run_id": report.run_id,
        "occurred_at_ms": report.occurred_at_ms,
        "app_version": report.app_version,
        "os": report.os,
        "tool_id": report.tool_id,
        "source": report.source,
        "cwd": report.cwd,
        "project": report.project,
        "error_code": report.error_code,
        "error_message": report.error_message,
        "status": report.status,
        "stdout": report.stdout,
        "stderr": report.stderr,
        "debug_context": report.debug_context,
    })
}

/// Record a failed desktop dispatch. Called by both one-shot and streamed
/// APIs so a modal closing cannot discard the final report.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct DispatchDiagnosticIdentity {
    run_id: String,
    occurred_at_ms: u64,
    tool_id: String,
    source: String,
    provenance: String,
    cwd: Option<String>,
    project: Option<String>,
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct DispatchDiagnosticIdentity;

/// Snapshot process-global identity before dispatch. The active project may
/// change after the dispatcher releases its call guard, so recording must not
/// look these fields up once a result has returned.
pub(crate) fn capture_dispatch_identity(
    tool_id: &str,
    args_json: &str,
    run_id: Option<&str>,
) -> DispatchDiagnosticIdentity {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args = serde_json::from_str::<Value>(args_json).ok();
        let source = upeg_runtime::toolbox_tool(tool_id).map_or_else(
            || "registry".to_string(),
            |tool| tool.invoker.label().to_string(),
        );
        let project = context_string(args.as_ref(), "projectRoot")
            .or_else(|| context_string(args.as_ref(), "project_root"))
            .or_else(|| context_string(args.as_ref(), "project"))
            .or_else(|| {
                upeg_runtime::project_scope::active_project_root()
                    .map(|path| path.display().to_string())
            });
        DispatchDiagnosticIdentity {
            run_id: run_id.map(str::to_string).unwrap_or_else(generated_run_id),
            occurred_at_ms: now_ms(),
            tool_id: tool_id.to_string(),
            source,
            provenance: upeg_runtime::tool_provenance(tool_id).label(),
            cwd: context_string(args.as_ref(), upeg_core::EXECUTION_CONTEXT_CWD)
                .or_else(|| {
                    upeg_runtime::project_scope::active_project_root()
                        .map(|path| path.display().to_string())
                })
                .or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .map(|path| path.display().to_string())
                }),
            project,
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (tool_id, args_json, run_id);
        DispatchDiagnosticIdentity
    }
}

pub(crate) fn record_dispatch_diagnostic(
    identity: DispatchDiagnosticIdentity,
    result: &CanonicalToolResult,
) {
    #[cfg(not(target_arch = "wasm32"))]
    if !result.ok {
        record_native(identity, result);
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (identity, result);
}

#[cfg(not(target_arch = "wasm32"))]
fn record_native(identity: DispatchDiagnosticIdentity, result: &CanonicalToolResult) {
    let Some(error) = result.error.as_ref() else {
        return;
    };
    let details = error
        .details
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    let streams =
        upeg_core::ProcessErrorStreams::from_details(details.as_ref()).unwrap_or_default();
    let debug_context = details_with_identity(details, &identity);
    let draft = upeg_core::diagnostics::DiagnosticDraft {
        run_id: identity.run_id,
        occurred_at_ms: identity.occurred_at_ms,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        tool_id: Some(identity.tool_id),
        source: identity.source,
        cwd: identity.cwd,
        project: identity.project,
        error_code: error.code.clone(),
        error_message: error.message.clone(),
        status: "tool_error".to_string(),
        stdout: streams.stdout,
        stderr: streams.stderr,
        debug_context,
    };
    if let Ok(mut store) = upeg_sources::store::Store::open() {
        let _ = store.append_diagnostic(draft);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn context_string(args: Option<&Value>, key: &str) -> Option<String> {
    args?
        .get(upeg_core::EXECUTION_CONTEXT_ARG)
        .and_then(|context| context.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[cfg(not(target_arch = "wasm32"))]
fn details_with_identity(
    details: Option<Value>,
    identity: &DispatchDiagnosticIdentity,
) -> Option<Value> {
    let mut details = match details {
        Some(Value::Object(object)) => object,
        Some(other) => serde_json::Map::from_iter([("error_details".to_string(), other)]),
        None => serde_json::Map::new(),
    };
    details.insert(
        "diagnostic_context".to_string(),
        serde_json::json!({
            "provenance": identity.provenance,
            "cwd": identity.cwd,
            "project_root": identity.project,
        }),
    );
    Some(Value::Object(details))
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn generated_run_id() -> String {
    static NEXT_RUN: AtomicU64 = AtomicU64::new(0);
    format!(
        "desktop-{}-{}-{}",
        std::process::id(),
        now_ms(),
        NEXT_RUN.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn identity_uses_project_root_and_cwd_stamped_on_the_actual_call() {
        let identity = capture_dispatch_identity(
            "missing.diagnostic.tool",
            r#"{"_upeg":{"projectRoot":"/workspace/project","cwd":"/workspace/project/subdir"}}"#,
            Some("caller-run"),
        );
        assert_eq!(identity.run_id, "caller-run");
        assert_eq!(identity.project.as_deref(), Some("/workspace/project"));
        assert_eq!(identity.cwd.as_deref(), Some("/workspace/project/subdir"));
        assert_eq!(identity.source, "registry");
    }

    #[test]
    fn generated_dispatch_run_ids_are_unique() {
        assert_ne!(generated_run_id(), generated_run_id());
    }
}
