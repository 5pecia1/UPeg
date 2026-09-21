//! Caller identity: *who* is asking, not only *from where*.
//!
//! [`Surface`] answers "which door did this call come through". It is
//! stamped by the surface and unspoofable, which is why Chain-step
//! approval has been keyed on it (`docs/architecture/chain.md`). But a
//! surface is a door, not a person: two callers arriving through the same
//! HTTP listener with two different bearer tokens are indistinguishable
//! to it, and one of them may be a person while the other is an agent
//! acting on its own.
//!
//! [`Principal`] is the next boundary down. It pairs the surface with a
//! [`PrincipalRole`] — how much authority the caller *proved* — and it is
//! stamped by the runtime the same way the surface is
//! (`upeg-runtime/src/execution.rs`), so a caller cannot write its own
//! into the call envelope.
//!
//! The vocabulary is deliberately three-valued rather than a bool,
//! because "not an operator" has two honestly different reasons:
//! a program that authenticated as a program ([`PrincipalRole::Agent`]),
//! and a program the OS user launched in-process with no human gesture
//! behind this particular call ([`PrincipalRole::Local`]).

use crate::types::Surface;

/// How much authority a caller proved.
///
/// Ordering of authority is *not* a lattice worth encoding: the only
/// question anyone asks is [`PrincipalRole::may_approve`], and it draws
/// exactly one line — across the process boundary this host
/// authenticates at.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum PrincipalRole {
    /// A person with operator authority over this host: an in-process
    /// human surface (`cli`/`tui`/`desktop` — the OS user account *is*
    /// the proof), or an HTTP request carrying the host's operator
    /// bearer token from `~/.upeg/server.json`.
    Operator,
    /// A program authenticated as a program: an HTTP request carrying one
    /// of the configured agent tokens, or any caller this host could not
    /// identify at all. It may run tools; it may never lift a
    /// human-approval barrier — the operator handed out a token that says
    /// "you are not me", and this is that configuration being enforced.
    Agent,
    /// In-process on the OS user's own machine, but driven by a program
    /// rather than a person sitting at a surface: the MCP stdio lane.
    ///
    /// The OS user vouched for it by launching it, which is the same
    /// boundary `cli`/`tui`/`desktop` rest on — so this role is not
    /// *barred* from approving. What governs it is the surface gate: a
    /// chain must name `mcp` in `approval_surfaces` before an MCP client
    /// can approve anything, and no default does
    /// (`docs/architecture/chain.md`).
    Local,
}

/// Every role, in declaration order. Used by exhaustive tests and by
/// surfaces that render the vocabulary.
pub const ALL_PRINCIPAL_ROLES: &[PrincipalRole] = &[
    PrincipalRole::Operator,
    PrincipalRole::Agent,
    PrincipalRole::Local,
];

impl PrincipalRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Agent => "agent",
            Self::Local => "local",
        }
    }

    /// Parse a role label. Case-insensitive; padded values are rejected,
    /// exactly like [`Surface::parse`].
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "operator" => Self::Operator,
            "agent" => Self::Agent,
            "local" => Self::Local,
            _ => return None,
        })
    }

    /// May a caller with this role lift a Chain step's
    /// `requires_approval` barrier — *if* the chain's
    /// `approval_surfaces` also allows the surface it came from?
    ///
    /// Two independent gates, and this is the one about identity. It says
    /// no to exactly one role: [`Self::Agent`], the caller this host
    /// authenticated as "not the operator". Saying no to [`Self::Local`]
    /// as well would look stricter and be wrong — it rests on the same
    /// OS-user boundary the in-process human surfaces do, and barring it
    /// here would silently void an explicit `approval_surfaces = ["mcp"]`
    /// that a chain author deliberately wrote.
    pub const fn may_approve(self) -> bool {
        !matches!(self, Self::Agent)
    }
}

/// Key inside a [`Principal`] JSON object carrying the role label.
pub const PRINCIPAL_ROLE_KEY: &str = "role";

/// Key inside a [`Principal`] JSON object carrying the surface label.
/// Mirrors [`crate::EXECUTION_CONTEXT_SURFACE`] so the principal block
/// reads standalone in a log line or an error message.
pub const PRINCIPAL_SURFACE_KEY: &str = "surface";

/// Stamped caller identity: the role a caller proved, and the surface it
/// proved it on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Principal {
    pub role: PrincipalRole,
    pub surface: Surface,
}

impl Principal {
    pub const fn new(role: PrincipalRole, surface: Surface) -> Self {
        Self { role, surface }
    }

    /// The role a surface carries when nothing narrower is known.
    ///
    /// Total over [`Surface`] on purpose: every call gets a principal, so
    /// no dispatch path can forget to stamp one and leave a chain
    /// deciding against an absent identity.
    ///
    /// * `cli` / `tui` / `desktop` — in-process human surfaces. The
    ///   person at the OS user account is the caller; there is no token
    ///   to check because there is no process boundary to check it at.
    /// * `mcp` — the stdio lane: in-process, but driven by a program.
    /// * `http` / `pwa` / `ext` — the call crossed a listener. The
    ///   listener is what knows better (a bearer token distinguishes an
    ///   operator from an agent), so it overrides this default with
    ///   [`crate::Principal::new`]; until it does, the honest floor is
    ///   the lowest authority.
    pub const fn for_surface(surface: Surface) -> Self {
        let role = match surface {
            Surface::Cli | Surface::Tui | Surface::Desktop => PrincipalRole::Operator,
            Surface::Mcp => PrincipalRole::Local,
            Surface::Http | Surface::Pwa | Surface::Ext => PrincipalRole::Agent,
        };
        Self::new(role, surface)
    }

    pub const fn may_approve(self) -> bool {
        self.role.may_approve()
    }

    pub fn to_json(self) -> serde_json::Value {
        serde_json::json!({
            PRINCIPAL_ROLE_KEY: self.role.label(),
            PRINCIPAL_SURFACE_KEY: self.surface.label(),
        })
    }

    /// Read a stamped principal back out of a call envelope block.
    ///
    /// `None` for anything that is not a complete, recognized principal —
    /// a half-written block proves nothing, so it is not repaired into a
    /// default that would grant authority nobody stamped.
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        let role = PrincipalRole::parse(value.get(PRINCIPAL_ROLE_KEY)?.as_str()?)?;
        let surface = Surface::parse(value.get(PRINCIPAL_SURFACE_KEY)?.as_str()?)?;
        Some(Self::new(role, surface))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ALL_SURFACES;

    #[test]
    fn role_labels_round_trip() {
        for role in ALL_PRINCIPAL_ROLES {
            assert_eq!(PrincipalRole::parse(role.label()), Some(*role));
        }
    }

    #[test]
    fn role_parse_ignores_case_but_rejects_whitespace() {
        assert_eq!(
            PrincipalRole::parse("OPERATOR"),
            Some(PrincipalRole::Operator)
        );
        assert_eq!(PrincipalRole::parse(" operator "), None);
        assert_eq!(PrincipalRole::parse("root"), None);
    }

    #[test]
    fn only_agent_is_excluded_from_approval() {
        // There is a single boundary: a caller this host has
        // authenticated as "not the operator". local stays inside the
        // OS user boundary, so it is not blocked here; the surface
        // gate decides where approval is allowed.
        for role in ALL_PRINCIPAL_ROLES {
            assert_eq!(
                role.may_approve(),
                *role != PrincipalRole::Agent,
                "{}",
                role.label()
            );
        }
    }

    #[test]
    fn per_surface_default_principal_is_global_for_every_surface() {
        for surface in ALL_SURFACES {
            let principal = Principal::for_surface(*surface);
            assert_eq!(principal.surface, *surface);
        }
    }

    #[test]
    fn only_surfaces_crossing_a_listener_are_excluded_from_approval_by_default() {
        for surface in ALL_SURFACES {
            // http/pwa/ext cross a process boundary — until a token
            // says otherwise this host cannot identify the caller, so
            // the floor is agent. The rest stay inside the OS user
            // boundary.
            let crosses_a_listener = matches!(surface, Surface::Http | Surface::Pwa | Surface::Ext);
            assert_eq!(
                Principal::for_surface(*surface).may_approve(),
                !crosses_a_listener,
                "{}",
                surface.label()
            );
        }
        assert_eq!(
            Principal::for_surface(Surface::Mcp).role,
            PrincipalRole::Local,
            "stdio MCP is a program spawned by the OS user, not a surface a person sits at"
        );
    }

    #[test]
    fn principal_round_trips_through_json() {
        for surface in ALL_SURFACES {
            for role in ALL_PRINCIPAL_ROLES {
                let principal = Principal::new(*role, *surface);
                assert_eq!(Principal::from_json(&principal.to_json()), Some(principal));
            }
        }
    }

    #[test]
    fn half_written_principal_blocks_are_not_principals() {
        for broken in [
            serde_json::json!({}),
            serde_json::json!({ PRINCIPAL_ROLE_KEY: "operator" }),
            serde_json::json!({ PRINCIPAL_SURFACE_KEY: "cli" }),
            serde_json::json!({ PRINCIPAL_ROLE_KEY: "root", PRINCIPAL_SURFACE_KEY: "cli" }),
            serde_json::json!({ PRINCIPAL_ROLE_KEY: "operator", PRINCIPAL_SURFACE_KEY: "pigeon" }),
            serde_json::json!("operator"),
        ] {
            assert_eq!(Principal::from_json(&broken), None, "{broken}");
        }
    }
}
