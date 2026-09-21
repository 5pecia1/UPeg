//! Structured failure envelope for the External invoker.
//!
//! Before this module a failing command collapsed into one opaque
//! string — `` `cargo` exited exit status: 1: <stderr> `` — which threw
//! stdout away entirely and buried the exit code inside prose. That is
//! precisely backwards for developer tooling: `cargo fmt --check`,
//! `clippy`, `cargo test`, `flutter analyze`, and `gh pr checks` all
//! write their diagnostics to **stdout**, so agents and humans alike
//! got nothing actionable.
//!
//! [`ExternalExit`] keeps the exit disposition typed and both captured
//! streams intact, and lowers them into the canonical
//! [`upeg_core::ToolError`] `details` object every surface already
//! knows how to carry.

use std::process::ExitStatus;

use serde_json::Value;
use upeg_core::{ProcessErrorDetails, ProcessTermination};

use super::error::{CaptureCompletion, CapturedOutput};

/// Maximum bytes of one captured stream copied into the failure
/// envelope.
///
/// The capture layer's own ceiling is the untrusted-output wire budget
/// (tens of megabytes — sized for `File` outputs, not for diagnostics).
/// An error envelope travels through CLI `--json`, the HTTP body, MCP
/// `structuredContent`, the FRB bridge, and the last-outcome store, so
/// it gets its own, far smaller budget: enough for a full compiler
/// diagnostic run, small enough that no surface has to defend itself.
const MAX_DIAGNOSTIC_STREAM_BYTES: usize = 64 * 1024;

/// Appended when a stream is cut at [`MAX_DIAGNOSTIC_STREAM_BYTES`] so
/// a reader never mistakes a truncated tail for the real end of output.
const DIAGNOSTIC_TRUNCATION_MARKER: &str = "\n… output truncated by upeg";

/// `details.cancelled` — the flag that tells a consumer this run was
/// stopped on request rather than by its own exit or its own budget.
///
/// Spelled here rather than in `upeg_core::ProcessErrorDetails` because
/// cancellation is not a *process* disposition at all: the child was
/// killed mid-run and never reported one, which is why the envelope
/// carries `exit_code: null` alongside it. The key sits next to
/// `timed_out` on the wire and reads the same way.
const CANCELLED_DETAIL_KEY: &str = "cancelled";

/// How a captured External run ended, from the failure envelope's point
/// of view.
///
/// [`ProcessTermination`] covers everything the *process* can report.
/// Cancellation is the one disposition it cannot: nobody asked the child
/// anything, upeg terminated its group because the caller went away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExternalTermination {
    Process(ProcessTermination),
    Cancelled,
}

/// A child process that ran but did not succeed, with the diagnostics
/// it produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExternalExit {
    command: String,
    termination: ExternalTermination,
    stdout: String,
    stderr: String,
}

impl ExternalExit {
    /// How this run ended — the discriminator the dispatcher reads to
    /// pick the failure's canonical error code.
    pub(super) const fn termination(&self) -> ExternalTermination {
        self.termination
    }

    /// Build the envelope for a completed-but-failed capture.
    pub(super) fn from_capture(command: &str, output: &CapturedOutput) -> Self {
        Self {
            command: command.to_string(),
            termination: termination_of(&output.completion),
            stdout: diagnostic_text(&output.stdout),
            stderr: diagnostic_text(&output.stderr),
        }
    }

    /// One-line operator message.
    ///
    /// `` `cargo` exited with code 1 ``, `` `cargo` was killed by
    /// signal 9 ``, `` `cargo` timed out after 30000 ms `` — with the
    /// first non-empty stderr line appended only when there is one, so
    /// the message never ends in a dangling colon.
    pub(super) fn message(&self) -> String {
        let mut message = format!("`{}` {}", self.command, self.termination_phrase());
        if let Some(line) = self.stderr.lines().find(|line| !line.trim().is_empty()) {
            message.push_str(": ");
            message.push_str(line.trim_end());
        }
        message
    }

    fn termination_phrase(&self) -> String {
        let process = match self.termination {
            ExternalTermination::Cancelled => return "was cancelled".to_string(),
            ExternalTermination::Process(process) => process,
        };
        match process {
            ProcessTermination::Exited { code } => format!("exited with code {code}"),
            ProcessTermination::Signaled { signal } => format!("was killed by signal {signal}"),
            ProcessTermination::TimedOut { timeout_ms } => {
                format!("timed out after {timeout_ms} ms")
            }
            ProcessTermination::Unknown => "exited abnormally".to_string(),
        }
    }

    /// Canonical `ToolError::details` payload.
    ///
    /// A cancelled run reports the process side as `Unknown` — which is
    /// the truth, it was killed before it could report anything — and
    /// adds [`CANCELLED_DETAIL_KEY`] so the reason is not left to be
    /// guessed from a null exit code.
    pub(super) fn details(&self) -> Value {
        let process = match self.termination {
            ExternalTermination::Process(process) => process,
            ExternalTermination::Cancelled => ProcessTermination::Unknown,
        };
        let mut details = ProcessErrorDetails {
            termination: process,
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
        }
        .to_value();
        if self.termination == ExternalTermination::Cancelled
            && let Some(object) = details.as_object_mut()
        {
            object.insert(CANCELLED_DETAIL_KEY.to_string(), Value::Bool(true));
        }
        details
    }
}

fn termination_of(completion: &CaptureCompletion) -> ExternalTermination {
    match completion {
        CaptureCompletion::Cancelled => ExternalTermination::Cancelled,
        CaptureCompletion::TimedOut { timeout_ms } => {
            ExternalTermination::Process(ProcessTermination::TimedOut {
                timeout_ms: *timeout_ms,
            })
        }
        CaptureCompletion::Exited(status) => {
            ExternalTermination::Process(exit_termination(*status))
        }
    }
}

#[cfg(unix)]
fn exit_termination(status: ExitStatus) -> ProcessTermination {
    use std::os::unix::process::ExitStatusExt as _;

    if let Some(code) = status.code() {
        return ProcessTermination::Exited { code };
    }
    status
        .signal()
        .map_or(ProcessTermination::Unknown, |signal| {
            ProcessTermination::Signaled { signal }
        })
}

#[cfg(not(unix))]
fn exit_termination(status: ExitStatus) -> ProcessTermination {
    status.code().map_or(ProcessTermination::Unknown, |code| {
        ProcessTermination::Exited { code }
    })
}

/// Decode captured bytes as lossy UTF-8, capped at the diagnostic
/// budget on a char boundary.
fn diagnostic_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    if text.len() <= MAX_DIAGNOSTIC_STREAM_BYTES {
        return text.into_owned();
    }
    let mut cut = MAX_DIAGNOSTIC_STREAM_BYTES;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut truncated = text[..cut].to_string();
    truncated.push_str(DIAGNOSTIC_TRUNCATION_MARKER);
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exit(termination: ProcessTermination, stderr: &str) -> ExternalExit {
        exit_with_termination(ExternalTermination::Process(termination), stderr)
    }

    fn exit_with_termination(termination: ExternalTermination, stderr: &str) -> ExternalExit {
        ExternalExit {
            command: "cargo".to_string(),
            termination,
            stdout: String::new(),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn exit_code_message_reads_the_code() {
        assert_eq!(
            exit(ProcessTermination::Exited { code: 1 }, "").message(),
            "`cargo` exited with code 1"
        );
    }

    #[test]
    fn signal_message_reads_the_signal_number() {
        assert_eq!(
            exit(ProcessTermination::Signaled { signal: 9 }, "").message(),
            "`cargo` was killed by signal 9"
        );
    }

    #[test]
    fn timeout_message_reads_milliseconds() {
        assert_eq!(
            exit(ProcessTermination::TimedOut { timeout_ms: 30000 }, "").message(),
            "`cargo` timed out after 30000 ms"
        );
    }

    #[test]
    fn stderr_appends_only_first_line() {
        assert_eq!(
            exit(
                ProcessTermination::Exited { code: 1 },
                "error: first\nerror: second"
            )
            .message(),
            "`cargo` exited with code 1: error: first"
        );
    }

    #[test]
    fn blank_stderr_leaves_no_dangling_colon() {
        assert_eq!(
            exit(ProcessTermination::Exited { code: 1 }, "  \n\n").message(),
            "`cargo` exited with code 1"
        );
    }

    #[test]
    fn cancel_message_says_cancelled() {
        assert_eq!(
            exit_with_termination(ExternalTermination::Cancelled, "").message(),
            "`cargo` was cancelled"
        );
    }

    #[test]
    fn cancelled_run_details_carry_cancelled_key() {
        let details = exit_with_termination(ExternalTermination::Cancelled, "").details();

        assert_eq!(details[CANCELLED_DETAIL_KEY], Value::Bool(true));
        assert_eq!(
            details["exit_code"],
            Value::Null,
            "a cancelled child had no chance to report an exit code"
        );
    }

    #[test]
    fn non_cancelled_details_have_no_cancelled_key() {
        let details = exit(ProcessTermination::Exited { code: 1 }, "").details();

        assert!(details.get(CANCELLED_DETAIL_KEY).is_none());
    }

    #[test]
    fn diagnostic_output_is_cut_at_budget_with_marker() {
        let source = "x".repeat(MAX_DIAGNOSTIC_STREAM_BYTES + 10);
        let truncated = diagnostic_text(source.as_bytes());
        assert!(truncated.ends_with(DIAGNOSTIC_TRUNCATION_MARKER));
        assert_eq!(
            truncated.len(),
            MAX_DIAGNOSTIC_STREAM_BYTES + DIAGNOSTIC_TRUNCATION_MARKER.len()
        );
    }

    #[test]
    fn diagnostic_output_respects_multibyte_boundaries() {
        let repeats = MAX_DIAGNOSTIC_STREAM_BYTES / "가".len() + 4;
        let source = "가".repeat(repeats);
        let truncated = diagnostic_text(source.as_bytes());
        assert!(truncated.starts_with('가'));
        assert!(truncated.ends_with(DIAGNOSTIC_TRUNCATION_MARKER));
    }
}
