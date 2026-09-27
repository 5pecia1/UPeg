//! Private argument preparation and shared in-process dispatch.

use super::{
    CanonicalToolResult, DISPATCH_UNIMPLEMENTED_ERROR_CODE, INVALID_ARGS_JSON_ERROR_CODE,
    INVALID_TOOL_ID_ERROR_CODE, TOOL_NOT_FOUND_ERROR_CODE, ToolId, apply_gui_context,
    gui_execution_context_for_pin, register_toolkit_runtime, shape_approval_arg,
};

/// Register generated discovery metadata without loading toolkit code.
pub(super) fn register_embedded_toolkit_metadata() {
    if let Err(error) = upeg_toolkit_catalog::register_embedded_metadata() {
        tracing::error!(%error, "upeg toolkit catalog metadata registration failed");
    }
}

#[allow(
    clippy::result_large_err,
    reason = "FRB canonical error is the established dispatch envelope"
)]
pub(super) fn prepare_web_toolkit_args(
    tool_id: &str,
    args_json: &str,
    board_key: Option<&str>,
    pin_id: Option<&str>,
    approve: bool,
) -> Result<String, CanonicalToolResult> {
    if let Err(error) = ToolId::parse_canonical(tool_id) {
        return Err(CanonicalToolResult::error(
            INVALID_TOOL_ID_ERROR_CODE,
            format!("tool_id validation error: {error}"),
        ));
    }
    let args = serde_json::from_str(args_json).map_err(|error| {
        CanonicalToolResult::error(
            INVALID_ARGS_JSON_ERROR_CODE,
            format!("args_json parse error: {error}"),
        )
    })?;
    // A browser Worker cannot retain this lease while it executes, but the
    // context snapshot itself must be atomic: a project switch cannot land
    // between reading a pin preset and applying the GUI execution context.
    let _project_call = upeg_runtime::project_scope::begin_call()
        .map_err(|message| CanonicalToolResult::error("project_switching", message))?;
    let context =
        gui_execution_context_for_pin(tool_id, board_key, pin_id).map_err(|error| *error)?;
    let effective_args =
        shape_approval_arg(apply_gui_context(args, &context, pin_id, tool_id), approve);
    serde_json::to_string(&effective_args)
        .map_err(|error| CanonicalToolResult::error("args_json_encode_failed", error.to_string()))
}

/// Shared dispatch body: id validation, args parsing, desktop execution
/// context, approval shaping, registry dispatch.
///
/// `pub(crate)` so the streaming entry point (`api::dispatch_stream`)
/// runs the *same* path with a progress sink and a cancellation token
/// installed around it, rather than forking the arg-shaping logic.
#[cfg(test)]
pub(crate) fn dispatch_tool_impl(
    tool_id: &str,
    args_json: &str,
    board_key: Option<&str>,
    approve: bool,
) -> CanonicalToolResult {
    dispatch_tool_impl_for_pin(tool_id, args_json, board_key, None, approve)
}

pub(crate) fn dispatch_tool_impl_for_pin(
    tool_id: &str,
    args_json: &str,
    board_key: Option<&str>,
    pin_id: Option<&str>,
    approve: bool,
) -> CanonicalToolResult {
    if let Err(err) = ToolId::parse_canonical(tool_id) {
        return CanonicalToolResult::error(
            INVALID_TOOL_ID_ERROR_CODE,
            format!("tool_id validation error: {err}"),
        );
    }

    let args: serde_json::Value = match serde_json::from_str(args_json) {
        Ok(v) => v,
        Err(err) => {
            return CanonicalToolResult::error(
                INVALID_ARGS_JSON_ERROR_CODE,
                format!("args_json parse error: {err}"),
            );
        }
    };

    // Take the project call lease before reading board presets/context and
    // retain it through dispatch. Without this, a project switch could swap
    // registry state between the preset read and the eventual tool call.
    let _project_call = match upeg_runtime::project_scope::begin_call() {
        Ok(guard) => guard,
        Err(message) => return CanonicalToolResult::error("project_switching", message),
    };

    let context = match gui_execution_context_for_pin(tool_id, board_key, pin_id) {
        Ok(context) => context,
        Err(err) => return *err,
    };
    // Approval shaping runs *after* the context merge so it is the last
    // writer: a board pin's saved args preset is caller-supplied data
    // too, and it must not be able to smuggle the reserved key in as a
    // default.
    let args = shape_approval_arg(apply_gui_context(args, &context, pin_id, tool_id), approve);

    if let Err(message) = register_toolkit_runtime() {
        return CanonicalToolResult::error("toolkit_catalog", message);
    }
    match upeg_runtime::try_runtime_dispatch(tool_id, &args) {
        Some(result) => CanonicalToolResult::from(result),
        None if upeg_runtime::toolbox_tool(tool_id).is_none() => CanonicalToolResult::error(
            TOOL_NOT_FOUND_ERROR_CODE,
            format!("tool `{tool_id}` is not registered"),
        ),
        None => CanonicalToolResult::error(
            DISPATCH_UNIMPLEMENTED_ERROR_CODE,
            format!("dispatch not implemented for `{tool_id}`"),
        ),
    }
}
