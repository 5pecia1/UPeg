use std::io;

use crate::infrastructure::auth::TokenSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BindScope {
    Loopback,
    NonLoopback,
}

impl BindScope {
    pub(super) fn classify(addr: &str) -> Self {
        if is_loopback_bind(addr) {
            Self::Loopback
        } else {
            Self::NonLoopback
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BindRefusal {
    ConsentMissing,
    TokenNotInjected,
}

impl BindRefusal {
    pub(super) fn into_error(self, addr: &str) -> io::Error {
        let message = match self {
            Self::ConsentMissing => format!(
                "refusing non-loopback HTTP bind `{addr}` without explicit consent; \
                 use 127.0.0.1/::1 or set UPEG_HTTP_ALLOW_NON_LOOPBACK=1"
            ),
            Self::TokenNotInjected => format!(
                "refusing non-loopback HTTP bind `{addr}` with an auto-generated bearer token; \
                 inject one via UPEG_HTTP_TOKEN or --token-file"
            ),
        };
        io::Error::new(io::ErrorKind::PermissionDenied, message)
    }
}

pub(super) fn evaluate(
    addr: &str,
    consent: bool,
    token: TokenSource,
) -> Result<BindScope, BindRefusal> {
    match (BindScope::classify(addr), consent, token) {
        (BindScope::Loopback, _, _) => Ok(BindScope::Loopback),
        (BindScope::NonLoopback, false, _) => Err(BindRefusal::ConsentMissing),
        (BindScope::NonLoopback, true, TokenSource::Generated) => {
            Err(BindRefusal::TokenNotInjected)
        }
        (BindScope::NonLoopback, true, TokenSource::Provided) => Ok(BindScope::NonLoopback),
    }
}

pub(super) fn is_loopback_bind(addr: &str) -> bool {
    if addr.trim_start().starts_with("localhost:") {
        return true;
    }
    addr.parse::<std::net::SocketAddr>()
        .map(|socket| socket.ip().is_loopback())
        .unwrap_or(false)
}

pub(super) fn consent_from_env() -> bool {
    std::env::var(crate::infrastructure::paths::env::HTTP_ALLOW_NON_LOOPBACK)
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WILDCARD: &str = "0.0.0.0:7173";

    #[test]
    fn loopback_bind_is_permitted_without_consent_even_with_a_generated_token() {
        for addr in ["127.0.0.1:7173", "[::1]:7173", "localhost:7173"] {
            assert!(is_loopback_bind(addr), "{addr} should be loopback");
            assert_eq!(
                evaluate(addr, false, TokenSource::Generated),
                Ok(BindScope::Loopback),
                "{addr} needs neither consent nor an injected token"
            );
        }
    }

    #[test]
    fn non_loopback_bind_without_consent_is_refused_whatever_the_token() {
        for token in [TokenSource::Generated, TokenSource::Provided] {
            assert_eq!(
                evaluate(WILDCARD, false, token),
                Err(BindRefusal::ConsentMissing),
                "{token:?} token cannot substitute for consent"
            );
        }
        let err = BindRefusal::ConsentMissing.into_error(WILDCARD);
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            err.to_string().contains("UPEG_HTTP_ALLOW_NON_LOOPBACK"),
            "error should name the consent switch; got {err}"
        );
    }

    #[test]
    fn non_loopback_bind_with_consent_but_generated_token_is_refused() {
        assert_eq!(
            evaluate(WILDCARD, true, TokenSource::Generated),
            Err(BindRefusal::TokenNotInjected)
        );
        let err = BindRefusal::TokenNotInjected.into_error(WILDCARD);
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        let text = err.to_string();
        assert!(
            text.contains("UPEG_HTTP_TOKEN") && text.contains("--token-file"),
            "error should name both injection paths; got {text}"
        );
    }

    #[test]
    fn non_loopback_bind_with_consent_and_injected_token_is_permitted() {
        assert_eq!(
            evaluate(WILDCARD, true, TokenSource::Provided),
            Ok(BindScope::NonLoopback)
        );
    }

    #[test]
    fn bind_scope_classifies_public_and_unparseable_addresses_as_non_loopback() {
        assert_eq!(BindScope::classify(WILDCARD), BindScope::NonLoopback);
        assert_eq!(
            BindScope::classify("192.168.0.10:7173"),
            BindScope::NonLoopback
        );
        assert_eq!(BindScope::classify("not-an-addr"), BindScope::NonLoopback);
    }
}
