mod gates;
mod report;

use crate::domain::execution::context::{ExecutionContext, prepare_tool_args};
use crate::domain::execution::dispatch;
use gates::{TriggerKey, TriggerWatchState};
use report::{
    FIELD_SEPARATOR, FireStatus, ROW_TERMINATOR, TriggerOutcome, TriggerReport, WatchPrinter,
    format_status_report,
};
use std::io::Write as _;
use std::path::Path;
use std::time::{Instant, SystemTime};
use upeg_core::Surface;
use upeg_runtime::{
    FiredTrigger, ScheduleCondition, TriggerBinding, TriggerSource, apply_trigger_context,
};

/// How often [`run_watch`] re-polls every registered trigger.
///
/// Derived from the runtime's own constant rather than re-declared: it is also
/// the resolution the loader validates `every:<duration>` against
/// ([`upeg_runtime::WATCH_POLL_INTERVAL`]), so a local copy could drift into
/// accepting schedules this loop can never honor.
const WATCH_POLL_INTERVAL: std::time::Duration = upeg_runtime::WATCH_POLL_INTERVAL;

/// The condition column of `upeg trigger list` when a trigger declares none.
///
/// Emitted as an empty field rather than omitted: a TSV row whose arity depends
/// on the data cannot be split by column. No declared condition is ever blank
/// (the loader rejects one), so the empty field is unambiguous.
const ABSENT_CONDITION_FIELD: &str = "";

/// Columns in one `upeg trigger list` row: tool id, source, host support,
/// condition, diagnostic. Fixed for every row, whatever the binding declares —
/// the row is built as an array of this width so the compiler enforces it.
const TRIGGER_LIST_COLUMNS: usize = 5;

/// Host-support column for a source the built-in trigger runtime services.
const HOST_SUPPORTED_FIELD: &str = "supported";
/// Host-support column for a source needing a platform adapter this host lacks.
const HOST_UNSUPPORTED_FIELD: &str = "unsupported-host";

/// Stand-in modification time for hosts whose filesystem reports no mtime.
/// The path still counts as *present* (so creation fires) but a constant stamp
/// never changes, so such a host simply never sees "modified".
const MTIME_UNAVAILABLE: SystemTime = SystemTime::UNIX_EPOCH;

/// Which filesystem entry kind a path trigger watches.
///
/// Keeps the `file` / `directory` split typed instead of threading a predicate
/// closure plus a wire string through every call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathTriggerKind {
    File,
    Directory,
}

impl PathTriggerKind {
    /// The trigger source this kind services.
    const fn source(self) -> TriggerSource {
        match self {
            Self::File => TriggerSource::File,
            Self::Directory => TriggerSource::Directory,
        }
    }

    /// Whether an existing path's metadata is of the watched kind.
    fn accepts(self, metadata: &std::fs::Metadata) -> bool {
        match self {
            Self::File => metadata.is_file(),
            Self::Directory => metadata.is_dir(),
        }
    }
}

/// Observe a watched path: `None` when it is absent or is not of the declared
/// kind, `Some(mtime)` when it is present.
fn observe_path_mtime(path: &Path, kind: PathTriggerKind) -> Option<SystemTime> {
    let metadata = std::fs::metadata(path).ok()?;
    if !kind.accepts(&metadata) {
        return None;
    }
    Some(metadata.modified().unwrap_or(MTIME_UNAVAILABLE))
}

pub fn format_list(json: bool) -> String {
    let triggers = upeg_runtime::registered_trigger_bindings();
    if json {
        let values: Vec<_> = triggers
            .iter()
            .map(|trigger| {
                serde_json::json!({
                    "toolId": trigger.tool_id,
                    "source": trigger.source,
                    "condition": trigger.condition,
                    "runtimeSupported": upeg_runtime::trigger_source_has_builtin_runtime(&trigger.source),
                    "diagnostic": upeg_runtime::trigger_source_diagnostic(&trigger.source),
                })
            })
            .collect();
        let mut out = serde_json::to_string_pretty(&values).unwrap_or_else(|_| "[]".into());
        out.push('\n');
        return out;
    }
    let mut out = String::new();
    for trigger in triggers {
        let row: [&str; TRIGGER_LIST_COLUMNS] = [
            trigger.tool_id,
            &trigger.source,
            if upeg_runtime::trigger_source_has_builtin_runtime(&trigger.source) {
                HOST_SUPPORTED_FIELD
            } else {
                HOST_UNSUPPORTED_FIELD
            },
            trigger
                .condition
                .as_deref()
                .unwrap_or(ABSENT_CONDITION_FIELD),
            upeg_runtime::trigger_source_diagnostic(&trigger.source),
        ];
        out.push_str(&row.join(FIELD_SEPARATOR));
        out.push_str(ROW_TERMINATOR);
    }
    out
}

pub fn run_once(active_board: Option<&str>) -> String {
    // Single-shot invocations carry no cross-poll state: every trigger whose
    // condition holds *right now* fires once. The watch loop supplies the gates
    // that turn "holds" into "just changed".
    format_status_report(&run_once_inner(active_board, &mut None))
}

fn run_once_inner(
    active_board: Option<&str>,
    state: &mut Option<TriggerWatchState>,
) -> Vec<TriggerReport> {
    let now = Instant::now();
    // One clipboard read serves every clipboard trigger in this poll.
    let mut clipboard = ClipboardPoll::new(&read_clipboard_text);
    upeg_runtime::registered_trigger_bindings()
        .into_iter()
        .map(|trigger| {
            let outcome = match runtime_args(&trigger, state, now, &mut clipboard) {
                Ok(Some(args)) => dispatch_fired(&trigger, args, active_board),
                Ok(None) => TriggerOutcome::Idle {
                    diagnostic: upeg_runtime::trigger_source_diagnostic(&trigger.source),
                },
                Err(diagnostic) => TriggerOutcome::Unsupported { diagnostic },
            };
            TriggerReport::new(TriggerKey::from_binding(&trigger), outcome)
        })
        .collect()
}

/// Dispatch a trigger whose condition just held, stamping the context that says
/// *which* trigger started the call.
fn dispatch_fired(
    trigger: &TriggerBinding,
    args: serde_json::Value,
    active_board: Option<&str>,
) -> TriggerOutcome {
    let mut args = apply_cli_execution_context(args, active_board, trigger.tool_id);
    args = apply_trigger_context(args, &fired_label(trigger));
    let outcome = match crate::app::cli_surface_error(trigger.tool_id) {
        Some(message) => {
            dispatch::Outcome::Failure(dispatch::dispatch_failure("surface_error", message))
        }
        None => dispatch::dispatch_tool(trigger.tool_id, &args),
    };
    match outcome {
        dispatch::Outcome::Success(success) => TriggerOutcome::Fired {
            status: FireStatus::Ok,
            text: Some(dispatch::success_primary_text(&success)),
        },
        dispatch::Outcome::Failure(_) => TriggerOutcome::Fired {
            status: FireStatus::ToolError,
            text: None,
        },
        dispatch::Outcome::NotFound => TriggerOutcome::Fired {
            status: FireStatus::NotFound,
            text: None,
        },
    }
}

/// The `_upeg.trigger` label for a binding that just fired.
///
/// A binding whose source does not parse is reported as `unsupported` by
/// [`runtime_args`] and never reaches a dispatch, so the raw-source fallback
/// cannot be observed in practice.
fn fired_label(trigger: &TriggerBinding) -> String {
    FiredTrigger::from_binding(trigger).map_or_else(|unknown| unknown.0, |fired| fired.to_string())
}

pub fn run_watch(active_board: Option<&str>) -> ! {
    // Global hotkeys are event-driven, not poll-driven: when the opt-in
    // `hotkey-trigger` feature is built, service them on a background thread so
    // the poll loop below is untouched. Headless hosts log a clear diagnostic
    // and leave the rest of the watch running.
    #[cfg(feature = "hotkey-trigger")]
    crate::adapters::hotkey::spawn_watch(active_board);

    let mut state = Some(TriggerWatchState::default());
    let mut printer = WatchPrinter::default();
    #[allow(
        clippy::print_stdout,
        reason = "`run_watch` is the user-facing CLI watch loop; stdout is its product"
    )]
    loop {
        print!(
            "{}",
            printer.render(&run_once_inner(active_board, &mut state))
        );
        let _ = std::io::stdout().flush();
        std::thread::sleep(WATCH_POLL_INTERVAL);
    }
}

/// The one clipboard observation a poll makes, shared by every clipboard trigger.
///
/// [`read_clipboard_text`] shells out to a platform command, so reading it per
/// binding meant spawning a subprocess per clipboard trigger per second. The
/// read happens at most once per poll, and lazily: a poll with no clipboard
/// trigger spawns nothing at all.
struct ClipboardPoll<'a> {
    read: &'a dyn Fn() -> Result<String, String>,
    observed: Option<Result<String, String>>,
}

impl<'a> ClipboardPoll<'a> {
    const fn new(read: &'a dyn Fn() -> Result<String, String>) -> Self {
        Self {
            read,
            observed: None,
        }
    }

    /// This poll's clipboard text, reading it on first use.
    fn observe(&mut self) -> Result<&str, String> {
        let observed = self.observed.get_or_insert_with(|| (self.read)());
        match observed {
            Ok(text) => Ok(text.as_str()),
            Err(diagnostic) => Err(diagnostic.clone()),
        }
    }
}

fn runtime_args(
    trigger: &TriggerBinding,
    state: &mut Option<TriggerWatchState>,
    now: Instant,
    clipboard: &mut ClipboardPoll<'_>,
) -> Result<Option<serde_json::Value>, String> {
    match trigger.source.parse::<TriggerSource>() {
        Ok(TriggerSource::Webhook) => Ok(None),
        Ok(TriggerSource::Schedule) => schedule_trigger_args(trigger, state, now),
        Ok(TriggerSource::File) => {
            path_trigger_args(trigger, PathTriggerKind::File, state, observe_path_mtime)
        }
        Ok(TriggerSource::Directory) => path_trigger_args(
            trigger,
            PathTriggerKind::Directory,
            state,
            observe_path_mtime,
        ),
        Ok(TriggerSource::Clipboard) => clipboard.observe().map(|text| {
            let fire_text = match state {
                // Watch loop: only fire when *this* trigger's last seen value changes.
                Some(state) => state
                    .clipboard
                    .evaluate(&TriggerKey::from_binding(trigger), text),
                // Single-shot: fire whenever the clipboard is non-empty.
                None => (!text.is_empty()).then(|| text.to_string()),
            };
            fire_text.map(|text| serde_json::json!({ "input": text }))
        }),
        // Host-bound sources have no built-in runtime; surface the enum's own
        // diagnostic rather than re-deriving it from the wire string.
        Ok(source @ TriggerSource::Hotkey) => Err(source.diagnostic().to_string()),
        Err(unknown) => Err(format!("unknown trigger source `{}`", unknown.0)),
    }
}

/// `schedule` triggers carry no payload; the whole decision is *whether* this
/// poll is the one that fires.
fn schedule_trigger_args(
    trigger: &TriggerBinding,
    state: &mut Option<TriggerWatchState>,
    now: Instant,
) -> Result<Option<serde_json::Value>, String> {
    // The loader rejects malformed expressions at load time; a binding
    // registered from Rust can still carry one, so parse defensively.
    let condition = ScheduleCondition::parse_optional(trigger.condition.as_deref())
        .map_err(|error| error.to_string())?;
    let due = match state {
        Some(state) => state
            .schedule
            .is_due(&TriggerKey::from_binding(trigger), condition, now),
        // Single-shot: `upeg trigger run` is itself the scheduled moment.
        None => true,
    };
    Ok(due.then(|| serde_json::json!({})))
}

/// `file` / `directory` triggers pass the watched path to the tool. `observe`
/// is injected so the arg-shaping can be tested without touching the disk.
fn path_trigger_args(
    trigger: &TriggerBinding,
    kind: PathTriggerKind,
    state: &mut Option<TriggerWatchState>,
    observe: impl Fn(&Path, PathTriggerKind) -> Option<SystemTime>,
) -> Result<Option<serde_json::Value>, String> {
    let source = kind.source().as_str();
    let path = trigger
        .condition
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("{source} trigger requires condition = path"))?;
    let observed = observe(Path::new(path), kind);
    let fires = match state {
        // Watch loop: fire on creation and on modification, never on "still there".
        Some(state) => state
            .path
            .is_changed(&TriggerKey::from_binding(trigger), observed),
        // Single-shot: fire when the path is there right now.
        None => observed.is_some(),
    };
    Ok(fires.then(|| serde_json::json!({ "path": path })))
}

fn read_clipboard_text() -> Result<String, String> {
    let commands: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbpaste", &[])]
    } else if cfg!(target_os = "windows") {
        &[("powershell", &["-NoProfile", "-Command", "Get-Clipboard"])]
    } else {
        &[
            ("wl-paste", &["--no-newline"]),
            ("xclip", &["-selection", "clipboard", "-o"]),
        ]
    };
    for (program, args) in commands {
        let output = std::process::Command::new(program).args(*args).output();
        let Ok(output) = output else {
            continue;
        };
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }
    Err("clipboard trigger requires pbpaste, wl-paste, xclip, or PowerShell Get-Clipboard on this host".into())
}

/// Trigger-fired dispatches route through the same synthesis layer as
/// direct CLI calls: board scope (+ pin preset when the tool is pinned
/// on that board) and the `cli` surface label. Invalid board input
/// degrades to a global context — a watch loop must keep firing.
pub(super) fn apply_cli_execution_context(
    args: serde_json::Value,
    active_board: Option<&str>,
    tool_id: &str,
) -> serde_json::Value {
    let context = crate::app::cli_execution_context_for(Surface::Cli, active_board, tool_id)
        .unwrap_or_else(|_| ExecutionContext::global(Surface::Cli));
    prepare_tool_args(args, &context, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::time::Duration;

    fn binding(source: TriggerSource, condition: Option<&str>) -> TriggerBinding {
        binding_for("demo.tool", source, condition)
    }

    fn binding_for(
        tool_id: &'static str,
        source: TriggerSource,
        condition: Option<&str>,
    ) -> TriggerBinding {
        TriggerBinding {
            tool_id,
            source: source.as_str().to_string(),
            condition: condition.map(str::to_string),
        }
    }

    /// A clipboard that always reads back `text`, counting every read so a test
    /// can prove the poll shells out once rather than once per binding.
    fn counting_clipboard(
        text: &'static str,
        reads: &Cell<usize>,
    ) -> impl Fn() -> Result<String, String> {
        move || {
            reads.set(reads.get() + 1);
            Ok(text.to_string())
        }
    }

    /// A fake filesystem observation: the path is present with a fixed mtime.
    fn present(mtime_secs: u64) -> impl Fn(&Path, PathTriggerKind) -> Option<SystemTime> {
        move |_, _| Some(SystemTime::UNIX_EPOCH + Duration::from_secs(mtime_secs))
    }

    /// A fake filesystem observation: the path is absent.
    fn absent(_: &Path, _: PathTriggerKind) -> Option<SystemTime> {
        None
    }

    #[test]
    fn one_shot_run_rejects_unparsable_schedule_condition() {
        let trigger = binding(TriggerSource::Schedule, Some("every:soon"));
        let error = runtime_args(
            &trigger,
            &mut None,
            Instant::now(),
            &mut ClipboardPoll::new(&read_clipboard_text),
        )
        .expect_err("an unparsable period must surface as a diagnostic");
        assert!(error.contains("every:soon"), "{error}");
    }

    #[test]
    fn watch_loop_every_schedule_does_not_fire_every_poll() {
        let trigger = binding(TriggerSource::Schedule, Some("every:1h"));
        let mut state = Some(TriggerWatchState::default());
        let start = Instant::now();
        assert!(
            runtime_args(
                &trigger,
                &mut state,
                start,
                &mut ClipboardPoll::new(&read_clipboard_text)
            )
            .expect("schedule is supported")
            .is_some(),
            "the watch-start poll must fire"
        );
        for tick in 1..4 {
            assert!(
                runtime_args(
                    &trigger,
                    &mut state,
                    start + Duration::from_secs(tick),
                    &mut ClipboardPoll::new(&read_clipboard_text)
                )
                .expect("schedule is supported")
                .is_none(),
                "a 1-hour period must not fire on the {tick}-second poll"
            );
        }
    }

    #[test]
    fn one_shot_schedule_always_fires() {
        for condition in [None, Some("now"), Some("every:1h")] {
            let trigger = binding(TriggerSource::Schedule, condition);
            assert_eq!(
                runtime_args(
                    &trigger,
                    &mut None,
                    Instant::now(),
                    &mut ClipboardPoll::new(&read_clipboard_text)
                )
                .expect("schedule is supported"),
                Some(serde_json::json!({})),
                "one-shot run must fire for {condition:?}"
            );
        }
    }

    #[test]
    fn path_trigger_without_condition_returns_diagnostic() {
        for kind in [PathTriggerKind::File, PathTriggerKind::Directory] {
            let trigger = binding(kind.source(), None);
            let error = path_trigger_args(&trigger, kind, &mut None, absent)
                .expect_err("a pathless trigger must surface as a diagnostic");
            assert!(error.contains(kind.source().as_str()), "{error}");
            assert!(error.contains("condition = path"), "{error}");
        }
    }

    #[test]
    fn one_shot_path_trigger_fires_only_when_path_present() {
        let trigger = binding(TriggerSource::File, Some("/tmp/demo.txt"));
        assert_eq!(
            path_trigger_args(&trigger, PathTriggerKind::File, &mut None, present(10))
                .expect("path triggers are supported"),
            Some(serde_json::json!({ "path": "/tmp/demo.txt" }))
        );
        assert_eq!(
            path_trigger_args(&trigger, PathTriggerKind::File, &mut None, absent)
                .expect("path triggers are supported"),
            None
        );
    }

    #[test]
    fn watch_loop_path_trigger_fires_only_on_change() {
        let trigger = binding(TriggerSource::File, Some("/tmp/demo.txt"));
        let mut state = Some(TriggerWatchState::default());
        let fire =
            |state: &mut Option<TriggerWatchState>,
             observe: &dyn Fn(&Path, PathTriggerKind) -> Option<SystemTime>| {
                path_trigger_args(&trigger, PathTriggerKind::File, state, observe)
                    .expect("path triggers are supported")
            };
        // Already present at watch start: silent baseline, then unchanged.
        assert_eq!(fire(&mut state, &present(10)), None);
        assert_eq!(fire(&mut state, &present(10)), None);
        // Modified: fires with the watched path.
        assert_eq!(
            fire(&mut state, &present(11)),
            Some(serde_json::json!({ "path": "/tmp/demo.txt" }))
        );
        // Removed: silent.
        assert_eq!(fire(&mut state, &absent), None);
    }

    #[test]
    fn hotkey_trigger_returns_host_adapter_diagnostic() {
        let trigger = binding(TriggerSource::Hotkey, Some("ctrl+shift+u"));
        let error = runtime_args(
            &trigger,
            &mut None,
            Instant::now(),
            &mut ClipboardPoll::new(&read_clipboard_text),
        )
        .expect_err("hotkey has no built-in runtime");
        assert_eq!(error, TriggerSource::Hotkey.diagnostic());
    }

    #[test]
    fn all_trigger_list_rows_have_same_column_count() {
        // The condition column used to be omitted when absent, so a row's arity
        // depended on its data and no consumer could split by column.
        const TOOL_ID: &str = "triglist.arity";
        upeg_runtime::set_trigger_bindings(
            TOOL_ID,
            vec![
                TriggerBinding {
                    tool_id: TOOL_ID,
                    source: TriggerSource::Clipboard.as_str().to_string(),
                    condition: None,
                },
                TriggerBinding {
                    tool_id: TOOL_ID,
                    source: TriggerSource::File.as_str().to_string(),
                    condition: Some("/tmp/demo.txt".to_string()),
                },
            ],
        );
        let out = format_list(false);
        let rows: Vec<&str> = out.lines().collect();
        // Every registered binding in the process is listed, so assert a
        // property of every row rather than a fixed row set.
        for row in &rows {
            assert_eq!(row.split('\t').count(), TRIGGER_LIST_COLUMNS, "{row}");
        }
        let without_condition = rows
            .iter()
            .find(|row| row.starts_with(&format!("{TOOL_ID}\tclipboard\t")))
            .expect("the conditionless trigger must be listed");
        let with_condition = rows
            .iter()
            .find(|row| row.starts_with(&format!("{TOOL_ID}\tfile\t")))
            .expect("the conditioned trigger must be listed");
        assert_eq!(
            without_condition.split('\t').nth(3),
            Some(ABSENT_CONDITION_FIELD)
        );
        assert_eq!(with_condition.split('\t').nth(3), Some("/tmp/demo.txt"));
        upeg_runtime::set_trigger_bindings(TOOL_ID, Vec::new());
    }

    #[test]
    fn clipboard_is_read_once_per_poll() {
        // Reading per binding spawned a `pbpaste`/`wl-paste` subprocess per
        // clipboard trigger per second.
        let reads = Cell::new(0);
        let read = counting_clipboard("copied", &reads);
        let mut clipboard = ClipboardPoll::new(&read);
        let trigger = binding(TriggerSource::Clipboard, None);
        for _ in 0..3 {
            runtime_args(&trigger, &mut None, Instant::now(), &mut clipboard)
                .expect("clipboard is supported");
        }
        assert_eq!(reads.get(), 1, "a single poll must read only once");
    }

    #[test]
    fn two_clipboard_triggers_both_fire_on_one_change() {
        // A single shared gate let whichever trigger was evaluated first consume
        // the change; the rest were starved forever.
        let first = binding_for("demo.first", TriggerSource::Clipboard, None);
        let second = binding_for("demo.second", TriggerSource::Clipboard, None);
        let mut state = Some(TriggerWatchState::default());
        let now = Instant::now();

        let baseline_reads = Cell::new(0);
        let baseline = counting_clipboard("before", &baseline_reads);
        let mut poll = ClipboardPoll::new(&baseline);
        for trigger in [&first, &second] {
            assert_eq!(
                runtime_args(trigger, &mut state, now, &mut poll).expect("clipboard is supported"),
                None,
                "the watch-start poll must be a quiet baseline"
            );
        }
        assert_eq!(baseline_reads.get(), 1);

        let change_reads = Cell::new(0);
        let changed = counting_clipboard("after", &change_reads);
        let mut poll = ClipboardPoll::new(&changed);
        for trigger in [&first, &second] {
            assert_eq!(
                runtime_args(trigger, &mut state, now, &mut poll).expect("clipboard is supported"),
                Some(serde_json::json!({ "input": "after" })),
                "{} must fire on the same change",
                trigger.tool_id
            );
        }
        assert_eq!(
            change_reads.get(),
            1,
            "the two bindings must share one observation"
        );
    }

    #[test]
    fn unreadable_clipboard_gives_every_binding_a_diagnostic() {
        let read = || Err("no clipboard command".to_string());
        let mut clipboard = ClipboardPoll::new(&read);
        let trigger = binding(TriggerSource::Clipboard, None);
        for _ in 0..2 {
            assert_eq!(
                runtime_args(&trigger, &mut None, Instant::now(), &mut clipboard).unwrap_err(),
                "no clipboard command"
            );
        }
    }

    #[test]
    fn fired_trigger_leaves_source_and_condition_label() {
        // The tool id alone could not tell a tool *which* of its triggers fired.
        assert_eq!(
            fired_label(&binding(TriggerSource::File, Some("/tmp/demo.txt"))),
            "file:/tmp/demo.txt"
        );
        assert_eq!(
            fired_label(&binding(TriggerSource::Clipboard, None)),
            "clipboard"
        );
    }

    #[test]
    fn retired_typing_source_is_reported_as_unknown() {
        let trigger = upeg_runtime::TriggerBinding {
            tool_id: "demo.tool",
            source: "typing".to_string(),
            condition: None,
        };
        let error = runtime_args(
            &trigger,
            &mut None,
            Instant::now(),
            &mut ClipboardPoll::new(&read_clipboard_text),
        )
        .expect_err("typing is no longer a source");
        assert_eq!(error, "unknown trigger source `typing`");
    }
}
