//! Tests for the HTTP serve-side guards and startup-log presentation.
//!
//! Lives in a sibling file (`mod serve_guard_tests;` in `mod.rs`) to keep
//! `mod.rs` under the workspace's 1000-line-per-file budget.
//! `super::` resolves to `crate::surfaces::http`, so private helpers
//! (`effective_publish`, `startup_log`, ...) remain accessible without
//! widening their visibility. The pure bind-policy table is covered next
//! to its implementation in `bind_policy.rs`; the tests here pin the
//! wiring — that `serve_inner` actually consults it — and the log lines.

use std::time::Duration;

use super::{
    PublishFailure, ServerOptions, effective_publish, publish_failure_action, serve_with_ready,
    startup_log,
};
use crate::infrastructure::auth::{ResolvedToken, TokenSource};
use crate::infrastructure::discovery::HostOrigin;

/// Obviously fake credential for assertions that a token is *not*
/// echoed. Never a real secret.
const FAKE_TOKEN: &str = "not-a-real-token";

fn opts_with(token: &str, source: TokenSource, publish_discovery: bool) -> ServerOptions {
    ServerOptions {
        addr: "127.0.0.1:0".into(),
        token: ResolvedToken {
            token: token.into(),
            source,
        },
        publish_discovery,
        allow_non_loopback: false,
        notifications_enabled: false,
        cors_origins: Vec::new(),
        origin: HostOrigin::Explicit,
    }
}

fn render(opts: &ServerOptions, publish: bool) -> String {
    let mut buf: Vec<u8> = Vec::new();
    startup_log(&mut buf, opts, "http://127.0.0.1:7173", publish).expect("write log");
    String::from_utf8(buf).expect("utf8")
}

/// Wiring pin: the serve path must refuse a non-loopback bind whose token
/// was auto-generated *before* touching the network, even when consent
/// is present. Run on a helper thread so a regression (the server coming
/// up) surfaces as a failed assertion instead of a hung test.
#[test]
fn serve_refuses_non_loopback_bind_with_generated_token_before_binding() {
    let opts = ServerOptions {
        addr: "0.0.0.0:0".into(),
        allow_non_loopback: true,
        ..opts_with(FAKE_TOKEN, TokenSource::Generated, false)
    };
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();

    let server = std::thread::spawn(move || serve_with_ready(opts, ready_tx));
    let ready = ready_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("serve must report readiness or refusal");

    let err = ready.expect_err("a generated token must never reach a non-loopback listener");
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(
        err.to_string().contains("UPEG_HTTP_TOKEN"),
        "refusal must point at token injection; got {err}"
    );
    assert!(
        !err.to_string().contains(FAKE_TOKEN),
        "refusal must not echo the token"
    );
    let returned = server.join().expect("serve thread must not panic");
    assert!(
        returned.is_err(),
        "serve must return the same refusal it signalled"
    );
}

#[test]
fn effective_publish_keeps_caller_intent_on_loopback() {
    let opts_yes = opts_with(FAKE_TOKEN, TokenSource::Generated, true);
    let opts_no = opts_with(FAKE_TOKEN, TokenSource::Generated, false);
    assert!(effective_publish(&opts_yes, false));
    assert!(!effective_publish(&opts_no, false));
}

#[test]
fn startup_log_announces_publication_without_echoing_the_token() {
    let opts = opts_with(FAKE_TOKEN, TokenSource::Generated, true);
    let out = render(&opts, true);
    assert!(
        out.contains("bearer token published to server.json"),
        "must announce publication; got {out}"
    );
    assert!(
        !out.contains(FAKE_TOKEN),
        "must not leak the token when it was published"
    );
    assert!(
        !out.contains("discovery publish suppressed"),
        "no suppression notice when intent matches effect"
    );
}

/// Under `--daemon` stderr is the log file, so the token must never be
/// written there — not even when the caller asked for publish and the
/// gate said no. A non-loopback bind always runs on an injected token
/// (`bind_policy`), so no operator is left without credentials by this.
#[test]
fn startup_log_never_prints_the_bearer_token_even_when_publish_is_suppressed() {
    let opts = opts_with(FAKE_TOKEN, TokenSource::Generated, true);
    let out = render(&opts, false);
    assert!(
        out.contains("discovery publish suppressed"),
        "must explain why publish did not happen; got {out}"
    );
    assert!(
        !out.contains(FAKE_TOKEN),
        "the bearer token must never be logged; got {out}"
    );
    assert!(
        !out.contains("published to server.json"),
        "must not falsely claim publication; got {out}"
    );
}

#[test]
fn startup_log_stays_silent_about_a_provided_token() {
    let opts = opts_with(FAKE_TOKEN, TokenSource::Provided, true);
    let out = render(&opts, false);
    assert!(
        !out.contains(FAKE_TOKEN),
        "explicit tokens must not be echoed; got {out}"
    );
    assert!(
        !out.contains("published to server.json"),
        "no publication line for non-published runs; got {out}"
    );
}

#[test]
fn publish_failure_action_aborts_only_when_a_live_owner_holds_discovery() {
    assert_eq!(
        publish_failure_action(true, std::io::ErrorKind::AlreadyExists),
        PublishFailure::Abort,
        "a lane that intends to publish, racing a live owner, must abort rather than serve headless"
    );
    assert_eq!(
        publish_failure_action(true, std::io::ErrorKind::PermissionDenied),
        PublishFailure::Tolerate,
        "non-AlreadyExists failures stay tolerant (e.g. unwritable config root)"
    );
    assert_eq!(
        publish_failure_action(false, std::io::ErrorKind::AlreadyExists),
        PublishFailure::Tolerate,
        "a lane that never intended to publish must not abort on a stale file it doesn't own"
    );
}
