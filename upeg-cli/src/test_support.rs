//! Test helpers for Controlled Embed backend testing.
//!
//! This module provides utilities for tests that need to manage the
//! global Controlled Embed backend state, particularly when running
//! in parallel.

use std::sync::{Arc, Mutex, OnceLock};

/// Returns a static mutex that tests can use to coordinate exclusive
/// access to the Controlled Embed backend during parallel test execution.
///
/// This prevents race conditions when multiple tests try to install
/// different backend implementations simultaneously.
pub(crate) fn controlled_embed_backend_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// A RAII guard that restores the [`NoopControlledEmbedBackend`] when dropped.
///
/// Install a custom backend before running your test, then let this guard
/// go out of scope to automatically restore the no-op backend. This ensures
/// tests don't leak state into subsequent tests.
pub(crate) struct RestoreNoopControlledEmbedBackend;

impl Drop for RestoreNoopControlledEmbedBackend {
    fn drop(&mut self) {
        upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
            upeg_runtime::controlled_embed::NoopControlledEmbedBackend,
        ));
    }
}

/// Serializes tests that point `UPEG_HOME` at a scratch pegboard store.
/// Every test that mutates the env var must hold this lock for its whole
/// body so parallel tests never observe each other's scratch store.
#[cfg(test)]
pub(crate) fn pegboard_home_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Run `f` with `UPEG_HOME` pointing at a fresh scratch store seeded
/// with the default pegboard state plus whatever `seed` adds. The
/// default boards/pins are kept so concurrently-running tests that read
/// the shared state (`upeg board list`, `/v1/boards`, …) still see the
/// canonical dev/trading/personal boards.
#[cfg(test)]
#[allow(
    unsafe_code,
    reason = "std::env::set_var/remove_var are unsafe since edition 2024; every caller \
              holds `pegboard_home_test_lock` for the whole closure, and the seeded state \
              is a superset of the defaults so concurrent readers keep their invariants"
)]
pub(crate) fn with_seeded_pegboard_home<R>(
    label: &str,
    seed: impl FnOnce(&mut upeg_sources::pegboard::PegboardState),
    f: impl FnOnce() -> R,
) -> R {
    let _guard = pegboard_home_test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let home = std::env::temp_dir().join(format!("upeg-home-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).expect("create scratch UPEG_HOME");

    let mut state = upeg_sources::pegboard::default_state();
    seed(&mut state);
    let store = upeg_sources::pegboard::state_path_from_root(&home);
    upeg_sources::pegboard::save_state_to_path(&store, &state).expect("seed pegboard store");

    let previous = std::env::var_os(upeg_core::paths::env::UPEG_HOME);
    unsafe {
        std::env::set_var(upeg_core::paths::env::UPEG_HOME, &home);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    match previous {
        Some(value) => unsafe {
            std::env::set_var(upeg_core::paths::env::UPEG_HOME, value);
        },
        None => unsafe {
            std::env::remove_var(upeg_core::paths::env::UPEG_HOME);
        },
    }
    let _ = std::fs::remove_dir_all(&home);
    match result {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Run `f` with Project Manifest detection forced ON.
///
/// The repo's own gates run with `UPEG_PROJECT_MANIFEST_PATH=off`
/// (Justfile `hermetic_project_manifest`, scripts/test_baseline.py) so
/// the dogfood `/<repo>/upeg.toml` can never leak `dev.*` into a
/// generated fixture or an assertion about the built-in toolbox. A test
/// that is specifically ABOUT detection has to opt back in — that is
/// what this does, restoring the previous value afterwards.
///
/// Caller must already hold [`pegboard_home_test_lock`] (via
/// [`with_seeded_pegboard_home`]) or another serializing guard: this
/// mutates process-global env.
#[cfg(test)]
#[allow(
    unsafe_code,
    reason = "std::env::set_var/remove_var are unsafe since edition 2024; callers hold the \
              pegboard-home test lock for the whole closure, so no other test observes the swap"
)]
pub(crate) fn with_project_manifest_detection<R>(f: impl FnOnce() -> R) -> R {
    let key = upeg_sources::project::PROJECT_MANIFEST_PATH_ENV;
    let previous = std::env::var_os(key);
    unsafe {
        std::env::remove_var(key);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    match previous {
        Some(value) => unsafe {
            std::env::set_var(key, value);
        },
        None => unsafe {
            std::env::remove_var(key);
        },
    }
    match result {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
