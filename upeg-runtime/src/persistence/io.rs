//! Path-based file IO for the persisted `Tweaks` record (native only).
//!
//! Byte-level only: this module reads / writes UTF-8 strings, and leaves
//! JSON parsing to [`upeg_core::parse_tweaks`]. That split keeps the io
//! surface unit-testable (no serde required) and lets each surface decide
//! which on-disk file path it wants.
//!
//! Wasm builds store preferences in `localStorage`, not the filesystem —
//! they don't compile this module (see [`super`]).

use std::fs::{File, OpenOptions};
use std::io::{self, Write as _};
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
    if path.file_name().is_some_and(|name| name == TWEAKS_FILENAME) {
        let _lock = lock_preferences(path)?;
        return write_atomically(path, value);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, value)
}

/// Patch a preferences file under the same lock as other native writers.
pub fn update_to_path<T>(
    path: &Path,
    edit: impl FnOnce(Option<&str>) -> io::Result<(Option<String>, T)>,
) -> io::Result<T> {
    let _lock = lock_preferences(path)?;
    let current = match std::fs::read_to_string(path) {
        Ok(current) => Some(current),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let (value, result) = edit(current.as_deref())?;
    if let Some(value) = value {
        write_atomically(path, &value)?;
    }
    Ok(result)
}

/// Merge selected preference fields under the shared writer lock, preserving
/// fields added by newer versions and edits from other surfaces.
pub fn patch_tweaks_to_path(
    path: &Path,
    patch: &serde_json::Map<String, serde_json::Value>,
) -> io::Result<upeg_core::prefs::Tweaks> {
    update_to_path(path, |current| {
        let mut record: serde_json::Map<String, serde_json::Value> = match current {
            Some(text) => serde_json::from_str(text)?,
            None => serde_json::from_value(serde_json::to_value(super::native_default_tweaks())?)?,
        };
        record.extend(patch.clone());
        let json = serde_json::to_string(&record)?;
        let parsed = upeg_core::prefs::parse_stored_tweaks(&json)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid preferences"))?;
        Ok((Some(json), parsed))
    })
}

fn lock_preferences(path: &Path) -> io::Result<(upeg_core::paths::StorageLease, File)> {
    let storage =
        upeg_core::paths::StorageLease::for_artifact(path, "config").map_err(io::Error::other)?;
    let lock_path = path.with_extension("lock");
    std::fs::create_dir_all(parent_dir(path))?;
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    file.lock()?;
    Ok((storage, file))
}

fn write_atomically(path: &Path, value: &str) -> io::Result<()> {
    let mut temporary = tempfile::NamedTempFile::new_in(parent_dir(path))?;
    temporary.write_all(value.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
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
    fn stale_surface_edits_preserve_other_fields_and_unknown_values() {
        use serde_json::{Map, json};
        let dir = temp_path("stale-surfaces");
        let path = dir.join(TWEAKS_FILENAME);
        save_to_path(&path, r#"{"theme":"Light","accent":"Green","locale":"En","show_holes":true,"pin_handle_style":"Peg","local_http_host":false,"future":{"feature":42}}"#).unwrap();
        let mut cli = Map::new();
        cli.insert("theme".into(), json!("Dark"));
        patch_tweaks_to_path(&path, &cli).unwrap();
        let mut gui = Map::new();
        gui.insert("accent".into(), json!("Cyan"));
        patch_tweaks_to_path(&path, &gui).unwrap();
        let mut tui = Map::new();
        tui.insert("locale".into(), json!("Ko"));
        let merged = patch_tweaks_to_path(&path, &tui).unwrap();
        assert_eq!(merged.theme, upeg_core::prefs::Theme::Dark);
        assert_eq!(merged.accent, upeg_core::prefs::Accent::Cyan);
        assert_eq!(merged.locale, upeg_core::prefs::Locale::Ko);
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(json["future"]["feature"], 42);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_and_load_round_trip() {
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
    fn save_creates_missing_parent_directories() {
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
    fn load_from_missing_path_returns_none() {
        let missing = temp_path("missing").join("never-written.json");
        assert!(load_from_path(&missing).is_none());
    }

    #[test]
    fn load_from_directory_returns_none() {
        // Reading a directory path (not a file) is an IO error — must be
        // surfaced as None rather than panicking.
        let dir = temp_path("dir-as-file");
        std::fs::create_dir_all(&dir).expect("setup dir");
        assert!(load_from_path(&dir).is_none());
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn settings_path_construction_appends_canonical_filename() {
        let dir = std::path::Path::new("/tmp/upeg-cfg");
        let path = tweaks_path_in(dir);
        assert_eq!(path, dir.join(TWEAKS_FILENAME));
        assert!(path.ends_with(TWEAKS_FILENAME));
    }
}
