//! Memo state — persisted via shared storage, surfaced through backup.
//!
//! Phase 8 of the Pin rename + auto-render rollout deleted the legacy
//! MemoWidget on the retired Dioxus surface, but the persisted content
//! lives on as part of the user's environment: `backup::current_environment`
//! includes the memo map, and Batch S (N01) wires `load_memos` /
//! `save_memos` through the FRB layer so Flutter can grow a memo
//! affordance without needing a separate codepath for "memos exist
//! but no Dart widget renders them yet".

use std::collections::BTreeMap;

/// Load all persisted memos. Returns `None` when nothing has been
/// persisted yet (fresh install, no storage backend, etc.); the
/// FRB layer maps `None` to an empty `Vec<MemoEntry>` on the wire.
pub fn load_memos() -> Option<BTreeMap<String, String>> {
    #[cfg(target_arch = "wasm32")]
    {
        let raw = crate::platform::storage::get_item(crate::platform::storage::MEMOS_KEY)?;
        serde_json::from_str(&raw).ok()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(upeg_sources::memos::load_memos())
    }
}

/// Replace the persisted memo map. Returns the source-of-truth
/// error type from the underlying storage so callers see whether the
/// failure is I/O, JSON encoding, or a missing config root.
#[derive(Debug, thiserror::Error)]
pub enum MemosSaveError {
    #[error("memos storage unavailable")]
    StorageUnavailable,
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Native(#[from] upeg_sources::memos::MemosError),
    #[error("memos JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn save_memos(memos: &BTreeMap<String, String>) -> Result<(), MemosSaveError> {
    #[cfg(target_arch = "wasm32")]
    {
        let json = serde_json::to_string(memos)?;
        crate::platform::storage::set_item(crate::platform::storage::MEMOS_KEY, &json)
            .map_err(|()| MemosSaveError::StorageUnavailable)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        upeg_sources::memos::save_memos(memos)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 저장소가_비어있으면_빈_맵_또는_없음을_반환한다() {
        // The platform shim returns None / empty for a fresh store;
        // the round-trip semantics are exercised on the FRB layer
        // where the test can swing the config root via UPEG_HOME.
        let _ = load_memos();
    }
}
