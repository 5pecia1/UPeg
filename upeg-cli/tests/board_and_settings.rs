#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect to identify setup or command failures"
)]

use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use upeg_core::{ArgsPreset, PinId, Placement};
use upeg_sources::store::Store;

const TOOL_ID: &str = "num.hex_to_decimal";

fn cli(root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_upeg"));
    command
        .args(args)
        .env("UPEG_HOME", root)
        .env("UPEG_PROJECT_MANIFEST_PATH", "off")
        .env("UPEG_TOOLKITS_DIR", root.join("empty-toolkits"))
        .env("UPEG_WASM_DIR", root.join("empty-wasm"))
        .current_dir(root);
    command
}

fn run(root: &Path, args: &[&str]) -> Output {
    cli(root, args).output().expect("run upeg")
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 stdout")
}

fn run_json(root: &Path, args: &[&str]) -> Value {
    serde_json::from_str(&success(run(root, args))).expect("JSON output")
}

#[test]
fn scoped_help_succeeds_on_stdout_even_for_a_board_that_does_not_exist() {
    let root = tempfile::tempdir().expect("config root");
    for args in [
        &["board", "some-board", "--help"][..],
        &["board", "on", "create", "--help"],
    ] {
        let output = run(root.path(), args);
        assert!(output.stderr.is_empty());
        let help = success(output);
        for name in [
            "pin-list", "pin-show", "pin-edit", "rename", "delete", "call",
        ] {
            assert!(help.contains(name), "{help}");
        }
    }
    let help = success(run(
        root.path(),
        &["board", "some-board", "pin-edit", "--help"],
    ));
    assert!(help.contains("--reset-size"), "{help}");
}

#[cfg(unix)]
#[test]
fn personal_board_pin_edit_targets_exact_duplicate_and_preserves_presets() {
    let root = tempfile::tempdir().expect("config root");
    let root = root.path();
    const PINNED_TOOL: &str = "fixture.echo";
    let toolkits = root.join("empty-toolkits");
    std::fs::create_dir(&toolkits).expect("toolkits directory");
    std::fs::write(
        toolkits.join("fixture.toml"),
        r#"id = "fixture"
[[tools]]
id = "echo"
display_label = "Echo"
description = "Echo a preset"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", 'printf "%s" "$1"', "upeg-test", "{input}"]
surfaces = ["cli", "desktop"]

[[tools.inputs]]
name = "input"
type = "string"
required = true
"#,
    )
    .expect("write local tool");
    let board = run_json(root, &["board", "create", "  Script Board  ", "--json"]);
    assert_eq!(
        board,
        json!({"key": "script-board", "title": "Script Board", "action": "created"})
    );
    let key = board["key"].as_str().expect("board key");
    assert_eq!(
        success(run(root, &["board", "create", "Script Board"])),
        "script-board-2\n"
    );
    assert!(!run(root, &["board", "create", " "]).status.success());

    success(run(root, &["board", key, "pin", PINNED_TOOL]));
    {
        let path = upeg_core::paths::store_path_in(root);
        let mut store = Store::open_at(&path).expect("open fixture store");
        let original = Placement::new(PINNED_TOOL, 0, 0).with_args_preset(Some(
            ArgsPreset::parse(r#"{"input":"ff"}"#).expect("valid preset"),
        ));
        store
            .upsert_placement(key, &original)
            .expect("seed original preset");
        let mut second = Placement::new(PINNED_TOOL, 0, 2);
        second.pin_id = PinId::parse("second-pin").expect("pin id");
        second.args_preset = Some(ArgsPreset::parse(r#"{"input":"2a"}"#).expect("valid preset"));
        store
            .upsert_placement(key, &second)
            .expect("insert duplicate instance");
        let mut unavailable = Placement::new("fixture.unavailable", 1, 6);
        unavailable.pin_id = PinId::parse("unavailable-pin").expect("pin id");
        store
            .upsert_placement(key, &unavailable)
            .expect("insert unavailable fixture");
    }

    let args = ["board", key, "pin-list", "--json"];
    let before = run_json(root, &args);
    let original = &before[0];
    let preset = &before[1]["args_preset"];
    assert_eq!(before.as_array().expect("pins").len(), 3);
    assert_eq!(original["pin_id"], PINNED_TOOL);
    assert_eq!(before[1]["pin_id"], "second-pin");
    assert_eq!(before[1]["tool_id"], PINNED_TOOL);
    assert_eq!(preset, &json!({"input":"2a"}));
    assert_eq!(
        run_json(root, &["board", key, "list", "--json"])
            .as_array()
            .expect("tools")
            .len(),
        1
    );

    let call = ["board", key, "call", PINNED_TOOL, "--local"];
    assert_eq!(success(run(root, &call)), "ff\n");
    let edited = run_json(
        root,
        &[
            "board",
            key,
            "pin-edit",
            "second-pin",
            "--size",
            "2x3",
            "--color",
            "#aabbcc",
            "--json",
        ],
    );
    assert_eq!(edited["size_override"], json!({"cols":2,"rows":3}));
    assert_eq!(edited["color"], "#AABBCC");
    assert_eq!(&edited["args_preset"], preset);
    assert_eq!(success(run(root, &call)), "ff\n");
    let after = run_json(root, &args);
    assert_eq!(&after[0], original);
    assert_eq!(after[2], before[2]);
    assert!(before[2]["size"].is_null());
    let hidden = run_json(
        root,
        &[
            "board",
            key,
            "pin-edit",
            "unavailable-pin",
            "--size",
            "2x1",
            "--json",
        ],
    );
    assert_eq!(hidden["size"], json!({"cols": 2, "rows": 1}));
    assert_eq!(
        run_json(
            root,
            &[
                "board",
                key,
                "pin-edit",
                "unavailable-pin",
                "--reset-size",
                "--json"
            ]
        ),
        before[2]
    );
    assert_eq!(
        run_json(root, &["board", key, "pin-show", "second-pin", "--json"]),
        edited
    );
    assert!(
        success(run(root, &["board", key, "pin-show", "second-pin"])).starts_with("second-pin\t")
    );
    for invalid_args in [
        &["pin-edit", "missing", "--color", "#ABCABC"][..],
        &[
            "pin-edit",
            "second-pin",
            "--size",
            "0x3",
            "--color",
            "#ABCABC",
        ],
        &[
            "pin-edit",
            "second-pin",
            "--size",
            "2x3",
            "--color",
            "invalid",
        ],
        &[
            "pin-edit",
            "second-pin",
            "--reset-color",
            "--color",
            "#ABCABC",
        ],
        &["pin-edit", "second-pin"],
    ] {
        let argv: Vec<_> = ["board", key]
            .into_iter()
            .chain(invalid_args.iter().copied())
            .collect();
        let result = run(root, &argv);
        assert!(!result.status.success(), "{argv:?}");
        assert!(result.stdout.is_empty(), "{argv:?}");
    }
    assert_eq!(run_json(root, &args), after);

    let reset = run_json(
        root,
        &[
            "board",
            key,
            "pin-edit",
            "second-pin",
            "--reset-size",
            "--reset-color",
            "--json",
        ],
    );
    assert_eq!(reset, before[1]);
    assert_eq!(
        run_json(root, &["board", key, "rename", "Renamed", "--json"])["key"],
        key
    );
    assert_eq!(
        run_json(root, &["board", key, "pin-list", "--json"]),
        before
    );
    assert_eq!(
        run_json(root, &["board", key, "delete", "--json"])["title"],
        "Renamed"
    );
    assert!(!run(root, &["board", key, "pin-list"]).status.success());
    assert!(
        run(root, &["board", "script-board-2", "pin-list"])
            .status
            .success()
    );
}

#[test]
fn settings_patch_shared_record_concurrently_and_reject_invalid_values() {
    let root = tempfile::tempdir().expect("config root");
    let root = root.path();
    let path = upeg_core::paths::tweaks_path_in(root);
    std::fs::write(&path, r#"{"theme":"Light","accent":"Green","show_holes":false,"locale":"En","local_http_host":true,"pin_handle_style":"Dots","future_setting":{"a":1}}"#).expect("seed prefs");

    let mut first = cli(root, &["settings", "set", "theme", "dark"])
        .spawn()
        .expect("set theme");
    let mut second = cli(root, &["settings", "set", "locale", "ko"])
        .spawn()
        .expect("set locale");
    assert!(first.wait().expect("theme status").success());
    assert!(second.wait().expect("locale status").success());
    assert_eq!(
        run_json(root, &["settings", "set", "accent", "pink", "--json"]),
        json!({"key":"accent","value":"pink"})
    );
    assert_eq!(
        run_json(root, &["settings", "get", "--json"]),
        json!({"theme":"dark","locale":"ko","accent":"pink"})
    );
    assert_eq!(success(run(root, &["settings", "get", "theme"])), "dark\n");
    let stored = std::fs::read_to_string(&path).expect("read shared prefs");
    let record: Value = serde_json::from_str(&stored).expect("prefs JSON");
    assert_eq!(record["show_holes"], false);
    assert_eq!(record["local_http_host"], true);
    assert_eq!(record["pin_handle_style"], "Dots");
    assert_eq!(record["future_setting"], json!({"a":1}));
    assert!(
        !run(root, &["settings", "set", "locale", "invalid"])
            .status
            .success()
    );
    assert_eq!(
        std::fs::read_to_string(path).expect("unchanged prefs"),
        stored
    );
}

#[test]
fn resizing_at_right_edge_clamps_and_pushes_the_neighbor_without_replacing_it() {
    let root = tempfile::tempdir().expect("config root");
    let root = root.path();
    let key = "crowded";
    success(run(root, &["board", "create", key]));
    {
        let path = upeg_core::paths::store_path_in(root);
        let mut store = Store::open_at(&path).expect("fixture store");
        for (id, col) in [
            ("neighbor", upeg_core::BOARD_COLS - 2),
            ("target", upeg_core::BOARD_COLS - 1),
        ] {
            let mut pin = Placement::new(TOOL_ID, col, 0);
            pin.pin_id = PinId::parse(id).expect("pin id");
            pin.span = Some(upeg_core::PinSpan::new(
                upeg_core::ColSpan::new(1).expect("valid width"),
                upeg_core::RowSpan::new(1).expect("valid height"),
            ));
            store.upsert_placement(key, &pin).expect("seed pin");
        }
    }
    let edit = [
        "board", key, "pin-edit", "target", "--size", "2x2", "--json",
    ];
    let grown = run_json(root, &edit);
    assert_eq!(
        grown["cell"],
        json!({"row":0,"col":upeg_core::BOARD_COLS - 2})
    );
    assert_eq!(grown["size_override"], json!({"cols":2,"rows":2}));
    let neighbor = run_json(root, &["board", key, "pin-show", "neighbor", "--json"]);
    assert_eq!(neighbor["cell"], json!({"row":1,"col":0}));
    assert_eq!(neighbor["size_override"], json!({"cols":1,"rows":1}));
    assert_eq!(run_json(root, &edit), grown);
    assert_eq!(
        run_json(root, &["board", key, "pin-show", "neighbor", "--json"]),
        neighbor
    );
}

#[test]
fn last_board_delete_uses_the_normal_empty_profile_defaults() {
    let root = tempfile::tempdir().expect("config root");
    let root = root.path();
    success(run(root, &["board", "create", "Last Custom"]));
    for key in ["dev", "trading", "personal", "last-custom"] {
        success(run(root, &["board", key, "delete"]));
    }
    let boards = run_json(root, &["board", "list", "--json"]);
    let boards = boards.as_array().expect("boards");
    assert_eq!(boards.len(), 3, "{boards:?}");
    assert!(
        !run(root, &["board", "last-custom", "pin-list"])
            .status
            .success()
    );
}

#[test]
fn a_new_profile_uses_the_terminal_locale_hint_until_a_locale_is_saved() {
    let root = tempfile::tempdir().expect("config root");
    let root = root.path();
    let output = cli(root, &["settings", "get", "locale"])
        .env("UPEG_LOCALE", "ko_KR.UTF-8")
        .env("LC_ALL", "en_US.UTF-8")
        .output()
        .expect("run settings get");
    assert_eq!(success(output), "ko\n");
    success(run(root, &["settings", "set", "locale", "en"]));
    let output = cli(root, &["settings", "get", "locale"])
        .env("UPEG_LOCALE", "ko")
        .output()
        .expect("run settings get");
    assert_eq!(success(output), "en\n");
}

#[test]
fn project_board_metadata_stays_owned_by_its_manifest() {
    let root = tempfile::tempdir().expect("config root");
    let root = root.path();
    let marker = root.join(".upeg");
    std::fs::create_dir(&marker).expect("project directory");
    std::fs::write(
        marker.join("project.toml"),
        "schema_version = 1\n[[boards]]\nid = \"project-only\"\nlabel = \"Project Board\"\n",
    )
    .expect("write board declaration");
    for args in [
        &["board", "project-only", "rename", "Other"][..],
        &["board", "project-only", "delete"],
    ] {
        let root_arg = root.to_str().expect("UTF-8 temp path");
        let argv: Vec<_> = ["--project", root_arg]
            .into_iter()
            .chain(args.iter().copied())
            .collect();
        let output = cli(root, &argv)
            .output()
            .expect("run project board command");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("declared in the project manifest")
        );
    }
}
