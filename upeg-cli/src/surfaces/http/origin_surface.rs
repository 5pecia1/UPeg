//! Who a `/v1/…` request is: the surface it is dispatched as, and the
//! [`Principal`] it proves.
//!
//! Split out of `mod.rs` because it answers a question of its own —
//! *who is asking?* — that the route handlers only consume. The two
//! halves are one question asked at two granularities, and they share a
//! single rule: **only the operator token is believed.** An agent token
//! authenticates a program; a program does not get to say which human
//! surface it is sitting at. The header contract is documented for
//! clients in `docs/architecture/http-api.md`.

use axum::http::HeaderMap;
use upeg_core::{Principal, PrincipalRole, Surface};

use super::{HttpState, bearer_role};

/// Request header naming the local surface an attaching client is
/// calling *from*.
///
/// Attach mode used to stamp every forwarded call `http`, which made a
/// person's own terminal a stranger the moment a host came up: `upeg call
/// <chain> -a approve=true` was refused because `http` is not an approval
/// surface, and the TUI had no `--local` escape at all
/// (`docs/architecture/chain.md`). The surface a call *arrives on* is not
/// the same question as the surface a person is *sitting at*, and this
/// header is how a local client answers the second one.
///
/// It is honored only under both conditions
/// ([`origin_surface_from_headers`]): the request is authenticated with
/// this host's **operator** token — the token lives in
/// `~/.upeg/server.json`, readable by the same OS user, which is exactly
/// the trust boundary "local client" means — and the value names one of
/// [`ATTACH_ORIGIN_SURFACES`]. Anything else falls back to `http`, so a
/// forged header on an unauthenticated request, or on one that
/// authenticated as an agent, buys nothing.
pub(crate) const ORIGIN_SURFACE_HEADER: &str = "x-upeg-origin-surface";

/// The surfaces an attaching client may declare in
/// [`ORIGIN_SURFACE_HEADER`].
///
/// These are the two surfaces in this binary that attach to a host
/// instead of dispatching in-process (`upeg call` / `upeg board … call`,
/// and the TUI). Desktop dispatches through FRB in-process and never
/// attaches, and no browser-delivered surface is in the list: a `pwa` or
/// `ext` client reaching the REST plane over a pairing token is exactly
/// the caller `http` describes.
///
/// The MCP proxy lane (`upeg mcp` forwarding to `/mcp`) deliberately does
/// not send this header — an MCP client is a program, and `mcp` stays
/// `mcp` no matter which process relays it.
pub(crate) const ATTACH_ORIGIN_SURFACES: &[Surface] = &[Surface::Cli, Surface::Tui];

/// Which surface this request is dispatched as: the one its
/// [`ORIGIN_SURFACE_HEADER`] declares, or `http`.
///
/// Two things must hold for the header to be believed, and each is a
/// separate way of being wrong:
///
/// 1. this request proved [`PrincipalRole::Operator`] — a tokenless
///    router (`router()`, used for bring-up and tests) authenticates
///    nobody and an agent token authenticates a program, so neither can
///    claim to be a person's terminal;
/// 2. the value names a surface in [`ATTACH_ORIGIN_SURFACES`].
///
/// Failing either is not an error, it is simply `http`: the caller gets
/// the surface it would have had before the header existed.
pub(super) fn origin_surface_from_headers(state: &HttpState, headers: &HeaderMap) -> Surface {
    if bearer_role(state, headers) != Some(PrincipalRole::Operator) {
        return Surface::Http;
    }
    headers
        .get(ORIGIN_SURFACE_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .and_then(Surface::parse)
        .filter(|surface| ATTACH_ORIGIN_SURFACES.contains(surface))
        .unwrap_or(Surface::Http)
}

/// The [`Principal`] this request proves: what its bearer authenticated
/// as, paired with the surface it is dispatched on.
///
/// The role comes from the token and nothing else — a caller cannot
/// promote itself with a header, and `_upeg.principal` is wiped out of
/// whatever it sent (`upeg-runtime/src/execution.rs`). A request that
/// authenticates as nobody (the tokenless bring-up router) is an agent:
/// the lowest authority is the only honest floor for a caller this host
/// cannot identify.
///
/// The surface half is [`origin_surface_from_headers`], so the two
/// always agree — an operator attaching from a terminal is
/// `{ operator, cli }`, and the same request without the header is
/// `{ operator, http }`.
pub(super) fn principal_from_headers(state: &HttpState, headers: &HeaderMap) -> Principal {
    principal_on_surface(state, headers, origin_surface_from_headers(state, headers))
}

/// The [`Principal`] this request proves on a surface the *route* fixes
/// rather than the request.
///
/// The `/mcp` lane is the case that needs it: its surface is
/// [`Surface::Mcp`] by construction — an MCP client is a program, and no
/// header moves it (`docs/architecture/http-api.md`) — but the role is
/// still a question only the bearer can answer. Without this the lane
/// fell back to [`Principal::for_surface`], which reads `mcp` as the
/// in-process stdio lane and hands every caller
/// [`PrincipalRole::Local`] — an approving role — including one holding
/// an agent token.
///
/// The role rule is [`principal_from_headers`]'s, unchanged: the token
/// and nothing else, with the lowest authority as the floor for a caller
/// this host cannot identify.
pub(super) fn principal_on_surface(
    state: &HttpState,
    headers: &HeaderMap,
    surface: Surface,
) -> Principal {
    Principal::new(
        bearer_role(state, headers).unwrap_or(PrincipalRole::Agent),
        surface,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const TOKEN: &str = "origin-surface-test-token";

    const AGENT_TOKEN: &str = "origin-surface-test-agent-token";

    fn 상태(token: &str) -> HttpState {
        에이전트_있는_상태(token, Vec::new())
    }

    fn 에이전트_있는_상태(token: &str, agents: Vec<String>) -> HttpState {
        HttpState {
            tokens: Arc::new(crate::infrastructure::auth::HostTokens::with_agents(
                token, agents,
            )),
            skip_origin_guard: true,
            notifications_enabled: false,
            origin_policy: Arc::new(super::super::cors::OriginPolicy::default()),
        }
    }

    fn 에이전트_상태() -> HttpState {
        에이전트_있는_상태(TOKEN, vec![AGENT_TOKEN.to_string()])
    }

    fn 에이전트_헤더(surface: &str) -> HeaderMap {
        헤더(&[
            ("authorization", &format!("Bearer {AGENT_TOKEN}")),
            (ORIGIN_SURFACE_HEADER, surface),
        ])
    }

    fn 헤더(entries: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in entries {
            headers.insert(
                axum::http::HeaderName::from_bytes(name.as_bytes()).expect("유효한 헤더 이름"),
                value.parse().expect("유효한 헤더 값"),
            );
        }
        headers
    }

    fn 인증된_헤더(surface: &str) -> HeaderMap {
        헤더(&[
            ("authorization", &format!("Bearer {TOKEN}")),
            (ORIGIN_SURFACE_HEADER, surface),
        ])
    }

    #[test]
    fn 인증된_요청의_attach_surface는_그대로_인정된다() {
        for surface in ATTACH_ORIGIN_SURFACES {
            assert_eq!(
                origin_surface_from_headers(&상태(TOKEN), &인증된_헤더(surface.label())),
                *surface,
                "{}",
                surface.label()
            );
        }
    }

    #[test]
    fn 헤더가_없으면_전송_surface인_http다() {
        let headers = 헤더(&[("authorization", &format!("Bearer {TOKEN}"))]);
        assert_eq!(
            origin_surface_from_headers(&상태(TOKEN), &headers),
            Surface::Http
        );
    }

    #[test]
    fn 인증되지_않은_요청의_헤더는_선언이_아니라_소음이다() {
        // 토큰이 틀린 요청, 그리고 애초에 토큰을 요구하지 않는 라우터.
        let 틀린_토큰 = 헤더(&[
            ("authorization", "Bearer wrong-token"),
            (ORIGIN_SURFACE_HEADER, "cli"),
        ]);
        assert_eq!(
            origin_surface_from_headers(&상태(TOKEN), &틀린_토큰),
            Surface::Http
        );

        assert_eq!(
            origin_surface_from_headers(&상태(""), &인증된_헤더("cli")),
            Surface::Http,
            "인증하지 않는 호스트는 아무도 믿을 수 없다"
        );
    }

    #[test]
    fn 허용_목록_밖의_surface는_http로_떨어진다() {
        for surface in [Surface::Desktop, Surface::Pwa, Surface::Ext, Surface::Mcp] {
            assert_eq!(
                origin_surface_from_headers(&상태(TOKEN), &인증된_헤더(surface.label())),
                Surface::Http,
                "{}",
                surface.label()
            );
        }
    }

    #[test]
    fn surface가_아닌_값은_오류가_아니라_http다() {
        assert_eq!(
            origin_surface_from_headers(&상태(TOKEN), &인증된_헤더("carrier-pigeon")),
            Surface::Http
        );
    }

    #[test]
    fn operator_토큰은_operator_주체를_증명한다() {
        let principal = principal_from_headers(&에이전트_상태(), &인증된_헤더("cli"));

        assert_eq!(principal.role, PrincipalRole::Operator);
        assert_eq!(
            principal.surface,
            Surface::Cli,
            "attach 헤더가 함께 인정된다"
        );
    }

    #[test]
    fn agent_토큰은_헤더가_있어도_http_surface의_agent다() {
        // 이것이 agent 토큰의 전부다: 데이터 평면에 들어오지만
        // 사람이 앉아있는 표면을 자칭할 수는 없다.
        let principal = principal_from_headers(&에이전트_상태(), &에이전트_헤더("cli"));

        assert_eq!(principal.role, PrincipalRole::Agent);
        assert_eq!(principal.surface, Surface::Http);
        assert!(!principal.may_approve());
    }

    #[test]
    fn 인증되지_않은_요청은_가장_낮은_권한을_받는다() {
        let 틀린_토큰 = 헤더(&[
            ("authorization", "Bearer wrong-token"),
            (ORIGIN_SURFACE_HEADER, "cli"),
        ]);

        let principal = principal_from_headers(&에이전트_상태(), &틀린_토큰);

        assert_eq!(principal.role, PrincipalRole::Agent);
        assert_eq!(principal.surface, Surface::Http);
    }

    #[test]
    fn 토큰을_요구하지_않는_호스트도_아무도_operator로_승격시키지_않는다() {
        let principal = principal_from_headers(&상태(""), &인증된_헤더("cli"));

        assert_eq!(principal.role, PrincipalRole::Agent);
        assert_eq!(principal.surface, Surface::Http);
    }
}
