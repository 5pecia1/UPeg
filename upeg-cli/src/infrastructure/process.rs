//! OS process-liveness probing.
//!
//! One owner for the whole workspace. Two subsystems ask "is the process
//! that wrote this file still alive?":
//!
//! - [`super::discovery`] — reaping a stale `server.json` (PRD §5.3).
//! - `upeg-frb`'s desktop single-instance lock (PRD §5.9), which reads
//!   the pid out of `desktop.lock`.
//!
//! `upeg-frb` depends on this crate (the desktop shell embeds the host),
//! so it calls [`pid_alive`] through `upeg_cli::pid_alive` rather than
//! keeping a private copy. The probe deliberately shells out instead of
//! calling `kill(2)`/`OpenProcess` directly — the workspace denies
//! `unsafe_code`, and neither path is hot.

/// `kill -0 <pid>` (Unix) / `tasklist /FI "PID eq N"` (Windows) —
/// cross-platform liveness check without `unsafe`.
///
/// Returns `false` for any probe failure (missing `kill`/`tasklist`,
/// permission denied, unparseable output). Callers treat `false` as
/// "assume gone", which is the safe direction for both consumers: a
/// stale discovery record gets re-published, and a stale instance lock
/// gets re-taken.
pub fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        // `/FO CSV` quotes each column so the PID can be matched as a whole
        // field. A plain substring search would report pid 123 alive when
        // the listing only contains 5123.
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output();
        match out {
            Ok(o) => tasklist_reports_pid(&String::from_utf8_lossy(&o.stdout), pid),
            Err(_) => false,
        }
    }
}

/// Whether `tasklist /FO CSV /NH` output lists `pid` as the exact value of
/// its PID column (the second quoted field). Split out as a pure function so
/// the exact-match logic is unit-testable off Windows; gated to
/// `windows`/`test` builds so it is neither dead code nor untestable.
#[cfg(any(windows, test))]
fn tasklist_reports_pid(stdout: &str, pid: u32) -> bool {
    const PID_COLUMN: usize = 1;
    const CSV_FIELD_SEPARATOR: &str = "\",\"";
    const CSV_QUOTE: char = '"';
    let target = pid.to_string();
    stdout.lines().any(|line| {
        // Row shape: "Image Name","PID","Session Name","Session#","Mem Usage".
        line.split(CSV_FIELD_SEPARATOR)
            .nth(PID_COLUMN)
            .map(|field| field.trim_matches(CSV_QUOTE).trim())
            == Some(target.as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pid far above every platform's `pid_max`, so it can never name a
    /// live process on the machine running the test.
    const DEAD_PID: u32 = 99_999_999;

    #[test]
    fn pid_alive는_자신의_pid에_대해_참을_반환한다() {
        assert!(pid_alive(std::process::id()));
    }

    #[test]
    fn pid_alive는_명백히_죽은_pid에_대해_거짓을_반환한다() {
        assert!(!pid_alive(DEAD_PID));
    }

    #[test]
    fn tasklist_파싱은_정확한_pid_컬럼만_매칭한다() {
        let row = "\"upeg.exe\",\"1234\",\"Console\",\"1\",\"10,000 K\"";
        assert!(tasklist_reports_pid(row, 1234));
    }

    #[test]
    fn tasklist_파싱은_부분_문자열_pid를_살아있다고_오판하지_않는다() {
        // 5123 만 나열된 출력에서 123 을 살아있다고 보면 안 된다.
        let row = "\"upeg.exe\",\"5123\",\"Console\",\"1\",\"10,000 K\"";
        assert!(!tasklist_reports_pid(row, 123));
    }

    #[test]
    fn tasklist_파싱은_이미지_이름에_섞인_pid를_무시한다() {
        // PID 컬럼이 아닌 곳에 같은 숫자가 있어도 살아있다고 보면 안 된다.
        let row = "\"1234.exe\",\"5678\",\"Console\",\"1\",\"10,000 K\"";
        assert!(!tasklist_reports_pid(row, 1234));
    }

    #[test]
    fn tasklist_파싱은_빈_결과를_거짓으로_처리한다() {
        let info = "INFO: No tasks are running which match the specified criteria.";
        assert!(!tasklist_reports_pid(info, 1234));
    }
}
