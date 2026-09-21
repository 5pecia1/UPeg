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

//! `upeg plugin new` end-to-end scaffolding coverage.
//!
//! Deliberately NOT feature-gated on `wasm-plugin`: `plugin new` is pure
//! file scaffolding and must keep working under
//! `--no-default-features` builds that skip the wasmtime/extism dep tax.

use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const fn upeg_bin() -> &'static str {
    env!("CARGO_BIN_EXE_upeg")
}

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn scratch_dir(label: &str) -> std::path::PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "upeg_plugin_new_{label}_{}_{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Runs `upeg plugin new` with `HOME` and every runtime-source dir
/// pointed at `isolation_root` (never the real host `~/.upeg`) — `upeg`
/// auto-loads `~/.upeg/wasm/*.wasm` on every startup now that
/// `wasm-plugin` is a default feature, so any invocation of the real
/// binary must isolate these paths, same as `typed_inputs_contracts.rs`/
/// `three_sources_e2e.rs` already do.
fn run_plugin_new(isolation_root: &Path, args: &[&str]) -> Output {
    Command::new(upeg_bin())
        .arg("plugin")
        .arg("new")
        .args(args)
        .env("HOME", isolation_root)
        .env("UPEG_TOOLKITS_DIR", isolation_root.join("no-toolkits"))
        .env("UPEG_WASM_DIR", isolation_root.join("no-wasm"))
        .env("UPEG_MCP_IMPORTS_DIR", isolation_root.join("no-mcp"))
        // The repo root carries a dogfood `upeg.toml`; without this the
        // spawned binary would auto-detect it and register `dev.*`.
        .env(
            upeg_sources::project::PROJECT_MANIFEST_PATH_ENV,
            upeg_sources::project::PROJECT_MANIFEST_OVERRIDE_OFF,
        )
        .output()
        .unwrap_or_else(|err| panic!("run upeg plugin new {args:?}: {err}"))
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

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn the_scaffold_creates_a_cargo_toml_and_lib_rs_with_the_name_substituted() {
    let dir = scratch_dir("basic");
    let output = run_plugin_new(&dir, &["dxtest", "--dir", dir.to_str().unwrap()]);
    assert_success(&output, "plugin new dxtest");

    let crate_dir = dir.join("dxtest");
    let cargo_toml = read(&crate_dir.join("Cargo.toml"));
    let lib_rs = read(&crate_dir.join("src/lib.rs"));

    assert!(cargo_toml.contains("name = \"dxtest\""));
    assert!(cargo_toml.contains("upeg-plugin-api = { git = \"https://github.com/5pecia1/UPeg\" }"));
    assert!(lib_rs.contains("id = \"dxtest.hello\""));
    assert!(lib_rs.contains("toolkit = \"dxtest\""));
    assert!(lib_rs.contains("pub fn dxtest_hello(name: &str) -> String"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cargo build --target wasm32-unknown-unknown --release"));
    assert!(stdout.contains("upeg plugin install"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_local_flag_creates_a_path_dependency_cargo_toml() {
    let dir = scratch_dir("local");
    let output = run_plugin_new(
        &dir,
        &[
            "dxlocal",
            "--dir",
            dir.to_str().unwrap(),
            "--local",
            "/workspaces/UPeg",
        ],
    );
    assert_success(&output, "plugin new dxlocal --local");

    let cargo_toml = read(&dir.join("dxlocal/Cargo.toml"));
    assert!(
        cargo_toml.contains("upeg-plugin-api = { path = \"/workspaces/UPeg/upeg-plugin-api\" }")
    );
    assert!(!cargo_toml.contains("git ="));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_invalid_name_is_refused() {
    let dir = scratch_dir("invalid_name");
    let output = run_plugin_new(&dir, &["Not-Valid", "--dir", dir.to_str().unwrap()]);
    assert_failure(&output, "plugin new Not-Valid");
    assert!(!dir.join("Not-Valid").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_nonempty_directory_is_refused() {
    let dir = scratch_dir("nonempty");
    let target = dir.join("dxtaken");
    std::fs::create_dir_all(&target).expect("pre-create target dir");
    std::fs::write(target.join("keep.txt"), b"pre-existing").expect("seed file");

    let output = run_plugin_new(&dir, &["dxtaken", "--dir", dir.to_str().unwrap()]);
    assert_failure(&output, "plugin new dxtaken (nonempty target)");
    // The pre-existing file must survive untouched.
    assert!(target.join("keep.txt").exists());
    assert!(!target.join("Cargo.toml").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_directory_can_be_scaffolded_again() {
    let dir = scratch_dir("empty_target");
    let target = dir.join("dxempty");
    std::fs::create_dir_all(&target).expect("pre-create empty target dir");

    let output = run_plugin_new(&dir, &["dxempty", "--dir", dir.to_str().unwrap()]);
    assert_success(&output, "plugin new dxempty (pre-existing empty dir)");
    assert!(target.join("Cargo.toml").exists());

    let _ = std::fs::remove_dir_all(&dir);
}
