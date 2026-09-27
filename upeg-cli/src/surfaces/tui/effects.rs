use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};
use serde_json::Value;
use std::io;
use std::time::SystemTime;
use upeg_core::ToolMeta;
use upeg_core::prefs::{Locale, Tweaks};
use upeg_sources::pegboard;

use crate::domain::execution::context::{ExecutionContext, prepare_tool_args};
use crate::domain::execution::dispatch::dispatch_tool;

use super::controls::clamp_state_to_area;
use super::grid::{PlacementHint, set_placement_hints};
use super::model::{RunToken, State, initial_tui_filters};
use super::msg::{Mouse, Msg, key_stroke_from_crossterm, mouse_kind_from_crossterm};
use super::update::update;
use super::view::{RenderContext, render_with_context_and_pin_colors};

mod run;

use run::{
    IDLE_EVENT_POLL_INTERVAL, QUIT_CANCEL_GRACE, RUNNING_EVENT_POLL_INTERVAL, RunningDispatch,
};

#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
    /// Side effect: run the named tool with these args. The event loop calls
    /// `dispatch_tool` and transitions into `View::Result` via `Msg::ToolDone`.
    ///
    /// `run` is the identity the model minted for this dispatch
    /// (`State::start_active_run`). The loop stamps it onto every
    /// `Msg::ToolProgress` it forwards, which is what keeps a straggling
    /// chunk from an earlier run out of a later run's pane.
    Dispatch {
        run: RunToken,
        tool_id: &'static str,
        args: Value,
    },
    /// Side effect: persist the current `State.tweaks` to the shared
    /// config-root `.upeg-tweaks.json` file. Fired after Settings-View value-cycling
    /// (Stage E-3) so a desktop session reads the TUI's edits on next
    /// cold start.
    SaveTweaks,
    /// Side effect: persist the cached `State.boards` / `State.layouts`
    /// back to the shared coordinate-based pegboard state file. Fired after
    /// every Phase-4+ board / pin / reorder mutation so the desktop UI
    /// picks the change up on its next mount.
    SavePegboard,
    /// Side effect: persist only the active board/tag filters as the shared
    /// pegboard selection. This keeps filter-only changes distinct from
    /// board/layout mutations.
    SavePegboardSelection,
    /// Side effect: copy `text` to the system clipboard (F2 in Result/
    /// Detail). No clipboard crate exists anywhere in the workspace yet
    /// (the CLI's clipboard *trigger* adapter shells out to platform
    /// paste programs to *read* rather than depending on one — see
    /// `upeg-cli/src/adapters/triggers.rs::read_clipboard_text`); this
    /// mirrors that same no-new-dependency approach for writing. The
    /// outcome is best-effort and surfaced via `State.status_message`
    /// (set by the event loop below) rather than a panic.
    CopyToClipboard(String),
    /// Side effect: ask the in-flight dispatch to stop (Esc / q while
    /// `View::Running`). A no-op when nothing is running, and a request
    /// rather than a guarantee even when something is — the tool still
    /// ends in one final envelope, which `Msg::ToolDone` renders.
    CancelRun,
}

/// Snapshot of the shared Pegboard layout. Same board ordering as native
/// Desktop; tools unavailable on TUI still render as pinned metadata.
#[cfg(test)]
pub fn list_tools() -> Vec<&'static ToolMeta> {
    list_tools_for_board(None)
}

/// Snapshot of the shared Pegboard layout for one Board tab. `None` means the
/// union of all board layouts.
#[cfg(test)]
pub fn list_tools_for_board(board: Option<&str>) -> Vec<&'static ToolMeta> {
    list_tools_for_board_and_tag(board, None)
}

/// Snapshot of the shared Pegboard layout for one Board tab plus one
/// effective Tag, loading the on-disk state fresh. Test-only after
/// Phase 2 because the runtime path now reads from `State.visible_tools`
/// (state-cached) so a freshly-pinned tool surfaces immediately. Tests
/// keep the disk-loading variant to verify the storage shape.
#[cfg(test)]
pub fn list_tools_for_board_and_tag(
    board: Option<&str>,
    tag: Option<&str>,
) -> Vec<&'static ToolMeta> {
    let board = board.map(str::trim).filter(|v| !v.is_empty());
    let tag = tag.map(str::trim).filter(|v| !v.is_empty());
    upeg_sources::pegboard::tools_for_board_and_tag(board, tag)
}

/// Run the TUI event loop. Sets up raw mode + alternate screen, returns
/// when the user quits, and tears down terminal state on the way out.
/// `board` and `tag` apply the corresponding pegboard filter; pass
/// `None` for the default unfiltered view.
pub fn serve_with_filters(board: Option<&str>, tag: Option<&str>) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let pegboard = resolve_initial_pegboard();
    let mut state = State::with_filters(resolve_startup_filters(board, tag, &pegboard));
    state.tweaks = resolve_initial_tweaks();
    state.boards = pegboard.boards;
    state.layouts = pegboard.layouts;
    state.tool_picker_recent = crate::adapters::execution_log::recent_search_signals().recent;
    state.sync_filter_cursors();
    let mut seen_pegboard_rev = pegboard::state_change_rev();
    let mut seen_tweaks_mtime = tweaks_modified_time();
    // At most one dispatch runs at a time, and `update` knows it:
    // `State::active_run` is the model's copy of this very `Option`, so a
    // second Run is refused in the pure layer instead of producing an
    // `Effect::Dispatch` this loop would have to drop on the floor.
    let mut in_flight: Option<RunningDispatch> = None;
    let result: io::Result<()> = (|| {
        loop {
            // Fold the worker's output into State before this frame is
            // drawn, so the live tail is at most one poll interval old.
            let finished = in_flight.as_ref().and_then(|run| {
                run.pump(&mut state)
                    .map(|outcome| (run.run(), run.presentation_host(), run.tool_id(), outcome))
            });
            if let Some((run, host, tool_id, outcome)) = finished {
                let _ = update(
                    &mut state,
                    Msg::ToolDone {
                        run,
                        host,
                        tool_id,
                        outcome,
                    },
                );
                in_flight = None;
            }
            reload_external_pegboard_if_changed(
                &mut state,
                &mut seen_pegboard_rev,
                pegboard::state_change_rev(),
                load_current_pegboard_state,
            );
            reload_external_tweaks_if_changed(
                &mut state,
                &mut seen_tweaks_mtime,
                tweaks_modified_time(),
                load_current_tweaks,
            );
            // Snapshot the cached canonical layout for this frame.
            // `State` owns the live pegboard copy during a TUI session, so
            // freshly-pinned tools are visible before the best-effort disk
            // save round-trips.
            let layout_pairs = state.visible_placements();
            let tools: Vec<&'static ToolMeta> =
                layout_pairs.iter().map(|(_, tool)| *tool).collect();
            let hints: Vec<Option<PlacementHint>> = layout_pairs
                .iter()
                .map(|(placement, _)| {
                    let (w, h) = upeg_runtime::pegboard::effective_size(placement);
                    Some(PlacementHint {
                        x: placement.x,
                        y: placement.y,
                        w,
                        h,
                    })
                })
                .collect();
            let pin_colors: Vec<Option<upeg_core::PinColorHex>> = layout_pairs
                .iter()
                .map(|(placement, _)| placement.color.clone())
                .collect();
            set_placement_hints(hints);
            if state.cursor >= tools.len() {
                state.cursor = tools.len().saturating_sub(1);
            }
            let area = terminal
                .size()
                .map(|s| Rect::new(0, 0, s.width, s.height))?;
            clamp_state_to_area(&mut state, &tools, area);
            let context = RenderContext {
                footer_status: runtime_footer_status(state.tweaks.locale),
            };
            terminal.draw(|f| {
                render_with_context_and_pin_colors(f, &state, &tools, &pin_colors, &context);
            })?;

            let poll_interval = if in_flight.is_some() {
                RUNNING_EVENT_POLL_INTERVAL
            } else {
                IDLE_EVENT_POLL_INTERVAL
            };
            let effect = if event::poll(poll_interval)? {
                match event::read()? {
                    Event::Key(k) => key_stroke_from_crossterm(k).map(|stroke| {
                        update(
                            &mut state,
                            Msg::KeyPress {
                                stroke,
                                tools: &tools,
                                area: Some(area),
                            },
                        )
                    }),
                    Event::Mouse(m) => mouse_kind_from_crossterm(m.kind).map(|kind| {
                        let area = terminal
                            .size()
                            .map(|s| Rect::new(0, 0, s.width, s.height))
                            .unwrap_or_default();
                        update(
                            &mut state,
                            Msg::Mouse {
                                mouse: Mouse {
                                    column: m.column,
                                    row: m.row,
                                    kind,
                                },
                                tools: &tools,
                                area,
                            },
                        )
                    }),
                    _ => None,
                }
            } else {
                None
            };

            if let Some(effect) = effect {
                match effect {
                    Effect::Quit => {
                        // Ask the child process to stop on the way out
                        // and then actually wait for it: `cancel()` only
                        // trips a flag, and a worker that never observes
                        // it before the session ends leaves its child
                        // parented to a dead UI. The wait is bounded so
                        // an invoker that ignores cancellation cannot
                        // hold the terminal hostage.
                        if let Some(run) = in_flight.take() {
                            run.cancel_and_wait(QUIT_CANCEL_GRACE);
                        }
                        break;
                    }
                    Effect::None => {}
                    Effect::Dispatch { run, tool_id, args } => {
                        // `update` refuses a second Run while one is in
                        // flight (`refuse_second_run`), so this is the
                        // only place a worker is born and the assert-by-
                        // construction holds: one loop, one dispatch.
                        if in_flight.is_none() {
                            let (board, preset) = dispatch_board_scope(&state, tool_id);
                            in_flight =
                                Some(RunningDispatch::spawn(run, tool_id, args, board, preset));
                        }
                    }
                    Effect::CancelRun => {
                        if let Some(run) = &in_flight {
                            run.cancel();
                        }
                    }
                    Effect::SaveTweaks => {
                        // Best-effort. A read-only `toolkits_dir` or a
                        // missing config root leaves the in-memory edit
                        // intact for this session; the TUI has no toast
                        // affordance for failures so we swallow the
                        // error rather than risk splattering the alt
                        // screen with stderr output.
                        let _ = persist_tweaks(&state.tweaks);
                        seen_tweaks_mtime = tweaks_modified_time();
                    }
                    Effect::SavePegboard => {
                        // Same best-effort policy as SaveTweaks. The
                        // in-memory cache is the user's source of truth
                        // for this session; failing to round-trip just
                        // delays sync with the desktop UI.
                        let _ = persist_pegboard(&state);
                        seen_pegboard_rev = pegboard::state_change_rev();
                    }
                    Effect::SavePegboardSelection => {
                        let _ = persist_pegboard_selection(&state);
                        seen_pegboard_rev = pegboard::state_change_rev();
                    }
                    Effect::CopyToClipboard(text) => {
                        let ok = write_clipboard_text(&text).is_ok();
                        state.status_message =
                            Some(clipboard_status_message(state.tweaks.locale, ok));
                    }
                }
            }
        }
        Ok(())
    })();

    let mut stdout = io::stdout();
    let _ = execute!(stdout, DisableMouseCapture, LeaveAlternateScreen);
    let _ = disable_raw_mode();
    result
}

/// Pick up another surface's pegboard edits. Change detection is the
/// store's write revision ([`pegboard::state_change_rev`]) — under WAL
/// the database file's mtime does not reliably move on commit, so an
/// mtime probe here would silently stop seeing desktop edits.
fn reload_external_pegboard_if_changed(
    state: &mut State,
    seen_rev: &mut Option<u64>,
    current_rev: Option<u64>,
    load_state: impl FnOnce() -> Option<pegboard::PegboardState>,
) {
    if current_rev == *seen_rev || !state.can_reload_external_pegboard() {
        return;
    }
    let Some(next) = load_state() else {
        return;
    };
    state.replace_pegboard_from_external_change(next);
    *seen_rev = current_rev;
}

fn load_current_pegboard_state() -> Option<pegboard::PegboardState> {
    let path = pegboard::state_path_from_env()?;
    pegboard::load_state_from_path(&path).ok()
}

fn reload_external_tweaks_if_changed(
    state: &mut State,
    seen_mtime: &mut Option<SystemTime>,
    current_mtime: Option<SystemTime>,
    load_tweaks: impl FnOnce() -> Option<Tweaks>,
) {
    if current_mtime == *seen_mtime || !state.can_reload_external_tweaks() {
        return;
    }
    let Some(next) = load_tweaks() else {
        return;
    };
    state.replace_tweaks(next);
    *seen_mtime = current_mtime;
}

fn tweaks_modified_time() -> Option<SystemTime> {
    upeg_core::paths::tweaks_path()
        .as_deref()
        .and_then(upeg_runtime::persistence::modified_time)
}

fn load_current_tweaks() -> Option<Tweaks> {
    let path = upeg_core::paths::tweaks_path()?;
    let hint = read_locale_hint_from_env();
    Some(upeg_runtime::persistence::bootstrap_tweaks(&path, &hint))
}

/// The board scope for a TUI dispatch: the active board filter plus the
/// pin's saved args preset from the session's cached layouts. `All`
/// (union view) dispatches globally.
fn dispatch_board_scope(
    state: &State,
    tool_id: &str,
) -> (Option<upeg_core::BoardKey>, Option<upeg_core::ArgsPreset>) {
    let Some(board) = state
        .filters
        .board
        .as_deref()
        .and_then(|key| upeg_core::BoardKey::parse(key).ok())
    else {
        return (None, None);
    };
    let preset = state
        .layouts
        .get(board.as_str())
        .and_then(|placements| {
            placements
                .iter()
                .find(|placement| placement.tool_id == tool_id)
        })
        .and_then(|placement| placement.args_preset.clone());
    (Some(board), preset)
}

/// PRD §5.8 — TUI is L4 client-only. When a host is up we forward
/// the dispatch over HTTP so the run is visible in the host's
/// execution log / live clients panel. Otherwise we fall back to
/// in-process dispatch (standalone mode). Either way the call carries
/// the active board's execution context (+ pin preset merge) through
/// the shared runtime synthesis layer.
fn dispatch_through_host_or_local(
    tool_id: &'static str,
    args: Value,
    board: Option<upeg_core::BoardKey>,
    preset: Option<upeg_core::ArgsPreset>,
    attached_host: Option<crate::infrastructure::discovery::DiscoveredHost>,
) -> crate::domain::execution::dispatch::Outcome {
    // D-3: attaching does not turn the TUI into an HTTP client, so the
    // context is the same on both routes. The origin-surface header tells
    // the host which local surface is asking and it stamps
    // `_upeg.surface` from that (`infrastructure::attach`), so a
    // forwarded run is answered as `tui` — the same caller identity the
    // standalone route produces.
    let context = ExecutionContext::for_optional_board(upeg_core::Surface::Tui, board, preset);
    let args = prepare_tool_args(args, &context, None);
    // D-1: a Project Manifest tool exists only in this process's toolbox
    // (the host resolved its own `upeg.toml`, or none), so it never
    // auto-attaches — same rule the CLI's `dispatch_local_or_attached`
    // applies.
    if let Some(host) = attached_host {
        // D-2: the host would otherwise run External tools from the
        // daemon's working directory.
        let args = crate::domain::execution::context::with_caller_cwd(args);
        dispatch_over_attach(&host, tool_id, &args, context.board_key())
    } else {
        dispatch_tool(tool_id, &args)
    }
}

/// Run `tool_id` on the attached host, watching it while it runs.
///
/// D-4: the buffered route (`POST /v1/tools/{id}`) answers once, at the
/// end, which silently costs an attached TUI both halves of a live call
/// — the tail stays empty for the whole run and `Esc` reaches nobody,
/// because the progress sink and the cancellation token installed on
/// this worker have no wire to travel on. So the streaming route
/// (`POST /v1/tools/{id}/stream`) is tried first: its `chunk` lines feed
/// the same sink a standalone dispatch feeds, and hanging up on its body
/// is what the host reads as a cancel.
///
/// A host that predates the route answers `404`. That falls back to the
/// buffered route rather than failing — and says so in the tail, because
/// a pane that stays empty for ten minutes should say why.
fn dispatch_over_attach(
    host: &crate::infrastructure::discovery::DiscoveredHost,
    tool_id: &str,
    args: &Value,
    board: Option<&upeg_core::BoardKey>,
) -> crate::domain::execution::dispatch::Outcome {
    use crate::infrastructure::attach::{self, LiveCall, StreamedDispatch};

    let live = LiveCall::ambient();
    let streamed = match board {
        Some(board) => attach::dispatch_tool_on_board_streamed(
            host,
            board.as_str(),
            tool_id,
            args,
            upeg_core::Surface::Tui,
            &live,
        ),
        None => attach::dispatch_tool_streamed(host, tool_id, args, upeg_core::Surface::Tui, &live),
    };
    match streamed {
        Ok(StreamedDispatch::Streamed(outcome)) => return outcome,
        Ok(StreamedDispatch::RouteUnavailable) => announce_buffered_attach(),
        Err(error) => return attach_error_outcome(&error),
    }
    let buffered = match board {
        Some(board) => attach::dispatch_tool_on_board(
            host,
            board.as_str(),
            tool_id,
            args,
            upeg_core::Surface::Tui,
        ),
        None => attach::dispatch_tool(host, tool_id, args, upeg_core::Surface::Tui),
    };
    buffered.unwrap_or_else(|error| attach_error_outcome(&error))
}

/// What the tail says when the host cannot stream: the truth, once,
/// instead of ten silent minutes.
///
/// Written through the ambient progress sink rather than the status bar
/// because it belongs to *this run's* output — the next run against a
/// newer host must not inherit the note.
const NO_LIVE_OUTPUT_NOTE: &str = "… attached: no live output (host has no streaming route)\n";

fn announce_buffered_attach() {
    if let Some(reporter) = upeg_runtime::ProgressReporter::capture() {
        reporter.report(
            upeg_runtime::ProgressStream::Stderr,
            NO_LIVE_OUTPUT_NOTE.to_string(),
        );
    }
}

fn attach_error_outcome(error: &std::io::Error) -> crate::domain::execution::dispatch::Outcome {
    crate::domain::execution::dispatch::Outcome::Failure(
        crate::domain::execution::dispatch::dispatch_failure(
            "attach_error",
            format!("attach: {error}"),
        ),
    )
}

/// Order in which the TUI tries platform hints to seed the [`Tweaks::locale`]
/// on a first run when no prefs file exists yet. `UPEG_LOCALE` always wins
/// because it's the deliberate per-process override; `$LC_ALL` outranks
/// `$LANG` to match POSIX precedence.
pub const TUI_LOCALE_ENV_PRECEDENCE: &[&str] = &["UPEG_LOCALE", "LC_ALL", "LANG"];

/// Pure helper: first non-empty hint from a callback walking
/// `TUI_LOCALE_ENV_PRECEDENCE`. Split out so the precedence rule is
/// unit-testable without mutating the real process env.
pub fn pick_locale_hint(env_lookup: impl Fn(&str) -> Option<String>) -> String {
    for name in TUI_LOCALE_ENV_PRECEDENCE {
        if let Some(value) = env_lookup(name)
            && !value.trim().is_empty()
        {
            return value;
        }
    }
    String::new()
}

fn read_locale_hint_from_env() -> String {
    pick_locale_hint(|name| std::env::var(name).ok())
}

/// Resolve the initial `Tweaks` for a TUI session. Reads the shared
/// `<config_root>/.upeg-tweaks.json` file via `bootstrap_tweaks` so
/// Flutter desktop edits and TUI edits round-trip through the same record;
/// detects the first-run locale from process env when no file exists.
fn resolve_initial_tweaks() -> Tweaks {
    let path = upeg_core::paths::tweaks_path();
    let hint = read_locale_hint_from_env();
    match path {
        Some(p) => upeg_runtime::persistence::bootstrap_tweaks(&p, &hint),
        None => Tweaks {
            locale: Locale::detect_from_str(&hint),
            ..Tweaks::default_const()
        },
    }
}

/// Read the shared coordinate-based pegboard state into a [`PegboardState`].
/// Missing file / unreadable home produces `default_state` so first-
/// run TUI sessions still show the canonical dev / trading / personal
/// boards. The state file location matches the desktop UI's path
/// (`upeg_sources::sources::config_root_from_env`) so the two surfaces
/// share storage by construction.
fn resolve_initial_pegboard() -> pegboard::PegboardState {
    pegboard::load_state()
}

fn resolve_startup_filters(
    board: Option<&str>,
    tag: Option<&str>,
    pegboard: &pegboard::PegboardState,
) -> super::model::TuiFilters {
    let initial_board = board.or(pegboard.selection.board_key.as_deref());
    let initial_tag = tag.or(Some(pegboard.selection.tag.as_str()));
    initial_tui_filters(initial_board, initial_tag)
}

/// Best-effort write of the cached pegboard cache back to disk.
/// Mirrors `persist_tweaks`'s policy: swallow IO errors so a transient
/// filesystem hiccup never wedges the alt-screen UI.
fn persist_pegboard(state: &State) -> Result<(), pegboard::PegboardStateError> {
    pegboard::save_state(&state.pegboard_snapshot())
}

fn persist_pegboard_selection(state: &State) -> Result<(), pegboard::PegboardStateError> {
    pegboard::save_selection_value(state.pegboard_snapshot().selection)
}

/// Best-effort serialize + write of the current `Tweaks` to the shared
/// prefs file. Returns the underlying error (path resolution, serde,
/// IO) without retrying; the event loop logs and continues so a
/// long-running TUI session never wedges on a transient filesystem
/// hiccup.
fn persist_tweaks(tweaks: &Tweaks) -> Result<(), String> {
    let path =
        upeg_core::paths::tweaks_path().ok_or_else(|| "config_root unavailable".to_string())?;
    let json = serde_json::to_string(tweaks).map_err(|e| format!("serialize: {e}"))?;
    upeg_runtime::persistence::io::save_to_path(&path, &json).map_err(|e| format!("write: {e}"))
}

/// Best-effort clipboard writer for the F2 copy command. No clipboard
/// crate (e.g. `arboard`) exists anywhere in the workspace, so this
/// shells out to the same family of platform-native programs the
/// read-only clipboard *trigger* adapter already assumes are present
/// (`upeg-cli/src/adapters/triggers.rs::read_clipboard_text`), rather
/// than introducing a new dependency for one write path.
fn write_clipboard_text(text: &str) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        pipe_to_clipboard_program("pbcopy", &[], text)
    } else if cfg!(target_os = "windows") {
        pipe_to_clipboard_program("clip", &[], text)
    } else {
        pipe_to_clipboard_program("wl-copy", &[], text)
            .or_else(|_| pipe_to_clipboard_program("xclip", &["-selection", "clipboard"], text))
    }
}

fn pipe_to_clipboard_program(program: &str, args: &[&str], text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = std::process::Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("{program}: {err}"))?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| format!("{program}: no stdin"))?;
        stdin
            .write_all(text.as_bytes())
            .map_err(|err| format!("{program}: {err}"))?;
    }
    let status = child
        .wait()
        .map_err(|err| format!("{program}: wait failed: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}

fn clipboard_status_message(locale: Locale, ok: bool) -> String {
    let key = if ok {
        "tui.clipboard.copied"
    } else {
        "tui.clipboard.failed"
    };
    crate::i18n::t(locale, key).to_string()
}

fn runtime_footer_status(locale: Locale) -> String {
    // PRD §5.8: TUI is L4 client-only. It never hosts (no `server.json`
    // publish from this surface), and it surfaces the live host's
    // endpoint when one exists so the operator sees the shared-state
    // picture at a glance. The base label comes from upeg-runtime
    // (intentionally untranslated technical jargon: MCP / HTTP /
    // Trigger); only the trailing surface-attachment suffix is
    // routed through the i18n catalog.
    let base =
        upeg_runtime::NetworkStatus::current(upeg_runtime::registered_trigger_bindings().len())
            .label();
    let suffix = match crate::infrastructure::discovery::read() {
        Some(info) => crate::i18n::t_args(
            locale,
            "tui.footer.attached",
            &[("endpoint", info.endpoint.as_str())],
        ),
        None => crate::i18n::t(locale, "tui.footer.standalone").to_string(),
    };
    format!("{base}{suffix}")
}

#[cfg(test)]
mod tweaks_bootstrap_tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::{Duration, UNIX_EPOCH};

    use super::super::model::{SettingsField, View};

    fn fake_env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + 'static {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn upeg_locale_env_beats_lang_and_lc_all() {
        let lookup = fake_env(&[
            ("UPEG_LOCALE", "ko"),
            ("LC_ALL", "en_US.UTF-8"),
            ("LANG", "en_US.UTF-8"),
        ]);
        assert_eq!(pick_locale_hint(lookup), "ko");
    }

    #[test]
    fn lc_all_beats_lang_when_upeg_locale_absent() {
        let lookup = fake_env(&[("LC_ALL", "ko_KR.UTF-8"), ("LANG", "en_US.UTF-8")]);
        assert_eq!(pick_locale_hint(lookup), "ko_KR.UTF-8");
    }

    #[test]
    fn lang_env_var_is_last_resort() {
        let lookup = fake_env(&[("LANG", "ko_KR.UTF-8")]);
        assert_eq!(pick_locale_hint(lookup), "ko_KR.UTF-8");
    }

    #[test]
    fn empty_or_blank_values_are_skipped() {
        // An exported-but-empty UPEG_LOCALE must not block the fallback
        // to LANG.
        let lookup = fake_env(&[
            ("UPEG_LOCALE", "   "),
            ("LC_ALL", ""),
            ("LANG", "ko_KR.UTF-8"),
        ]);
        assert_eq!(pick_locale_hint(lookup), "ko_KR.UTF-8");
    }

    #[test]
    fn missing_env_returns_empty_string() {
        let lookup = |_: &str| None;
        assert_eq!(pick_locale_hint(lookup), "");
    }

    #[test]
    fn precedence_constant_matches_documented_order() {
        assert_eq!(TUI_LOCALE_ENV_PRECEDENCE, ["UPEG_LOCALE", "LC_ALL", "LANG"]);
    }

    #[test]
    fn mtime_change_reflects_external_settings_into_state() {
        let mut state = State {
            tweaks: Tweaks {
                locale: Locale::En,
                ..Tweaks::default_const()
            },
            ..State::default()
        };
        let old = UNIX_EPOCH;
        let new = UNIX_EPOCH + Duration::from_secs(1);
        let mut seen = Some(old);

        reload_external_tweaks_if_changed(&mut state, &mut seen, Some(new), || {
            Some(Tweaks {
                locale: Locale::Ko,
                ..Tweaks::default_const()
            })
        });

        assert_eq!(seen, Some(new));
        assert_eq!(state.tweaks.locale, Locale::Ko);
    }

    #[test]
    fn settings_view_defers_external_settings_reload() {
        let mut state = State {
            view: View::Settings {
                focused_field: SettingsField::Locale,
            },
            ..State::default()
        };
        let old = UNIX_EPOCH;
        let new = UNIX_EPOCH + Duration::from_secs(1);
        let mut seen = Some(old);

        reload_external_tweaks_if_changed(&mut state, &mut seen, Some(new), || {
            panic!("must not load external settings while editing settings")
        });

        assert_eq!(seen, Some(old));
    }
}

#[cfg(test)]
mod pegboard_reload_tests {
    use super::super::model::{BoardEditMode, View};
    use super::*;
    use std::collections::BTreeMap;
    use upeg_core::Placement;
    use upeg_sources::pegboard::{BoardData, PegboardState};

    const OLD_REV: Option<u64> = Some(1);
    const NEW_REV: Option<u64> = Some(2);

    fn one_board_state(key: &str) -> PegboardState {
        PegboardState {
            boards: vec![BoardData {
                guidance: upeg_core::BoardGuidance::default(),
                key: key.into(),
                title: key.into(),
            }],
            layouts: BTreeMap::from([(
                key.into(),
                vec![Placement::new("num.hex_to_decimal", 0, 0)],
            )]),
            selection: pegboard::PegboardSelection::default(),
        }
    }

    #[test]
    fn startup_filters_use_shared_selection_without_cli_args() {
        let mut pegboard = one_board_state("gui");
        pegboard.selection = pegboard::PegboardSelection {
            board_key: Some("gui".into()),
            tag: "pure".into(),
        };

        let filters = resolve_startup_filters(None, None, &pegboard);

        assert_eq!(filters.board.as_deref(), Some("gui"));
        assert_eq!(filters.tag.as_deref(), Some("pure"));
    }

    #[test]
    fn startup_filters_cli_args_beat_shared_selection() {
        let mut pegboard = one_board_state("gui");
        pegboard.selection = pegboard::PegboardSelection {
            board_key: Some("gui".into()),
            tag: "pure".into(),
        };

        let filters = resolve_startup_filters(Some("cli"), Some("convert"), &pegboard);

        assert_eq!(filters.board.as_deref(), Some("cli"));
        assert_eq!(filters.tag.as_deref(), Some("convert"));
    }

    #[test]
    fn rev_change_reflects_external_pegboard_into_state() {
        let mut state = State::default();
        let mut seen = OLD_REV;

        reload_external_pegboard_if_changed(&mut state, &mut seen, NEW_REV, || {
            Some(one_board_state("gui"))
        });

        assert_eq!(seen, NEW_REV);
        assert_eq!(
            state.boards.first().map(|board| board.key.as_str()),
            Some("gui")
        );
        assert!(state.layouts.contains_key("gui"));
    }

    #[test]
    fn external_pegboard_reload_also_reflects_shared_selection() {
        let mut state = State::with_filters(initial_tui_filters(Some("dev"), Some("all")));
        let mut seen = OLD_REV;
        let mut next = one_board_state("gui");
        next.selection = pegboard::PegboardSelection {
            board_key: Some("gui".into()),
            tag: "pure".into(),
        };

        reload_external_pegboard_if_changed(&mut state, &mut seen, NEW_REV, || Some(next));

        assert_eq!(state.filters.board.as_deref(), Some("gui"));
        assert_eq!(state.filters.tag.as_deref(), Some("pure"));
    }

    #[test]
    fn edit_dialog_defers_reload_without_consuming_rev() {
        let mut state = State {
            view: View::BoardEditor {
                mode: BoardEditMode::AddBoard,
                buffer: String::new(),
            },
            ..State::default()
        };
        let mut seen = OLD_REV;

        reload_external_pegboard_if_changed(&mut state, &mut seen, NEW_REV, || {
            panic!("must not load pegboard while deferred")
        });

        assert_eq!(seen, OLD_REV);
        assert!(matches!(state.view, View::BoardEditor { .. }));
    }

    #[test]
    fn failed_external_pegboard_read_does_not_consume_rev() {
        let mut state = State::default();
        let mut seen = OLD_REV;

        reload_external_pegboard_if_changed(&mut state, &mut seen, NEW_REV, || None);

        assert_eq!(seen, OLD_REV);
        assert!(state.boards.is_empty());
    }
}
