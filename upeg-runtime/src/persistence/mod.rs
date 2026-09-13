//! Filesystem persistence for shared domain records (native only).
//!
//! `upeg-core` stays domain-only: it owns pure types, serialization, and path
//! *computation* ([`upeg_core::paths`]). The actual `std::fs` reads/writes and
//! `metadata` probes live here, one layer up in the runtime, so the domain
//! crate carries no filesystem dependency.
//!
//! Wasm builds persist through `localStorage`, not the filesystem, and never
//! compile this module.

#![cfg(not(target_arch = "wasm32"))]

mod bootstrap;
pub mod io;

pub use bootstrap::bootstrap_tweaks;

use std::path::Path;
use std::time::SystemTime;

/// Last-modified timestamp for `path`, or `None` when it cannot be read
/// (missing file, permission denied, platform without mtime support). Surfaces
/// poll this to detect edits made by another surface.
pub fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "upeg-persistence-{label}-{}-{nanos}",
            std::process::id(),
        ))
    }

    #[test]
    fn 존재하지_않는_경로의_mtime은_없음이다() {
        let missing = temp_path("mtime-missing").join("never-written");
        assert!(modified_time(&missing).is_none());
    }

    #[test]
    fn 기록된_파일의_mtime을_읽는다() {
        let dir = temp_path("mtime-present");
        std::fs::create_dir_all(&dir).expect("setup dir");
        let path = dir.join("probe");
        std::fs::write(&path, "versioned").expect("write probe");

        assert!(modified_time(&path).is_some());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
