//! Persisted memo content exposed to the Flutter frontend.
//!
//! N01 closes the long-tail "memos persist round-trip" gap. The
//! shared `upeg-pegboard-ui::features::memos` module owns the
//! key-value semantics + the serde shape; this FRB layer is a thin
//! adapter that hands Flutter a `Vec<MemoEntry>` (FRB can't ship a
//! `BTreeMap` directly so we serialise to an ordered list of typed
//! entries — keeps the surface concrete with no `Map<String, dynamic>`
//! on the Dart side).
//!
//! Native: persists in the shared SQLite store under the upeg config
//! root via `upeg-sources::memos`. Wasm: routes through the existing
//! localStorage shim.

use upeg_pegboard_ui::features::memos::{load_memos as ui_load_memos, save_memos as ui_save_memos};

use super::boot::FrbError;

/// One persisted memo: a stable key + body. The wire shape is a
/// flat `Vec<MemoEntry>` so flutter_rust_bridge can emit a concrete
/// Dart class instead of `Map<String, String>` (matches the rest of
/// this crate's typed-DTO style).
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct MemoEntry {
    pub key: String,
    pub body: String,
}

/// Snapshot of all persisted memos, sorted by `key` for a stable
/// Dart-visible order. Empty after a fresh install or when storage
/// is unavailable (no config root, etc.).
#[flutter_rust_bridge::frb(sync)]
pub fn load_memos() -> Vec<MemoEntry> {
    ui_load_memos()
        .unwrap_or_default()
        .into_iter()
        .map(|(key, body)| MemoEntry { key, body })
        .collect()
}

/// Replace the persisted memo map with `entries`. Returns
/// `FrbError::Io` if the backing store rejects the write (native:
/// disk error or missing config root; wasm: localStorage quota /
/// disabled).
#[flutter_rust_bridge::frb(sync)]
pub fn save_memos(entries: Vec<MemoEntry>) -> Result<(), FrbError> {
    let map: std::collections::BTreeMap<String, String> =
        entries.into_iter().map(|e| (e.key, e.body)).collect();
    ui_save_memos(&map).map_err(|err| FrbError::Io {
        message: err.to_string(),
    })
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use upeg_sources::memos::{load_memos_from_path, memos_path_from_root, save_memos_to_path};

    /// The FRB `load_memos` / `save_memos` wrap the env-derived store
    /// (which can collide across parallel tests). Round-trip semantics
    /// belong with the path-based upeg-sources tests (the path is the
    /// store database file); this test only asserts the DTO shape
    /// conversion stays one-to-one.
    #[test]
    fn memo_entry는_path_round_trip을_정확히_보존한다() {
        let dir = std::env::temp_dir().join(format!("upeg-frb-memos-dto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tempdir");

        let path = memos_path_from_root(&dir);
        let mut map = std::collections::BTreeMap::new();
        map.insert("scratch".to_string(), "hello world".to_string());
        map.insert("todo".to_string(), "ship it".to_string());
        save_memos_to_path(&path, &map).expect("save");
        let loaded = load_memos_from_path(&path).expect("load");

        // Convert through the FRB DTO shape and back.
        let entries: Vec<MemoEntry> = loaded
            .into_iter()
            .map(|(key, body)| MemoEntry { key, body })
            .collect();
        let round: std::collections::BTreeMap<String, String> =
            entries.into_iter().map(|e| (e.key, e.body)).collect();
        assert_eq!(round, map);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 빈_엔트리_벡터_저장은_빈_맵으로_왕복한다() {
        let dir = std::env::temp_dir().join(format!("upeg-frb-memos-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tempdir");

        let path = memos_path_from_root(&dir);
        save_memos_to_path(&path, &std::collections::BTreeMap::new()).expect("save");
        let loaded = load_memos_from_path(&path).expect("load");
        assert!(loaded.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
