//! CORS policy for the HTTP surface (Task B1 — host-attach foundation).
//!
//! Browser-origin clients (a PWA served over `http(s)`, the chrome-ext
//! popup) cannot call `/v1/*` without a CORS layer: browsers send a
//! preflight `OPTIONS` and require `Access-Control-Allow-*` response
//! headers before attempting the real request, and refuse to hand JS
//! the response body of a cross-origin request that lacks them.
//!
//! [`OriginPolicy`] is the single source of truth for "which Origins
//! may reach the HTTP surface" — both the `tower_http` CORS layer
//! (adds the browser-facing headers) and [`super::origin_and_host_allowed`]
//! (the server-side guard that actually enforces the check; the CORS
//! layer only controls what a *browser* is willing to expose to page
//! JS, so non-browser HTTP clients are still gated by the guard) read
//! it. Keeping both on one struct means they cannot drift apart.
//!
//! Policy:
//!   - `chrome-extension://<any-id>` — always allowed. Not
//!     DNS-resolvable, so it can't be used for a DNS-rebinding attack
//!     the way an attacker-controlled `http(s)://` Origin can; treating
//!     it as loopback-equivalent only widens the Origin check (the
//!     Host check in [`super::origin_and_host_allowed`] still enforces
//!     the request landed on a loopback bind).
//!   - `http://127.0.0.1:*`, `http://localhost:*`, `http://[::1]:*` —
//!     loopback web origins, allowed by default (a PWA served locally
//!     needs this to reach the same-machine host).
//!   - Explicit operator-configured web origins (`--cors-origin`,
//!     plumbed through [`super::ServerOptions::cors_origins`]) — exact
//!     string match. Never a wildcard (`*`) — [`OriginPolicy::new`]
//!     rejects one.
//!
//! Methods: `GET`, `POST`. Headers: [`allowed_headers`] — the two every
//! request carries plus the custom ones this surface's own routes
//! define. Exposed: [`exposed_headers`]. Credentials are NOT allowed
//! (`tower_http`'s default) — the bearer token travels explicitly in the
//! `Authorization` header, never as a cookie, so there is nothing for
//! `Access-Control-Allow-Credentials` to protect.

use axum::http::{HeaderName, Method, header};
use tower_http::cors::{AllowOrigin, CorsLayer};

use super::BOARD_SCOPE_HEADER;
use super::rpc::mcp_sse::MCP_SESSION_HEADER;

/// `chrome-extension://<id>` prefix — any extension id is accepted,
/// see the module doc for the loopback-equivalence rationale.
const CHROME_EXTENSION_ORIGIN_PREFIX: &str = "chrome-extension://";

/// The literal wildcard Origin string. Rejected by [`OriginPolicy::new`]
/// — the data plane never CORS-allows every website, only the fixed
/// set of loopback/extension origins plus an explicit allowlist.
const WILDCARD_ORIGIN: &str = "*";

const ALLOWED_METHODS: [Method; 2] = [Method::GET, Method::POST];

/// Request headers a browser may send to this surface.
///
/// The list is the surface's own request contract, minus levers a
/// browser could not pull anyway:
///
///   * `Authorization` / `Content-Type` — every call carries them.
///   * [`MCP_SESSION_HEADER`] — the `/mcp` lane issues it on
///     `initialize` and reads it back on later requests. Omitting it
///     failed the preflight, so a browser MCP client could hold a
///     session id it was never allowed to send
///     (`upeg_cli::surfaces` module docs).
///   * [`BOARD_SCOPE_HEADER`] — the `/mcp` lane's board scope. It grants
///     nothing: the board pin gate still decides, against the user's own
///     pegboard.
///
/// [`super::ORIGIN_SURFACE_HEADER`] is deliberately absent. It is
/// honored only for `cli`/`tui` and only under the operator token, and
/// no browser-delivered surface is either of those — allowing it would
/// advertise a lever that is inert from a browser.
fn allowed_headers() -> [HeaderName; 4] {
    [
        header::AUTHORIZATION,
        header::CONTENT_TYPE,
        HeaderName::from_static(MCP_SESSION_HEADER),
        HeaderName::from_static(BOARD_SCOPE_HEADER),
    ]
}

/// Response headers page JS may read.
///
/// A browser hides every non-safelisted response header from script
/// unless it is named here, so `initialize` answering with
/// [`MCP_SESSION_HEADER`] was invisible to exactly the clients that need
/// it — a session id the server issues and the client can never read is
/// no session id at all.
fn exposed_headers() -> [HeaderName; 1] {
    [HeaderName::from_static(MCP_SESSION_HEADER)]
}

/// One configured web origin was rejected by [`OriginPolicy::new`].
#[derive(Debug, thiserror::Error)]
pub enum OriginPolicyError {
    #[error(
        "--cors-origin cannot be `*` (wildcard web origins are never allowed; \
         loopback origins are already allowed by default)"
    )]
    Wildcard,
    #[error("--cors-origin `{0}` must start with `http://` or `https://`")]
    MissingScheme(String),
    #[error("--cors-origin `{0}` must be a bare origin (scheme + host[:port]), no path/query")]
    Malformed(String),
}

/// Origins allowed to reach the HTTP surface's data plane from a
/// browser. See the module doc for the full policy.
#[derive(Debug, Clone, Default)]
pub struct OriginPolicy {
    /// Exact web origins from `--cors-origin`, e.g. `https://app.example.com`.
    web_origins: Vec<String>,
}

impl OriginPolicy {
    /// Validate and store the operator-configured web-origin allowlist.
    /// Loopback and chrome-extension origins are implicit — callers
    /// only pass the extra web origins here.
    pub fn new(web_origins: Vec<String>) -> Result<Self, OriginPolicyError> {
        for origin in &web_origins {
            validate_web_origin(origin)?;
        }
        Ok(Self { web_origins })
    }

    /// True if the raw `Origin` header value `origin` may reach the
    /// HTTP surface: chrome-extension, loopback web origin, or the
    /// configured allowlist.
    pub fn allows(&self, origin: &str) -> bool {
        is_extension_origin(origin)
            || is_loopback_web_origin(origin)
            || self.web_origins.iter().any(|allowed| allowed == origin)
    }

    /// Build the `tower_http` CORS layer for this policy. Shares
    /// [`Self::allows`] with the server-side origin guard so a
    /// preflight can never promise more than the guard will honor.
    pub fn cors_layer(&self) -> CorsLayer {
        let policy = self.clone();
        CorsLayer::new()
            .allow_methods(ALLOWED_METHODS)
            .allow_headers(allowed_headers())
            .expose_headers(exposed_headers())
            .allow_origin(AllowOrigin::predicate(move |origin, _parts| {
                origin.to_str().is_ok_and(|origin| policy.allows(origin))
            }))
    }
}

fn validate_web_origin(origin: &str) -> Result<(), OriginPolicyError> {
    if origin == WILDCARD_ORIGIN {
        return Err(OriginPolicyError::Wildcard);
    }
    let Some(rest) = strip_origin_scheme(origin) else {
        return Err(OriginPolicyError::MissingScheme(origin.to_string()));
    };
    if rest.is_empty() || rest.contains('/') || rest.contains(char::is_whitespace) {
        return Err(OriginPolicyError::Malformed(origin.to_string()));
    }
    Ok(())
}

fn is_extension_origin(origin: &str) -> bool {
    origin.trim().starts_with(CHROME_EXTENSION_ORIGIN_PREFIX)
}

fn is_loopback_web_origin(origin: &str) -> bool {
    match strip_origin_scheme(origin.trim()) {
        Some(rest) => is_loopback_authority(rest),
        None => false,
    }
}

fn strip_origin_scheme(value: &str) -> Option<&str> {
    value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
}

/// Shared by [`is_loopback_web_origin`] (Origin header, CORS + guard)
/// and [`super::origin_and_host_allowed`]'s `Host` header check —
/// one parser for "is this authority loopback" everywhere it matters.
pub(super) fn is_loopback_authority(authority: &str) -> bool {
    let authority = authority.trim();
    if authority.is_empty() {
        return false;
    }
    // strip port
    let host = if let Some(end) = authority.strip_prefix('[')
        && let Some(close) = end.find(']')
    {
        &end[..close]
    } else if let Some(idx) = authority.rfind(':') {
        &authority[..idx]
    } else {
        authority
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1") || host.starts_with("127.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_origins_are_allowed_by_default() {
        let policy = OriginPolicy::default();
        assert!(policy.allows("chrome-extension://abcdefghijklmnop"));
    }

    #[test]
    fn loopback_web_origins_are_allowed_by_default() {
        let policy = OriginPolicy::default();
        assert!(policy.allows("http://127.0.0.1:5173"));
        assert!(policy.allows("http://localhost:5173"));
        assert!(policy.allows("http://[::1]:5173"));
    }

    #[test]
    fn unconfigured_external_web_origins_are_rejected() {
        let policy = OriginPolicy::default();
        assert!(!policy.allows("https://example.com"));
    }

    #[test]
    fn configured_web_origins_are_allowed() {
        let policy =
            OriginPolicy::new(vec!["https://app.example.com".to_string()]).expect("valid origin");
        assert!(policy.allows("https://app.example.com"));
        assert!(!policy.allows("https://other.example.com"));
    }

    #[test]
    fn wildcard_origins_are_rejected_in_configuration() {
        let err = OriginPolicy::new(vec!["*".to_string()]).expect_err("wildcard must be rejected");
        assert!(matches!(err, OriginPolicyError::Wildcard));
    }

    #[test]
    fn schemeless_origins_are_rejected_in_configuration() {
        let err =
            OriginPolicy::new(vec!["app.example.com".to_string()]).expect_err("missing scheme");
        assert!(matches!(err, OriginPolicyError::MissingScheme(_)));
    }

    #[test]
    fn mcp_session_header_is_allowed_for_sending_and_reading() {
        // Browsers block request headers not allowed at preflight and
        // hide response headers not exposed from scripts. The session id
        // is issued by the server and echoed back by the client, so both
        // directions are needed.
        let session = HeaderName::from_static(MCP_SESSION_HEADER);
        assert!(allowed_headers().contains(&session));
        assert!(exposed_headers().contains(&session));
    }

    #[test]
    fn origin_surface_header_is_not_exposed_to_browsers() {
        // The header is honored only for `cli`/`tui` + operator token.
        // A browser-delivered surface can be neither, so exposing it
        // would advertise a lever that cannot work.
        let origin_surface = HeaderName::from_static(super::super::ORIGIN_SURFACE_HEADER);
        assert!(!allowed_headers().contains(&origin_surface));
        assert!(!exposed_headers().contains(&origin_surface));
    }

    #[test]
    fn origins_with_paths_are_rejected_in_configuration() {
        let err = OriginPolicy::new(vec!["https://app.example.com/path".to_string()])
            .expect_err("origin must not carry a path");
        assert!(matches!(err, OriginPolicyError::Malformed(_)));
    }
}
