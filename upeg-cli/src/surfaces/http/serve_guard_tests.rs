//! Tests for the HTTP serve-side guards and startup-log presentation.
//!
//! Lives in a sibling file (attached via `#[path = ...]` from `http.rs`)
//! to keep `http.rs` under the workspace's 1000-line-per-file budget.
//! `super::` resolves to `crate::surfaces::http`, so private helpers
//! (`effective_publish`, `startup_log`, ...) remain accessible without
//! widening their visibility.

use super::{
    PublishFailure, ServerOptions, effective_publish, guard_http_bind_with_consent,
    is_loopback_bind, publish_failure_action, startup_log,
};
use crate::infrastructure::auth::{ResolvedToken, TokenSource};
use crate::infrastructure::discovery::HostOrigin;

#[test]
fn 루프백_http_바인드는_추가_동의_없이도_허용된다() {
    for addr in ["127.0.0.1:7173", "[::1]:7173", "localhost:7173"] {
        assert!(is_loopback_bind(addr), "{addr} should be loopback");
        assert!(
            guard_http_bind_with_consent(addr, false).is_ok(),
            "{addr} should be allowed"
        );
    }
}

#[test]
fn 루프백이_아닌_http_바인드는_추가_동의를_요구한다() {
    let err = guard_http_bind_with_consent("0.0.0.0:7173", false)
        .expect_err("wildcard bind must be refused");
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(
        err.to_string().contains("UPEG_HTTP_ALLOW_NON_LOOPBACK"),
        "error should name the consent switch; got {err}"
    );
    assert!(
        guard_http_bind_with_consent("0.0.0.0:7173", true).is_ok(),
        "explicit consent should allow non-loopback bind"
    );
}

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

#[test]
fn 유효한_공개_루프백은_의도를_유지한다() {
    let opts_yes = opts_with("t", TokenSource::Generated, true);
    let opts_no = opts_with("t", TokenSource::Generated, false);
    assert!(effective_publish(&opts_yes, false));
    assert!(!effective_publish(&opts_no, false));
}

#[test]
fn 토큰_공개가_유효하면_시작_로그는_게시된_토큰을_알린다() {
    let opts = opts_with("secret-abc", TokenSource::Generated, true);
    let out = render(&opts, true);
    assert!(
        out.contains("bearer token published to server.json"),
        "must announce publication; got {out}"
    );
    assert!(
        !out.contains("secret-abc"),
        "must not leak the token when it was published"
    );
    assert!(
        !out.contains("discovery publish suppressed"),
        "no suppression notice when intent matches effect"
    );
}

#[test]
fn 토큰_공개가_억제되면_시작_로그는_생성된_토큰만_출력한다() {
    // The reported P1: caller asked for publish, gate said no, operator
    // must still receive the credential.
    let opts = opts_with("secret-xyz", TokenSource::Generated, true);
    let out = render(&opts, false);
    assert!(
        out.contains("discovery publish suppressed"),
        "must explain why publish did not happen; got {out}"
    );
    assert!(
        out.contains("bearer token (this run): secret-xyz"),
        "must print the generated token so operators have credentials; got {out}"
    );
    assert!(
        !out.contains("published to server.json"),
        "must not falsely claim publication; got {out}"
    );
}

#[test]
fn 시작_로그는_제공된_토큰에_대해_조용히_유지된다() {
    let opts = opts_with("provided-token", TokenSource::Provided, true);
    let out = render(&opts, false);
    assert!(
        !out.contains("provided-token"),
        "explicit tokens must not be echoed; got {out}"
    );
    assert!(
        !out.contains("published to server.json"),
        "no publication line for non-published runs; got {out}"
    );
}

#[test]
fn publish_failure_action은_live_owner의_alreadyexists만_abort한다() {
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
