//! Shared HTTPS plumbing for the network toolkits (`eth`, `weather`).
//!
//! Mirrors `upeg-loader`'s `https_agent()` (same timeout discipline) without
//! depending on `upeg-loader` itself — that crate is native-only end to end
//! and pulls in the full subprocess/chain/llm/embed loader stack, which
//! `upeg-tools` must stay free of so it can still compile for the `wasm32`
//! (Flutter web) build.
//!
//! Native-only for the same physical reason as its callers: a real HTTP
//! request needs a socket, which the `wasm32` sandbox doesn't have. Nothing
//! here is `#[tool]`-annotated, so — unlike `eth.rs`/`weather.rs`, whose
//! `StaticToolMeta` must exist on every target — the whole module is gated
//! off at the `mod` declaration in `toolkits/mod.rs`.
//!
//! Each caller keeps its own agent (and therefore its own timeout): the
//! deadlines differ on purpose, so they stay named in the toolkit that
//! justifies them rather than being flattened into one shared number here.

use std::time::Duration;

/// HTTP statuses treated as success. Anything outside becomes
/// [`HttpError::Status`], carrying the body the server sent with it.
const SUCCESS_STATUS: std::ops::Range<u16> = 200..300;

/// How much of a non-2xx response body to keep in the error message. Enough
/// for an API's `{"error":true,"reason":"..."}` to survive, short enough that
/// an HTML error page doesn't flood a CLI/TUI line.
const ERROR_BODY_MAX_CHARS: usize = 200;

/// Marker appended to an error body cut short by [`ERROR_BODY_MAX_CHARS`].
const TRUNCATION_MARKER: &str = "…";

/// Request header naming a JSON request body.
#[cfg(feature = "eth")]
const CONTENT_TYPE_HEADER: &str = "Content-Type";
/// Media type for the JSON bodies this module posts.
#[cfg(feature = "eth")]
const JSON_MEDIA_TYPE: &str = "application/json";

/// Why an HTTP call failed. Callers prefix the `Display` form with their own
/// toolkit's wording (`"weather {error}"`, `"eth RPC {error}"`), so the stage
/// that failed is named here exactly once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// The request never completed — DNS, TLS, connect, or timeout.
    Transport(String),
    /// The response arrived but its body could not be read.
    BodyRead(String),
    /// The response arrived with a non-2xx status.
    Status {
        /// The status code the server returned.
        status: u16,
        /// The response body, truncated to [`ERROR_BODY_MAX_CHARS`].
        body: String,
    },
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(cause) => write!(formatter, "request failed: {cause}"),
            Self::BodyRead(cause) => write!(formatter, "response read failed: {cause}"),
            Self::Status { status, body } => {
                write!(formatter, "request failed: HTTP {status}: {body}")
            }
        }
    }
}

/// Build an HTTPS agent whose connect/read/write deadline is `timeout`.
/// Statuses are not raised as transport errors — [`read_body`] classifies
/// them instead, so the error body survives.
pub fn build_agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .build(),
    )
}

/// Cut `body` down to [`ERROR_BODY_MAX_CHARS`] characters for use in an error
/// message, marking it when something was dropped.
fn truncate_error_body(body: &str) -> String {
    let trimmed = body.trim();
    let mut kept: String = trimmed.chars().take(ERROR_BODY_MAX_CHARS).collect();
    if kept.chars().count() < trimmed.chars().count() {
        kept.push_str(TRUNCATION_MARKER);
    }
    kept
}

/// Read a completed response's body, mapping a non-2xx status to
/// [`HttpError::Status`] *after* the body has been read so the server's own
/// explanation reaches the caller.
fn read_body(
    response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<String, HttpError> {
    let mut response = response.map_err(|error| HttpError::Transport(error.to_string()))?;
    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|error| HttpError::BodyRead(error.to_string()))?;
    if !SUCCESS_STATUS.contains(&status) {
        return Err(HttpError::Status {
            status,
            body: truncate_error_body(&text),
        });
    }
    Ok(text)
}

/// GET `url`, returning the response body on a 2xx status.
#[cfg(feature = "weather")]
pub fn get(agent: &ureq::Agent, url: &str) -> Result<String, HttpError> {
    read_body(agent.get(url).call())
}

/// POST `body` to `url` as JSON, returning the response body on a 2xx status.
#[cfg(feature = "eth")]
pub fn post_json(agent: &ureq::Agent, url: &str, body: &str) -> Result<String, HttpError> {
    read_body(
        agent
            .post(url)
            .header(CONTENT_TYPE_HEADER, JSON_MEDIA_TYPE)
            .send(body),
    )
}

// Pure tests only — nothing here opens a socket, so the suite stays offline
// and instant. The `get`/`post_json` wiring is exercised by the toolkits that
// call it; what's worth pinning is the error shaping every caller renders.
#[cfg(test)]
mod tests {
    use super::*;

    // ─── SUCCESS_STATUS ─────────────────────────────────────────

    #[test]
    fn success_status_range_covers_only_2xx() {
        assert!(SUCCESS_STATUS.contains(&200));
        assert!(SUCCESS_STATUS.contains(&204));
        assert!(SUCCESS_STATUS.contains(&299));
        assert!(!SUCCESS_STATUS.contains(&199));
        assert!(!SUCCESS_STATUS.contains(&300));
        assert!(!SUCCESS_STATUS.contains(&400));
    }

    // ─── truncate_error_body ────────────────────────────────────

    #[test]
    fn short_error_body_is_kept_verbatim() {
        assert_eq!(
            truncate_error_body(r#"{"error":true,"reason":"bad value"}"#),
            r#"{"error":true,"reason":"bad value"}"#
        );
    }

    #[test]
    fn error_body_surrounding_whitespace_is_trimmed() {
        assert_eq!(truncate_error_body("  boom \n"), "boom");
    }

    #[test]
    fn long_error_body_is_truncated_with_marker() {
        let body = "x".repeat(ERROR_BODY_MAX_CHARS + 10);
        let truncated = truncate_error_body(&body);
        assert_eq!(
            truncated.chars().count(),
            ERROR_BODY_MAX_CHARS + TRUNCATION_MARKER.chars().count()
        );
        assert!(truncated.ends_with(TRUNCATION_MARKER));
    }

    #[test]
    fn error_body_truncation_respects_char_boundaries() {
        let body = "가".repeat(ERROR_BODY_MAX_CHARS + 1);
        let truncated = truncate_error_body(&body);
        assert!(truncated.starts_with('가'));
        assert!(truncated.ends_with(TRUNCATION_MARKER));
    }

    // ─── HttpError Display ──────────────────────────────────────

    #[test]
    fn transport_failure_display_names_the_stage() {
        assert_eq!(
            HttpError::Transport("connection refused".to_string()).to_string(),
            "request failed: connection refused"
        );
    }

    #[test]
    fn body_read_failure_display_names_the_stage() {
        assert_eq!(
            HttpError::BodyRead("stream closed".to_string()).to_string(),
            "response read failed: stream closed"
        );
    }

    #[test]
    fn status_failure_display_includes_status_code_and_body() {
        assert_eq!(
            HttpError::Status {
                status: 400,
                body: r#"{"reason":"days must be <= 16"}"#.to_string(),
            }
            .to_string(),
            r#"request failed: HTTP 400: {"reason":"days must be <= 16"}"#
        );
    }
}
