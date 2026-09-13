//! Shared Tool dispatch — single entry point that turns a `(tool_id, args)`
//! pair into a canonical result. Every surface (CLI/MCP/HTTP/TUI/daemon)
//! routes through here.
//!
//! dispatch is now uniform. Built-in tools register typed
//! `Fn(DispatchArgs) -> ToolResult` closures via
//! `upeg_runtime::register_runtime_dispatcher`, so chain Tools, WASM Toolkits,
//! and TOML-loaded External/HTTP/LLM tools all flow through the same lookup. The
//! one-time built-in registration is fired lazily by `dispatch_tool`
//! itself — tests and surfaces don't have to call it explicitly.
//!
//! registration was moved into `upeg-tools::dispatch::register_all`
//! so non-CLI surfaces (notably the wasm32 desktop UI) can wire built-ins
//! without depending on the CLI's axum/extism transitives.
//!
//! Returns `Outcome` (sentinel-style) so callers can format protocol
//! responses their own way (MCP: `content`+`isError`, HTTP: `200`+JSON
//! body, CLI: stdout+exit code).

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::Value;
use upeg_core::{ProcessErrorStreams, ToolFailure, ToolResult, ToolSuccess};
use upeg_runtime::{ProgressEvent, SharedProgressSink, with_progress_sink};
use upeg_tools::RegisteredDispatch;

// Canonical output-text conversion lives in `upeg-runtime`; the CLI keeps
// these named entry points so existing callers (surfaces, adapters) don't
// have to learn a new import path.
pub use upeg_runtime::output_value_text;
pub use upeg_runtime::tool_success_primary_text as success_primary_text;

/// Where a running tool's incremental output goes on this surface.
///
/// The rule this encodes: **stdout belongs to the result.** A machine
/// reading `--json` or `--field` must never have progress text mixed
/// into what it parses, and it has no use for it anyway — so those modes
/// print nothing while the tool runs. A human running the same tool gets
/// the child's stdout *and* stderr mirrored to the terminal's stderr as
/// they arrive, prefix-free, so a ten-minute build looks alive; stdout
/// still carries exactly the final primary output it always did.
///
/// The consequence is deliberate: in the human modes a successful
/// command's stdout is seen twice — live on stderr while it runs, then
/// once more on stdout as the result. Keeping stdout a clean, single
/// representation of the result is worth the echo, and redirecting
/// stderr (`2>/dev/null`) removes it.
///
/// Not gated on `isatty`: piping progress to a file or a log collector
/// is a legitimate thing to want, and a TTY check would make the
/// behaviour untestable and surprising under `tee`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveOutput {
    /// Mirror chunks to this process's stderr as they arrive.
    Terminal,
    /// Print nothing while the tool runs (machine-readable modes).
    Silent,
}

/// Run `f` with this surface's live-output policy installed.
///
/// Everything dispatched inside `f` — including a `Chain` tool's steps,
/// which run on this same thread — reports through the installed sink.
/// [`LiveOutput::Silent`] installs nothing at all, so the invoker skips
/// its forwarding work entirely.
pub fn with_live_output<T>(live: LiveOutput, f: impl FnOnce() -> T) -> T {
    match live {
        LiveOutput::Silent => f(),
        LiveOutput::Terminal => {
            with_progress_sink(mirror_sink(Arc::new(Mutex::new(std::io::stderr()))), f)
        }
    }
}

/// A progress sink that writes every chunk verbatim to `writer`.
///
/// Verbatim is the whole point: the chunks already carry the child's own
/// newlines and carriage returns, so a `cargo` progress bar redraws and
/// a compiler diagnostic keeps the exact bytes a user would copy. A
/// poisoned lock or a closed pipe is swallowed — progress is
/// best-effort, and the final envelope is the contract.
pub(crate) fn mirror_sink<W>(writer: Arc<Mutex<W>>) -> SharedProgressSink
where
    W: std::io::Write + Send + 'static,
{
    Arc::new(move |event: ProgressEvent| {
        let Ok(mut writer) = writer.lock() else {
            return;
        };
        let _ = writer.write_all(event.chunk.as_bytes());
        let _ = writer.flush();
    })
}

/// Tool dispatch outcome.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Tool ran and returned canonical outputs.
    Success(ToolSuccess),
    /// Tool is registered but returned a canonical failure.
    Failure(ToolFailure),
    /// `id` is not in the registry at all.
    NotFound,
}

/// Run the named Tool with the given arguments.
///
/// Lookup order:
///   1. `toolbox_tool(id)` — confirms the tool exists in the registry
///      (built-in inventory or runtime). Missing → `NotFound`.
///   2. `try_runtime_dispatch(id, args)` — built-ins, TOML-loaded External
///      tools, chain Tools, and WASM Toolkits all live here.
///   3. Tool exists but no dispatcher → `Failure(dispatch not implemented…)`.
///      Reachable only when meta is registered without a paired dispatcher.
pub fn dispatch_tool(id: &str, args: &Value) -> Outcome {
    let started = Instant::now();
    let outcome = match upeg_tools::dispatch_registered(id, args) {
        RegisteredDispatch::NotFound => Outcome::NotFound,
        RegisteredDispatch::Ran(ToolResult::Success(success)) => Outcome::Success(success),
        RegisteredDispatch::Ran(ToolResult::Failure(failure)) => Outcome::Failure(failure),
        RegisteredDispatch::Unimplemented => Outcome::Failure(dispatch_failure(
            "dispatch_not_implemented",
            format!("dispatch not implemented for `{id}`"),
        )),
    };
    crate::adapters::execution_log::record_dispatch(id, args, &outcome, started.elapsed());
    outcome
}

impl Outcome {
    pub fn primary_text(&self) -> Option<String> {
        match self {
            Self::Success(success) => Some(success_primary_text(success)),
            Self::Failure(failure) => Some(failure.error.message.clone()),
            Self::NotFound => None,
        }
    }
}

/// Label prefixing each captured stream in the human-readable failure
/// rendering.
const STDERR_DIAGNOSTIC_LABEL: &str = "stderr:";
const STDOUT_DIAGNOSTIC_LABEL: &str = "stdout:";

/// Full operator-facing text for a failure: the tool's own message,
/// followed by whatever the child process wrote.
///
/// Every non-JSON surface uses this one renderer so a `cargo clippy`
/// failure reads the same in the shell, in the TUI, and in an MCP
/// client. Streams are labeled but not indented — compiler diagnostics
/// are meant to be copied verbatim.
#[must_use]
pub fn failure_text(failure: &ToolFailure) -> String {
    let mut text = failure.error.message.clone();
    let Some(streams) = ProcessErrorStreams::from_details(failure.error.details.as_ref()) else {
        return text;
    };
    for (label, stream) in [
        (STDERR_DIAGNOSTIC_LABEL, &streams.stderr),
        (STDOUT_DIAGNOSTIC_LABEL, &streams.stdout),
    ] {
        if stream.is_empty() {
            continue;
        }
        text.push('\n');
        text.push_str(label);
        text.push('\n');
        text.push_str(stream.trim_end_matches('\n'));
    }
    text
}

pub fn dispatch_failure(code: impl Into<String>, message: impl Into<String>) -> ToolFailure {
    upeg_core::ToolFailure {
        error: upeg_core::ToolError {
            code: code.into(),
            message: message.into(),
            details: None,
        },
    }
}

#[allow(
    clippy::unreachable,
    reason = "cli.text_result is a fixed valid runtime text result id, so fallback success cannot fail"
)]
pub fn text_success(text: impl Into<String>) -> ToolSuccess {
    match upeg_runtime::single_text_result("cli.text_result", Ok(text.into())) {
        ToolResult::Success(success) => success,
        ToolResult::Failure(_) => unreachable!("fallback text success cannot fail"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use upeg_runtime::{ProgressReporter, ProgressStream};

    fn 거울에_비친_글자(live_body: impl FnOnce(&Arc<Mutex<Vec<u8>>>)) -> String {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        live_body(&buffer);
        let bytes = buffer.lock().expect("거울 버퍼 잠금").clone();
        String::from_utf8(bytes).expect("거울 버퍼는 UTF-8이다")
    }

    #[test]
    fn 거울_sink는_청크를_그대로_이어_붙인다() {
        let text = 거울에_비친_글자(|buffer| {
            let reporter = ProgressReporter::new(mirror_sink(Arc::clone(buffer)));
            reporter.report(ProgressStream::Stderr, "compiling\n".to_string());
            reporter.report(ProgressStream::Stdout, "done\n".to_string());
        });

        assert_eq!(text, "compiling\ndone\n");
    }

    #[test]
    fn 조용한_모드는_sink를_설치하지_않는다() {
        let installed = with_live_output(LiveOutput::Silent, || {
            upeg_runtime::active_progress_sink().is_some()
        });
        assert!(
            !installed,
            "--json/--field 경로는 기계용이라 아무것도 흘리지 않는다"
        );
    }

    #[test]
    fn 터미널_모드는_dispatch_동안_sink를_설치한다() {
        let installed = with_live_output(LiveOutput::Terminal, || {
            upeg_runtime::active_progress_sink().is_some()
        });
        assert!(installed);
        assert!(
            upeg_runtime::active_progress_sink().is_none(),
            "sink은 호출 범위를 넘겨 살아남지 않는다"
        );
    }

    #[test]
    fn hex_로_dec_정상을_검증한다() {
        let outcome = dispatch_tool("num.hex_to_decimal", &json!({"input": "0xff"}));
        assert_eq!(outcome.primary_text().as_deref(), Some("255"));
    }

    #[test]
    fn hex_로_dec_도구_오류를_검증한다() {
        match dispatch_tool("num.hex_to_decimal", &json!({"input": "0xZZ"})) {
            Outcome::Failure(failure) => assert!(failure.error.message.contains("invalid hex")),
            other => panic!("expected ToolError, got {other:?}"),
        }
    }

    #[test]
    fn 알수없는_도구는_아닌_발견됨을_반환한다() {
        assert_eq!(dispatch_tool("no.such.tool", &json!({})), Outcome::NotFound);
    }

    #[test]
    fn base64_왕복_왕복을_검증한다() {
        let enc = dispatch_tool("convert.base64_encode", &json!({"input": "hello"}));
        assert_eq!(enc.primary_text().as_deref(), Some("aGVsbG8="));
        let dec = dispatch_tool("convert.base64_decode", &json!({"input": "aGVsbG8="}));
        assert_eq!(dec.primary_text().as_deref(), Some("hello"));
    }

    #[test]
    fn 누락된_필수_인자는_schema_검증을_실패한다() {
        match dispatch_tool("num.hex_to_decimal", &json!({})) {
            Outcome::Failure(failure) => assert!(failure.error.message.contains("is required")),
            other => panic!("expected ToolError, got {other:?}"),
        }
    }

    #[test]
    fn uuid_v7는_36_문자_정규_문자열을_반환한다() {
        match dispatch_tool("id.uuid_v7", &json!({})) {
            Outcome::Success(success) => {
                let s = success_primary_text(&success);
                assert_eq!(s.len(), 36);
                assert_eq!(s.chars().nth(14), Some('7'));
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    /// Register a runtime tool + dispatcher and confirm `dispatch_tool` routes
    /// to it. This is the runtime adapter contract: a tool added at runtime
    /// (TOML/WASM) must be callable without touching the hardcoded match.
    #[test]
    fn dispatch는_내장이_아닌에_대해_로_runtime_dispatcher를_라우팅한다() {
        let id = "test.dispatch.runtime_echo";
        upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
            id,
            toolkit: "test",
            local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
                .expect("test ToolMeta id must be canonical")
                .local(),
            tags: &[],
            display_label: "Test tool",
            description: "",
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            source: upeg_core::Source::UserInput,
            pin: upeg_core::PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: upeg_core::Invoker::Function,
            surfaces: upeg_core::ALL_SURFACES,
            boards: &[],
        });
        upeg_runtime::register_single_text_runtime_dispatcher(id, |args| {
            let s = args.get("msg").and_then(|v| v.as_str()).unwrap_or("");
            Ok(format!("got: {s}"))
        });

        let outcome = dispatch_tool(id, &json!({"msg": "iter37"}));
        assert_eq!(outcome.primary_text().as_deref(), Some("got: iter37"));
    }

    #[test]
    fn dispatch는_runtime_dispatcher_오류를_도구_오류로_전파한다() {
        let id = "test.dispatch.runtime_err";
        upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
            id,
            toolkit: "test",
            local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
                .expect("test ToolMeta id must be canonical")
                .local(),
            tags: &[],
            display_label: "Test tool",
            description: "",
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            source: upeg_core::Source::UserInput,
            pin: upeg_core::PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: upeg_core::Invoker::Function,
            surfaces: upeg_core::ALL_SURFACES,
            boards: &[],
        });
        upeg_runtime::register_single_text_runtime_dispatcher(id, |_| Err("plugin said no".into()));

        match dispatch_tool(id, &json!({})) {
            Outcome::Failure(failure) => assert_eq!(failure.error.message, "plugin said no"),
            other => panic!("expected ToolError, got {other:?}"),
        }
    }

    #[test]
    fn 내장_등록_보장은_멱등이다() {
        // Two calls in a row must be cheap and must leave the registry
        // pointing at the same closures (last write wins, same closure body
        // each time → equivalent behavior).
        upeg_tools::register_all();
        upeg_tools::register_all();
        // After init, hex_to_decimal resolves through try_runtime_dispatch — proves
        // the built-in took the runtime-dispatcher path rather than the old
        // hardcoded match.
        let r = upeg_runtime::try_runtime_dispatch("num.hex_to_decimal", &json!({"input": "0x10"}));
        assert_eq!(r.map(upeg_runtime::tool_result_text), Some(Ok("16".into())));
    }

    #[test]
    fn dispatch_runtime가_없는_dispatcher는_아닌_구현된을_반환한다() {
        // Tool is in the toolbox but has no dispatcher and isn't a built-in.
        let id = "test.dispatch.no_handler";
        upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
            id,
            toolkit: "test",
            local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
                .expect("test ToolMeta id must be canonical")
                .local(),
            tags: &[],
            display_label: "Test tool",
            description: "",
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            source: upeg_core::Source::UserInput,
            pin: upeg_core::PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: upeg_core::Invoker::Function,
            surfaces: upeg_core::ALL_SURFACES,
            boards: &[],
        });
        match dispatch_tool(id, &json!({})) {
            Outcome::Failure(failure) => assert!(
                failure.error.message.contains("dispatch not implemented"),
                "expected `not implemented`, got: {}",
                failure.error.message,
            ),
            other => panic!("expected ToolError, got {other:?}"),
        }
    }
}
