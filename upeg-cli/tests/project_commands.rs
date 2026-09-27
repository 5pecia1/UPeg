#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration assertions"
)]

use std::path::Path;
use std::process::{Command, Output};

fn run(home: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_upeg"))
        .current_dir(cwd)
        .env("UPEG_HOME", home)
        .env("UPEG_TOOLKITS_DIR", home.join("toolkits"))
        .env("UPEG_PROJECT_MANIFEST_PATH", "off")
        .env("UPEG_WASM_DIR", home.join("wasm"))
        .env("UPEG_MCP_IMPORTS_DIR", home.join("imports"))
        .args(args)
        .output()
        .expect("run upeg")
}

fn toolkit(path: &Path, command: &str) {
    std::fs::write(path, format!("id = 'project_cli_test'\n[[tools]]\nid = 'run'\npegboard_units = 'U1'\ninvoker = 'External'\ncommand = '{command}'\n")).expect("write Toolkit");
}

#[test]
fn cli_initializes_validates_and_resolves_a_duplicate_tool_choice() {
    let temp = tempfile::tempdir().expect("isolated fixture");
    let root = temp.path().join("project");
    let home = temp.path().join("home");
    std::fs::create_dir_all(&root).expect("project root");
    std::fs::create_dir_all(home.join("toolkits")).expect("global toolkits");
    let root_arg = root.to_str().expect("UTF-8 root");
    let initialized = run(
        &home,
        &root,
        &["project", "init", root_arg, "--name", "CLI project"],
    );
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    assert!(root.join(".upeg/project.toml").is_file());
    assert!(root.join(".upeg/toolkits").is_dir());

    toolkit(&home.join("toolkits/global.toml"), "echo");
    toolkit(&root.join(".upeg/toolkits/local.toml"), "echo");
    let unresolved = run(
        &home,
        &root,
        &["--project", root_arg, "project", "validate", "--json"],
    );
    assert!(
        !unresolved.status.success(),
        "unresolved conflict must fail validation"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&unresolved.stdout).expect("JSON report");
    assert_eq!(report["conflicts"][0]["tool_id"], "project_cli_test.run");
    assert_eq!(report["conflicts"][0]["blocked"], true);
    let blocked_call = run(
        &home,
        &root,
        &[
            "--project",
            root_arg,
            "call",
            "project_cli_test.run",
            "--json",
        ],
    );
    assert!(!blocked_call.status.success());
    let blocked_text = format!(
        "{}{}",
        String::from_utf8_lossy(&blocked_call.stdout),
        String::from_utf8_lossy(&blocked_call.stderr)
    );
    assert!(
        blocked_text.contains("project_tool_conflict"),
        "{blocked_text}"
    );

    let chosen = run(
        &home,
        &root,
        &[
            "--project",
            root_arg,
            "project",
            "choose",
            "project_cli_test.run",
            "project",
        ],
    );
    assert!(
        chosen.status.success(),
        "{}",
        String::from_utf8_lossy(&chosen.stderr)
    );
    let resolved = run(
        &home,
        &root,
        &["--project", root_arg, "project", "validate", "--json"],
    );
    assert!(
        resolved.status.success(),
        "{}",
        String::from_utf8_lossy(&resolved.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&resolved.stdout).expect("JSON report");
    assert_eq!(report["conflicts"][0]["choice"], "project");
    assert_eq!(report["conflicts"][0]["blocked"], false);
}
