//! Bounded retry + backoff around the initial spawn + handshake
//! (`initialize` then `tools/list`) for an MCP-import upstream (E-6).
//!
//! `UpstreamServer::spawn` itself never retries — direct callers (tests
//! included) rely on a single attempt failing fast. This module wraps
//! it for the one caller that benefits from resilience against a
//! transient hiccup (the server hasn't started answering stdio yet):
//! `register_server`/`register_dir`. Attempts are capped so a
//! *permanently* failing import — dead command, upstream that never
//! responds — still gives up in bounded time instead of retrying
//! forever, independent of whatever eventually calls this module.

use std::time::Duration;

use super::{ImportError, ParsedToolsList, UpstreamConfig, UpstreamServer, is_subprocess_dead};

/// Attempts a permanently failing upstream gets before
/// `register_server` gives up. `3` — enough to ride out a slow process
/// start without turning a genuinely dead upstream into a long hang.
pub(crate) const MCP_IMPORT_MAX_SPAWN_ATTEMPTS: u32 = 3;

/// Fixed pause between spawn+handshake attempts.
pub(crate) const MCP_IMPORT_SPAWN_RETRY_BACKOFF: Duration = Duration::from_millis(200);

/// Spawn the upstream and run the `initialize` + `tools/list` handshake,
/// retrying transient failures up to [`MCP_IMPORT_MAX_SPAWN_ATTEMPTS`]
/// times with [`MCP_IMPORT_SPAWN_RETRY_BACKOFF`] between attempts.
/// Non-transient failures (bad command, an explicit RPC error) return
/// on the first attempt — retrying a config mistake wastes the
/// caller's time without changing the outcome.
pub(crate) fn spawn_and_list_with_bounded_retry(
    name: &str,
    config: &UpstreamConfig,
) -> Result<(UpstreamServer, ParsedToolsList), ImportError> {
    let mut last_err: Option<ImportError> = None;
    for attempt in 1..=MCP_IMPORT_MAX_SPAWN_ATTEMPTS {
        let outcome = UpstreamServer::spawn(name, config).and_then(|mut server| {
            let parsed = server.tools_list()?;
            Ok((server, parsed))
        });
        match outcome {
            Ok(ready) => return Ok(ready),
            Err(err) if attempt < MCP_IMPORT_MAX_SPAWN_ATTEMPTS && is_retryable(&err) => {
                last_err = Some(err);
                std::thread::sleep(MCP_IMPORT_SPAWN_RETRY_BACKOFF);
            }
            Err(err) => return Err(err),
        }
    }
    // Every loop iteration either returns directly or (on the LAST
    // attempt) falls into the non-retryable arm above and returns —
    // `attempt < MAX` is false exactly when `attempt == MAX`, the
    // final iteration. This is unreachable, but a descriptive Err
    // beats `unreachable!()` panicking if a future refactor changes
    // the bound without updating this comment.
    Err(last_err.unwrap_or_else(|| {
        ImportError::Protocol(format!(
            "server `{name}` failed after {MCP_IMPORT_MAX_SPAWN_ATTEMPTS} attempts with no recorded error"
        ))
    }))
}

/// `true` for spawn/handshake errors worth retrying: the upstream
/// hasn't started answering yet ([`ImportError::Timeout`]), or its
/// pipe closed before it did ([`is_subprocess_dead`]). `false` for
/// errors retrying can't fix: an unknown command
/// ([`ImportError::Spawn`]), an explicit RPC error response, or any
/// other protocol-shape violation unrelated to a closed pipe.
fn is_retryable(err: &ImportError) -> bool {
    matches!(err, ImportError::Timeout { .. }) || is_subprocess_dead(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_오류는_재시도_대상이다() {
        let err = ImportError::Timeout {
            server: "s".into(),
            method: "initialize",
            timeout_ms: 1,
        };
        assert!(is_retryable(&err));
    }

    #[test]
    fn 알수없는_명령_오류는_재시도_대상이_아니다() {
        let err = ImportError::Spawn {
            command: "definitely-not-a-real-binary".into(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        };
        assert!(!is_retryable(&err));
    }

    #[test]
    fn 명시적_rpc_오류는_재시도_대상이_아니다() {
        let err = ImportError::Rpc {
            method: "tools/list",
            code: -32000,
            message: "boom".into(),
        };
        assert!(!is_retryable(&err));
    }
}
