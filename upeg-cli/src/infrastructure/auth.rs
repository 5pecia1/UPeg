//! Bearer-token generation and resolution. PRD §5.4.
//!
//! 32 random bytes encoded as URL-safe base64 without padding (43 chars).
//! `getrandom` is the only crypto-grade entropy source already in the
//! workspace; no new dependency.
//!
//! Two kinds of token reach the HTTP surface, and they differ in
//! *authority*, not in shape:
//!
//! * the **operator** token ([`resolve_token`]) — the one published to
//!   `~/.upeg/server.json`. Presenting it means "I am the person who
//!   started this host": the origin-surface header is believed, and a
//!   gated Chain step can be approved.
//! * **agent** tokens ([`resolve_agent_tokens`]) — optional, configured
//!   by the operator for programs it wants to let in. They authenticate
//!   and nothing more ([`upeg_core::PrincipalRole::Agent`]).
//!
//! Agent tokens are configured through the environment
//! ([`crate::infrastructure::paths::env::HTTP_AGENT_TOKENS`]) rather than
//! a repeatable `--agent-token` flag, for two reasons that a flag cannot
//! answer: a command line is world-readable in `ps` output on every
//! platform upeg targets, and the desktop-embedded host has no argv of
//! its own to put flags on — it inherits an environment. The operator
//! token already has the same escape hatch (`UPEG_HTTP_TOKEN` /
//! `--token-file`) for the same reason.

use std::path::Path;

use upeg_core::PrincipalRole;

/// Token resolution outcome — informs startup logging and whether the
/// caller should advertise the token to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    /// Provided via `--token-file` or `UPEG_HTTP_TOKEN`.
    Provided,
    /// Newly generated for this run (the default).
    Generated,
}

#[derive(Debug, Clone)]
pub struct ResolvedToken {
    pub token: String,
    pub source: TokenSource,
}

/// Pick the bearer token for the about-to-start HTTP server:
///
///   1. `explicit` argument (CLI flag value) wins,
///   2. then a non-empty `UPEG_HTTP_TOKEN`,
///   3. then `--token-file <path>` content (trimmed),
///   4. otherwise a freshly generated 32-byte token.
pub fn resolve_token(
    explicit: Option<&str>,
    token_file: Option<&Path>,
) -> std::io::Result<ResolvedToken> {
    if let Some(value) = explicit
        && !value.trim().is_empty()
    {
        return Ok(ResolvedToken {
            token: value.trim().to_string(),
            source: TokenSource::Provided,
        });
    }
    if let Ok(value) = std::env::var(super::paths::env::HTTP_TOKEN)
        && !value.trim().is_empty()
    {
        return Ok(ResolvedToken {
            token: value.trim().to_string(),
            source: TokenSource::Provided,
        });
    }
    if let Some(path) = token_file {
        let raw = std::fs::read_to_string(path)?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("token file `{}` is empty", path.display()),
            ));
        }
        return Ok(ResolvedToken {
            token: trimmed.to_string(),
            source: TokenSource::Provided,
        });
    }
    Ok(ResolvedToken {
        token: generate_token().map_err(|e| std::io::Error::other(format!("getrandom: {e}")))?,
        source: TokenSource::Generated,
    })
}

/// 32 random bytes → URL-safe base64 (no padding). Centralised here
/// so every HTTP surface produces identically-shaped tokens. Propagates
/// `getrandom` failure so callers can decide whether to log or fail.
pub fn generate_token() -> Result<String, getrandom::Error> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)?;
    Ok(url_safe_base64_no_pad(&bytes))
}

fn url_safe_base64_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let b = &bytes[i..i + 3];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        out.push(ALPHABET[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let n = u32::from(bytes[i]) << 16;
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
    } else if rem == 2 {
        let n = (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8);
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 생성된_토큰은_43자의_url_안전_문자열이다() {
        let token = generate_token().unwrap();
        assert_eq!(token.len(), 43, "32-byte input → 43-char base64-url no-pad");
        assert!(
            token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "token must be URL-safe base64 alphabet only; got `{token}`"
        );
    }

    #[test]
    fn 서로_다른_호출은_서로_다른_토큰을_생성한다() {
        // Birthday-bound non-collision check; 32 bytes of entropy makes
        // a same-process repeat unimaginably rare.
        let mut seen = std::collections::HashSet::new();
        for _ in 0..16 {
            assert!(seen.insert(generate_token().unwrap()));
        }
    }

    #[test]
    fn 명시적_인자는_env보다_우선한다() {
        let resolved = resolve_token(Some("supplied-token"), None).expect("resolve");
        assert_eq!(resolved.token, "supplied-token");
        assert_eq!(resolved.source, TokenSource::Provided);
    }

    #[test]
    fn agent_토큰_목록은_공백과_중복을_정리한다() {
        assert_eq!(
            parse_agent_tokens(" alpha , beta ,, alpha , "),
            vec!["alpha".to_string(), "beta".to_string()]
        );
        assert!(parse_agent_tokens("   ").is_empty());
        assert!(parse_agent_tokens("").is_empty());
    }

    #[test]
    fn operator_토큰과_agent_토큰은_서로_다른_역할을_증명한다() {
        let tokens = HostTokens::with_agents("op-token", vec!["agent-token".to_string()]);

        assert_eq!(tokens.role_for("op-token"), Some(PrincipalRole::Operator));
        assert_eq!(tokens.role_for("agent-token"), Some(PrincipalRole::Agent));
        assert_eq!(tokens.role_for("nonsense"), None);
        assert_eq!(
            tokens.role_for(""),
            None,
            "빈 bearer는 아무것도 증명하지 않는다"
        );
    }

    #[test]
    fn 토큰이_하나도_없는_호스트만_bearer를_요구하지_않는다() {
        assert!(!HostTokens::with_agents("", Vec::new()).requires_bearer());
        assert!(HostTokens::with_agents("op", Vec::new()).requires_bearer());
        assert!(HostTokens::with_agents("", vec!["agent".to_string()]).requires_bearer());
    }

    #[test]
    fn 토큰을_요구하지_않는_호스트는_아무_역할도_증명하지_못한다() {
        // 브링업/테스트용 tokenless 라우터: 인증하지 않으므로 누구도
        // operator로 승격시키지 않는다.
        let tokens = HostTokens::with_agents("", vec!["agent-token".to_string()]);

        assert_eq!(tokens.role_for(""), None);
        assert_eq!(tokens.role_for("anything"), None);
        assert_eq!(
            tokens.role_for("agent-token"),
            Some(PrincipalRole::Agent),
            "operator 토큰이 없어도 명시적으로 설정된 agent 토큰은 여전히 agent다"
        );
    }

    #[test]
    fn 빈_토큰_파일은_거부된이다() {
        let path = std::env::temp_dir().join("upeg-auth-empty-token");
        std::fs::write(&path, "   \n").unwrap();
        let err = resolve_token(None, Some(&path)).expect_err("must reject empty token file");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        let _ = std::fs::remove_file(&path);
    }
}

/// Separator between entries in
/// [`crate::infrastructure::paths::env::HTTP_AGENT_TOKENS`].
const AGENT_TOKEN_SEPARATOR: char = ',';

/// The operator token this host answers to, plus any agent tokens it
/// accepts. The one place that maps a presented bearer to the authority
/// it proves.
#[derive(Debug, Clone, Default)]
pub struct HostTokens {
    operator: String,
    agents: Vec<String>,
}

impl HostTokens {
    /// Pair an operator token with the agent tokens the environment
    /// declares.
    pub fn new(operator: impl Into<String>) -> Self {
        Self {
            operator: operator.into(),
            agents: resolve_agent_tokens(),
        }
    }

    /// Explicit both-halves constructor, for tests and for callers that
    /// already hold a token list.
    pub fn with_agents(operator: impl Into<String>, agents: Vec<String>) -> Self {
        Self {
            operator: operator.into(),
            agents,
        }
    }

    /// What authority does `presented` prove?
    ///
    /// `None` means the token matched nothing — an unauthenticated
    /// request, which the bearer middleware rejects outright.
    ///
    /// An empty operator token is the tokenless bring-up router: it
    /// authenticates nobody, so it can prove nobody. Callers there get
    /// `None` and the surface decides what an anonymous local request
    /// means; an empty `presented` never matches anything either.
    pub fn role_for(&self, presented: &str) -> Option<PrincipalRole> {
        if presented.is_empty() {
            return None;
        }
        if !self.operator.is_empty() && constant_time_eq(presented, &self.operator) {
            return Some(PrincipalRole::Operator);
        }
        self.agents
            .iter()
            .any(|agent| constant_time_eq(presented, agent))
            .then_some(PrincipalRole::Agent)
    }

    /// Does `presented` authenticate at all? Convenience over
    /// [`Self::role_for`] for the middleware, which only needs yes/no.
    pub fn accepts(&self, presented: &str) -> bool {
        self.role_for(presented).is_some()
    }

    /// Does this host check bearer tokens at all?
    ///
    /// False only for the tokenless bring-up router used by tests and
    /// early startup — it holds no token of either kind, so there is
    /// nothing for a request to match and every route is open.
    pub fn requires_bearer(&self) -> bool {
        !self.operator.is_empty() || !self.agents.is_empty()
    }
}

/// Agent bearer tokens declared in the environment, in declaration order
/// with blanks dropped and duplicates collapsed. Empty when the variable
/// is unset — agent access is opt-in.
pub fn resolve_agent_tokens() -> Vec<String> {
    std::env::var(super::paths::env::HTTP_AGENT_TOKENS)
        .map(|raw| parse_agent_tokens(&raw))
        .unwrap_or_default()
}

/// Pure parser behind [`resolve_agent_tokens`], so the split/trim/dedupe
/// rules are testable without touching the process environment.
fn parse_agent_tokens(raw: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    for candidate in raw
        .split(AGENT_TOKEN_SEPARATOR)
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        if !tokens.iter().any(|existing| existing == candidate) {
            tokens.push(candidate.to_string());
        }
    }
    tokens
}

/// Length-independent-prefix comparison for secrets. Mirrors the
/// HTTP surface's own guard so an agent token gets the same treatment
/// the operator token has always had.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
