//! Path-based file IO for the persisted `Tweaks` record (native only).
//!
//! Byte-level only: this module reads / writes UTF-8 strings, and leaves
//! JSON parsing to [`upeg_core::parse_tweaks`]. That split keeps the io
//! surface unit-testable (no serde required) and lets each surface decide
//! which on-disk file path it wants.
//!
//! Wasm builds store preferences in `localStorage`, not the filesystem —
//! they don't compile this module (see [`super`]).

use std::io;
use std::path::{Path, PathBuf};

pub use upeg_core::paths::TWEAKS_FILENAME;

/// Convenience: build the canonical Tweaks path given a per-user
/// config directory. Delegates to the pure path computation in `upeg-core`.
pub fn tweaks_path_in(dir: &Path) -> PathBuf {
    upeg_core::paths::tweaks_path_in(dir)
}

/// Write `value` to `path`, creating parent directories on the way.
///
/// Errors mirror the underlying filesystem: missing parent permission,
/// read-only mount, etc. Callers typically surface the error to the user
/// as transient ("write failed: …") rather than retrying.
pub fn save_to_path(path: &Path, value: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, value)
}

/// Read the contents of `path` into a string. Returns `None` for any
/// IO failure (file missing, permission denied, non-UTF-8 bytes). Callers
/// fall back to defaults rather than panicking on a corrupted prefs file.
pub fn load_from_path(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "upeg-prefs-io-{label}-{}-{nanos}",
            std::process::id(),
        ))
    }

    #[test]
    fn 저장과_로드는_왕복한다() {
        let dir = temp_path("rt");
        let path = dir.join("tweaks.json");
        let payload = r#"{"theme":"Dark","accent":"Pink","show_holes":true,"locale":"Ko"}"#;

        save_to_path(&path, payload).expect("write");
        let loaded = load_from_path(&path).expect("read");
        assert_eq!(loaded, payload);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn 저장은_누락된_상위_디렉터리를_만든다() {
        let dir = temp_path("mkdir").join("nested").join("deeper");
        let path = dir.join("tweaks.json");
        assert!(
            !dir.exists(),
            "precondition: nested parent must not already exist"
        );

        save_to_path(&path, "payload").expect("write through missing parents");
        assert!(path.exists());
        assert_eq!(load_from_path(&path).as_deref(), Some("payload"));

        let _ = std::fs::remove_file(&path);
        // Clean up nested dirs best-effort; tests are isolated by pid+nanos.
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn 없는_경로에서_로드하면_없음을_반환한다() {
        let missing = temp_path("missing").join("never-written.json");
        assert!(load_from_path(&missing).is_none());
    }

    #[test]
    fn 디렉터리에서_로드하면_없음을_반환한다() {
        // Reading a directory path (not a file) is an IO error — must be
        // surfaced as None rather than panicking.
        let dir = temp_path("dir-as-file");
        std::fs::create_dir_all(&dir).expect("setup dir");
        assert!(load_from_path(&dir).is_none());
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn 설정_경로_생성은_정규_파일명을_덧붙인다() {
        let dir = std::path::Path::new("/tmp/upeg-cfg");
        let path = tweaks_path_in(dir);
        assert_eq!(path, dir.join(TWEAKS_FILENAME));
        assert!(path.ends_with(TWEAKS_FILENAME));
    }
}
