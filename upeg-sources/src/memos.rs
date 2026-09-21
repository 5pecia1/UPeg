//! Native persistence facade for memos.
//!
//! Memos are a flat `String → String` map persisted in the shared SQLite
//! store ([`crate::store::Store`], `memos` table) — the JSON side file
//! (`memos.v1.json`) is gone. Callers keep the same facade surface as
//! before: env-derived convenience load/save plus path-based helpers so
//! tests can point at an ephemeral database without racing on the
//! process-wide `UPEG_HOME` env var. A "memos path" is now the store
//! database path ([`upeg_core::paths::STORE_FILE`]), mirroring
//! `pegboard::state_path_from_root`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::store::{Store, StoreError};

#[derive(Debug, thiserror::Error)]
pub enum MemosError {
    #[error("config root unavailable; set UPEG_HOME, HOME, or APPDATA")]
    ConfigRootUnavailable,
    #[error("memos store: {0}")]
    Store(#[from] StoreError),
}

/// Store database path under an explicit config root.
pub fn memos_path_from_root(root: &Path) -> PathBuf {
    upeg_core::paths::store_path_in(root)
}

/// Store database path under the env-derived config root.
pub fn memos_path_from_env() -> Option<PathBuf> {
    upeg_core::paths::store_path()
}

/// Load memos from the store at an arbitrary database path. A fresh
/// (or absent) database yields an empty map; tombstoned rows are
/// filtered out.
pub fn load_memos_from_path(path: &Path) -> Result<BTreeMap<String, String>, MemosError> {
    Ok(Store::open_at(path)?.load_memos()?)
}

/// Replace the persisted memo map in the store at an arbitrary database
/// path (one write transaction; removed keys become tombstones).
pub fn save_memos_to_path(path: &Path, memos: &BTreeMap<String, String>) -> Result<(), MemosError> {
    Ok(Store::open_at(path)?.save_memos(memos)?)
}

/// Convenience: load from the env-derived store. Returns an empty map
/// when no config root is available (no panic — matches how
/// `pegboard::load_state` falls back).
pub fn load_memos() -> BTreeMap<String, String> {
    memos_path_from_env()
        .and_then(|path| load_memos_from_path(&path).ok())
        .unwrap_or_default()
}

/// Convenience: save to the env-derived store. Errors when no config
/// root is available (matches `pegboard::save_state`).
pub fn save_memos(memos: &BTreeMap<String, String>) -> Result<(), MemosError> {
    let path = memos_path_from_env().ok_or(MemosError::ConfigRootUnavailable)?;
    save_memos_to_path(&path, memos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_tempdir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("upeg-sources-memos-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tempdir");
        dir
    }

    #[test]
    fn fresh_store_returns_empty_map() {
        let dir = fresh_tempdir("fresh");
        let path = memos_path_from_root(&dir);
        let memos = load_memos_from_path(&path).expect("load");
        assert!(memos.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = fresh_tempdir("roundtrip");
        let path = memos_path_from_root(&dir);
        let mut memos = BTreeMap::new();
        memos.insert("scratch".to_string(), "hello".to_string());
        memos.insert("todo".to_string(), "ship".to_string());
        save_memos_to_path(&path, &memos).expect("save");
        let loaded = load_memos_from_path(&path).expect("load");
        assert_eq!(loaded, memos);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn removed_key_is_gone_on_next_load() {
        let dir = fresh_tempdir("tombstone");
        let path = memos_path_from_root(&dir);
        let mut memos = BTreeMap::new();
        memos.insert("keep".to_string(), "body".to_string());
        memos.insert("drop".to_string(), "gone".to_string());
        save_memos_to_path(&path, &memos).expect("first save");

        memos.remove("drop");
        save_memos_to_path(&path, &memos).expect("key-removal save");

        let loaded = load_memos_from_path(&path).expect("load");
        assert_eq!(loaded, memos, "removed key must not appear on load");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
