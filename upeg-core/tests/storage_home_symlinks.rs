#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! HOME-ancestry vs. app-root symlink handling (see `storage_paths.rs`'s
//! `canonicalized_profile_home` and `check_storage_path` doc comments).
//!
//! The storage guard protects the app root (`~/.upeg`) and everything under
//! it, not HOME or HOME's ancestors: HOME (or a directory above it) is
//! routinely a symlink on real systems — Fedora Silverblue/Kinoite/Bazzite
//! (`/home -> /var/home`), macOS temp homes (`/var -> /private/var`), FreeBSD
//! (`/home -> /usr/home`). These tests exercise the real filesystem (unlike
//! `paths.rs`'s inline unit tests, which only simulate lookups) so they cover
//! the actual `inspect_storage`/`check_storage_path` fs calls end to end.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::symlink;

use upeg_core::paths::{
    Platform, StorageLayout, StoragePathError, env, inspect_storage, resolve_storage_with_lookup,
};

#[test]
fn home_reached_through_a_symlinked_ancestor_resolves_transparently() {
    let tmp = tempfile::tempdir().unwrap();
    let real_home_root = tmp.path().join("var-home");
    fs::create_dir(&real_home_root).unwrap();
    let home_alias = tmp.path().join("home");
    symlink(&real_home_root, &home_alias).unwrap();
    let home = home_alias.join("user");
    fs::create_dir(&home).unwrap();

    let location = resolve_storage_with_lookup(
        |name| (name == env::HOME).then(|| home.clone()),
        Platform::current(),
        inspect_storage,
    )
    .expect("a symlinked HOME ancestor must still resolve");

    let expected_root = fs::canonicalize(&home).unwrap().join(".upeg");
    assert_eq!(location.root, expected_root);
    assert_eq!(location.layout, StorageLayout::SplitV2);
}

#[test]
fn a_symlink_at_the_app_root_itself_is_still_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    fs::create_dir(&home).unwrap();
    let actual_storage = tmp.path().join("actual-storage");
    fs::create_dir(&actual_storage).unwrap();
    // Unlike a symlinked HOME ancestor, a symlink at the app root (`~/.upeg`)
    // is exactly what the guard exists to reject.
    symlink(&actual_storage, home.join(".upeg")).unwrap();

    let result = resolve_storage_with_lookup(
        |name| (name == env::HOME).then(|| home.clone()),
        Platform::current(),
        inspect_storage,
    );

    assert!(
        matches!(result, Err(StoragePathError::Unsafe(_))),
        "expected an Unsafe error, got {result:?}"
    );
}
