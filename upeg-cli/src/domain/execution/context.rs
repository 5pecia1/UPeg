//! CLI-side seam over the runtime execution-context layer.
//!
//! The synthesis layer itself ([`upeg_runtime::ExecutionContext`],
//! [`upeg_runtime::apply_execution_context`], preset merging, and the
//! [`upeg_runtime::ProjectContext`] port) was lowered into `upeg-runtime`
//! so every surface — including FRB desktop — shares one contract. This
//! module keeps the CLI-crate conveniences:
//!   - [`prepare_tool_args`] binds the filesystem `ProjectContext`
//!     adapter (`upeg-sources`) so surfaces in this binary don't repeat
//!     the probe choice.
//!   - [`dispatch_tool_on_surface`] / [`dispatch_tool_call`] pair the
//!     context application with the surface visibility gate.

use serde_json::Value;
use upeg_core::{EXECUTION_CONTEXT_ARG, EXECUTION_CONTEXT_CWD, Surface};
pub(crate) use upeg_runtime::ExecutionContext;
use upeg_runtime::toolbox_tool;

use crate::domain::execution::dispatch;

/// Apply `context` (+ optional trigger annotation) to `args` using the
/// filesystem-backed project probe shared by all native surfaces.
pub(crate) fn prepare_tool_args(
    args: Value,
    context: &ExecutionContext,
    trigger: Option<&str>,
) -> Value {
    upeg_runtime::apply_execution_context(
        &upeg_sources::project::FilesystemProjectContext,
        args,
        context,
        trigger,
    )
}

/// Dispatch with a surface gate. A registered tool that is *not* on the
/// caller's surface is collapsed into `Outcome::NotFound` — the same
/// outcome an unknown id produces.
///
/// Why the conflation: the MCP and HTTP protocols must not reveal which
/// tools exist on other surfaces. A distinct "blocked for this surface"
/// response is a side channel an attacker can use to enumerate the
/// registry (`tool exists` vs `tool does not exist`). Surfaces that
/// want a richer diagnostic (e.g. CLI's `ensure_cli_surface`) probe
/// the registry directly *before* dispatching and produce their own
/// human-facing error.
pub(crate) fn dispatch_tool_on_surface(
    tool_id: &str,
    args: &Value,
    surface: Surface,
) -> dispatch::Outcome {
    if let Some(meta) = toolbox_tool(tool_id)
        && !meta.is_on_surface(surface)
    {
        return dispatch::Outcome::NotFound;
    }
    dispatch::dispatch_tool(tool_id, args)
}

/// Context-applied, surface-gated dispatch: the one call every surface
/// in this binary routes a `(tool_id, args)` pair through.
pub(crate) fn dispatch_tool_call(
    tool_id: &str,
    args: Value,
    context: &ExecutionContext,
    trigger: Option<&str>,
) -> dispatch::Outcome {
    let args = prepare_tool_args(args, context, trigger);
    dispatch_tool_on_surface(tool_id, &args, context.surface())
}

/// D-1: must this tool be dispatched in-process even when a host is
/// reachable?
///
/// A Project Manifest (`upeg.toml`) is resolved per *process working
/// directory*. A host started from somewhere else resolved a different
/// manifest — or none — so auto-attaching a `dev.*` tool the local
/// toolbox knows about produces `unknown tool` from a host that never
/// loaded it. Provenance is the authority (`upeg-runtime::provenance`),
/// not the id shape: only tools the loader actually registered from the
/// project manifest are pinned to the local process.
pub(crate) fn requires_local_dispatch(tool_id: &str) -> bool {
    upeg_runtime::tool_provenance(tool_id).is_project_manifest()
}

/// D-2: stamp the caller's absolute working directory into the `_upeg`
/// block before shipping args to a host.
///
/// Without it the host runs `External` tools from the *daemon's* cwd,
/// so a relative `args_template` silently resolves against the wrong
/// tree. `_upeg.cwd` is one of the two caller-supplied context keys the
/// runtime's merge deliberately preserves
/// (`upeg-runtime::execution`), and `upeg-loader`'s External invoker
/// applies it as the child process's working directory.
///
/// Best-effort: an unreadable cwd leaves `args` untouched rather than
/// failing the call — the host then behaves exactly as it did before.
pub(crate) fn with_caller_cwd(args: Value) -> Value {
    let Ok(cwd) = std::env::current_dir() else {
        return args;
    };
    stamp_caller_cwd(args, &cwd.to_string_lossy())
}

/// Pure core of [`with_caller_cwd`] — the caller supplies the directory
/// so this is unit-testable without touching the process cwd.
fn stamp_caller_cwd(args: Value, cwd: &str) -> Value {
    let mut object = match args {
        Value::Object(map) => map,
        Value::Null => serde_json::Map::new(),
        other => {
            let mut map = serde_json::Map::new();
            map.insert("input".into(), other);
            map
        }
    };
    let mut context = object
        .remove(EXECUTION_CONTEXT_ARG)
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    context.insert(EXECUTION_CONTEXT_CWD.into(), Value::String(cwd.to_string()));
    object.insert(EXECUTION_CONTEXT_ARG.into(), Value::Object(context));
    Value::Object(object)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 호출자_cwd_각인은_기존_맥락을_보존한다() {
        let args = serde_json::json!({
            "input": "hi",
            EXECUTION_CONTEXT_ARG: { "approvedSteps": ["format"] },
        });

        let out = stamp_caller_cwd(args, "/workspaces/upeg");

        let context = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("맥락 블록");
        assert_eq!(context[EXECUTION_CONTEXT_CWD], "/workspaces/upeg");
        assert_eq!(context["approvedSteps"][0], "format");
        assert_eq!(out["input"], "hi");
    }

    #[test]
    fn 호출자_cwd_각인은_null_인자도_객체로_승격한다() {
        let out = stamp_caller_cwd(Value::Null, "/tmp/project");

        assert_eq!(
            out[EXECUTION_CONTEXT_ARG][EXECUTION_CONTEXT_CWD],
            "/tmp/project"
        );
    }

    #[test]
    fn 등록되지_않은_도구는_로컬_전용_dispatch를_요구하지_않는다() {
        assert!(
            !requires_local_dispatch("test.context.never_registered"),
            "provenance가 없는 도구는 project manifest 출신이 아니다"
        );
    }

    #[test]
    fn project_manifest_provenance_도구는_로컬_전용_dispatch를_요구한다() {
        let id = "test.context.project_scoped";
        upeg_runtime::register_tool_provenance(
            id,
            upeg_runtime::ToolProvenance::ProjectManifest {
                path: "/workspaces/upeg/upeg.toml".into(),
            },
        );

        assert!(requires_local_dispatch(id));

        upeg_runtime::clear_tool_provenance(id);
        assert!(!requires_local_dispatch(id));
    }
}
