use crate::domain::execution::dispatch;
use crate::domain::execution::dispatch::LiveOutput;
use crate::error::CliError;
use upeg_core::Surface;

use super::file_output::FileOutputOptions;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CallOutputMode {
    Primary,
    Json,
    Field(String),
    Pretty,
}

/// How one `upeg call` result reaches the operator: the stdout rendering
/// [`CallOutputMode`] plus where any `File` primary output is written
/// ([`FileOutputOptions`]). The two are only ever decided and consumed
/// together for a single call, so they travel as one argument.
pub(super) struct CallOutputPlan {
    pub(super) mode: CallOutputMode,
    pub(super) file: FileOutputOptions,
}

impl CallOutputMode {
    pub(super) fn from_flags(json: bool, field: Option<String>, pretty: bool) -> Self {
        if json {
            Self::Json
        } else if let Some(field) = field {
            Self::Field(field)
        } else if pretty {
            Self::Pretty
        } else {
            Self::Primary
        }
    }

    /// Whether this rendering may have the running tool's output
    /// mirrored to stderr while it works.
    ///
    /// The split is machine-vs-human, not verbose-vs-quiet: `--json` and
    /// `--field` exist to be parsed by something, and that something has
    /// nowhere to put progress text. The two human renderings get it,
    /// because a ten-minute build with a silent terminal is the very
    /// complaint this answers.
    pub(super) const fn live_output(&self) -> LiveOutput {
        match self {
            Self::Primary | Self::Pretty => LiveOutput::Terminal,
            Self::Json | Self::Field(_) => LiveOutput::Silent,
        }
    }
}

pub(super) fn run_dispatch_outcome(
    tool_id: String,
    outcome: dispatch::Outcome,
    mode: CallOutputMode,
) -> Result<String, CliError> {
    match outcome {
        dispatch::Outcome::Success(success) => render_call_success(&success, &mode),
        dispatch::Outcome::Failure(failure) => render_call_failure(&failure, &mode),
        dispatch::Outcome::NotFound => render_call_not_found(tool_id, &mode),
    }
}

fn render_call_success(
    success: &upeg_core::ToolSuccess,
    mode: &CallOutputMode,
) -> Result<String, CliError> {
    match mode {
        CallOutputMode::Primary => Ok(format!("{}\n", dispatch::success_primary_text(success))),
        CallOutputMode::Json => Ok(json_line(success.to_canonical_json())),
        CallOutputMode::Field(id) => render_call_field(success, id),
        CallOutputMode::Pretty => Ok(render_pretty_success(success)),
    }
}

fn render_call_failure(
    failure: &upeg_core::ToolFailure,
    mode: &CallOutputMode,
) -> Result<String, CliError> {
    match mode {
        CallOutputMode::Json => Err(CliError::stdout_failure(json_line(canonical_failure_json(
            failure,
        )))),
        CallOutputMode::Primary | CallOutputMode::Field(_) | CallOutputMode::Pretty => {
            // Human mode gets the full diagnostics, not just the
            // summary line: a failing `cargo clippy` is useless without
            // the stdout it wrote. `CliError::message()` prefixes
            // `upeg: ` and main writes the whole thing to stderr.
            Err(CliError::tool_failed(dispatch::failure_text(failure)))
        }
    }
}

fn render_call_not_found(tool_id: String, mode: &CallOutputMode) -> Result<String, CliError> {
    if !matches!(mode, CallOutputMode::Json) {
        return Err(CliError::UnknownTool(tool_id));
    }
    let mut message = format!("unknown tool `{}`", crate::display_id(&tool_id));
    message.push_str(&crate::unknown_tool_hint(&tool_id, Some(Surface::Cli)));
    let failure = dispatch::dispatch_failure("unknown_tool", message);
    Err(CliError::stdout_failure(json_line(canonical_failure_json(
        &failure,
    ))))
}

fn render_call_field(success: &upeg_core::ToolSuccess, id: &str) -> Result<String, CliError> {
    let Some(entry) = success.outputs.iter().find(|entry| entry.id == id) else {
        return Err(CliError::tool_failed(format!(
            "output field `{id}` not found; expected {}",
            expected_output_ids(success)
        )));
    };
    Ok(format!("{}\n", dispatch::output_value_text(&entry.value)))
}

/// Indent applied to each line of a multi-line `--pretty` value.
const PRETTY_CONTINUATION_INDENT: &str = "  ";

fn render_pretty_success(success: &upeg_core::ToolSuccess) -> String {
    let mut out = String::new();
    for entry in &success.outputs {
        let label = entry.label.as_deref().unwrap_or(entry.id.as_str());
        push_pretty_row(&mut out, label, &dispatch::output_value_text(&entry.value));
    }
    out
}

/// One `--pretty` row.
///
/// A single-line value stays on the label line (`label: value`). A
/// multi-line value — a diff, a file listing, a compiler log — moves to
/// indented lines under a bare `label:`, because continuation lines
/// with no label and no indent are indistinguishable from the next
/// field's value.
fn push_pretty_row(out: &mut String, label: &str, value: &str) {
    out.push_str(label);
    out.push(':');
    if !value.contains('\n') {
        out.push(' ');
        out.push_str(value);
        out.push('\n');
        return;
    }
    out.push('\n');
    // A value that ends in a newline (every command's stdout does) would
    // otherwise render one trailing line of bare indentation.
    for line in value.strip_suffix('\n').unwrap_or(value).split('\n') {
        out.push_str(PRETTY_CONTINUATION_INDENT);
        out.push_str(line);
        out.push('\n');
    }
}

fn expected_output_ids(success: &upeg_core::ToolSuccess) -> String {
    let ids = success
        .outputs
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();
    if ids.is_empty() {
        "no output fields".to_string()
    } else {
        ids.join(", ")
    }
}

fn canonical_failure_json(failure: &upeg_core::ToolFailure) -> serde_json::Value {
    let mut value = failure.to_canonical_json();
    if let serde_json::Value::Object(object) = &mut value {
        object.insert("primary_output_id".to_string(), serde_json::Value::Null);
        object.insert("outputs".to_string(), serde_json::Value::Array(Vec::new()));
    }
    value
}

fn json_line(value: serde_json::Value) -> String {
    let mut out = serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string());
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use upeg_core::{
        OutputEntry, OutputKind, OutputValue, ProcessErrorDetails, ProcessTermination, ToolError,
        ToolFailure, ToolSuccess,
    };

    use super::{CallOutputMode, render_call_failure, render_pretty_success};
    use crate::error::CliError;

    const 여러_줄_값: &str = "first\nsecond";

    fn 성공(id: &str, value: &str) -> ToolSuccess {
        ToolSuccess::new(
            Some(id.to_string()),
            vec![OutputEntry {
                id: id.to_string(),
                label: None,
                kind: OutputKind::String,
                value: OutputValue::String(value.to_string()),
            }],
        )
        .expect("테스트 성공 결과는 유효하다")
    }

    fn 외부_실패() -> ToolFailure {
        ToolFailure {
            error: ToolError {
                code: "tool_error".to_string(),
                message: "`cargo` exited with code 1".to_string(),
                details: Some(
                    ProcessErrorDetails {
                        termination: ProcessTermination::Exited { code: 1 },
                        stdout: "warning: unused import\n".to_string(),
                        stderr: "error: could not compile\n".to_string(),
                    }
                    .to_value(),
                ),
            },
        }
    }

    #[test]
    fn 사람용_모드만_실행_중_출력을_흘려보낸다() {
        use crate::domain::execution::dispatch::LiveOutput;

        assert_eq!(CallOutputMode::Primary.live_output(), LiveOutput::Terminal);
        assert_eq!(CallOutputMode::Pretty.live_output(), LiveOutput::Terminal);
        assert_eq!(CallOutputMode::Json.live_output(), LiveOutput::Silent);
        assert_eq!(
            CallOutputMode::Field("result".to_string()).live_output(),
            LiveOutput::Silent
        );
    }

    #[test]
    fn pretty는_한_줄_값을_라벨_뒤에_붙인다() {
        assert_eq!(render_pretty_success(&성공("result", "42")), "result: 42\n");
    }

    #[test]
    fn pretty는_값의_마지막_줄바꿈으로_빈_줄을_만들지_않는다() {
        assert_eq!(
            render_pretty_success(&성공("result", "a\nb\n")),
            "result:\n  a\n  b\n"
        );
    }

    #[test]
    fn pretty는_여러_줄_값을_들여쓴다() {
        // Continuation lines with no label and no indent are
        // indistinguishable from the next field's value.
        assert_eq!(
            render_pretty_success(&성공("result", 여러_줄_값)),
            "result:\n  first\n  second\n"
        );
    }

    #[test]
    fn 사람용_실패는_표준오류와_표준출력을_라벨과_함께_보여준다() {
        let error = render_call_failure(&외부_실패(), &CallOutputMode::Primary)
            .expect_err("실패는 CliError가 된다");
        let message = error.message();
        assert!(
            message.starts_with("upeg: `cargo` exited with code 1"),
            "{message}"
        );
        assert!(
            message.contains("stderr:\nerror: could not compile"),
            "{message}"
        );
        assert!(
            message.contains("stdout:\nwarning: unused import"),
            "{message}"
        );
    }

    #[test]
    fn json_실패는_details를_그대로_싣는다() {
        let error = render_call_failure(&외부_실패(), &CallOutputMode::Json)
            .expect_err("실패는 CliError가 된다");
        let CliError::StdoutFailure { stdout } = error else {
            panic!("--json 실패는 stdout 페이로드를 쓴다");
        };
        let payload: serde_json::Value =
            serde_json::from_str(&stdout).expect("stdout은 canonical JSON 한 줄이다");
        assert_eq!(payload["ok"], json!(false));
        assert_eq!(payload["error"]["details"]["exit_code"], json!(1));
        assert_eq!(
            payload["error"]["details"]["stdout"],
            json!("warning: unused import\n")
        );
    }

    #[test]
    fn details가_없는_실패는_메시지만_남긴다() {
        let failure = ToolFailure {
            error: ToolError {
                code: "tool_error".to_string(),
                message: "boom".to_string(),
                details: None,
            },
        };
        let error = render_call_failure(&failure, &CallOutputMode::Primary)
            .expect_err("실패는 CliError가 된다");
        assert_eq!(error.message(), "upeg: boom");
    }
}
