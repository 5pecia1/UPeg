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

/// Must this tool be dispatched in-process even when a host is
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

/// Stamp the caller's absolute working directory into the `_upeg`
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
    fn caller_cwd_stamp_preserves_existing_context() {
        let args = serde_json::json!({
            "input": "hi",
            EXECUTION_CONTEXT_ARG: { "approvedSteps": ["format"] },
        });

        let out = stamp_caller_cwd(args, "/workspaces/upeg");

        let context = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("context block");
        assert_eq!(context[EXECUTION_CONTEXT_CWD], "/workspaces/upeg");
        assert_eq!(context["approvedSteps"][0], "format");
        assert_eq!(out["input"], "hi");
    }

    #[test]
    fn caller_cwd_stamp_promotes_null_args_to_object() {
        let out = stamp_caller_cwd(Value::Null, "/tmp/project");

        assert_eq!(
            out[EXECUTION_CONTEXT_ARG][EXECUTION_CONTEXT_CWD],
            "/tmp/project"
        );
    }

    #[test]
    fn unregistered_tool_does_not_require_local_dispatch() {
        assert!(
            !requires_local_dispatch("test.context.never_registered"),
            "a tool without provenance does not come from a project manifest"
        );
    }

    #[test]
    fn project_manifest_provenance_tool_requires_local_dispatch() {
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
