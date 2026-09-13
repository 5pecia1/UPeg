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

static 카운터: AtomicUsize = AtomicUsize::new(0);

const 도구_로컬_ID: &str = "two_lines";
/// Writes to both streams, so the test can tell "mirrored live" from
/// "printed the final primary output".
const 자식_명령: &str = "echo out-one; echo err-one 1>&2; echo out-two";

struct 픽스처 {
    root: PathBuf,
    toolkits: PathBuf,
    home: PathBuf,
    toolkit: String,
}

impl Drop for 픽스처 {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl 픽스처 {
    fn 새로() -> Self {
        let n = 카운터.fetch_add(1, Ordering::SeqCst);
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
id = "{도구_로컬_ID}"
description = "live output fixture"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "{자식_명령}"]
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

    fn 도구_id(&self) -> String {
        format!("{}.{도구_로컬_ID}", self.toolkit)
    }

    /// Run `upeg call` with the given extra flags; returns (stdout, stderr).
    fn 호출(&self, extra: &[&str]) -> (String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_upeg"))
            .arg("call")
            .arg(self.도구_id())
            // `--local` keeps the call in-process: an attached host
            // would answer with a final envelope over HTTP and there
            // would be nothing to mirror.
            .arg("--local")
            .args(extra)
            .env("UPEG_TOOLKITS_DIR", &self.toolkits)
            .env("UPEG_HOME", &self.home)
            .output()
            .expect("upeg 바이너리 실행");
        (
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

#[test]
fn 사람용_호출은_실행_중_출력을_표준오류로_비춘다() {
    let fixture = 픽스처::새로();

    let (stdout, stderr) = fixture.호출(&[]);

    assert!(
        stderr.contains("out-one") && stderr.contains("out-two"),
        "자식 stdout이 실행 중 표준오류로 비춰야 한다: {stderr:?}"
    );
    assert!(
        stderr.contains("err-one"),
        "자식 stderr도 실행 중 표준오류로 비춰야 한다: {stderr:?}"
    );
    // stdout stays exactly the final primary output (plus `upeg call`'s
    // own trailing newline) — the mirror never moves onto it.
    assert_eq!(stdout, "out-one\nout-two\n\n");
}

#[test]
fn json_모드는_실행_중_아무것도_흘리지_않는다() {
    let fixture = 픽스처::새로();

    let (stdout, stderr) = fixture.호출(&["--json"]);

    assert!(
        !stderr.contains("out-one") && !stderr.contains("err-one"),
        "기계용 모드는 진행 출력을 만들지 않는다: {stderr:?}"
    );
    let envelope: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("--json은 canonical 봉투 한 줄이다");
    assert_eq!(envelope["ok"], true);
}

#[test]
fn field_모드도_조용하다() {
    let fixture = 픽스처::새로();

    let (_, stderr) = fixture.호출(&["--field", "result"]);

    assert!(
        !stderr.contains("out-one"),
        "--field는 파싱될 값을 위한 모드다: {stderr:?}"
    );
}
