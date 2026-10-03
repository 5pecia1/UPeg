#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Golden test for the shared storage-marker/pending contract.
//!
//! `upeg-core/tests/fixtures/storage-contract/v1/` is the canonical,
//! cross-repo fixture set for the on-disk contract that decides which
//! physical directory an app treats as its storage root
//! (`storage-layout.json` / `.storage-migration.json`). This crate owns the
//! fixtures; other ecosystem repos vendor the same directory and assert the
//! same outcomes against their own reader/writer. This test is UPeg's own
//! ground truth: it loads `cases.json` and drives the real
//! `upeg_core::paths` resolver against fixtures materialized on disk, so the
//! fixtures and the code they describe can never silently drift apart.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use upeg_core::paths::{self, Platform, StoragePathError};

const FIXTURES_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/storage-contract/v1"
);

fn read_manifest() -> Value {
    let bytes = fs::read(Path::new(FIXTURES_ROOT).join("cases.json"))
        .expect("cases.json must exist under the storage-contract fixture directory");
    serde_json::from_slice(&bytes).expect("cases.json must be valid JSON")
}

fn read_fixture_text(relative: &str) -> String {
    fs::read_to_string(Path::new(FIXTURES_ROOT).join(relative))
        .unwrap_or_else(|error| panic!("failed to read fixture {relative}: {error}"))
}

fn substitute(text: &str, root: &Path, target: Option<&Path>) -> String {
    let mut text = text.replace(
        "ROOT",
        root.to_str().expect("temp root must be valid UTF-8"),
    );
    if let Some(target) = target {
        text = text.replace(
            "TARGET",
            target.to_str().expect("temp target must be valid UTF-8"),
        );
    }
    text
}

fn write_marker_kind(dir: &Path, kind: &str, bytes: &[u8]) {
    let name = match kind {
        "marker" => paths::STORAGE_MARKER,
        "pending" => paths::MIGRATION_PENDING,
        other => panic!("unknown fixture kind: {other}"),
    };
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(name), bytes).unwrap();
}

fn error_kind(error: &StoragePathError) -> &'static str {
    match error {
        StoragePathError::Marker(_) => "marker",
        StoragePathError::Pending(_) => "pending",
        StoragePathError::Ambiguous(_, _) => "ambiguous",
        StoragePathError::Busy(_) => "busy",
        StoragePathError::Unavailable => "unavailable",
        StoragePathError::Unsafe(_) => "unsafe",
        StoragePathError::Io { .. } => "io",
        StoragePathError::MissingJournal(_) => "missing_journal",
    }
}

fn current_platform_applies(case: &Value) -> bool {
    let Some(platforms) = case.get("platforms").and_then(Value::as_array) else {
        return true;
    };
    let current = if cfg!(windows) { "windows" } else { "unix" };
    platforms
        .iter()
        .filter_map(Value::as_str)
        .any(|platform| platform == current)
}

#[test]
fn storage_contract_cases_match_upeg_core_paths_resolution() {
    let manifest = read_manifest();
    let cases = manifest["cases"]
        .as_array()
        .expect("cases.json must have a top-level \"cases\" array");
    assert!(
        !cases.is_empty(),
        "the contract fixture set must not be empty"
    );

    for case in cases {
        let id = case["id"].as_str().expect("each case needs an \"id\"");
        if !current_platform_applies(case) {
            continue;
        }

        let unique = uuid::Uuid::new_v4();
        let root: PathBuf =
            std::env::temp_dir().join(format!("upeg-storage-contract-{unique}-root"));
        let target: PathBuf =
            std::env::temp_dir().join(format!("upeg-storage-contract-{unique}-target"));
        fs::create_dir_all(&root).unwrap_or_else(|error| panic!("[{id}] create root: {error}"));

        let target_used = case.get("target_marker").is_some();
        if target_used {
            fs::create_dir_all(&target)
                .unwrap_or_else(|error| panic!("[{id}] create target: {error}"));
        }

        if let Some(root_marker) = case.get("root_marker") {
            let kind = root_marker["kind"].as_str().unwrap();
            let file = root_marker["file"].as_str().unwrap();
            let raw = read_fixture_text(file);
            let substituted = substitute(&raw, &root, target_used.then_some(target.as_path()));
            write_marker_kind(&root, kind, substituted.as_bytes());
        }
        if let Some(target_marker) = case.get("target_marker") {
            let kind = target_marker["kind"].as_str().unwrap();
            let file = target_marker["file"].as_str().unwrap();
            let raw = read_fixture_text(file);
            let substituted = substitute(&raw, &root, Some(&target));
            write_marker_kind(&target, kind, substituted.as_bytes());
        }
        if let Some(root_content) = case.get("root_content").and_then(Value::as_array) {
            for relative in root_content.iter().filter_map(Value::as_str) {
                let path = root.join(relative);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                fs::write(&path, b"contract fixture content").unwrap();
            }
        }

        let result = paths::resolve_storage_with_lookup(
            |key| (key == "UPEG_HOME").then(|| root.clone()),
            Platform::Unix,
            paths::inspect_storage,
        );

        let expect = &case["expect"];
        match expect["outcome"].as_str().unwrap() {
            "ok" => {
                let location =
                    result.unwrap_or_else(|error| panic!("[{id}] expected Ok, got {error:?}"));
                let expected_layout = expect["layout"].as_str().unwrap();
                assert_eq!(
                    location.layout.as_str(),
                    expected_layout,
                    "[{id}] unexpected resolved layout"
                );
                let expected_root = match expect["resolved_root"].as_str().unwrap() {
                    "ROOT" => &root,
                    "TARGET" => &target,
                    other => panic!("[{id}] unknown resolved_root token: {other}"),
                };
                assert_eq!(
                    &location.root, expected_root,
                    "[{id}] unexpected resolved root"
                );
            }
            "error" => {
                let error = result
                    .err()
                    .unwrap_or_else(|| panic!("[{id}] expected Err, got Ok"));
                let expected_kind = expect["error_kind"].as_str().unwrap();
                assert_eq!(
                    error_kind(&error),
                    expected_kind,
                    "[{id}] unexpected error kind, got {error:?}"
                );
            }
            other => panic!("[{id}] unknown expected outcome: {other}"),
        }

        let _ = fs::remove_dir_all(&root);
        if target_used {
            let _ = fs::remove_dir_all(&target);
        }
    }
}
