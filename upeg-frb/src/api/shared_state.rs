//! Version stamps for native shared state.
//!
//! Flutter uses this read-only surface to detect changes made by another
//! surface. The source of truth remains the shared storage under `~/.upeg`:
//! the tweaks JSON file (stamped by mtime) and the SQLite store (stamped by
//! its write-revision counter — under WAL the database file's mtime does
//! not reliably move on commit, so an mtime stamp would silently stop
//! seeing TUI edits).

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct SharedStateVersionsDto {
    pub tweaks: Option<u64>,
    pub pegboard: Option<u64>,
}

#[flutter_rust_bridge::frb(sync)]
pub fn load_shared_state_versions() -> SharedStateVersionsDto {
    SharedStateVersionsDto {
        tweaks: modified_epoch_millis(upeg_core::paths::tweaks_path()),
        pegboard: pegboard_change_rev(),
    }
}

/// Write revision of the shared pegboard store; `None` until the store
/// exists and has seen at least one write.
#[cfg(not(target_arch = "wasm32"))]
fn pegboard_change_rev() -> Option<u64> {
    upeg_sources::pegboard::state_change_rev()
}

/// Wasm32 fallback: always "no version". The PWA has no shared `~/.upeg`
/// store another surface could have written; callers already treat `None`
/// as "nothing to report".
#[cfg(target_arch = "wasm32")]
fn pegboard_change_rev() -> Option<u64> {
    None
}

pub(crate) fn modified_epoch_millis(path: Option<std::path::PathBuf>) -> Option<u64> {
    let time = modified_time(path.as_deref())?;
    let duration = time.duration_since(std::time::UNIX_EPOCH).ok()?;
    u64::try_from(duration.as_millis()).ok()
}

/// Last-modified time for `path`, delegated to `upeg-runtime`'s native mtime
/// probe.
///
/// `upeg_runtime::persistence` (the module this calls into) is itself
/// `cfg(not(target_arch = "wasm32"))` — see its module doc: "Wasm builds
/// persist through `localStorage`, not the filesystem, and never compile
/// this module." There is no `~/.upeg` shared filesystem in a PWA for
/// another surface to have modified, so cross-surface version detection is a
/// native-only concept; this only makes sense to run there.
#[cfg(not(target_arch = "wasm32"))]
fn modified_time(path: Option<&std::path::Path>) -> Option<std::time::SystemTime> {
    upeg_runtime::persistence::modified_time(path?)
}

/// Wasm32 fallback: always "no version". See the native overload's doc for
/// why file-mtime polling has no wasm32 equivalent (no shared filesystem
/// with another surface to poll). Callers already treat `None` as "nothing
/// to report" (`SharedStateVersionsDto`'s fields are optional), so this is a
/// behavior-preserving no-op rather than an invented storage layer.
#[cfg(target_arch = "wasm32")]
fn modified_time(_path: Option<&std::path::Path>) -> Option<std::time::SystemTime> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "upeg-frb-shared-state-{label}-{}",
            std::process::id(),
        ))
    }

    #[test]
    fn file_mtime_converts_to_epoch_millis_version() {
        let path = temp_path("mtime");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, "versioned").expect("write versioned file");

        let version = modified_epoch_millis(Some(path.clone())).expect("mtime version");

        assert!(version > 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn shared_state_versions_carry_tweaks_and_pegboard_versions() {
        let versions = load_shared_state_versions();

        assert!(versions.tweaks.is_none_or(|version| version > 0));
        assert!(versions.pegboard.is_none_or(|version| version > 0));
    }
}
