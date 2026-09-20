#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::print_stdout,
    clippy::string_add,
    clippy::manual_let_else,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! `upeg plugin install` / `upeg plugin list` end-to-end coverage.
//!
//! Feature-gated: both subcommands load plugin manifests through
//! `upeg-wasm`, same as `upeg wasm load`.
#![cfg(feature = "wasm-plugin")]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Same checked-in fixture `upeg-wasm`'s own e2e test uses — declares
/// `test.wasm.echo` / `test.wasm.shout` under toolkit `test`.
static WASM_FIXTURE: &[u8] = include_bytes!("../../upeg-wasm/tests/fixtures/test_plugin.wasm");

const fn upeg_bin() -> &'static str {
    env!("CARGO_BIN_EXE_upeg")
}

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// One isolated `$UPEG_WASM_DIR` + `$HOME` pair per test — matches the
/// existing `three_sources_e2e.rs`/`typed_inputs_contracts.rs` pattern
/// of driving the real `upeg` binary via env var overrides instead of
/// mutating process-global env inside the test process itself.
struct Fixture {
    root: PathBuf,
    wasm_dir: PathBuf,
    home: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "upeg_plugin_install_{label}_{}_{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let wasm_dir = root.join("wasm");
        let home = root.join("home");
        std::fs::create_dir_all(&wasm_dir).expect("create wasm dir");
        std::fs::create_dir_all(&home).expect("create home dir");
        Self {
            root,
            wasm_dir,
            home,
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(upeg_bin())
            .args(args)
            .env("UPEG_WASM_DIR", &self.wasm_dir)
            .env("HOME", &self.home)
            // The repo root carries a dogfood `upeg.toml`; without this the
            // spawned binary would auto-detect it and register `dev.*`.
            .env(
                upeg_sources::project::PROJECT_MANIFEST_PATH_ENV,
                upeg_sources::project::PROJECT_MANIFEST_OVERRIDE_OFF,
            )
            .output()
            .unwrap_or_else(|err| panic!("run upeg {args:?}: {err}"))
    }

    fn write_source_file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(name);
        std::fs::write(&path, bytes).expect("write source file");
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn assert_failure(output: &Output, context: &str) {
    assert!(
        !output.status.success(),
        "{context} unexpectedly succeeded\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout),
    );
}

#[test]
fn a_valid_plugin_is_copied_after_validation_and_prints_its_tool_ids() {
    let fixture = Fixture::new("valid");
    let source = fixture.write_source_file("test_plugin.wasm", WASM_FIXTURE);

    let output = fixture.run(&["plugin", "install", source.to_str().unwrap()]);
    assert_success(&output, "plugin install (valid fixture)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test.wasm.echo"));
    assert!(stdout.contains("test.wasm.shout"));

    let installed = fixture.wasm_dir.join("test_plugin.wasm");
    assert!(installed.exists());
    assert_eq!(std::fs::read(&installed).unwrap(), WASM_FIXTURE);
}

#[test]
fn corrupted_bytes_fail_validation_and_are_not_copied() {
    let fixture = Fixture::new("invalid");
    let source = fixture.write_source_file("garbage.wasm", b"not a real wasm module");

    let output = fixture.run(&["plugin", "install", source.to_str().unwrap()]);
    assert_failure(&output, "plugin install (garbage bytes)");
    assert!(!fixture.wasm_dir.join("garbage.wasm").exists());
    // Directory must stay empty — a failed validation is not a partial install.
    let remaining: Vec<_> = std::fs::read_dir(&fixture.wasm_dir)
        .expect("read wasm dir")
        .collect();
    assert!(
        remaining.is_empty(),
        "wasm dir should stay empty on failure"
    );
}

#[test]
fn an_existing_file_with_different_content_is_refused_without_force() {
    let fixture = Fixture::new("conflict");
    std::fs::write(
        fixture.wasm_dir.join("test_plugin.wasm"),
        b"pre-existing different bytes",
    )
    .expect("seed conflicting file");
    let source = fixture.write_source_file("test_plugin.wasm", WASM_FIXTURE);

    let output = fixture.run(&["plugin", "install", source.to_str().unwrap()]);
    assert_failure(
        &output,
        "plugin install (conflicting existing file, no --force)",
    );
    assert_eq!(
        std::fs::read(fixture.wasm_dir.join("test_plugin.wasm")).unwrap(),
        b"pre-existing different bytes"
    );

    let forced = fixture.run(&["plugin", "install", "--force", source.to_str().unwrap()]);
    assert_success(
        &forced,
        "plugin install --force (conflicting existing file)",
    );
    assert_eq!(
        std::fs::read(fixture.wasm_dir.join("test_plugin.wasm")).unwrap(),
        WASM_FIXTURE
    );
}

#[test]
fn list_prints_a_friendly_note_when_empty_and_shows_tools_after_install() {
    let fixture = Fixture::new("list");

    let empty = fixture.run(&["plugin", "list"]);
    assert_success(&empty, "plugin list (empty)");
    let empty_stdout = String::from_utf8_lossy(&empty.stdout);
    assert!(empty_stdout.contains("no plugins installed"));

    let source = fixture.write_source_file("test_plugin.wasm", WASM_FIXTURE);
    let install = fixture.run(&["plugin", "install", source.to_str().unwrap()]);
    assert_success(&install, "plugin install (before list)");

    let listed = fixture.run(&["plugin", "list"]);
    assert_success(&listed, "plugin list (after install)");
    let stdout = String::from_utf8_lossy(&listed.stdout);
    assert!(stdout.contains("test_plugin.wasm"));
    assert!(stdout.contains("test"));
    assert!(stdout.contains("test.wasm.echo"));
    assert!(stdout.contains("test.wasm.shout"));
}
