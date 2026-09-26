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
//! keeping a private copy. Discovery uses the separate conservative
//! [`probe_process`] verdict; desktop lock semantics stay unchanged.

/// Conservative verdict for an on-disk host record. Only [`Self::Dead`]
/// permits stale-record cleanup; [`Self::Unknown`] must never authorize
/// an attach or delete a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessProbe {
    Alive,
    Dead,
    Unknown,
}

/// Query a recorded host PID without conflating permission or probe
/// failures with a missing process. PID reuse remains possible.
pub(crate) fn probe_process(pid: u32) -> ProcessProbe {
    #[cfg(unix)]
    {
        let Ok(raw) = i32::try_from(pid) else {
            return ProcessProbe::Unknown;
        };
        let Some(pid) = rustix::process::Pid::from_raw(raw) else {
            return ProcessProbe::Dead;
        };
        match rustix::process::test_kill_process(pid) {
            Ok(()) => ProcessProbe::Alive,
            Err(rustix::io::Errno::SRCH) => ProcessProbe::Dead,
            Err(_) => ProcessProbe::Unknown,
        }
    }
    #[cfg(windows)]
    {
        let output = std::process::Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .output();
        let Ok(output) = output else {
            return ProcessProbe::Unknown;
        };
        if !output.status.success() {
            return ProcessProbe::Unknown;
        }
        parse_tasklist_process(&output.stdout, pid)
    }
}

/// Parse every row of an unfiltered `tasklist /FO CSV /NH` listing.
/// A diagnostic or malformed row makes the verdict unknown.
#[cfg(any(windows, test))]
fn parse_tasklist_process(stdout: &[u8], target: u32) -> ProcessProbe {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(stdout);
    let mut found = false;
    let mut any = false;
    for record in reader.byte_records() {
        let Ok(fields) = record else {
            return ProcessProbe::Unknown;
        };
        if fields.len() != 5 {
            return ProcessProbe::Unknown;
        }
        let Some(pid_field) = fields.get(1) else {
            return ProcessProbe::Unknown;
        };
        let Some(pid) = std::str::from_utf8(pid_field)
            .ok()
            .and_then(|field| field.parse::<u32>().ok())
        else {
            return ProcessProbe::Unknown;
        };
        any = true;
        found |= pid == target;
    }
    if found {
        ProcessProbe::Alive
    } else if any {
        ProcessProbe::Dead
    } else {
        ProcessProbe::Unknown
    }
}

/// `kill -0 <pid>` (Unix) / `tasklist /FI "PID eq N"` (Windows) —
/// cross-platform liveness check without `unsafe`.
///
/// Preserves the desktop single-instance lock's existing boolean
/// contract: probe failures return `false`. Discovery uses
/// [`probe_process`] instead, so permission and query failures never
/// become evidence that an HTTP host has exited.
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
    fn pid_alive_returns_true_for_own_pid() {
        assert!(pid_alive(std::process::id()));
    }

    #[test]
    fn pid_alive_returns_false_for_an_obviously_dead_pid() {
        assert!(!pid_alive(DEAD_PID));
    }

    #[test]
    fn tasklist_parsing_matches_only_the_exact_pid_column() {
        let row = "\"upeg.exe\",\"1234\",\"Console\",\"1\",\"10,000 K\"";
        assert!(tasklist_reports_pid(row, 1234));
    }

    #[test]
    fn tasklist_parsing_does_not_misjudge_a_partial_string_pid_as_alive() {
        // Output listing only 5123 must not read 123 as alive.
        let row = "\"upeg.exe\",\"5123\",\"Console\",\"1\",\"10,000 K\"";
        assert!(!tasklist_reports_pid(row, 123));
    }

    #[test]
    fn tasklist_parsing_ignores_a_pid_embedded_in_the_image_name() {
        // The same digits outside the PID column must not read as alive.
        let row = "\"1234.exe\",\"5678\",\"Console\",\"1\",\"10,000 K\"";
        assert!(!tasklist_reports_pid(row, 1234));
    }

    #[test]
    fn tasklist_parsing_treats_an_empty_result_as_false() {
        let info = "INFO: No tasks are running which match the specified criteria.";
        assert!(!tasklist_reports_pid(info, 1234));
    }

    #[test]
    fn process_probe_distinguishes_live_dead_and_unknown() {
        assert_eq!(probe_process(std::process::id()), ProcessProbe::Alive);
        assert_eq!(probe_process(DEAD_PID), ProcessProbe::Dead);
        assert_eq!(
            parse_tasklist_process(b"not CSV", 1234),
            ProcessProbe::Unknown
        );
        assert_eq!(parse_tasklist_process(b"", 1234), ProcessProbe::Unknown);
        assert_eq!(
            parse_tasklist_process(
                b"\"upeg.exe\",\"1234\",\"Console\",\"1\",\"10,000 K\"",
                1234
            ),
            ProcessProbe::Alive
        );
    }
}
