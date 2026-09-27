//! Dispatch-to-diagnostic mapping for the CLI family of surfaces.
//!
//! The execution log remains metadata-only. This adapter writes a separate
//! report only when a call failed, reusing the canonical error details the
//! invoker already captured.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use upeg_core::{ProcessErrorStreams, diagnostics::DiagnosticDraft};
use upeg_runtime::{tool_provenance, toolbox_tool};
use upeg_sources::store::Store;

use crate::domain::execution::dispatch::Outcome;

/// Identity captured before a dispatcher can finish and another project can
/// replace the process-global Toolbox. It is never populated from tool args.
pub struct DispatchDiagnosticContext {
    run_id: String,
    occurred_at_ms: u64,
    tool_id: String,
    source: String,
    provenance: String,
    cwd: Option<String>,
    project: Option<String>,
}

/// Snapshot the invocation identity before dispatch starts.
pub fn capture_dispatch(id: &str, args: &Value) -> DispatchDiagnosticContext {
    let cwd = context_string(args, upeg_core::EXECUTION_CONTEXT_CWD).or_else(|| {
        std::env::current_dir()
            .ok()
            .map(|path| path.display().to_string())
    });
    let project = project_from_args(args).or_else(|| {
        upeg_runtime::project_scope::active_project_root().map(|path| path.display().to_string())
    });
    let source = toolbox_tool(id).map_or_else(
        || "registry".to_string(),
        |tool| tool.invoker.label().to_string(),
    );
    DispatchDiagnosticContext {
        run_id: run_id(),
        occurred_at_ms: now_ms(),
        tool_id: id.to_string(),
        source,
        provenance: tool_provenance(id).label(),
        cwd,
        project,
    }
}

/// Best-effort failure capture. A diagnostic write must never replace the
/// tool's original result or turn a useful failure into a store failure.
pub fn record_dispatch(context: DispatchDiagnosticContext, outcome: &Outcome) {
    let Outcome::Failure(failure) = outcome else {
        if matches!(outcome, Outcome::NotFound) {
            record_not_found(context);
        }
        return;
    };
    let streams =
        ProcessErrorStreams::from_details(failure.error.details.as_ref()).unwrap_or_default();
    let debug_context = details_with_identity(failure.error.details.clone(), &context);
    let draft = DiagnosticDraft {
        run_id: context.run_id,
        occurred_at_ms: context.occurred_at_ms,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        tool_id: Some(context.tool_id),
        source: context.source,
        cwd: context.cwd,
        project: context.project,
        error_code: failure.error.code.clone(),
        error_message: failure.error.message.clone(),
        status: "tool_error".to_string(),
        stdout: streams.stdout,
        stderr: streams.stderr,
        debug_context,
    };
    append(draft);
}

fn record_not_found(context: DispatchDiagnosticContext) {
    let debug_context = details_with_identity(None, &context);
    append(DiagnosticDraft {
        run_id: context.run_id,
        occurred_at_ms: context.occurred_at_ms,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        tool_id: Some(context.tool_id.clone()),
        source: context.source,
        cwd: context.cwd,
        project: context.project,
        error_code: "tool_not_found".to_string(),
        error_message: format!("tool `{}` is not registered", context.tool_id),
        status: "not_found".to_string(),
        stdout: String::new(),
        stderr: String::new(),
        debug_context,
    });
}

fn append(draft: DiagnosticDraft) {
    let Ok(mut store) = Store::open() else {
        return;
    };
    let _ = store.append_diagnostic(draft);
}

fn project_from_args(args: &Value) -> Option<String> {
    ["projectRoot", "project_root", "project"]
        .iter()
        .find_map(|key| context_string(args, key))
}

fn context_string(args: &Value, key: &str) -> Option<String> {
    args.get(upeg_core::EXECUTION_CONTEXT_ARG)
        .and_then(|context| context.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn details_with_identity(
    details: Option<Value>,
    context: &DispatchDiagnosticContext,
) -> Option<Value> {
    let mut details = match details {
        Some(Value::Object(object)) => object,
        Some(other) => serde_json::Map::from_iter([("error_details".to_string(), other)]),
        None => serde_json::Map::new(),
    };
    details.insert(
        "diagnostic_context".to_string(),
        serde_json::json!({
            "provenance": context.provenance,
            "cwd": context.cwd,
            "project_root": context.project,
        }),
    );
    Some(Value::Object(details))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

fn run_id() -> String {
    static NEXT_RUN: AtomicU64 = AtomicU64::new(0);
    format!(
        "cli-{}-{}-{}",
        std::process::id(),
        now_ms(),
        NEXT_RUN.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_root_is_read_from_the_real_execution_context_key() {
        let args = serde_json::json!({"_upeg": {"projectRoot": "/work/demo"}});
        assert_eq!(project_from_args(&args).as_deref(), Some("/work/demo"));
        let legacy = serde_json::json!({"_upeg": {"project": "legacy"}});
        assert_eq!(project_from_args(&legacy).as_deref(), Some("legacy"));
        assert_eq!(
            project_from_args(&serde_json::json!({"project": "nope"})),
            None
        );
    }

    #[test]
    fn run_ids_are_unique_within_one_millisecond() {
        let first = run_id();
        let second = run_id();
        assert_ne!(first, second);
    }

    #[cfg(unix)]
    #[test]
    fn external_nonzero_streams_survive_diagnostic_store_reopen() {
        const TOOL_ID: &str = "diagnostics.external_failure";
        const PROJECT_ROOT: &str = "/workspace/scoped-project";
        crate::test_support::with_seeded_pegboard_home(
            "diagnostics-external",
            |_| {},
            || {
                let dir = std::env::temp_dir()
                    .join(format!("upeg-diagnostic-external-{}", std::process::id()));
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).expect("create external-tool fixture dir");
                std::fs::write(
                    dir.join("failure.toml"),
                    r#"id = "diagnostics"

[[tools]]
id = "external_failure"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "printf 'stdout marker\\n'; printf 'stderr marker\\n' >&2; exit 7"]"#,
                )
                .expect("write external-tool fixture");
                let loaded = upeg_loader::load_and_register_dir_verbose(&dir);
                assert_eq!(
                    loaded.loaded,
                    vec![TOOL_ID],
                    "fixture load failed: {:?}",
                    loaded.failed
                );

                let outcome = crate::domain::execution::dispatch::dispatch_tool(
                    TOOL_ID,
                    &serde_json::json!({
                        upeg_core::EXECUTION_CONTEXT_ARG: { "projectRoot": PROJECT_ROOT },
                    }),
                );
                assert!(matches!(outcome, Outcome::Failure(_)), "got {outcome:?}");

                let first = Store::open().expect("open diagnostic store after failure");
                let report = first
                    .read_diagnostics(Some(10))
                    .expect("read report")
                    .into_iter()
                    .find(|report| report.tool_id.as_deref() == Some(TOOL_ID))
                    .expect("external failure diagnostic");
                assert_eq!(report.error_code, "tool_error");
                assert_eq!(report.project.as_deref(), Some(PROJECT_ROOT));
                assert!(report.stdout.contains("stdout marker"));
                assert!(report.stderr.contains("stderr marker"));
                let debug: serde_json::Value = serde_json::from_str(
                    report.debug_context.as_deref().expect("diagnostic context"),
                )
                .expect("debug context JSON");
                assert_eq!(debug["diagnostic_context"]["provenance"], "local");
                let id = report.id;
                drop(first);

                let reopened = Store::open().expect("reopen diagnostic store");
                let report = reopened
                    .diagnostic(&id)
                    .expect("read diagnostic after reopen")
                    .expect("diagnostic survives reopen");
                assert!(report.stdout.contains("stdout marker"));
                assert!(report.stderr.contains("stderr marker"));
                let _ = std::fs::remove_dir_all(dir);
            },
        );
    }

    #[cfg(unix)]
    #[test]
    fn external_timeout_report_keeps_typed_timeout_detail_after_reopen() {
        const TOOL_ID: &str = "diagnostics.external_timeout";
        crate::test_support::with_seeded_pegboard_home(
            "diagnostics-timeout",
            |_| {},
            || {
                let dir = external_fixture_dir("timeout");
                write_external_tool(
                    &dir,
                    "external_timeout",
                    "printf 'before timeout\\n'; sleep 2",
                    Some(50),
                );
                let loaded = upeg_loader::load_and_register_dir_verbose(&dir);
                assert_eq!(
                    loaded.loaded,
                    vec![TOOL_ID],
                    "fixture load failed: {:?}",
                    loaded.failed
                );

                let outcome = crate::domain::execution::dispatch::dispatch_tool(
                    TOOL_ID,
                    &serde_json::json!({}),
                );
                assert!(matches!(outcome, Outcome::Failure(_)), "got {outcome:?}");
                let report = recent_report(TOOL_ID);
                assert!(report.stdout.contains("before timeout"));
                let debug: serde_json::Value =
                    serde_json::from_str(report.debug_context.as_deref().expect("timeout detail"))
                        .expect("timeout debug context JSON");
                assert_eq!(debug["timed_out"], serde_json::Value::Bool(true));
                let _ = std::fs::remove_dir_all(dir);
            },
        );
    }

    #[cfg(unix)]
    #[test]
    fn external_cancellation_report_keeps_partial_streams_and_cancelled_detail() {
        const TOOL_ID: &str = "diagnostics.external_cancelled";
        crate::test_support::with_seeded_pegboard_home(
            "diagnostics-cancel",
            |_| {},
            || {
                let dir = external_fixture_dir("cancel");
                write_external_tool(
                    &dir,
                    "external_cancelled",
                    "printf 'before cancellation\\n'; printf 'cancel stderr\\n' >&2; sleep 2",
                    None,
                );
                let loaded = upeg_loader::load_and_register_dir_verbose(&dir);
                assert_eq!(
                    loaded.loaded,
                    vec![TOOL_ID],
                    "fixture load failed: {:?}",
                    loaded.failed
                );

                let token = upeg_runtime::CancellationToken::new();
                let canceller = token.clone();
                let cancel_thread = std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    canceller.cancel();
                });
                let outcome = upeg_runtime::with_cancellation(token, || {
                    crate::domain::execution::dispatch::dispatch_tool(
                        TOOL_ID,
                        &serde_json::json!({}),
                    )
                });
                cancel_thread.join().expect("canceller thread");
                assert!(matches!(outcome, Outcome::Failure(_)), "got {outcome:?}");
                let report = recent_report(TOOL_ID);
                assert!(report.stdout.contains("before cancellation"));
                assert!(report.stderr.contains("cancel stderr"));
                let debug: serde_json::Value = serde_json::from_str(
                    report
                        .debug_context
                        .as_deref()
                        .expect("cancellation detail"),
                )
                .expect("cancellation debug context JSON");
                assert_eq!(debug["cancelled"], serde_json::Value::Bool(true));
                let _ = std::fs::remove_dir_all(dir);
            },
        );
    }

    #[cfg(unix)]
    fn external_fixture_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "upeg-diagnostic-external-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create external-tool fixture dir");
        dir
    }

    #[cfg(unix)]
    fn write_external_tool(
        dir: &std::path::Path,
        local_id: &str,
        script: &str,
        timeout_ms: Option<u64>,
    ) {
        let timeout =
            timeout_ms.map_or_else(String::new, |value| format!("\ntimeout_ms = {value}"));
        let toml = format!(
            "id = \"diagnostics\"\n\n[[tools]]\nid = \"{local_id}\"\npegboard_units = \"U1\"\ninvoker = \"External\"\ncommand = \"sh\"\nargs_template = [\"-c\", \"{script}\"]{timeout}"
        );
        std::fs::write(dir.join("failure.toml"), toml).expect("write external-tool fixture");
    }

    #[cfg(unix)]
    fn recent_report(tool_id: &str) -> upeg_sources::store::DiagnosticRecord {
        Store::open()
            .expect("open diagnostic store")
            .read_diagnostics(Some(10))
            .expect("read diagnostics")
            .into_iter()
            .find(|report| report.tool_id.as_deref() == Some(tool_id))
            .expect("diagnostic report")
    }
}
