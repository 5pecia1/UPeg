//! Embed-or-attach HTTP host bootstrap. PRD §5.2 / §5.9.
//!
//! Drives FRB `init_app` host discovery:
//!   - If a host is already running (`current_host()`), classify it
//!     via [`classify_reachable_host`]: `HostState::Embedded` when the
//!     reachable host is this process's own in-process embed (pid or
//!     `HostOrigin::Embedded` match), else `HostState::Attached`.
//!   - Else, if the user has not switched on the "Local HTTP host"
//!     preference (`Tweaks::local_http_host`, default OFF), return
//!     `HostState::NoHost`. A network listener is never opened without
//!     an explicit decision (FR-16).
//!   - Else, mark MCP imports pending, spawn a worker thread that runs
//!     the embedded HTTP host, and wait up to 3s for the bound
//!     endpoint; return `HostState::Embedded { endpoint }` on success,
//!     then register MCP imports off the boot path — see
//!     [`spawn_mcp_import_load`] for the async window that buys, and
//!     [`EmbedOutcome`] for why the pending mark has to come first.
//!
//! The embed worker thread is intentionally fire-and-forget — no
//! explicit stop signal. `Drop` of the discovery guard inside
//! `embedded_http_with_ready` handles cleanup at process exit.
//!
//! RC-7 (single instance): [`claim_embed_slot`] / [`release_embed_slot`]
//! form a process-wide win-once gate so two overlapping
//! [`ensure_host_for_desktop`] calls never spawn a second embed worker
//! thread. The slot is claimed right before the thread starts and
//! released on the exits that leave no worker behind (spawn error, bind
//! error); a successful embed — and a not-yet-signalled one — keeps it
//! claimed.

#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use upeg_cli::{HostOrigin, ServerInfo};

use crate::api::boot::HostState;

const EMBED_READY_TIMEOUT_SECS: u64 = 3;

/// Decide host state at app boot. Returns `HostState::Attached` /
/// `Embedded` / `NoHost` per the rules described in the module docs.
pub fn ensure_host_for_desktop() -> HostState {
    if let Some(info) = upeg_cli::current_host() {
        let state = classify_reachable_host(&info);
        tracing::info!(
            endpoint = %info.endpoint,
            embedded = matches!(state, HostState::Embedded { .. }),
            "upeg-frb found reachable host"
        );
        return state;
    }
    if !local_http_host_enabled() {
        tracing::info!("upeg-frb starting without host; `Local HTTP host` preference is off");
        return HostState::NoHost;
    }
    let (state, imports) = resolve_embed(embed_or_attach_http());
    match imports {
        ImportMark::Schedule => spawn_mcp_import_load(),
        ImportMark::HandBack => upeg_cli::clear_mcp_imports_pending(),
    }
    state
}

/// What an [`EmbedOutcome`] owes the imports-pending mark taken before
/// the embed thread was spawned.
#[derive(Debug, PartialEq, Eq)]
enum ImportMark {
    /// A host of ours is (or may still become) live: schedule the real
    /// load, which keeps the phase `Loading` until it reports `Done`.
    Schedule,
    /// No host of ours will answer: give the mark back so nothing waits
    /// on a load that was never scheduled.
    HandBack,
}

/// Fold an [`EmbedOutcome`] into the host state to report and the fate
/// of the pending mark. Pure, so the branch that used to have no answer
/// at all — the ready-timeout — is asserted like every other one.
fn resolve_embed(outcome: EmbedOutcome) -> (HostState, ImportMark) {
    match outcome {
        EmbedOutcome::Embedded { endpoint } => {
            (HostState::Embedded { endpoint }, ImportMark::Schedule)
        }
        // The worker thread is still alive and may bind a moment from
        // now. Whatever host it becomes is ours, so it gets its imports
        // — and the pending mark it is already publishing stays
        // truthful all the way to `Done`. Boot still reports `NoHost`:
        // nothing has answered yet.
        EmbedOutcome::StillComing => (HostState::NoHost, ImportMark::Schedule),
        // Somebody else's host: it loaded (or did not load) its own
        // imports in its own process.
        EmbedOutcome::Attached { endpoint } => {
            (HostState::Attached { endpoint }, ImportMark::HandBack)
        }
        EmbedOutcome::None => (HostState::NoHost, ImportMark::HandBack),
    }
}

/// What one embed attempt produced, in the detail the imports-pending
/// mark needs — which is finer than [`HostState`].
///
/// [`HostState::NoHost`] covers two opposite futures: "the embed thread
/// failed and nothing of ours will ever answer" and "the embed thread
/// has not signalled yet but may bind in a moment". They owe opposite
/// things to the pending mark, so the routing decision is made on this
/// type and collapsed to `HostState` only afterwards.
enum EmbedOutcome {
    /// The embed bound and signalled its endpoint.
    Embedded { endpoint: String },
    /// This process lost the embed-slot race and found the winner's
    /// host instead — no embed of ours, so no import load of ours.
    Attached { endpoint: String },
    /// The worker thread is alive but did not signal within
    /// [`EMBED_READY_TIMEOUT_SECS`]; it may still bind.
    StillComing,
    /// The embed failed outright (spawn error or bind error).
    None,
}

/// Whether the persisted desktop preference asks this process to become
/// the host. Reads the same tweaks record the Settings form writes;
/// an absent or unreadable record means OFF.
fn local_http_host_enabled() -> bool {
    upeg_pegboard_ui::features::tweaks::load_tweaks()
        .unwrap_or_default()
        .local_http_host
}

/// Register upstream MCP servers for the host we just embedded.
///
/// Off the boot path deliberately: each upstream spawn is bounded by
/// the import layer's own timeouts, and several slow servers would
/// otherwise hold the splash screen well past
/// [`EMBED_READY_TIMEOUT_SECS`]. Failures are logged by the loader and
/// are never fatal.
///
/// The accepted cost is an **async window**: `server.json` is already
/// published and the host is already answering requests when this
/// starts, so for a few seconds `/v1/tools` and `/mcp`'s `tools/list`
/// can be missing the imported tools. The HTTP JSON-RPC `/mcp`
/// endpoint still has no server→client push channel (the stdio `upeg
/// mcp` lane, which does, emits `notifications/tools/list_changed`
/// instead), so nothing *tells* a client when the window closes — but
/// the window is no longer invisible: this call marks the load
/// pending before it returns, and the host publishes that as
/// `importsPending` on `/healthz`, which `upeg host status --json` and
/// the desktop status bar both read. Clients recover by re-reading
/// `tools/list`. Documented in docs/architecture/mcp.md
/// ("desktop 내장 host: 비동기 창").
fn spawn_mcp_import_load() {
    // The worker thread, its name, and the "imports pending" phase the
    // window is now observable through all live in `upeg-cli`
    // (`infrastructure::mcp_imports`) — the `upeg mcp` stdio lane
    // schedules its load through the same code, so neither lane can
    // drift from the other.
    upeg_cli::spawn_mcp_imports_for_host();
}

/// Classify an already-reachable host as our own in-process embed or a
/// separate host process.
///
/// Two independent signals both mean "this process embedded it": the
/// recorded pid equals ours, or the recorded origin is
/// `HostOrigin::Embedded`. Checking both makes the call robust even if
/// one signal is stale — e.g. a legacy `server.json` written before
/// `origin` existed deserializes to the conservative default
/// `HostOrigin::Explicit`, so the pid check alone still catches the
/// same-process case. A separate `upeg http --daemon` (or foreground)
/// host has a different pid *and* `origin != Embedded`, so it stays
/// `Attached`.
fn classify_reachable_host(info: &ServerInfo) -> HostState {
    if info.pid == std::process::id() || info.origin == HostOrigin::Embedded {
        HostState::Embedded {
            endpoint: info.endpoint.clone(),
        }
    } else {
        HostState::Attached {
            endpoint: info.endpoint.clone(),
        }
    }
}

/// Spawn the embedded HTTP host in a worker thread and wait for the
/// ready signal, classified as an [`EmbedOutcome`].
///
/// Gated by [`claim_embed_slot`] (RC-7): a caller that loses the race
/// never starts a second embed worker thread. It briefly tries to
/// attach instead — the winner's host may already be reachable by the
/// time the loser runs.
///
/// The imports-pending mark is taken BEFORE the worker thread exists,
/// because the thread binds its listener and starts answering
/// `/healthz` before it signals ready here. Everything after the spawn
/// therefore has to hand the mark back on the paths where no load will
/// follow, which is [`ensure_host_for_desktop`]'s job on the outcome
/// this returns.
fn embed_or_attach_http() -> EmbedOutcome {
    if !claim_embed_slot() {
        tracing::info!("embed slot already claimed; attaching instead of starting a 2nd host");
        return upeg_cli::current_host().map_or(EmbedOutcome::None, |info| {
            EmbedOutcome::Attached {
                endpoint: info.endpoint,
            }
        });
    }

    let (tx, rx) = std::sync::mpsc::channel();
    // Before the spawn, not after the ready signal: from the instant
    // the worker binds, `/healthz` answers, and an unmarked host would
    // report `importsPending: false` while an import load is certain to
    // follow.
    upeg_cli::mark_mcp_imports_pending();
    let spawn_result = std::thread::Builder::new()
        .name("upeg-http-embed".into())
        .spawn(move || {
            // Desktop host opts into OS notifications (PRD §5.9).
            if let Err(err) = upeg_cli::embedded_http_with_ready(true, tx) {
                tracing::error!(error = %err, "embedded HTTP server failed");
            }
        });
    if let Err(err) = spawn_result {
        tracing::warn!(error = %err, "failed to spawn embedded HTTP thread");
        release_embed_slot();
        return EmbedOutcome::None;
    }

    match rx.recv_timeout(Duration::from_secs(EMBED_READY_TIMEOUT_SECS)) {
        Ok(Ok(endpoint)) => {
            tracing::info!(endpoint = %endpoint, "upeg-frb hosting embedded HTTP");
            // Stays claimed for the process lifetime — no release here.
            EmbedOutcome::Embedded { endpoint }
        }
        Ok(Err(err)) => {
            tracing::warn!(error = %err, "embedded HTTP bind failed");
            release_embed_slot();
            EmbedOutcome::None
        }
        // Not a failure: the thread still holds the channel and may bind
        // at any moment. The slot stays claimed for exactly that reason
        // — releasing it would let a later call spawn a SECOND embed
        // against the first one's pending bind.
        Err(_) => {
            tracing::warn!("embedded HTTP did not signal ready within {EMBED_READY_TIMEOUT_SECS}s");
            EmbedOutcome::StillComing
        }
    }
}

/// Win-once guard against starting a second in-process embed worker
/// thread (RC-7 desktop side). [`ensure_host_for_desktop`] can be
/// re-entered; without this slot two calls would race into
/// `embed_or_attach_http` and spawn two HTTP hosts. `false` is the
/// "free" state.
static EMBED_SLOT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Try to claim the embed slot. Returns `true` exactly once per
/// claim/release cycle — a concurrent or subsequent caller sees `false`
/// until [`release_embed_slot`] runs.
fn claim_embed_slot() -> bool {
    EMBED_SLOT
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .is_ok()
}

/// Release the embed slot. Called on every embed exit that leaves NO
/// worker thread behind (spawn error, bind error) so a later attempt
/// can retry. Never called on success, and never on the ready-timeout
/// path — a thread that has not signalled yet may still bind, and a
/// released slot would let a second embed race it.
fn release_embed_slot() {
    EMBED_SLOT.store(false, std::sync::atomic::Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMBED_ENDPOINT: &str = "http://127.0.0.1:9";

    /// The desktop lane marks imports pending BEFORE the embed thread
    /// exists, because that thread starts answering `/healthz` before it
    /// signals ready here. Every outcome then has to say what becomes of
    /// that mark — including the ready-timeout, which previously fell
    /// through `unwrap_or(NoHost)` and stamped nothing at all.
    #[test]
    fn embed_결과마다_imports_pending_마크의_운명이_정해진다() {
        let (state, mark) = resolve_embed(EmbedOutcome::Embedded {
            endpoint: EMBED_ENDPOINT.to_string(),
        });
        assert!(matches!(state, HostState::Embedded { endpoint } if endpoint == EMBED_ENDPOINT));
        assert_eq!(mark, ImportMark::Schedule);

        // ready 신호가 늦었을 뿐 스레드는 살아 있다 — 이 스레드가 바인드하면
        // 그 host 는 우리 것이고, 임포트도 우리가 실어야 한다.
        let (state, mark) = resolve_embed(EmbedOutcome::StillComing);
        assert!(matches!(state, HostState::NoHost));
        assert_eq!(
            mark,
            ImportMark::Schedule,
            "타임아웃 분기가 마크를 방치하거나 로드를 건너뛰면 안 된다"
        );

        let (state, mark) = resolve_embed(EmbedOutcome::Attached {
            endpoint: EMBED_ENDPOINT.to_string(),
        });
        assert!(matches!(state, HostState::Attached { endpoint } if endpoint == EMBED_ENDPOINT));
        assert_eq!(mark, ImportMark::HandBack);

        let (state, mark) = resolve_embed(EmbedOutcome::None);
        assert!(matches!(state, HostState::NoHost));
        assert_eq!(mark, ImportMark::HandBack);
    }

    /// The ordering itself: after the mark, and before any load is
    /// scheduled, this process already answers "imports are pending".
    #[test]
    fn imports_pending_마크는_로더_예약보다_먼저_보인다() {
        upeg_cli::mark_mcp_imports_pending();
        assert!(
            upeg_cli::mcp_import_phase().is_pending(),
            "embed 스레드가 /healthz 에 답하기 전에 이미 pending 이어야 한다"
        );

        upeg_cli::clear_mcp_imports_pending();
        assert!(
            !upeg_cli::mcp_import_phase().is_pending(),
            "host 가 뜨지 않았으면 마크를 돌려줘야 한다"
        );
    }

    #[test]
    fn claim_embed_slot은_한번만_성공하고_release후_다시_성공한다() {
        // Reset first so this test is independent of run order within
        // the shared test binary (the static is process-wide).
        release_embed_slot();

        assert!(claim_embed_slot(), "first claim must win");
        assert!(!claim_embed_slot(), "second claim before release must lose");

        release_embed_slot();
        assert!(claim_embed_slot(), "claim must succeed again after release");

        release_embed_slot();
    }

    /// Offset added to our own pid to build a guaranteed-different
    /// "foreign" pid for test fixtures — `wrapping_add` so it never
    /// panics on overflow regardless of build profile.
    const FOREIGN_PID_OFFSET: u32 = 1;
    const TEST_ENDPOINT: &str = "http://127.0.0.1:0";

    fn server_info(pid: u32, origin: HostOrigin) -> ServerInfo {
        ServerInfo {
            endpoint: TEST_ENDPOINT.to_string(),
            mcp_endpoint: format!("{TEST_ENDPOINT}/mcp"),
            token: "test-token".to_string(),
            pid,
            started_at_ms: 0,
            origin,
        }
    }

    #[test]
    fn classify_reachable_host는_자기_프로세스_또는_embedded면_embedded_아니면_attached다() {
        let own_pid = std::process::id();
        let foreign_pid = own_pid.wrapping_add(FOREIGN_PID_OFFSET);

        // Same pid, conservative origin → still our own embed.
        match classify_reachable_host(&server_info(own_pid, HostOrigin::Explicit)) {
            HostState::Embedded { endpoint } => assert_eq!(endpoint, TEST_ENDPOINT),
            other => panic!("expected Embedded for own pid, got {other:?}"),
        }

        // Foreign pid but origin says embedded → still our own embed
        // (e.g. the embed worker thread's record read back before this
        // call's pid comparison would matter).
        match classify_reachable_host(&server_info(foreign_pid, HostOrigin::Embedded)) {
            HostState::Embedded { endpoint } => assert_eq!(endpoint, TEST_ENDPOINT),
            other => panic!("expected Embedded for HostOrigin::Embedded, got {other:?}"),
        }

        // Foreign pid and non-embedded origin → a separate host
        // process (`upeg host start`, foreground or `--daemon`).
        match classify_reachable_host(&server_info(foreign_pid, HostOrigin::Explicit)) {
            HostState::Attached { endpoint } => assert_eq!(endpoint, TEST_ENDPOINT),
            other => panic!("expected Attached for foreign pid + Explicit origin, got {other:?}"),
        }
    }
}
