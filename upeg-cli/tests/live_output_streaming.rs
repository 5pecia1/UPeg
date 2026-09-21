#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! `upeg call` mirrors a running tool's output to stderr while it works,
//! and keeps stdout reserved for the result.
//!
//! Spawns the real binary because that is the only way to observe the
//! two streams separately — which is the entire contract under test.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const TOOL_LOCAL_ID: &str = "two_lines";
/// Writes to both streams, so the test can tell "mirrored live" from
/// "printed the final primary output".
const CHILD_COMMAND: &str = "echo out-one; echo err-one 1>&2; echo out-two";

struct Fixture {
    root: PathBuf,
    toolkits: PathBuf,
    home: PathBuf,
    toolkit: String,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let toolkit = format!("livestream{}_{n}", std::process::id());
        let root = std::env::temp_dir().join(format!("upeg-live-output-{toolkit}"));
        let toolkits = root.join("toolkits");
        let home = root.join("home");
        std::fs::create_dir_all(&toolkits).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            toolkits.join("live.toml"),
            format!(
                r#"id = "{toolkit}"

[[tools]]
id = "{TOOL_LOCAL_ID}"
description = "live output fixture"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "{CHILD_COMMAND}"]
surfaces = ["cli"]
"#
            ),
        )
        .unwrap();
        Self {
            root,
            toolkits,
            home,
            toolkit,
        }
    }

    fn tool_id(&self) -> String {
        format!("{}.{TOOL_LOCAL_ID}", self.toolkit)
    }

    /// Run `upeg call` with the given extra flags; returns (stdout, stderr).
    fn call(&self, extra: &[&str]) -> (String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_upeg"))
            .arg("call")
            .arg(self.tool_id())
            // `--local` keeps the call in-process: an attached host
            // would answer with a final envelope over HTTP and there
            // would be nothing to mirror.
            .arg("--local")
            .args(extra)
            .env("UPEG_TOOLKITS_DIR", &self.toolkits)
            .env("UPEG_HOME", &self.home)
            .output()
            .expect("run the upeg binary");
        (
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

#[test]
fn a_human_call_mirrors_running_output_to_stderr() {
    let fixture = Fixture::new();

    let (stdout, stderr) = fixture.call(&[]);

    assert!(
        stderr.contains("out-one") && stderr.contains("out-two"),
        "the child's stdout must be mirrored to stderr while running: {stderr:?}"
    );
    assert!(
        stderr.contains("err-one"),
        "the child's stderr must be mirrored to stderr while running: {stderr:?}"
    );
    // stdout stays exactly the final primary output (plus `upeg call`'s
    // own trailing newline) — the mirror never moves onto it.
    assert_eq!(stdout, "out-one\nout-two\n\n");
}

#[test]
fn json_mode_leaks_nothing_while_running() {
    let fixture = Fixture::new();

    let (stdout, stderr) = fixture.call(&["--json"]);

    assert!(
        !stderr.contains("out-one") && !stderr.contains("err-one"),
        "machine mode must not produce progress output: {stderr:?}"
    );
    let envelope: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("--json is a single canonical envelope");
    assert_eq!(envelope["ok"], true);
}

#[test]
fn field_mode_stays_quiet_too() {
    let fixture = Fixture::new();

    let (_, stderr) = fixture.call(&["--field", "result"]);

    assert!(
        !stderr.contains("out-one"),
        "--field is a mode for a value to be parsed: {stderr:?}"
    );
}
