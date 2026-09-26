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

use upeg_core::{RuntimeHost, Surface};

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
pub(super) fn gui_execution_context(
    tool_id: &str,
    board_key: Option<&str>,
) -> Result<upeg_runtime::ExecutionContext, Box<CanonicalToolResult>> {
    let surface = gui_surface();
    let Some(raw) = board_key else {
        return Ok(upeg_runtime::ExecutionContext::global(surface));
    };
    let board = upeg_core::BoardKey::parse(raw).map_err(|err| {
        Box::new(CanonicalToolResult::error(
            INVALID_BOARD_KEY_ERROR_CODE,
            format!("board_key validation error: {err}"),
        ))
    })?;
    let preset = board_args_preset(board.as_str(), tool_id);
    Ok(upeg_runtime::ExecutionContext::for_optional_board(
        surface,
        Some(board),
        preset,
    ))
}

/// The pin's saved args preset for `(board, tool)` from the shared
/// layout storage (SQLite store on native, localStorage on wasm).
fn board_args_preset(board: &str, tool_id: &str) -> Option<upeg_core::ArgsPreset> {
    let boards = upeg_pegboard_ui::features::boards::load_boards()
        .unwrap_or_else(upeg_pegboard_ui::features::boards::default_boards);
    let layouts = upeg_pegboard_ui::features::layouts::load_layouts(&boards)?;
    layouts
        .get(board)?
        .iter()
        .find(|placement| placement.tool_id == tool_id)
        .and_then(|placement| placement.args_preset.clone())
}

/// Bind the platform-appropriate `ProjectContext` probe: filesystem
/// `upeg.toml` discovery on native, registry-only on wasm (no
/// filesystem to walk).
pub(super) fn apply_gui_context(
    args: serde_json::Value,
    context: &upeg_runtime::ExecutionContext,
) -> serde_json::Value {
    #[cfg(not(target_arch = "wasm32"))]
    {
        upeg_runtime::apply_execution_context(
            &upeg_sources::project::FilesystemProjectContext,
            args,
            context,
            None,
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        upeg_runtime::apply_execution_context(
            &upeg_runtime::RegistryProjectContext,
            args,
            context,
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
