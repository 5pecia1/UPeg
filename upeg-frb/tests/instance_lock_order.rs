//! Boot-order contract of `api::boot::init_app`: the desktop instance lock is
//! taken before Toolkit registration, so a second launch that loses the lock
//! registers no Toolkits.
//!
//! This is its own integration-test binary on purpose. The Toolbox (its
//! registered Tools and Toolkits), the storage lease and `UPEG_HOME` are
//! process-global, so a binary shared with other tests could not tell
//! "`init_app` registered nothing" from "something else registered it first".

// The ordering under test is platform-independent; the scratch-home setup
// below is only exercised on Unix.
#![cfg(unix)]
#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration assertions"
)]

use upeg_frb::api::boot::{FrbError, init_app};

#[test]
#[allow(
    unsafe_code,
    reason = "std::env::set_var is unsafe since edition 2024; this binary holds a single test, \
              so nothing else reads or writes the environment while it runs"
)]
fn init_app_returns_already_running_before_registering_toolkits() {
    let scratch = tempfile::tempdir().expect("scratch UPEG_HOME");
    // Canonical so no ancestor is a symlink (`/var` on macOS), which the
    // storage resolver rejects.
    let home = scratch
        .path()
        .canonicalize()
        .expect("canonical scratch home");
    unsafe {
        std::env::set_var(upeg_core::paths::env::UPEG_HOME, &home);
    }

    // Another live instance already owns the lock; this very process stands
    // in for it, so the liveness probe sees a running pid.
    let lock = upeg_core::paths::desktop_lock_path().expect("lock path under the scratch home");
    std::fs::create_dir_all(lock.parent().expect("lock parent")).expect("create runtime dir");
    std::fs::write(&lock, format!("{}\n", std::process::id())).expect("write live lock");

    let tools_before = upeg_runtime::toolbox_tools().count();
    let toolkits_before = upeg_runtime::toolbox_toolkits().count();

    let error = init_app().expect_err("a live lock owner must stop the boot");

    assert!(
        matches!(error, FrbError::AlreadyRunning { pid } if pid == std::process::id()),
        "expected AlreadyRunning for the live owner, got {error:?}"
    );
    assert_eq!(
        upeg_runtime::toolbox_tools().count(),
        tools_before,
        "the losing instance must not register tools"
    );
    assert_eq!(
        upeg_runtime::toolbox_toolkits().count(),
        toolkits_before,
        "the losing instance must not register Toolkits"
    );
}
