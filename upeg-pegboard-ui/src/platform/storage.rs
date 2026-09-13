//! Desktop UI storage boundary.
//!
//! Feature modules own state semantics and serialization. This module owns the
//! wasm/native storage split so feature code does not branch on platform I/O.

#![allow(
    clippy::result_unit_err,
    reason = "storage::set_item's failure mode is platform-specific noise; callers today ignore the error and a typed error would inflate the surface without adding signal"
)]

pub const BOARDS_KEY: &str = "upeg.boards.v1";
pub const LAYOUTS_KEY: &str = "upeg.layouts.v2";
pub const MEMOS_KEY: &str = "upeg.memos.v1";
pub const SELECTED_BOARD_KEY: &str = "upeg.selected_board.v2";
pub const SELECTED_TAG_KEY: &str = "upeg.selected_tag.v2";
pub const TWEAKS_KEY: &str = "upeg.tweaks.v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PegboardSelectionValue {
    pub board_key: Option<String>,
    pub tag: String,
}

#[cfg(target_arch = "wasm32")]
pub fn get_item(key: &str) -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item(key)
        .ok()
        .flatten()
}

/// Native storage shim. localStorage doesn't exist outside the browser, so
/// shared pegboard state routes through `upeg-sources` while Desktop-only
/// tweaks persist in the per-user config root.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_item(key: &str) -> Option<String> {
    match key {
        BOARDS_KEY => upeg_sources::pegboard::boards_json(),
        LAYOUTS_KEY => upeg_sources::pegboard::layouts_json(),
        SELECTED_BOARD_KEY => upeg_sources::pegboard::selection_value().board_key,
        SELECTED_TAG_KEY => upeg_sources::pegboard::selected_tag_value(),
        TWEAKS_KEY => load_tweaks_native(),
        MEMOS_KEY => None,
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
pub fn set_item(key: &str, value: &str) -> Result<(), ()> {
    let storage = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .ok_or(())?;
    storage.set_item(key, value).map_err(|_| ())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_item(key: &str, value: &str) -> Result<(), ()> {
    match key {
        BOARDS_KEY => upeg_sources::pegboard::save_boards_json(value).map_err(|_| ()),
        LAYOUTS_KEY => upeg_sources::pegboard::save_layouts_json(value).map_err(|_| ()),
        SELECTED_TAG_KEY => upeg_sources::pegboard::save_selected_tag_value(value).map_err(|_| ()),
        TWEAKS_KEY => save_tweaks_native(value),
        MEMOS_KEY => Err(()),
        _ => Err(()),
    }
}

/// Canonical tweaks path, resolved straight from `upeg-core`'s
/// config-root owner. This crate is UI state — it must not reach up into
/// a surface crate (`upeg-cli`) just to learn where `$UPEG_HOME` is.
#[cfg(not(target_arch = "wasm32"))]
fn tweaks_path() -> Option<std::path::PathBuf> {
    upeg_core::paths::tweaks_path()
}

#[cfg(not(target_arch = "wasm32"))]
fn save_tweaks_native(value: &str) -> Result<(), ()> {
    let path = tweaks_path().ok_or(())?;
    upeg_runtime::persistence::io::save_to_path(&path, value).map_err(|_| ())
}

#[cfg(not(target_arch = "wasm32"))]
fn load_tweaks_native() -> Option<String> {
    upeg_runtime::persistence::io::load_from_path(&tweaks_path()?)
}

pub fn save_selected_tag(tag: &str) {
    let _ = set_item(SELECTED_TAG_KEY, tag);
}

pub fn load_selected_tag() -> Option<String> {
    get_item(SELECTED_TAG_KEY)
}

#[cfg(target_arch = "wasm32")]
pub fn load_pegboard_selection() -> PegboardSelectionValue {
    PegboardSelectionValue {
        board_key: get_item(SELECTED_BOARD_KEY).and_then(|key| {
            let trimmed = key.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }),
        tag: load_selected_tag().unwrap_or_else(|| crate::features::layouts::ALL_TAG.to_string()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_pegboard_selection() -> PegboardSelectionValue {
    let selection = upeg_sources::pegboard::selection_value();
    PegboardSelectionValue {
        board_key: selection.board_key,
        tag: selection.tag,
    }
}

#[cfg(target_arch = "wasm32")]
pub fn save_pegboard_selection(selection: &PegboardSelectionValue) -> Result<(), ()> {
    set_item(
        SELECTED_BOARD_KEY,
        selection.board_key.as_deref().unwrap_or(""),
    )?;
    set_item(SELECTED_TAG_KEY, &selection.tag)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_pegboard_selection(selection: &PegboardSelectionValue) -> Result<(), ()> {
    upeg_sources::pegboard::save_selection_value(upeg_sources::pegboard::PegboardSelection {
        board_key: selection.board_key.clone(),
        tag: selection.tag.clone(),
    })
    .map_err(|_| ())
}
