//! Structured [`ToolError::details`](super::ToolError::details) payload
//! for invokers that run a child process.
//!
//! The `External` invoker used to collapse a failing command into one
//! opaque string, throwing away stdout entirely — which is exactly where
//! `cargo fmt --check`, `clippy`, `cargo test`, `flutter analyze`, and
//! `gh pr checks` write their diagnostics. This type is the canonical
//! carrier that puts the exit disposition and both captured streams into
//! the failure envelope instead.
//!
//! It lives in `upeg-core` because the producer (`upeg-loader`'s External
//! dispatcher) and every presentation surface (CLI human/JSON rendering,
//! MCP `tools/call` text, FRB → Flutter) must agree on the key names
//! without any of them re-declaring string literals.

use serde_json::{Map, Value};

/// Key names inside the details object. Declared once so producer and
/// consumers cannot drift.
const EXIT_CODE_KEY: &str = "exit_code";
const SIGNAL_KEY: &str = "signal";
const TIMED_OUT_KEY: &str = "timed_out";
const STDOUT_KEY: &str = "stdout";
const STDERR_KEY: &str = "stderr";

/// How a child process stopped, as carried in the failure envelope.
///
/// Exactly one variant is true for any given failure, which is why this
/// is an enum rather than three independent optional fields: a process
/// cannot both exit with a code and be killed by a signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessTermination {
    /// The process ran to completion and returned `code`.
    Exited { code: i32 },
    /// The process was killed by `signal` (Unix).
    Signaled { signal: i32 },
    /// The declared `timeout_ms` elapsed and upeg terminated the
    /// process group.
    TimedOut { timeout_ms: u64 },
    /// The platform reported neither an exit code nor a signal.
    Unknown,
}

/// Structured failure details for a child-process invocation.
///
/// `stdout` / `stderr` are already capped by the producer — see the
/// External dispatcher's diagnostic budget — so an envelope built from
/// this type stays bounded no matter how loud the command was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessErrorDetails {
    pub termination: ProcessTermination,
    pub stdout: String,
    pub stderr: String,
}

impl ProcessErrorDetails {
    /// Serialize into the `details` object of a canonical `ToolError`.
    ///
    /// `exit_code` is always present (JSON `null` when the process did
    /// not exit normally) so consumers can branch on one stable key;
    /// `signal` and `timed_out` appear only for their own variants.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert(EXIT_CODE_KEY.to_string(), self.exit_code_value());
        match self.termination {
            ProcessTermination::Signaled { signal } => {
                object.insert(SIGNAL_KEY.to_string(), Value::from(signal));
            }
            ProcessTermination::TimedOut { .. } => {
                object.insert(TIMED_OUT_KEY.to_string(), Value::Bool(true));
            }
            ProcessTermination::Exited { .. } | ProcessTermination::Unknown => {}
        }
        object.insert(STDOUT_KEY.to_string(), Value::String(self.stdout.clone()));
        object.insert(STDERR_KEY.to_string(), Value::String(self.stderr.clone()));
        Value::Object(object)
    }

    fn exit_code_value(&self) -> Value {
        match self.termination {
            ProcessTermination::Exited { code } => Value::from(code),
            ProcessTermination::Signaled { .. }
            | ProcessTermination::TimedOut { .. }
            | ProcessTermination::Unknown => Value::Null,
        }
    }
}

/// Captured process streams read back out of an arbitrary `details`
/// value.
///
/// Presentation surfaces only ever need the two streams, and they must
/// tolerate a `details` object produced by some other invoker (or none
/// at all) — so the read path is deliberately narrower and more
/// forgiving than [`ProcessErrorDetails`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcessErrorStreams {
    pub stdout: String,
    pub stderr: String,
}

impl ProcessErrorStreams {
    /// Extract `stdout` / `stderr` from a `ToolError::details` value.
    /// Returns `None` when neither stream carries any text, so callers
    /// can skip the whole diagnostics block with one `let else`.
    #[must_use]
    pub fn from_details(details: Option<&Value>) -> Option<Self> {
        let object = details?.as_object()?;
        let streams = Self {
            stdout: string_field(object, STDOUT_KEY),
            stderr: string_field(object, STDERR_KEY),
        };
        if streams.is_empty() {
            return None;
        }
        Some(streams)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stdout.is_empty() && self.stderr.is_empty()
    }
}

fn string_field(object: &Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn 상세(termination: ProcessTermination) -> ProcessErrorDetails {
        ProcessErrorDetails {
            termination,
            stdout: "out".to_string(),
            stderr: "err".to_string(),
        }
    }

    #[test]
    fn 정상_종료는_exit_code만_담는다() {
        let value = 상세(ProcessTermination::Exited { code: 1 }).to_value();
        assert_eq!(value[EXIT_CODE_KEY], Value::from(1));
        assert_eq!(value.get(SIGNAL_KEY), None);
        assert_eq!(value.get(TIMED_OUT_KEY), None);
        assert_eq!(value[STDOUT_KEY], Value::from("out"));
        assert_eq!(value[STDERR_KEY], Value::from("err"));
    }

    #[test]
    fn 시그널_종료는_exit_code가_널이고_signal을_담는다() {
        let value = 상세(ProcessTermination::Signaled { signal: 9 }).to_value();
        assert_eq!(value[EXIT_CODE_KEY], Value::Null);
        assert_eq!(value[SIGNAL_KEY], Value::from(9));
    }

    #[test]
    fn 제한시간_초과는_timed_out_참을_담는다() {
        let value = 상세(ProcessTermination::TimedOut { timeout_ms: 200 }).to_value();
        assert_eq!(value[EXIT_CODE_KEY], Value::Null);
        assert_eq!(value[TIMED_OUT_KEY], Value::Bool(true));
    }

    #[test]
    fn 스트림_읽기는_빈_details를_건너뛴다() {
        assert_eq!(ProcessErrorStreams::from_details(None), None);
        assert_eq!(
            ProcessErrorStreams::from_details(Some(&serde_json::json!({"exit_code": 1}))),
            None
        );
    }

    #[test]
    fn 스트림_읽기는_문자열_필드만_취한다() {
        let details = serde_json::json!({"stdout": "o", "stderr": 5});
        let streams =
            ProcessErrorStreams::from_details(Some(&details)).expect("stdout이 비어있지 않다");
        assert_eq!(streams.stdout, "o");
        assert_eq!(streams.stderr, "");
    }
}
