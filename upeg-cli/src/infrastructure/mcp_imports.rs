//! Eager MCP-import registration for long-lived server processes.
//!
//! Exactly three lanes call [`load_for_host`]: `upeg host start`
//! (foreground and `--daemon`), the desktop-embedded host, and the
//! in-process `upeg mcp` stdio server. Every other lane — one-shot CLI
//! commands, the TUI, and `upeg mcp` while it proxies to a running host
//! — deliberately skips it so they never spawn upstream subprocesses;
//! they reach imported tools through the host instead.
//!
//! The three lanes do not all load the same way. The two host lanes load
//! eagerly and synchronously; the in-process `upeg mcp` stdio server
//! loads through [`spawn_load_for_host`] on a background thread, and
//! only when [`reexport_policy_from_env`] says some declaration opts
//! into `reexport = true` — otherwise the imported tools would land on
//! `ALL_SURFACES_EXCEPT_MCP` and that lane would have spawned every
//! upstream to list none of them (docs/architecture/mcp.md).
//!
//! There is no unload and no reload: restarting the host process
//! re-imports (docs/architecture/mcp.md). Failures are reported, never
//! fatal — a broken upstream declaration must not stop the host from
//! serving its local Toolbox.
//!
//! [`load_and_report_for_host`] is also the single choke point for
//! upeg's self-import recursion guard (E-6): all three lanes above
//! funnel through it, so checking
//! `upeg_sources::mcp_import::is_mcp_import_child()` exactly here
//! covers every caller without touching each lane individually. A
//! self-imported `upeg mcp` (`examples/mcp-imports/local.toml`,
//! `command = "upeg", args = ["mcp"]`) is itself one of these three
//! lanes when it runs in-process — without the guard it would load
//! imports again, spawning a grandchild `upeg mcp` that does the same,
//! forkbombing until the OS process/fd limit kills the tree.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::task::{Context, Poll};

use serde_json::{Map, Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use upeg_sources::{DirectoryStatus, McpImportLoad, RuntimeSourceConfig, mcp_import::McpReexport};

use super::discovery::ServerInfo;

// === Import-load phase (imports-pending signal) ===

/// Where this process stands in its one-shot MCP-import load.
///
/// Three lanes load imports and every one of them funnels through
/// [`load_and_report_for_host`], so this phase is written in exactly
/// one place and read by everything that has to answer "is this host
/// still bringing imported tools up?": the host's `/healthz`
/// (`importsPending`), `upeg host status --json` (through that same
/// `/healthz`), and the desktop status bar (through
/// `upeg-frb`'s status snapshot).
///
/// The distinction that matters is [`Self::Loading`] vs everything
/// else. `Loading` is set SYNCHRONOUSLY when a load is scheduled —
/// before the worker thread exists (see [`spawn_load_for_host`]) —
/// precisely so a client polling in the "thread spawned but not yet
/// running" window cannot read `NotStarted` and conclude that nothing
/// more is coming.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum McpImportPhase {
    /// No lane in this process ever scheduled a load. The honest
    /// answer for one-shot CLI commands, the TUI, a proxying `upeg
    /// mcp`, and an attach-only desktop — none of them import
    /// (docs/architecture/mcp.md).
    #[default]
    NotStarted,
    /// A load is scheduled or running. The only pending phase.
    Loading,
    /// The load finished. Counts are final for the process lifetime:
    /// there is no reload path, restarting the host re-imports.
    Done(McpImportTally),
    /// Deliberately loaded nothing: this process is itself somebody's
    /// MCP-import upstream child, so the E-6 recursion guard in
    /// [`load_and_report_for_host`] skipped the load.
    Skipped,
}

impl McpImportPhase {
    /// Are imported tools still on their way? `true` only while a load
    /// is scheduled or running — a client that sees `false` can trust
    /// the tool list it just read to be complete.
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Loading)
    }
}

/// Server/tool counts of a finished load. Counts only — never server
/// names: [`healthz_fields`] publishes this on an UNAUTHENTICATED
/// route, and which upstreams a user declared is their business.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct McpImportTally {
    pub servers_loaded: usize,
    pub servers_failed: usize,
    pub tools: usize,
}

impl McpImportTally {
    /// The one place the three counters are read off a finished
    /// [`McpImportLoad`], so every consumer counts the same way.
    fn of(load: &McpImportLoad) -> Self {
        Self {
            servers_loaded: load.loaded_server_count(),
            servers_failed: load.failed_server_count(),
            tools: load.tool_count(),
        }
    }
}

/// Process-wide phase. A `Mutex` (not a set of atomics) because the
/// phase and its counts must be read as one value — a reader that saw
/// `Done` with a half-published tally would report a wrong count.
/// Every critical section is a move of a `Copy` value, so no reader
/// ever blocks meaningfully, `/healthz` included.
static IMPORT_PHASE: Mutex<McpImportPhase> = Mutex::new(McpImportPhase::NotStarted);

/// Read the current phase. Never panics: a poisoned lock still holds a
/// valid phase, and a status probe must not inherit another thread's
/// panic.
pub fn import_phase() -> McpImportPhase {
    *IMPORT_PHASE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn set_import_phase(next: McpImportPhase) {
    *IMPORT_PHASE.lock().unwrap_or_else(PoisonError::into_inner) = next;
    publish_phase(next);
}

/// Transports that can tell an attached client a phase changed, instead
/// of waiting to be asked.
///
/// `/healthz` answers "is a load still running?" to whoever polls it;
/// this is the other direction, and it is why the phase is written in
/// exactly one place. A lane that can push — today the HTTP `/mcp` SSE
/// stream — subscribes and turns a finished load into
/// `notifications/tools/list_changed`, closing the window the polling
/// signal could only describe.
///
/// Unbounded senders: a process publishes a handful of transitions in
/// its whole life (there is no reload path), so a queue that a listener
/// stopped reading cannot grow meaningfully. What *can* grow is the
/// registry itself — see [`PhaseSubscription`].
static PHASE_LISTENERS: Mutex<Vec<(PhaseListenerId, UnboundedSender<McpImportPhase>)>> =
    Mutex::new(Vec::new());

/// Identity of one registry row, so a subscription can remove its own
/// entry and no other.
///
/// A `Vec` position would not do: rows before it may be pruned between
/// registration and drop, and an index would then name a stranger's
/// listener.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PhaseListenerId(u64);

/// Source of [`PhaseListenerId`]s. Monotonic and never reused, so a
/// stale id can only ever fail to match.
static NEXT_PHASE_LISTENER_ID: AtomicU64 = AtomicU64::new(0);

/// A live phase listener, and the registry row that feeds it.
///
/// The row is the reason this is a type rather than a bare receiver:
/// publishing is the only thing that used to prune, so a process that
/// answered `GET /mcp` a thousand times without the import phase ever
/// moving again kept a thousand senders — one per connection that had
/// long since hung up. Dropping the subscription now removes its own
/// row, and [`subscribe_phase_changes`] sweeps whatever a leaked drop
/// left behind.
pub(crate) struct PhaseSubscription {
    id: PhaseListenerId,
    receiver: UnboundedReceiver<McpImportPhase>,
}

impl PhaseSubscription {
    /// The next transition, in `Stream`-shaped form for the SSE body that
    /// consumes it. `None` means the registry row is gone and no further
    /// transition can arrive.
    pub(crate) fn poll_recv(&mut self, context: &mut Context<'_>) -> Poll<Option<McpImportPhase>> {
        self.receiver.poll_recv(context)
    }
}

impl Drop for PhaseSubscription {
    fn drop(&mut self) {
        PHASE_LISTENERS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(id, _)| *id != self.id);
    }
}

/// Listen for phase transitions. The subscription sees every transition
/// published after this call — never the phase as it stands now, which
/// the caller can read with [`import_phase`] if it needs both.
///
/// Registering also sweeps rows whose receiver is already gone, so the
/// registry stays bounded by *live* listeners even on the path where a
/// subscription's `Drop` never ran.
pub(crate) fn subscribe_phase_changes() -> PhaseSubscription {
    let (sender, receiver) = unbounded_channel();
    let id = PhaseListenerId(NEXT_PHASE_LISTENER_ID.fetch_add(1, Ordering::Relaxed));
    let mut listeners = PHASE_LISTENERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    listeners.retain(|(_, listener)| !listener.is_closed());
    listeners.push((id, sender));
    PhaseSubscription { id, receiver }
}

/// How many listeners the registry is holding. A test observer: nothing
/// in production asks, it only registers and prunes.
#[cfg(test)]
pub(crate) fn phase_listener_count() -> usize {
    PHASE_LISTENERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .len()
}

/// Fan a transition out, dropping the listeners that have gone away.
fn publish_phase(next: McpImportPhase) {
    PHASE_LISTENERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|(_, listener)| listener.send(next).is_ok());
}

/// The phase is process-wide; tests that write it must not interleave
/// with each other or with a reader. Lives at module scope (not inside
/// the test module) because the HTTP surface's SSE tests drive the phase
/// too, and two serialising mutexes would serialise nothing.
#[cfg(test)]
static PHASE_SERIAL: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn 페이즈_직렬화() -> std::sync::MutexGuard<'static, ()> {
    PHASE_SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Drive the phase from a test in another module — the one write path
/// tests outside this file get, so [`set_import_phase`] itself stays
/// private and the publish hook can never be bypassed.
#[cfg(test)]
pub(crate) fn publish_phase_for_tests(next: McpImportPhase) {
    set_import_phase(next);
}

/// Declare "this process is about to import" BEFORE anything can
/// observe the host.
///
/// The desktop lane's ordering problem: the embed worker thread binds
/// its listener and starts answering `/healthz` while the boot path is
/// still blocked on the ready signal — so the import load could only be
/// scheduled AFTER the host was already reachable, and every probe in
/// that gap read `importsPending: false` from a host that was in fact
/// about to import. Marking here closes the gap; the load itself is
/// scheduled later through [`spawn_load_for_host`], whose own mark is
/// idempotent.
///
/// Every caller owes a matching [`clear_pending`] on the paths where no
/// load follows. Both are re-exported from the crate root as
/// `mark_mcp_imports_pending` / `clear_mcp_imports_pending`, which is
/// how `upeg-frb`'s host bootstrap reaches them.
pub(crate) fn mark_pending() {
    set_import_phase(McpImportPhase::Loading);
}

/// Undo [`mark_pending`] when the host never came up, so a status probe
/// does not wait on imports that were never scheduled.
///
/// Only [`McpImportPhase::Loading`] is taken back: a load that actually
/// ran has already published `Done`/`Skipped`, and overwriting that
/// would erase a real tally. Leaving `Loading` in place instead would be
/// worse than the bug the mark guards — every client would wait forever
/// for tools nobody is fetching.
pub(crate) fn clear_pending() {
    let cleared = {
        let mut phase = IMPORT_PHASE.lock().unwrap_or_else(PoisonError::into_inner);
        let cleared = matches!(*phase, McpImportPhase::Loading);
        if cleared {
            *phase = McpImportPhase::NotStarted;
        }
        cleared
    };
    // Outside the critical section on purpose: a listener's channel is
    // its own lock, and holding the phase lock across a fan-out would
    // put every `/healthz` reader behind it.
    if cleared {
        publish_phase(McpImportPhase::NotStarted);
    }
}

/// JSON key for the one-bit answer every attached client actually
/// needs. Named constants because `/healthz`, `upeg host status
/// --json`, and their tests must all spell it the same way.
pub(crate) const IMPORTS_PENDING_FIELD: &str = "importsPending";
/// JSON key of the detail block that accompanies [`IMPORTS_PENDING_FIELD`].
pub(crate) const MCP_IMPORTS_FIELD: &str = "mcpImports";
const PHASE_FIELD: &str = "state";
const SERVERS_LOADED_FIELD: &str = "serversLoaded";
const SERVERS_FAILED_FIELD: &str = "serversFailed";
const TOOLS_FIELD: &str = "tools";
const PHASE_NOT_STARTED: &str = "not-started";
const PHASE_LOADING: &str = "loading";
const PHASE_DONE: &str = "done";
const PHASE_SKIPPED: &str = "skipped";
/// Phase reported for a host that answered `/healthz` without the
/// import fields. Not a back-compat shim — it is the honest reading of
/// "this endpoint told us nothing about imports".
const PHASE_UNKNOWN: &str = "unknown";

/// Render a phase as the `/healthz` detail block.
pub(crate) fn phase_json(phase: McpImportPhase) -> Value {
    match phase {
        McpImportPhase::NotStarted => json!({ PHASE_FIELD: PHASE_NOT_STARTED }),
        McpImportPhase::Loading => json!({ PHASE_FIELD: PHASE_LOADING }),
        McpImportPhase::Skipped => json!({ PHASE_FIELD: PHASE_SKIPPED }),
        McpImportPhase::Done(tally) => json!({
            PHASE_FIELD: PHASE_DONE,
            SERVERS_LOADED_FIELD: tally.servers_loaded,
            SERVERS_FAILED_FIELD: tally.servers_failed,
            TOOLS_FIELD: tally.tools,
        }),
    }
}

/// The `/healthz` fields this module owns: the pending flag plus the
/// phase detail. Returned as a map so the route handler stays a
/// two-line composition instead of learning this module's vocabulary.
pub(crate) fn healthz_fields(phase: McpImportPhase) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert(IMPORTS_PENDING_FIELD.to_string(), json!(phase.is_pending()));
    fields.insert(MCP_IMPORTS_FIELD.to_string(), phase_json(phase));
    fields
}

/// What a RUNNING host reports about its own import load, for `upeg
/// host status --json`.
///
/// `upeg host status` is a separate one-shot process: its own phase is
/// always `NotStarted` and saying so would be worse than useless. It
/// asks the host instead, over the same unauthenticated `/healthz` the
/// status command already probes for liveness. This is the one place
/// where that command reports the host's LIVE registration progress
/// rather than mere declarations (docs/architecture/mcp.md).
pub(crate) fn host_imports_json(server: &ServerInfo) -> Value {
    host_imports_json_from_healthz(super::attach::healthz_json(server).as_ref())
}

/// The block for a host there is nothing to ask: no `server.json`, or a
/// stale one whose `/healthz` no longer answers. Same `unknown` a
/// running-but-silent host produces, because it is the same fact — we
/// do not know — and `upeg host status --json` promises the block
/// either way (docs/architecture/mcp.md).
pub(crate) fn unknown_imports_json() -> Value {
    host_imports_json_from_healthz(None)
}

/// Pure half of [`host_imports_json`]: reshape a host's `/healthz`
/// document into the status block. A host that did not answer, or
/// answered without the import fields, reports `unknown` — never a
/// fabricated `false`, which would read as "everything is loaded".
fn host_imports_json_from_healthz(healthz: Option<&Value>) -> Value {
    let unknown = || json!({ PHASE_FIELD: PHASE_UNKNOWN });
    let Some(healthz) = healthz else {
        return unknown();
    };
    let (Some(pending), Some(detail)) = (
        healthz.get(IMPORTS_PENDING_FIELD).and_then(Value::as_bool),
        healthz.get(MCP_IMPORTS_FIELD).and_then(Value::as_object),
    ) else {
        return unknown();
    };
    let mut block = detail.clone();
    block.insert(IMPORTS_PENDING_FIELD.to_string(), json!(pending));
    Value::Object(block)
}

/// Thread name for the deferred loader spawned by
/// [`spawn_load_for_host`] — mirrors the desktop lane's
/// `upeg-mcp-imports` worker so both show up the same in a debugger.
const LOADER_THREAD_NAME: &str = "upeg-mcp-imports";

/// Register every declared upstream MCP server from the environment's
/// configured import directory. Blocking: each upstream spawn is bounded
/// by the import layer's own timeouts.
pub(crate) fn load_for_host() -> McpImportLoad {
    upeg_sources::load_mcp_imports_for_host(&RuntimeSourceConfig::from_env())
}

/// Does any declared upstream opt into `reexport = true`?
///
/// Reads the declaration files only — no upstream is spawned. The
/// in-process `upeg mcp` lane consults this before loading at all: with
/// every declaration on the default `McpReexport::Blocked`, imported
/// tools register on `ALL_SURFACES_EXCEPT_MCP`, so that lane would pay
/// the full spawn+handshake cost of every upstream and then list none
/// of their tools.
pub(crate) fn reexport_policy_from_env() -> McpReexport {
    upeg_sources::mcp_import_reexport_policy(&RuntimeSourceConfig::from_env())
}

/// Run [`load_and_report_for_host`] on a background thread and call
/// `on_loaded` once it finishes.
///
/// Off the caller's path deliberately, mirroring the desktop lane
/// (`upeg-frb/src/platform/host_bootstrap.rs`): a single unreachable
/// upstream costs three bounded attempts of `initialize` + `tools/list`
/// before it gives up, which a stdio server must not spend before
/// answering its own client's `initialize`.
///
/// Returns the join handle so a caller (today: tests) can wait for the
/// load; `serve` drops it and lets the thread run for the process
/// lifetime. `None` means the thread could not be spawned — reported on
/// stderr and otherwise non-fatal, exactly like a failing upstream.
pub(crate) fn spawn_load_for_host(
    on_loaded: impl FnOnce() + Send + 'static,
) -> Option<std::thread::JoinHandle<()>> {
    spawn_marked_loader(move || {
        load_and_report_for_host();
        on_loaded();
    })
}

/// Mark the load pending, then run `work` on the loader thread.
///
/// The marking happens BEFORE the thread exists, not inside it:
/// `/healthz` is already answering by the time the desktop lane
/// schedules a load, and a client that read `NotStarted` in the gap
/// between spawn and thread entry would conclude no imports were
/// coming. Split out from [`spawn_load_for_host`] so a test can pin
/// that ordering without spawning real upstream subprocesses.
///
/// Idempotent with respect to [`mark_pending`]: the desktop lane marks
/// pending even earlier (before its embed thread can answer `/healthz`
/// at all), and re-stamping the same phase here is a no-op.
fn spawn_marked_loader(
    work: impl FnOnce() + Send + 'static,
) -> Option<std::thread::JoinHandle<()>> {
    set_import_phase(McpImportPhase::Loading);
    let spawned = std::thread::Builder::new()
        .name(LOADER_THREAD_NAME.to_string())
        .spawn(work);
    match spawned {
        Ok(handle) => Some(handle),
        Err(err) => {
            // Nothing was scheduled after all. Back to `NotStarted`:
            // reporting `Loading` forever would leave every client
            // waiting for tools that will never arrive.
            set_import_phase(McpImportPhase::NotStarted);
            eprint_best_effort(&format!(
                "upeg:   ✗ mcp import loader thread failed to start: {err}\n"
            ));
            None
        }
    }
}

/// Schedule the load on a background thread and return immediately —
/// the desktop-embedded host's lane (`upeg-frb`'s host bootstrap).
///
/// The thread is deliberately detached: it owns nothing that outlives
/// the process, and the desktop must not block its boot path on an
/// unreachable upstream's bounded retries
/// (docs/architecture/mcp.md, "desktop 내장 host: 비동기 창"). The
/// phase this leaves behind ([`McpImportPhase::Loading`] until the
/// load finishes) is what closes that window's blind spot for
/// attached clients.
pub fn spawn_detached_load_for_host() {
    drop(spawn_load_for_host(|| {}));
}

/// Pure formatter for the startup summary. Returns an empty string when
/// nothing was declared, so a host with no imports stays silent.
pub(crate) fn format_summary(load: &McpImportLoad) -> String {
    let mut out = String::new();
    for (name, result) in &load.servers {
        match result {
            Ok(outcome) => {
                for skipped in &outcome.skipped {
                    out.push_str(&format!(
                        "upeg:   ~ mcp import `{name}` skipped tool `{}`: {}\n",
                        skipped.id, skipped.reason,
                    ));
                }
            }
            Err(err) => {
                out.push_str(&format!("upeg:   ✗ mcp import `{name}`: {err}\n"));
            }
        }
    }
    if let DirectoryStatus::Loaded { path, .. } = &load.directory
        && !load.servers.is_empty()
    {
        out.push_str(&format!(
            "upeg: imported {} tool(s) from {} upstream MCP server(s) at {} ({} failed, {} tool(s) skipped)\n",
            load.tool_count(),
            load.loaded_server_count(),
            path.display(),
            load.failed_server_count(),
            load.skipped_tool_count(),
        ));
    }
    out
}

/// Load and report in one step. `eprint!` (not stdout) because the
/// `upeg mcp` lane owns stdout for JSON-RPC frames.
///
/// Skips loading entirely when this process is itself somebody's
/// MCP-import upstream (E-6 self-import recursion guard, doc comment
/// above) — the original host that spawned it already loaded imports
/// before this process existed.
pub(crate) fn load_and_report_for_host() {
    if upeg_sources::mcp_import::is_mcp_import_child() {
        set_import_phase(McpImportPhase::Skipped);
        return;
    }
    set_import_phase(McpImportPhase::Loading);
    let load = load_for_host();
    let summary = format_summary(&load);
    if !summary.is_empty() {
        eprint_best_effort(&summary);
    }
    // Publish the tally BEFORE the load is retained (retaining moves
    // it) and only once every registration is live, so a client that
    // reads `Done` can immediately read the full tool list too.
    set_import_phase(McpImportPhase::Done(McpImportTally::of(&load)));
    retain_for_process_lifetime(load);
}

/// Keep the import registrations alive for as long as this process
/// serves.
///
/// `ImportOutcome::registration` is an OWNING handle: dropping it
/// deregisters every imported tool and releases the last `Arc` to the
/// upstream subprocess, which then exits. Before this, the summary was
/// formatted and the whole `McpImportLoad` fell off the end of
/// [`load_and_report_for_host`] — so a host reported "imported N
/// tool(s)" and then had zero of them a microsecond later, and the
/// child MCP server it had just spawned was already dead.
///
/// There is no unload and no reload path (docs/architecture/mcp.md):
/// restarting the host process re-imports. "Live until the process
/// ends" is therefore the exact lifetime wanted, and a deliberate,
/// once-per-process leak expresses it without a global mutex whose
/// only job would be to never be unlocked.
fn retain_for_process_lifetime(load: McpImportLoad) {
    let _retained: &'static mut McpImportLoad = Box::leak(Box::new(load));
}

/// Best-effort write to stderr. `eprint!`/`eprintln!` panic (`"failed
/// printing to stdout: ..."`, the standard library's message for
/// EVERY `print!`-family macro, stdout included) when the underlying
/// fd is gone. Before the E-6 recursion guard existed, a self-import
/// fork-bomb left many recursively-spawned processes sharing an
/// inherited stderr (`spawn_child` uses `Stdio::inherit()`) that the
/// top-level session could tear down mid-write — a startup summary
/// line is never worth crashing the host over, so this swallows the
/// write error the same way every stdout write in `surfaces::mcp`
/// already does (`let _ = writeln!(...)`).
fn eprint_best_effort(s: &str) {
    write_best_effort(std::io::stderr(), s);
}

/// Pure(ish) helper for [`eprint_best_effort`]: takes the writer
/// explicitly so a test can inject one that always fails, without
/// needing to actually close the process's real stderr fd.
fn write_best_effort<W: std::io::Write>(mut writer: W, s: &str) {
    let _ = writer.write_all(s.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A writer that always fails with `BrokenPipe` — simulates the
    /// E-6 scenario (inherited stderr torn down mid-write) without
    /// touching the real process stderr fd.
    struct AlwaysBrokenPipe;

    impl std::io::Write for AlwaysBrokenPipe {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
    }

    #[test]
    fn broken_pipe_라이터에_써도_패닉하지_않는다() {
        // `eprint!`/`eprintln!` panic on a write failure — this is
        // exactly the "71 broken pipe panics" symptom from E-6.
        // `write_best_effort` must swallow the error instead.
        write_best_effort(AlwaysBrokenPipe, "upeg: imported 1 tool(s)\n");
        // Reaching this line without panicking is the assertion.
    }

    fn empty_load() -> McpImportLoad {
        McpImportLoad {
            directory: DirectoryStatus::Unconfigured,
            servers: Vec::new(),
        }
    }

    #[test]
    fn 선언이_없으면_요약은_비어_있다() {
        assert!(format_summary(&empty_load()).is_empty());
    }

    #[test]
    fn 미설정_디렉터리는_로드해도_아무것도_등록하지_않는다() {
        let load = empty_load();
        assert_eq!(load.loaded_server_count(), 0);
        assert_eq!(load.tool_count(), 0);
    }

    use super::페이즈_직렬화;

    #[test]
    fn 초기_페이즈는_not_started이고_pending이_아니다() {
        assert_eq!(McpImportPhase::default(), McpImportPhase::NotStarted);
        assert!(!McpImportPhase::NotStarted.is_pending());
    }

    #[test]
    fn loading만_pending이다() {
        // 로딩 중일 때만 클라이언트가 tools/list를 다시 읽어야 한다.
        assert!(McpImportPhase::Loading.is_pending());
        assert!(!McpImportPhase::Skipped.is_pending());
        assert!(!McpImportPhase::Done(McpImportTally::default()).is_pending());
    }

    #[test]
    fn 페이즈는_설정한_값을_그대로_돌려준다() {
        let _serial = 페이즈_직렬화();
        let tally = McpImportTally {
            servers_loaded: 2,
            servers_failed: 1,
            tools: 7,
        };
        set_import_phase(McpImportPhase::Done(tally));
        assert_eq!(import_phase(), McpImportPhase::Done(tally));
        set_import_phase(McpImportPhase::NotStarted);
        assert_eq!(import_phase(), McpImportPhase::NotStarted);
    }

    #[test]
    fn 로더_스레드_예약은_작업이_돌기_전에_loading으로_전이한다() {
        // 스레드가 실제로 로드를 시작하기 전에 이미 pending 이어야
        // 한다 — 그 사이에 /healthz 를 읽은 클라이언트가 "더 올
        // 것이 없다"고 오해하면 안 된다. 실제 업스트림을 spawn하지
        // 않도록 로더 본문 대신 게이트로 막힌 closure를 넘긴다.
        let _serial = 페이즈_직렬화();
        set_import_phase(McpImportPhase::NotStarted);
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let handle = spawn_marked_loader(move || {
            // 예약 시점의 assert 가 끝날 때까지 작업은 시작조차 않는다.
            let _ = release_rx.recv();
            let _ = done_tx.send(());
        })
        .expect("로더 스레드 spawn");

        assert!(import_phase().is_pending(), "예약 직후 pending 이어야 한다");

        let _ = release_tx.send(());
        let _ = done_rx.recv();
        let _ = handle.join();
        set_import_phase(McpImportPhase::NotStarted);
    }

    #[test]
    fn pending_마크는_로더보다_먼저_찍히고_되돌릴_수_있다() {
        // desktop lane 은 embed 스레드를 spawn 하기 전에 마크한다 —
        // 그 스레드가 바인드하는 순간부터 /healthz 가 답하기 때문이다.
        let _serial = 페이즈_직렬화();
        set_import_phase(McpImportPhase::NotStarted);

        mark_pending();
        assert!(import_phase().is_pending(), "마크 직후 pending 이어야 한다");

        clear_pending();
        assert_eq!(
            import_phase(),
            McpImportPhase::NotStarted,
            "host 가 뜨지 않았으면 마크를 돌려줘야 한다"
        );
    }

    #[test]
    fn pending_마크_반납은_이미_끝난_로드의_집계를_지우지_않는다() {
        // clear 는 Loading 만 되돌린다. 끝난 로드를 NotStarted 로
        // 덮으면 "아무것도 임포트하지 않았다"는 거짓말이 된다.
        let _serial = 페이즈_직렬화();
        let tally = McpImportTally {
            servers_loaded: 1,
            servers_failed: 0,
            tools: 3,
        };
        set_import_phase(McpImportPhase::Done(tally));

        clear_pending();

        assert_eq!(import_phase(), McpImportPhase::Done(tally));
        set_import_phase(McpImportPhase::NotStarted);
    }

    #[test]
    fn 마크된_상태에서_로더를_예약해도_여전히_pending이다() {
        // desktop lane 은 mark_pending 뒤에 spawn_marked_loader 를
        // 부른다 — 두 번 찍어도 같은 phase 여야 한다(idempotent).
        let _serial = 페이즈_직렬화();
        set_import_phase(McpImportPhase::NotStarted);
        mark_pending();

        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let handle = spawn_marked_loader(move || {
            let _ = release_rx.recv();
            let _ = done_tx.send(());
        })
        .expect("로더 스레드 spawn");

        assert!(
            import_phase().is_pending(),
            "마크가 예약으로 깨지면 안 된다"
        );

        let _ = release_tx.send(());
        let _ = done_rx.recv();
        let _ = handle.join();
        set_import_phase(McpImportPhase::NotStarted);
    }

    #[test]
    fn 완료_페이즈_json은_상태와_세_가지_카운트를_담는다() {
        let value = phase_json(McpImportPhase::Done(McpImportTally {
            servers_loaded: 3,
            servers_failed: 1,
            tools: 12,
        }));
        assert_eq!(value[PHASE_FIELD], PHASE_DONE);
        assert_eq!(value[SERVERS_LOADED_FIELD], 3);
        assert_eq!(value[SERVERS_FAILED_FIELD], 1);
        assert_eq!(value[TOOLS_FIELD], 12);
    }

    #[test]
    fn 미완료_페이즈_json은_카운트를_지어내지_않는다() {
        for (phase, expected) in [
            (McpImportPhase::NotStarted, PHASE_NOT_STARTED),
            (McpImportPhase::Loading, PHASE_LOADING),
            (McpImportPhase::Skipped, PHASE_SKIPPED),
        ] {
            let value = phase_json(phase);
            assert_eq!(value[PHASE_FIELD], expected);
            assert!(
                value.get(TOOLS_FIELD).is_none(),
                "로드가 끝나지 않았는데 개수를 보고하면 안 된다: {value}"
            );
        }
    }

    #[test]
    fn healthz_필드는_pending_플래그와_상세_블록을_함께_낸다() {
        let fields = healthz_fields(McpImportPhase::Loading);
        assert_eq!(fields[IMPORTS_PENDING_FIELD], json!(true));
        assert_eq!(fields[MCP_IMPORTS_FIELD][PHASE_FIELD], PHASE_LOADING);

        let done = healthz_fields(McpImportPhase::Done(McpImportTally::default()));
        assert_eq!(done[IMPORTS_PENDING_FIELD], json!(false));
    }

    #[test]
    fn healthz_필드는_카운트_외의_것을_싣지_않는다() {
        // 이 라우트는 인증이 없다. 개수는 괜찮지만 사용자가 어떤
        // 서버를 선언했는지(이름, command, 경로)는 나가면 안 된다.
        // 필드 집합을 통째로 고정해, 나중에 필드를 늘리려면 이
        // 판단을 다시 하게 만든다.
        let fields = healthz_fields(McpImportPhase::Done(McpImportTally {
            servers_loaded: 1,
            servers_failed: 0,
            tools: 4,
        }));
        let detail = fields[MCP_IMPORTS_FIELD]
            .as_object()
            .expect("상세 블록은 객체다");

        let mut keys: Vec<&str> = detail.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected = [
            PHASE_FIELD,
            SERVERS_LOADED_FIELD,
            SERVERS_FAILED_FIELD,
            TOOLS_FIELD,
        ];
        expected.sort_unstable();
        assert_eq!(keys, expected, "인증 없는 라우트에 나갈 필드는 이 넷뿐이다");
        for (key, value) in detail {
            assert!(
                value.is_number() || value.is_string(),
                "{key} 는 스칼라여야 한다 (중첩 구조는 이름을 실어 나른다): {value}"
            );
        }
    }

    #[test]
    fn 호스트_healthz를_읽지_못하면_unknown을_보고한다() {
        let value = host_imports_json_from_healthz(None);
        assert_eq!(value[PHASE_FIELD], PHASE_UNKNOWN);
    }

    #[test]
    fn 임포트_필드가_없는_healthz도_unknown이다() {
        // 없는 필드를 false 로 채우면 "다 로드됐다"로 읽힌다.
        let body = json!({ "name": "upeg", "version": "0.0.0" });
        let value = host_imports_json_from_healthz(Some(&body));
        assert_eq!(value[PHASE_FIELD], PHASE_UNKNOWN);
    }

    #[test]
    fn 호스트_healthz의_임포트_블록은_pending과_함께_전달된다() {
        let body = json!({
            "name": "upeg",
            IMPORTS_PENDING_FIELD: true,
            MCP_IMPORTS_FIELD: { PHASE_FIELD: PHASE_LOADING },
        });
        let value = host_imports_json_from_healthz(Some(&body));
        assert_eq!(value[PHASE_FIELD], PHASE_LOADING);
        assert_eq!(value[IMPORTS_PENDING_FIELD], json!(true));
    }

    #[test]
    fn 실패한_서버는_요약에_이름과_사유가_남는다() {
        let load = McpImportLoad {
            directory: DirectoryStatus::Loaded {
                path: std::path::PathBuf::from("/tmp/mcp-imports"),
                count: 1,
            },
            servers: vec![(
                "broken".to_string(),
                Err(upeg_sources::mcp_import::ImportError::Protocol(
                    "handshake failed".to_string(),
                )),
            )],
        };

        let summary = format_summary(&load);

        assert!(summary.contains("mcp import `broken`"), "got {summary}");
        assert!(summary.contains("handshake failed"), "got {summary}");
        assert!(
            summary.contains("(1 failed"),
            "요약 줄이 실패 수를 보고해야 한다: {summary}"
        );
    }
}
