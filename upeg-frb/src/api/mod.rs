//! flutter_rust_bridge API surface: every function the Flutter app can
//! call, grouped by concern. All return types are `#[frb(non_opaque)]`
//! DTOs with owned fields so Dart sees plain classes, never opaque
//! handles. Rust stays the single source of truth — the Dart layer
//! delegates here rather than re-deriving domain logic.
//!
//! - Lifecycle: `boot` (instance lock + host start/stop), `shared_state`
//!   (cross-surface state versions), `backup` (export/import).
//! - Discovery + dispatch: `tools` (toolkit/tool listing + `dispatch_tool`),
//!   `dispatch_stream` (live output + cancel for one run),
//!   `palette` (⌘K search ranking).
//! - Pegboard: `pegboard` (boards/layouts/tags/pins/move), `pin_activation`.
//! - Settings: `tweaks` (theme/accent/show_holes/local-host switch),
//!   `i18n` (locale).
//! - Host status: `pause`, `status`, `events` (Stream broadcasters).
//! - Integration: `deep_link` (`upeg://` parse/encode), `embed`,
//!   `keyboard` (stroke → command resolution), `memos`, `last_outcomes`
//!   (persisted per-pin dispatch results).

pub mod backup;
pub mod board_details;
pub mod boot;
pub mod capability;
pub mod deep_link;
pub mod diagnostics;
pub mod dispatch_stream;
pub mod embed;
pub mod events;
pub mod i18n;
pub mod keyboard;
pub mod last_outcomes;
pub mod memos;
pub mod palette;
pub mod pause;
pub mod pegboard;
pub mod pin_activation;
pub mod project;
pub mod readiness;
pub mod shared_state;
pub mod status;
pub mod tools;
pub mod tweaks;
pub mod webview;

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod test_support {
    use std::sync::{Mutex, OnceLock};

    /// Serializes tests that change the HTTP host's process-local pause flag
    /// with tests that exercise its real router.
    pub(crate) fn host_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn storage_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    pub(crate) fn run_with_storage_backup<R>(f: impl FnOnce() -> R) -> R {
        let _guard = storage_lock().lock().expect("storage test lock");
        let original = crate::api::backup::export_backup().expect("export original backup");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        crate::api::backup::import_backup(original).expect("restore original backup");
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}
