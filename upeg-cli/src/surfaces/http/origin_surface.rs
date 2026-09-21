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

    fn state(token: &str) -> HttpState {
        state_with_agents(token, Vec::new())
    }

    fn state_with_agents(token: &str, agents: Vec<String>) -> HttpState {
        HttpState {
            tokens: Arc::new(crate::infrastructure::auth::HostTokens::with_agents(
                token, agents,
            )),
            skip_origin_guard: true,
            notifications_enabled: false,
            origin_policy: Arc::new(super::super::cors::OriginPolicy::default()),
        }
    }

    fn state_with_agent() -> HttpState {
        state_with_agents(TOKEN, vec![AGENT_TOKEN.to_string()])
    }

    fn agent_headers(surface: &str) -> HeaderMap {
        headers(&[
            ("authorization", &format!("Bearer {AGENT_TOKEN}")),
            (ORIGIN_SURFACE_HEADER, surface),
        ])
    }

    fn headers(entries: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in entries {
            headers.insert(
                axum::http::HeaderName::from_bytes(name.as_bytes()).expect("valid header name"),
                value.parse().expect("valid header value"),
            );
        }
        headers
    }

    fn operator_headers(surface: &str) -> HeaderMap {
        headers(&[
            ("authorization", &format!("Bearer {TOKEN}")),
            (ORIGIN_SURFACE_HEADER, surface),
        ])
    }

    #[test]
    fn attach_surface_of_authenticated_request_is_honored() {
        for surface in ATTACH_ORIGIN_SURFACES {
            assert_eq!(
                origin_surface_from_headers(&state(TOKEN), &operator_headers(surface.label())),
                *surface,
                "{}",
                surface.label()
            );
        }
    }

    #[test]
    fn missing_header_falls_back_to_transport_surface_http() {
        let headers = headers(&[("authorization", &format!("Bearer {TOKEN}"))]);
        assert_eq!(
            origin_surface_from_headers(&state(TOKEN), &headers),
            Surface::Http
        );
    }

    #[test]
    fn headers_on_unauthenticated_requests_are_noise_not_declaration() {
        // A request with a wrong token, and a router that requires no
        // token at all.
        let wrong_token = headers(&[
            ("authorization", "Bearer wrong-token"),
            (ORIGIN_SURFACE_HEADER, "cli"),
        ]);
        assert_eq!(
            origin_surface_from_headers(&state(TOKEN), &wrong_token),
            Surface::Http
        );

        assert_eq!(
            origin_surface_from_headers(&state(""), &operator_headers("cli")),
            Surface::Http,
            "a host that does not authenticate can trust nobody"
        );
    }

    #[test]
    fn surfaces_outside_the_allow_list_fall_back_to_http() {
        for surface in [Surface::Desktop, Surface::Pwa, Surface::Ext, Surface::Mcp] {
            assert_eq!(
                origin_surface_from_headers(&state(TOKEN), &operator_headers(surface.label())),
                Surface::Http,
                "{}",
                surface.label()
            );
        }
    }

    #[test]
    fn non_surface_values_are_http_not_an_error() {
        assert_eq!(
            origin_surface_from_headers(&state(TOKEN), &operator_headers("carrier-pigeon")),
            Surface::Http
        );
    }

    #[test]
    fn operator_token_proves_an_operator_principal() {
        let principal = principal_from_headers(&state_with_agent(), &operator_headers("cli"));

        assert_eq!(principal.role, PrincipalRole::Operator);
        assert_eq!(
            principal.surface,
            Surface::Cli,
            "the attach header is honored along the way"
        );
    }

    #[test]
    fn agent_token_is_an_agent_on_http_surface_despite_the_header() {
        // That is all an agent token is: it reaches the data plane but
        // cannot claim the surface a person sits at.
        let principal = principal_from_headers(&state_with_agent(), &agent_headers("cli"));

        assert_eq!(principal.role, PrincipalRole::Agent);
        assert_eq!(principal.surface, Surface::Http);
        assert!(!principal.may_approve());
    }

    #[test]
    fn unauthenticated_requests_get_the_lowest_authority() {
        let wrong_token = headers(&[
            ("authorization", "Bearer wrong-token"),
            (ORIGIN_SURFACE_HEADER, "cli"),
        ]);

        let principal = principal_from_headers(&state_with_agent(), &wrong_token);

        assert_eq!(principal.role, PrincipalRole::Agent);
        assert_eq!(principal.surface, Surface::Http);
    }

    #[test]
    fn a_host_requiring_no_token_promotes_nobody_to_operator() {
        let principal = principal_from_headers(&state(""), &operator_headers("cli"));

        assert_eq!(principal.role, PrincipalRole::Agent);
        assert_eq!(principal.surface, Surface::Http);
    }
}
