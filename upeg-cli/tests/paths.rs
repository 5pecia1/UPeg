#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration assertions"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};
use upeg_core::{BoardKey, BoardStoreKey, ProjectBoardNamespace, ProjectRoot};

struct Fixture {
    temp: tempfile::TempDir,
    home: PathBuf,
    user: PathBuf,
    cwd: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("fixture");
        let root = temp.path().canonicalize().expect("canonical fixture");
        let home = root.join("home");
        let user = root.join("user-storage");
        let cwd = home.join("work");
        std::fs::create_dir_all(&cwd).expect("cwd");
        Self {
            temp,
            home,
            user,
            cwd,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_upeg"));
        command
            .env_clear()
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("APPDATA", self.home.join("AppData/Roaming"))
            .env("UPEG_HOME", &self.user);
        command
    }

    fn project(root: &Path) {
        std::fs::create_dir_all(root.join(".upeg/toolkits")).expect("marker");
        std::fs::write(root.join(".upeg/project.toml"), "[[invalid project TOML")
            .expect("project TOML");
        std::fs::write(
            root.join(".upeg/toolkits/broken.toml"),
            "[[invalid Toolkit TOML",
        )
        .expect("Toolkit TOML");
    }

    fn read_only(&self, command: &mut Command) -> Output {
        let before = tree(self.temp.path());
        let output = command.output().expect("run upeg");
        let after = tree(self.temp.path());
        let changed: Vec<_> = before
            .keys()
            .chain(after.keys())
            .filter(|path| before.get(*path) != after.get(*path))
            .collect();
        assert!(
            after == before,
            "paths must not mutate the fixture; status={:?}; changed={changed:?}; stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn json(&self, command: &mut Command) -> Value {
        let output = self.read_only(command.args(["paths", "--json"]));
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
        serde_json::from_slice(&output.stdout).expect("pure JSON stdout")
    }
}

fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, directory: &Path, entries: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(directory).expect("read fixture") {
            let entry = entry.expect("entry");
            let path = entry.path();
            let relative = path.strip_prefix(root).expect("relative").to_path_buf();
            if entry.file_type().expect("type").is_dir() {
                entries.insert(relative, None);
                visit(root, &path, entries);
            } else {
                entries.insert(relative, Some(std::fs::read(&path).expect("fixture bytes")));
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn expected_project(root: &Path, origin: &str) -> Value {
    let project = ProjectRoot::new(root).expect("canonical project");
    json!({
        "root": project.as_path(),
        "marker": project.marker_dir(),
        "config": project.config_path(),
        "toolkits": project.toolkits_dir(),
        "namespace": ProjectBoardNamespace::for_project_root(project.as_path()).as_str(),
        "origin": origin,
    })
}

#[test]
fn paths_report_legacy_user_and_project_paths_without_loading_or_writing() {
    let fixture = Fixture::new();
    Fixture::project(&fixture.cwd);
    std::fs::create_dir_all(fixture.user.join("toolkits")).expect("global toolkits");
    std::fs::write(
        fixture.user.join("toolkits/broken.toml"),
        "[[invalid global TOML",
    )
    .expect("global TOML");
    std::fs::write(
        fixture.user.join("credentials.json"),
        "private-reference-sentinel",
    )
    .expect("credentials");
    std::fs::write(fixture.user.join("server.json"), "private-token-sentinel").expect("discovery");
    let report = fixture.json(fixture.command().arg("--project").arg(&fixture.cwd));
    let user = &fixture.user;
    assert_eq!(
        report,
        json!({
            "schema_version": 1,
            "layout": "legacy-v1",
            "user": {
                "config_dir": user,
                "data_dir": user,
                "state_dir": user,
                "cache_dir": user,
                "runtime_dir": user,
                "toolkits_dir": user.join("toolkits"),
                "wasm_dir": user.join("wasm"),
                "mcp_imports_dir": user.join("mcp-imports"),
                "toolkit_packs_dir": user.join("toolkit-packs"),
                "store": user.join("upeg.db"),
                "tweaks": user.join(".upeg-tweaks.json"),
                "desktop_lock": user.join("desktop.lock"),
                "credentials": user.join("credentials.json"),
                "execution_log": user.join("upeg.db"),
                "server_discovery": user.join("server.json"),
                "http_log": user.join("upeg-http.log"),
            },
            "project": expected_project(&fixture.cwd, "argument"),
        })
    );
    assert!(!report.to_string().contains("private-reference-sentinel"));
    assert!(!report.to_string().contains("private-token-sentinel"));
    assert!(!user.join("upeg.db").exists());
    assert!(!user.join("diagnostics").exists());
}

#[test]
fn explicit_project_beats_environment_selection_and_off() {
    let fixture = Fixture::new();
    Fixture::project(&fixture.cwd);
    let other = fixture.home.join("other");
    Fixture::project(&other);
    for override_value in [other.as_os_str(), std::ffi::OsStr::new("off")] {
        let report = fixture.json(
            fixture
                .command()
                .env("UPEG_PROJECT_MANIFEST_PATH", override_value)
                .arg("--project")
                .arg(&fixture.cwd),
        );
        assert_eq!(
            report["project"],
            expected_project(&fixture.cwd, "argument")
        );
    }
}

#[test]
fn environment_project_accepts_root_or_marker_and_preserves_provenance() {
    let fixture = Fixture::new();
    Fixture::project(&fixture.cwd);
    let other = fixture.temp.path().join("outside-home");
    Fixture::project(&other);
    for override_value in [&other, &other.join(".upeg")] {
        let report = fixture.json(
            fixture
                .command()
                .env("UPEG_PROJECT_MANIFEST_PATH", override_value),
        );
        assert_eq!(report["project"], expected_project(&other, "env_override"));
    }
}

#[test]
fn off_and_missing_environment_project_do_not_fall_back_to_detection() {
    let fixture = Fixture::new();
    Fixture::project(&fixture.cwd);
    for override_value in [PathBuf::from("OFF"), fixture.home.join("missing-project")] {
        let report = fixture.json(
            fixture
                .command()
                .env("UPEG_PROJECT_MANIFEST_PATH", override_value),
        );
        assert!(report["project"].is_null());
    }
}

#[test]
fn within_home_detection_uses_nearest_marker_after_working_directory() {
    let fixture = Fixture::new();
    Fixture::project(&fixture.cwd);
    let nearest = fixture.cwd.join("nested");
    Fixture::project(&nearest);
    let child = nearest.join("child");
    std::fs::create_dir_all(&child).expect("child");
    let report = fixture.json(fixture.command().arg("--working-directory").arg(&child));
    assert_eq!(report["project"], expected_project(&nearest, "detected"));
}

#[test]
fn outside_home_detection_does_not_adopt_an_ancestor() {
    let fixture = Fixture::new();
    let outside = fixture.temp.path().join("outside");
    Fixture::project(&outside);
    let child = outside.join("child");
    std::fs::create_dir_all(&child).expect("child");
    let report = fixture.json(fixture.command().current_dir(&child));
    assert!(report["project"].is_null());
    let report = fixture.json(fixture.command().current_dir(&outside));
    assert_eq!(report["project"], expected_project(&outside, "detected"));
}

#[test]
fn global_config_marker_is_not_auto_discovered_as_a_project() {
    let fixture = Fixture::new();
    Fixture::project(&fixture.home);
    let global = fixture.home.join(".upeg");
    for cwd in [&fixture.cwd, &fixture.home, &global] {
        let report = fixture.json(fixture.command().env("UPEG_HOME", &global).current_dir(cwd));
        assert!(report["project"].is_null());
    }
    #[cfg(unix)]
    {
        let report = fixture.json(fixture.command().env_remove("UPEG_HOME"));
        assert!(report["project"].is_null());
        assert_eq!(report["user"]["config_dir"], json!(global));
    }
}

#[test]
fn absent_marker_and_user_root_are_not_created() {
    let fixture = Fixture::new();
    let report = fixture.json(&mut fixture.command());
    assert!(report["project"].is_null());
    assert!(!fixture.cwd.join(".upeg").exists());
    assert!(!fixture.user.exists());
    let output = fixture.read_only(
        fixture
            .command()
            .arg("--project")
            .arg(&fixture.cwd)
            .args(["paths", "--json"]),
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("has no .upeg directory"));
    std::fs::write(fixture.cwd.join(".upeg"), "not a directory").expect("invalid marker");
    let output = fixture.read_only(
        fixture
            .command()
            .arg("--project")
            .arg(&fixture.cwd)
            .args(["paths", "--json"]),
    );
    assert!(!output.status.success());
}

#[test]
fn unavailable_home_reports_null_and_missing_home_does_not_create_storage() {
    let fixture = Fixture::new();
    let report = fixture.json(
        fixture
            .command()
            .env_remove("UPEG_HOME")
            .env_remove("HOME")
            .env_remove("APPDATA"),
    );
    assert!(
        report["user"]
            .as_object()
            .expect("user paths")
            .values()
            .all(Value::is_null)
    );
    assert!(report["project"].is_null());
    let missing = fixture.temp.path().join("missing-home");
    let report = fixture.json(
        fixture
            .command()
            .env_remove("UPEG_HOME")
            .env("HOME", &missing)
            .env("APPDATA", &missing),
    );
    let expected = missing.join(".upeg/config");
    assert_eq!(report["user"]["config_dir"], json!(expected));
    assert!(!missing.exists());
}

#[test]
fn all_existing_artifact_overrides_remain_verbatim_even_without_home() {
    let fixture = Fixture::new();
    let overrides = [
        ("UPEG_TOOLKITS_DIR", "toolkits_dir"),
        ("UPEG_WASM_DIR", "wasm_dir"),
        ("UPEG_MCP_IMPORTS_DIR", "mcp_imports_dir"),
        ("UPEG_TOOLKIT_CACHE_DIR", "toolkit_packs_dir"),
        ("UPEG_CREDENTIALS_PATH", "credentials"),
        ("UPEG_LOG_PATH", "execution_log"),
        ("UPEG_HTTP_LOG_PATH", "http_log"),
    ];
    for with_home in [true, false] {
        for value in [
            fixture.temp.path().join("explicit-artifact"),
            PathBuf::from("legacy/relative"),
            PathBuf::new(),
        ] {
            let mut command = fixture.command();
            if !with_home {
                command
                    .env_remove("UPEG_HOME")
                    .env_remove("HOME")
                    .env_remove("APPDATA");
            }
            let override_path = |field: &str| {
                if value.as_os_str().is_empty() {
                    value.clone()
                } else {
                    value.join(field)
                }
            };
            for (name, field) in overrides {
                command.env(name, override_path(field));
            }
            let report = fixture.json(&mut command);
            for (_, field) in overrides {
                assert_eq!(
                    report["user"][field],
                    json!(override_path(field)),
                    "{field}"
                );
            }
            if !with_home {
                assert!(report["user"]["config_dir"].is_null());
                assert!(report["user"]["store"].is_null());
            }
        }
    }
}

#[test]
fn relative_and_empty_user_home_are_not_rejected_or_normalized() {
    let fixture = Fixture::new();
    for root in [PathBuf::from("relative/storage"), PathBuf::new()] {
        let report = fixture.json(fixture.command().env("UPEG_HOME", &root));
        for field in [
            "config_dir",
            "data_dir",
            "state_dir",
            "cache_dir",
            "runtime_dir",
        ] {
            assert_eq!(report["user"][field], json!(root));
        }
        assert_eq!(report["user"]["store"], json!(root.join("upeg.db")));
        assert_eq!(
            report["user"]["toolkit_packs_dir"],
            json!(root.join("toolkit-packs"))
        );
    }
}

#[test]
fn project_namespace_depends_on_canonical_project_root_not_user_storage() {
    let fixture = Fixture::new();
    Fixture::project(&fixture.cwd);
    let second_project = fixture.home.join("second-project");
    Fixture::project(&second_project);
    let first = fixture.json(fixture.command().arg("--project").arg(&fixture.cwd));
    let changed_home = fixture.json(
        fixture
            .command()
            .env("UPEG_HOME", fixture.temp.path().join("second-user-storage"))
            .env("HOME", fixture.temp.path().join("second-home"))
            .arg("--project")
            .arg(fixture.cwd.join(".")),
    );
    let second = fixture.json(fixture.command().arg("--project").arg(&second_project));
    assert_eq!(first["project"], changed_home["project"]);
    assert_ne!(first["user"]["store"], changed_home["user"]["store"]);
    assert_ne!(
        first["project"]["namespace"],
        second["project"]["namespace"]
    );
    let board = BoardKey::parse("same-board").expect("board");
    let namespace = |root: &Path| {
        ProjectBoardNamespace::for_project_root(ProjectRoot::new(root).expect("project").as_path())
    };
    let first_key = BoardStoreKey::project(&namespace(&fixture.cwd), &board);
    let second_key = BoardStoreKey::project(&namespace(&second_project), &board);
    assert_eq!(
        first_key.as_str(),
        format!(
            "project:{}:same-board",
            first["project"]["namespace"].as_str().expect("namespace")
        )
    );
    assert_ne!(first_key, second_key);
}

#[test]
fn human_output_labels_legacy_layout_and_separates_user_from_project() {
    let fixture = Fixture::new();
    Fixture::project(&fixture.cwd);
    let output = fixture.read_only(fixture.command().args(["paths"]));
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("text");
    assert!(stdout.contains("layout\tlegacy-v1\n"));
    assert!(stdout.contains("user.config_dir\t"));
    assert!(stdout.contains("project.marker\t"));
    assert!(stdout.contains("project.origin\tdetected\n"));
}

#[test]
fn library_paths_entrypoint_is_read_only() {
    use clap::Parser as _;

    if std::env::var_os("UPEG_PATHS_LIBRARY_TEST").is_some() {
        let cli = upeg_cli::Cli::try_parse_from(["upeg", "--project", ".", "paths", "--json"])
            .expect("cli");
        let output = upeg_cli::run(cli).expect("library paths");
        let report: Value = serde_json::from_str(&output).expect("JSON");
        assert_eq!(report["project"]["origin"], "argument");
        return;
    }
    let fixture = Fixture::new();
    Fixture::project(&fixture.cwd);
    let mut child = Command::new(std::env::current_exe().expect("test executable"));
    child
        .env_clear()
        .current_dir(&fixture.cwd)
        .env("HOME", &fixture.home)
        .env("UPEG_HOME", &fixture.user)
        .env("UPEG_PATHS_LIBRARY_TEST", "1")
        .args(["--exact", "library_paths_entrypoint_is_read_only"]);
    let output = fixture.read_only(&mut child);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
