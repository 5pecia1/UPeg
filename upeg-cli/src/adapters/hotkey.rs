//! Feature-gated global-hotkey trigger adapter (`hotkey-trigger`).
//!
//! Registered hotkey triggers (`source = "hotkey"`, `condition = <accelerator>`)
//! are serviced here via the `global-hotkey` crate (Apache-2.0/MIT). Hotkeys are
//! event-driven, so this adapter runs on its own background thread and routes
//! every keypress through the same [`dispatch::dispatch_tool`] path the polling
//! adapters use — no bespoke execution path.
//!
//! Hosts without a graphical session (headless CI, no `DISPLAY`/Wayland) cannot
//! register system hotkeys; there the adapter surfaces a single clear diagnostic
//! and the rest of the trigger watch keeps running.

use crate::adapters::triggers::apply_cli_execution_context;
use crate::domain::execution::dispatch;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::collections::HashMap;
use std::str::FromStr as _;
use upeg_runtime::{FiredTrigger, TriggerSource, apply_trigger_context};

/// Trigger source string this adapter services, derived from the
/// [`upeg_runtime::TriggerSource`] enum so it cannot drift from the canonical
/// wire value.
const HOTKEY_SOURCE: &str = upeg_runtime::TriggerSource::Hotkey.as_str();

/// Diagnostic emitted when no graphical session is available to grab hotkeys.
const HEADLESS_DIAGNOSTIC: &str = "global hotkey registration requires a graphical session (X11/Wayland, or a Windows/macOS desktop); none is available on this host";

/// Spawn the hotkey watch on a background thread. On failure (e.g. headless
/// host, unparseable accelerator) the diagnostic is written to stderr once and
/// the thread exits, leaving the caller's poll loop unaffected.
pub(crate) fn spawn_watch(active_board: Option<&str>) {
    let board = active_board.map(str::to_owned);
    std::thread::Builder::new()
        .name("upeg-hotkey-watch".to_owned())
        .spawn(move || {
            // `run_hotkey_watch` only ever returns via `Err` (its `Ok` type is
            // uninhabited), so this match is exhaustive.
            let Err(diagnostic) = run_hotkey_watch(board.as_deref());
            report_diagnostic(&diagnostic);
        })
        .map(drop)
        .unwrap_or_else(|err| {
            report_diagnostic(&format!("could not start hotkey watch thread: {err}"));
        });
}

#[allow(
    clippy::print_stderr,
    reason = "hotkey watch runs off the main loop; stderr is the only channel to report a registration failure to the operator"
)]
fn report_diagnostic(diagnostic: &str) {
    eprintln!("hotkey trigger disabled: {diagnostic}");
}

/// Register every hotkey trigger and dispatch on keypress forever.
///
/// Returns `Err(diagnostic)` if the manager cannot be created, an accelerator is
/// missing/invalid, or the OS event channel closes. Never returns `Ok`.
fn run_hotkey_watch(active_board: Option<&str>) -> Result<std::convert::Infallible, String> {
    let bindings: Vec<upeg_runtime::TriggerBinding> = upeg_runtime::registered_trigger_bindings()
        .into_iter()
        .filter(|binding| binding.source == HOTKEY_SOURCE)
        .collect();

    let manager =
        GlobalHotKeyManager::new().map_err(|err| format!("{HEADLESS_DIAGNOSTIC} ({err})"))?;

    // hotkey id -> what to dispatch when that key fires: the canonical tool id
    // plus the `_upeg.trigger` label naming *which* accelerator fired, since a
    // tool may bind several.
    let mut routes: HashMap<u32, (&'static str, String)> = HashMap::with_capacity(bindings.len());
    for binding in &bindings {
        let accelerator = binding
            .condition
            .as_deref()
            .map(str::trim)
            .filter(|condition| !condition.is_empty())
            .ok_or_else(|| {
                format!(
                    "hotkey trigger for `{}` requires condition = accelerator (e.g. `CommandOrControl+Shift+KeyK`)",
                    binding.tool_id
                )
            })?;
        let hotkey = HotKey::from_str(accelerator).map_err(|err| {
            format!(
                "invalid hotkey accelerator `{accelerator}` for `{}`: {err}",
                binding.tool_id
            )
        })?;
        manager
            .register(hotkey)
            .map_err(|err| format!("failed to register hotkey `{accelerator}`: {err}"))?;
        routes.insert(
            hotkey.id(),
            (
                binding.tool_id,
                FiredTrigger::new(TriggerSource::Hotkey, Some(accelerator.to_string())).to_string(),
            ),
        );
    }

    let receiver = GlobalHotKeyEvent::receiver();
    loop {
        let event = receiver
            .recv()
            .map_err(|err| format!("hotkey event channel closed: {err}"))?;
        if event.state != HotKeyState::Pressed {
            continue;
        }
        let Some((tool_id, trigger_label)) = routes.get(&event.id) else {
            continue;
        };
        let tool_id = *tool_id;
        let mut args = apply_cli_execution_context(serde_json::json!({}), active_board, tool_id);
        args = apply_trigger_context(args, trigger_label);
        // Dispatch through the shared path; outcome text is reported for parity
        // with the polling adapters' one-line-per-fire output.
        let outcome = dispatch::dispatch_tool(tool_id, &args);
        if let Some(text) = outcome.primary_text() {
            report_fire(tool_id, &text);
        }
    }
}

#[allow(
    clippy::print_stdout,
    reason = "mirrors the polling watch loop: each fired trigger prints one line to stdout as its product"
)]
fn report_fire(tool_id: &str, text: &str) {
    println!("{tool_id}\t{HOTKEY_SOURCE}\t{text}");
}
