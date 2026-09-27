//! The execution context every FRB-dispatched call carries.
//!
//! One question, asked once: **which surface is this build?** The FRB
//! layer backs exactly two, and `RuntimeHost` already separates them —
//! `desktop` on native, `pwa` on the `wasm32` Flutter-web build. That
//! table lives in [`crate::api::capability::surface_for`] and this module
//! reads it rather than repeating it: a hard-coded `Surface::Desktop`
//! here made the PWA dispatch as `desktop`, which is one of the three
//! default Chain approval surfaces, so a browser tab could lift a barrier
//! meant for the machine the person is sitting at
//! (`upeg_loader::dispatcher::chain::approval` module docs).
//!
//! The module was `desktop_context` while `desktop` was the only answer.

use upeg_core::{
    EXECUTION_CONTEXT_ARG, EXECUTION_CONTEXT_PIN_ID, EXECUTION_CONTEXT_PIN_TOOL_ID, PinId,
    RuntimeHost, Surface,
};

use super::CanonicalToolResult;
use crate::api::capability::surface_for;

const INVALID_BOARD_KEY_ERROR_CODE: &str = "invalid_board_key";

/// The surface this build dispatches as.
///
/// Not a constant: `RuntimeHost::current()` is what tells the two builds
/// apart, and reading it here is what keeps the answer identical to the
/// one `dispatch_capability_for` gives the Flutter UI.
pub(super) fn gui_surface() -> Surface {
    surface_for(RuntimeHost::current())
}

/// GUI execution context: board scope (+ pin preset from the shared
/// layout storage) or global. Board pins dispatched from a board page
/// carry that board's context so tools observe the same `_upeg`
/// contract on every surface.
#[cfg(test)]
pub(super) fn gui_execution_context(
    tool_id: &str,
    board_key: Option<&str>,
) -> Result<upeg_runtime::ExecutionContext, Box<CanonicalToolResult>> {
    gui_execution_context_for_pin(tool_id, board_key, None)
}

pub(super) fn gui_execution_context_for_pin(
    tool_id: &str,
    board_key: Option<&str>,
    pin_id: Option<&str>,
) -> Result<upeg_runtime::ExecutionContext, Box<CanonicalToolResult>> {
    let surface = gui_surface();
    let Some(raw) = board_key else {
        if pin_id.is_some() {
            return Err(Box::new(CanonicalToolResult::error(
                "invalid_pin_id",
                "pin_id requires board_key".to_string(),
            )));
        }
        return Ok(upeg_runtime::ExecutionContext::global(surface));
    };
    let board = upeg_core::BoardKey::parse(raw).map_err(|err| {
        Box::new(CanonicalToolResult::error(
            INVALID_BOARD_KEY_ERROR_CODE,
            format!("board_key validation error: {err}"),
        ))
    })?;
    let pin_id = pin_id.map(PinId::parse).transpose().map_err(|err| {
        Box::new(CanonicalToolResult::error(
            "invalid_pin_id",
            format!("pin_id validation error: {err}"),
        ))
    })?;
    let preset = board_args_preset(board.as_str(), tool_id, pin_id.as_ref().map(PinId::as_str))?;
    Ok(upeg_runtime::ExecutionContext::for_optional_board(
        surface,
        Some(board),
        preset,
    ))
}

/// The pin's saved args preset for `(board, tool)` from the shared
/// layout storage (SQLite store on native, localStorage on wasm).
fn board_args_preset(
    board: &str,
    tool_id: &str,
    pin_id: Option<&str>,
) -> Result<Option<upeg_core::ArgsPreset>, Box<CanonicalToolResult>> {
    let boards = upeg_pegboard_ui::features::boards::load_boards()
        .unwrap_or_else(upeg_pegboard_ui::features::boards::default_boards);
    let layouts = upeg_pegboard_ui::features::layouts::load_layouts(&boards);
    let placement = layouts
        .as_ref()
        .and_then(|layouts| layouts.get(board))
        .and_then(|layout| {
            layout.iter().find(|placement| {
                placement.tool_id == tool_id
                    && pin_id.is_none_or(|id| placement.pin_id.as_str() == id)
            })
        });
    if pin_id.is_some() && placement.is_none() {
        return Err(Box::new(CanonicalToolResult::error(
            "invalid_pin_id",
            format!("pin is absent or does not match tool `{tool_id}` on board `{board}`"),
        )));
    }
    Ok(placement.and_then(|placement| placement.args_preset.clone()))
}

/// Bind the selected native project and its execution directory; wasm uses
/// registry-only context because it has no local project filesystem.
pub(super) fn apply_gui_context(
    args: serde_json::Value,
    context: &upeg_runtime::ExecutionContext,
    pin_id: Option<&str>,
    tool_id: &str,
) -> serde_json::Value {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut args = upeg_runtime::apply_execution_context(
            &upeg_sources::project::FilesystemProjectContext,
            args,
            context,
            None,
        );
        stamp_pin_context(&mut args, pin_id, tool_id);
        with_project_directory(
            args,
            upeg_sources::project::current_project_root().as_deref(),
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let mut args = upeg_runtime::apply_execution_context(
            &upeg_runtime::RegistryProjectContext,
            args,
            context,
            None,
        );
        stamp_pin_context(&mut args, pin_id, tool_id);
        args
    }
}

fn stamp_pin_context(args: &mut serde_json::Value, pin_id: Option<&str>, tool_id: &str) {
    let Some(pin_id) = pin_id else { return };
    let Some(context) = args
        .get_mut(EXECUTION_CONTEXT_ARG)
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    context.insert(EXECUTION_CONTEXT_PIN_ID.into(), pin_id.into());
    context.insert(EXECUTION_CONTEXT_PIN_TOOL_ID.into(), tool_id.into());
}

#[cfg(not(target_arch = "wasm32"))]
fn with_project_directory(
    mut args: serde_json::Value,
    root: Option<&std::path::Path>,
) -> serde_json::Value {
    if let (Some(root), Some(context)) = (
        root,
        args.get_mut(upeg_core::EXECUTION_CONTEXT_ARG)
            .and_then(serde_json::Value::as_object_mut),
    ) {
        context
            .entry(upeg_core::EXECUTION_CONTEXT_CWD.to_string())
            .or_insert_with(|| serde_json::Value::String(root.display().to_string()));
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn selected_project_supplies_the_gui_directory_without_overriding_explicit_call_context() {
        use serde_json::json;
        let root = Some(std::path::Path::new("/selected/project"));
        let selected = with_project_directory(json!({"_upeg": {"surface":"desktop"}}), root);
        assert_eq!(selected["_upeg"]["cwd"], "/selected/project");
        let explicit = with_project_directory(json!({"_upeg": {"cwd":"/explicit/target"}}), root);
        assert_eq!(explicit["_upeg"]["cwd"], "/explicit/target");
        let global = with_project_directory(json!({"_upeg": {"surface":"desktop"}}), None);
        assert!(global["_upeg"].get("cwd").is_none());
    }

    #[test]
    fn gui_surface_follows_this_builds_runtime_host() {
        // `pwa` on the wasm32 build, `desktop` otherwise. This test
        // compiles on both targets, and `cargo test --target
        // wasm32-unknown-unknown` proves the lower branch.
        let expected = if cfg!(target_arch = "wasm32") {
            Surface::Pwa
        } else {
            Surface::Desktop
        };
        assert_eq!(gui_surface(), expected);
    }

    #[test]
    fn call_without_board_builds_global_context_with_this_builds_surface() {
        let context = gui_execution_context("num.hex_to_decimal", None)
            .expect("a board-less context cannot fail");
        assert_eq!(context.surface(), gui_surface());
        assert_eq!(context.principal().surface, gui_surface());
    }

    #[test]
    fn invalid_board_key_is_rejected_with_canonical_error_envelope() {
        // A whitespace-only key. The failure to build a context must
        // surface as the canonical error envelope, not a silent global
        // fallback.
        let err = gui_execution_context("num.hex_to_decimal", Some("   "))
            .expect_err("invalid board key must be rejected");
        assert!(!err.ok);
    }

    #[test]
    fn caller_cannot_forge_pin_placement_in_reserved_args() {
        let context = upeg_runtime::ExecutionContext::board(
            gui_surface(),
            upeg_core::BoardKey::parse("board-a").unwrap(),
            upeg_runtime::empty_args_preset(),
        );
        let caller = serde_json::json!({
            "_upeg": {
                "board": "forged-board",
                "pinId": "forged-pin",
                "pinToolId": "test.other"
            }
        });
        let unpinned = apply_gui_context(caller.clone(), &context, None, "test.shared");
        assert_eq!(unpinned["_upeg"]["board"], "board-a");
        assert!(unpinned["_upeg"].get(EXECUTION_CONTEXT_PIN_ID).is_none());
        assert!(
            unpinned["_upeg"]
                .get(EXECUTION_CONTEXT_PIN_TOOL_ID)
                .is_none()
        );

        let pinned = apply_gui_context(caller, &context, Some("real-pin"), "test.shared");
        assert_eq!(pinned["_upeg"][EXECUTION_CONTEXT_PIN_ID], "real-pin");
        assert_eq!(
            pinned["_upeg"][EXECUTION_CONTEXT_PIN_TOOL_ID],
            "test.shared"
        );
    }
}
