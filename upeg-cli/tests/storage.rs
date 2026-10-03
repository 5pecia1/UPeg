#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration assertions"
)]

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Fixture {
    temp: tempfile::TempDir,
    home: PathBuf,
    source: PathBuf,
    target: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let source = home.join(".upeg");
        let target = temp.path().join("target");
        Self {
            temp,
            home,
            source,
            target,
        }
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_upeg"));
        command
            .env_clear()
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .current_dir(self.temp.path());
        command
    }
    fn legacy(&self) {
        std::fs::create_dir_all(self.source.join("toolkits")).unwrap();
        std::fs::write(
            self.source.join(".upeg-tweaks.json"),
            b"{\"theme\":\"Dark\"}",
        )
        .unwrap();
        std::fs::write(
            self.source.join("credentials.json"),
            b"private-literal-credential-fixture",
        )
        .unwrap();
        std::fs::write(
            self.source.join("toolkits/local.toml"),
            b"intentionally invalid toolkit fixture",
        )
        .unwrap();
    }
    fn plan(&self) -> PathBuf {
        let output = self.temp.path().join("plan.json");
        success(
            self.command()
                .args(["storage", "plan", "--source"])
                .arg(&self.source)
                .arg("--target")
                .arg(&self.target)
                .arg("--output")
                .arg(&output)
                .arg("--json"),
        );
        output
    }
    fn apply(&self, plan: &Path) {
        success(
            self.command()
                .args(["storage", "apply", "--plan"])
                .arg(plan)
                .args(["--yes", "--quiesced", "--json"]),
        );
    }
}

fn success(command: &mut Command) -> Value {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn failure(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(!output.status.success());
    output
}

#[test]
fn storage_cli_default_is_split_readonly_then_startup_pins_marker() {
    let fixture = Fixture::new();
    let paths = success(fixture.command().args(["paths", "--json"]));
    assert_eq!(paths["layout"], "split-v2");
    assert_eq!(
        paths["user"]["config_dir"],
        json!(fixture.source.join("config"))
    );
    assert_eq!(
        paths["user"]["toolkit_packs_dir"],
        json!(fixture.source.join("cache/toolkit-packs"))
    );
    let status = success(fixture.command().args(["storage", "status", "--json"]));
    assert_eq!(status["root"], json!(fixture.source));
    assert!(!fixture.source.exists());
    let output = fixture
        .command()
        .args(["tool", "list", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(
            &std::fs::read(fixture.source.join("storage-layout.json")).unwrap()
        )
        .unwrap(),
        json!({"schema_version":1,"app":"upeg","layout":"split-v2","root":fixture.source})
    );
    std::fs::write(fixture.source.join("credentials.json"), b"later-old-file").unwrap();
    assert_eq!(
        success(fixture.command().args(["paths", "--json"]))["layout"],
        "split-v2"
    );
}

#[test]
fn storage_cli_explicit_empty_root_stays_legacy_and_default_explicit_root_is_split() {
    let fixture = Fixture::new();
    let custom = fixture.temp.path().join("explicit");
    let paths = success(
        fixture
            .command()
            .env("UPEG_HOME", &custom)
            .args(["paths", "--json"]),
    );
    assert_eq!(paths["layout"], "legacy-v1");
    assert_eq!(paths["user"]["store"], json!(custom.join("upeg.db")));
    let paths = success(
        fixture
            .command()
            .env("UPEG_HOME", &fixture.source)
            .args(["paths", "--json"]),
    );
    assert_eq!(paths["layout"], "split-v2");
    assert!(!custom.exists());
}

#[test]
fn storage_cli_migrates_literal_files_keeps_projects_and_old_global_excluded() {
    let fixture = Fixture::new();
    fixture.legacy();
    for project in ["A", "B"] {
        let marker = fixture.home.join(project).join(".upeg");
        std::fs::create_dir_all(&marker).unwrap();
        std::fs::write(
            marker.join("project.toml"),
            format!("literal project {project}\n"),
        )
        .unwrap();
    }
    let old = success(fixture.command().args(["paths", "--json"]));
    assert_eq!(old["layout"], "legacy-v1");
    let plan = fixture.plan();
    assert!(!fixture.target.exists());
    assert!(!fixture.source.join("upeg.db").exists());
    fixture.apply(&plan);
    let paths = success(
        fixture
            .command()
            .current_dir(&fixture.home)
            .args(["paths", "--json"]),
    );
    assert_eq!(paths["layout"], "split-v2");
    assert_eq!(
        paths["user"]["config_dir"],
        json!(fixture.target.join("config"))
    );
    assert!(paths["project"].is_null());
    for project in ["A", "B"] {
        assert_eq!(
            std::fs::read(fixture.home.join(project).join(".upeg/project.toml")).unwrap(),
            format!("literal project {project}\n").as_bytes()
        );
    }
    assert_eq!(
        std::fs::read(fixture.target.join("config/credentials.json")).unwrap(),
        b"private-literal-credential-fixture"
    );
    success(
        fixture
            .command()
            .args(["storage", "verify", "--target"])
            .arg(&fixture.target)
            .arg("--json"),
    );
    success(
        fixture
            .command()
            .args(["storage", "rollback", "--target"])
            .arg(&fixture.target)
            .args(["--yes", "--quiesced", "--json"]),
    );
    assert_eq!(
        success(fixture.command().args(["paths", "--json"]))["layout"],
        "legacy-v1"
    );
    failure(
        fixture
            .command()
            .env("UPEG_HOME", &fixture.target)
            .args(["paths", "--json"]),
    );
}

#[test]
fn storage_cli_child_connection_anchors_storage_root_not_config_directory() {
    let fixture = Fixture::new();
    let output = fixture
        .command()
        .args(["board", "dev", "describe", "--description", "child fixture"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let connection = success(fixture.command().args(["board", "dev", "connect"]));
    let environment = &connection["mcpServers"]["upeg-dev"]["env"];
    assert_eq!(environment["UPEG_HOME"], json!(fixture.source));
    let paths = success(
        fixture
            .command()
            .env("UPEG_HOME", environment["UPEG_HOME"].as_str().unwrap())
            .args(["paths", "--json"]),
    );
    assert_eq!(
        paths["user"]["config_dir"],
        json!(fixture.source.join("config"))
    );
    assert_eq!(
        paths["user"]["store"],
        json!(fixture.source.join("data/upeg.db"))
    );
}

#[cfg(unix)]
#[test]
fn storage_cli_rebinds_only_literal_ecosystem_receipt_paths() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new();
    fixture.legacy();
    let toolkit = fixture.source.join("toolkits/ecosystem.toml");
    std::fs::write(&toolkit, b"abc").unwrap();
    std::fs::set_permissions(&toolkit, std::fs::Permissions::from_mode(0o644)).unwrap();
    let prefix = fixture.temp.path().join("prefix");
    let receipt_path = prefix.join("share/ecosystem/install.json");
    std::fs::create_dir_all(receipt_path.parent().unwrap()).unwrap();
    let fingerprint = json!({"sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad","mode":420});
    let before = json!({"managed_by":"ecosystem-installer-v3","prefix":prefix,"connections":["upeg","codex"],"upeg_toolkit":toolkit,
        "files":{toolkit.to_str().unwrap():fingerprint,"unrelated":{"sha256":"keep","mode":493}},"extra":{"keep":true}});
    let original_bytes = serde_json::to_vec(&before).unwrap();
    std::fs::write(&receipt_path, &original_bytes).unwrap();
    let plan = fixture.temp.path().join("receipt-plan.json");
    success(
        fixture
            .command()
            .args(["storage", "plan", "--source"])
            .arg(&fixture.source)
            .arg("--target")
            .arg(&fixture.target)
            .arg("--ecosystem-prefix")
            .arg(&prefix)
            .arg("--output")
            .arg(&plan)
            .arg("--json"),
    );
    fixture.apply(&plan);
    let new_toolkit = fixture.target.join("data/toolkits/ecosystem.toml");
    let mut expected = before;
    expected["upeg_toolkit"] = json!(new_toolkit);
    expected["files"]
        .as_object_mut()
        .unwrap()
        .remove(toolkit.to_str().unwrap());
    expected["files"][new_toolkit.to_str().unwrap()] = fingerprint;
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(&receipt_path).unwrap()).unwrap(),
        expected
    );
    assert_eq!(std::fs::read(&new_toolkit).unwrap(), b"abc");
    success(
        fixture
            .command()
            .args(["storage", "rollback", "--target"])
            .arg(&fixture.target)
            .args(["--yes", "--quiesced", "--json"]),
    );
    assert_eq!(std::fs::read(receipt_path).unwrap(), original_bytes);
    assert_eq!(std::fs::read(toolkit).unwrap(), b"abc");
}

#[test]
fn storage_cli_marker_errors_fail_before_loading_sources_or_creating_database() {
    let fixture = Fixture::new();
    fixture.legacy();
    std::fs::write(fixture.source.join("storage-layout.json"), b"broken-json").unwrap();
    let output = failure(fixture.command().args(["tool", "list"]));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("invalid storage marker"));
    assert!(!error.contains("loaded"));
    assert!(!fixture.source.join("upeg.db").exists());
}

#[test]
fn storage_cli_invalid_profile_home_never_uses_working_directory() {
    let fixture = Fixture::new();
    for invalid in ["", "relative-home"] {
        let output = failure(
            fixture
                .command()
                .env("HOME", invalid)
                .env("USERPROFILE", invalid)
                .args(["tool", "list"]),
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("unsafe storage path"));
        assert!(!fixture.temp.path().join("upeg.db").exists());
        assert!(!fixture.source.exists());
        assert!(!fixture.temp.path().join(".storage.lock").exists());
    }
}

#[test]
fn storage_cli_override_binding_changes_and_unacknowledged_apply_are_refused() {
    let fixture = Fixture::new();
    fixture.legacy();
    let plan = fixture.plan();
    failure(
        fixture
            .command()
            .args(["storage", "apply", "--plan"])
            .arg(&plan)
            .arg("--yes"),
    );
    failure(
        fixture
            .command()
            .env("UPEG_TOOLKITS_DIR", "external")
            .args(["storage", "apply", "--plan"])
            .arg(&plan)
            .args(["--yes", "--quiesced"]),
    );
    assert!(!fixture.target.exists());
    let external = fixture.temp.path().join("external");
    std::fs::create_dir_all(&external).unwrap();
    std::fs::write(external.join("tool.toml"), b"untouched external toolkit").unwrap();
    let report = success(
        fixture
            .command()
            .env("UPEG_TOOLKITS_DIR", &external)
            .args(["storage", "plan", "--source"])
            .arg(&fixture.source)
            .arg("--target")
            .arg(&fixture.target)
            .arg("--json"),
    );
    assert!(
        report["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| !entry["source"].as_str().unwrap().starts_with("toolkits"))
    );
    assert_eq!(
        std::fs::read(external.join("tool.toml")).unwrap(),
        b"untouched external toolkit"
    );
}

#[test]
fn storage_cli_status_reports_source_drift_and_verify_fails_after_cutover() {
    let fixture = Fixture::new();
    fixture.legacy();
    let plan = fixture.plan();
    fixture.apply(&plan);
    let clean = success(fixture.command().args(["storage", "status", "--json"]));
    assert_eq!(clean["source_modified_after_activation"], json!([]));
    std::fs::write(
        fixture.source.join("credentials.json"),
        b"tampered-after-cutover",
    )
    .unwrap();
    let drifted = success(fixture.command().args(["storage", "status", "--json"]));
    assert_eq!(
        drifted["source_modified_after_activation"],
        json!([fixture.source.join("credentials.json")])
    );
    let output = failure(
        fixture
            .command()
            .args(["storage", "verify", "--target"])
            .arg(&fixture.target),
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("source modified after activation"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
