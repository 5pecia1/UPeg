//! Who may satisfy a Chain step's `requires_approval` barrier.
//!
//! `approve = true` and `_upeg.approvedSteps` are ordinary members of the
//! call envelope: any caller can write them. They state *intent*, so on
//! their own they are a UX gate — they stop a mis-click, not a program.
//!
//! `_upeg.surface` and `_upeg.principal`, by contrast, are stamped by the
//! runtime and a caller-supplied value under either name is wiped before
//! dispatch (`upeg-runtime/src/execution.rs`). They are the caller
//! identity a dispatcher may trust, and this module turns them into the
//! authorization decision — in that order, because they answer two
//! different questions:
//!
//! 1. **Who is asking** ([`upeg_core::Principal`]): this host either
//!    authenticated the caller as *not the operator*, or it did not. An
//!    [`upeg_core::PrincipalRole::Agent`] — a request bearing an agent
//!    token, or one this host could not identify at all — is refused with
//!    [`APPROVAL_DENIED_FOR_PRINCIPAL_ERROR_CODE`] no matter which surface
//!    it came through, because widening `approval_surfaces` cannot grant
//!    authority the operator's own token policy withheld.
//! 2. **Where they are asking from** ([`upeg_core::Surface`]): a chain
//!    declares the surfaces whose approvals it honors, and an approval
//!    arriving from any other surface is refused with
//!    [`APPROVAL_DENIED_FOR_SURFACE_ERROR_CODE`].
//!
//! Both are still coarser than "which human". Neither can tell two
//! operator sessions apart, and neither binds an approval to a named
//! person — that would need real user accounts, which upeg does not have
//! (docs/rules/decisions.md). This module states exactly what it can
//! prove and no more.
//!
//! Nested chains are judged by the *outer* caller: a `_upeg` block in
//! step args is caller-written data, so the engine discards it and hands
//! the call's own block down unchanged
//! (`upeg_runtime::inherit_call_context`) — no step can fabricate a
//! surface or principal to open a barrier. The per-Tool policy a UI
//! reads *before* dispatch (`requires_approval` + `approval_surfaces`)
//! is registered through [`upeg_runtime::ToolApprovalPolicy`].

use serde_json::Value;
use upeg_core::{
    EXECUTION_CONTEXT_APPROVED_STEPS, EXECUTION_CONTEXT_ARG, EXECUTION_CONTEXT_PRINCIPAL,
    EXECUTION_CONTEXT_SURFACE, Principal, Surface,
};
use upeg_runtime::ToolApprovalPolicy;

/// Surfaces whose approvals a chain honors when it declares no
/// `approval_surfaces` of its own.
///
/// The three surfaces a person is *sitting at*. They share one property
/// that no other surface has: the caller is the OS user account itself,
/// so the call carries an operator [`upeg_core::PrincipalRole`] without
/// any token having to prove it.
///
/// All three have shipped the gesture: `upeg call <chain> -a
/// approve=true` on the CLI, a confirm dialog in front of the run on the
/// TUI, and the same in the desktop's run flow. Each one asks a person
/// first and sends `approve` / `_upeg.approvedSteps` only afterwards —
/// no UI dispatches a gated chain without that answer, and nothing is
/// ever auto-approved on the person's behalf. [`ToolApprovalPolicy`] is
/// what lets those UIs know a confirmation is needed *before* they
/// dispatch (`upeg-runtime/src/approval.rs`); the per-surface gestures
/// are tabulated in that module's docs.
///
/// Everything else stays out: `mcp` and `http` are programmatic callers
/// that fill the envelope themselves, and `pwa` / `ext` reach the runtime
/// over the pairing-token HTTP surface rather than from the machine the
/// person is sitting at. A chain that wants MCP, HTTP, or PWA approval names
/// it in `approval_surfaces`. The browser extension is never an approver: it has
/// no approval gesture, so a declaration naming `ext` is rejected at load
/// time and the runtime guard below also denies hand-built policies.
///
/// A terminal attached to a running host still counts as `cli`: the host
/// stamps the surface the request declares in its origin-surface header
/// (`upeg_cli::surfaces::http` module docs), so `upeg call` behaves the same
/// whether or not a host is up.
pub(crate) const DEFAULT_APPROVAL_SURFACES: &[Surface] =
    &[Surface::Cli, Surface::Tui, Surface::Desktop];

/// Separator between surface labels when an error message spells out a
/// surface list.
const SURFACE_LABEL_SEPARATOR: &str = "/";

/// Render a surface list the way every approval-related message spells
/// it, so the manifest-time error and the runtime error read alike.
pub(crate) fn surface_label_list(surfaces: &[Surface]) -> String {
    surfaces
        .iter()
        .map(|surface| surface.label())
        .collect::<Vec<_>>()
        .join(SURFACE_LABEL_SEPARATOR)
}

/// A rejected `approval_surfaces` declaration, positioned so the loader
/// can name the offending entry. Kept free of `LoadError` so this module
/// stays a pure policy unit; `parse::chain` does the mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApprovalSurfacesError {
    /// The list is present but authorizes nobody.
    Empty,
    /// Entry at `position` is blank.
    EmptyEntry { position: usize },
    /// Entry at `position` is not a surface label.
    Unknown { position: usize, surface: String },
    /// The browser extension cannot provide an approval gesture.
    ExtensionUnsupported { position: usize },
}

impl ApprovalSurfacesError {
    /// Message for the one caller that has no `LoadError` to map onto:
    /// a dispatcher built from a `ToolToml` that never went through
    /// loader validation.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::Empty => format!(
                "`approval_surfaces` is empty — a chain that authorizes no surface can never \
                 satisfy a `requires_approval` step (default: {default})",
                default = surface_label_list(DEFAULT_APPROVAL_SURFACES)
            ),
            Self::EmptyEntry { position } => {
                format!("`approval_surfaces[{position}]` is empty")
            }
            Self::Unknown { position, surface } => format!(
                "`approval_surfaces[{position}] = \"{surface}\"` is not a surface ({valid})",
                valid = surface_label_list(upeg_core::ALL_SURFACES)
            ),
            Self::ExtensionUnsupported { position } => format!(
                "`approval_surfaces[{position}] = \"ext\"` is unsupported because the browser extension has no approval gesture"
            ),
        }
    }
}

/// The surfaces one chain honors approvals from, and the chain they
/// belong to.
///
/// The chain id rides along because the refusal is only useful if it
/// names the command that WOULD work: an agent told "ask a person" still
/// has to guess what to ask for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ApprovalSurfaces {
    chain: String,
    surfaces: Vec<Surface>,
}

impl ApprovalSurfaces {
    /// Validate a manifest's `approval_surfaces` list for the chain
    /// `chain` (its canonical Tool id). `None` yields
    /// [`DEFAULT_APPROVAL_SURFACES`].
    pub(crate) fn parse(
        chain: &str,
        declared: Option<&[String]>,
    ) -> Result<Self, ApprovalSurfacesError> {
        let Some(declared) = declared else {
            return Ok(Self {
                chain: chain.to_string(),
                surfaces: DEFAULT_APPROVAL_SURFACES.to_vec(),
            });
        };
        if declared.is_empty() {
            return Err(ApprovalSurfacesError::Empty);
        }
        declared
            .iter()
            .enumerate()
            .map(|(position, raw)| {
                if raw.trim().is_empty() {
                    return Err(ApprovalSurfacesError::EmptyEntry { position });
                }
                match Surface::parse(raw) {
                    Some(Surface::Ext) => {
                        Err(ApprovalSurfacesError::ExtensionUnsupported { position })
                    }
                    Some(surface) => Ok(surface),
                    None => Err(ApprovalSurfacesError::Unknown {
                        position,
                        surface: raw.clone(),
                    }),
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|surfaces| Self {
                chain: chain.to_string(),
                surfaces,
            })
    }

    pub(crate) fn labels(&self) -> String {
        surface_label_list(&self.surfaces)
    }

    /// The effective surface set, as the registry publishes it to UIs
    /// ([`upeg_runtime::tool_approval_policy`]).
    pub(crate) fn policy(&self) -> ToolApprovalPolicy {
        ToolApprovalPolicy::gated(self.surfaces.clone())
    }

    pub(crate) fn honors(&self, surface: Surface) -> bool {
        self.surfaces.contains(&surface)
    }

    /// What a refused caller should do instead.
    ///
    /// `cli` is the only honoring surface whose gesture is a single
    /// copy-pasteable command, so it is worth spelling out exactly when
    /// this chain honors `cli`. The TUI and desktop gestures are
    /// dialogs a person answers in a running UI; naming a command for
    /// them would be a lie, so the honest answer there is the surface
    /// list itself plus "ask a person to run it there".
    fn hint(&self) -> String {
        if self.honors(Surface::Cli) {
            format!(
                "run `upeg call {chain} -a approve=true` in a terminal",
                chain = self.chain
            )
        } else {
            format!("ask a person to run `{chain}` there", chain = self.chain)
        }
    }

    /// Decide whether the gated `step_key`/`tool` may run for this call.
    ///
    /// Principal before surface: an agent that reached an authorized
    /// surface still may not approve, and telling it "wrong surface"
    /// would send it looking for a door that would not have opened
    /// either. The refusal names the reason that actually applies.
    pub(crate) fn authorize(&self, args: &Value, step_key: &str, tool: &str) -> StepApproval {
        if let Some(principal) = calling_principal(args)
            && !principal.may_approve()
        {
            return StepApproval::DeniedForPrincipal {
                principal,
                hint: self.hint(),
            };
        }
        match calling_surface(args) {
            CallingSurface::Known(Surface::Ext) => StepApproval::DeniedForSurface {
                caller: CallingSurface::Known(Surface::Ext),
                allowed: self.labels(),
                hint: self.hint(),
            },
            CallingSurface::Known(surface) if self.honors(surface) => {
                if approval_requested(args, step_key, tool) {
                    StepApproval::Approved
                } else {
                    StepApproval::NotRequested
                }
            }
            other => StepApproval::DeniedForSurface {
                caller: other,
                allowed: self.labels(),
                hint: self.hint(),
            },
        }
    }
}

/// What the call says about the surface it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CallingSurface {
    /// `_upeg.surface` names a surface upeg knows.
    Known(Surface),
    /// `_upeg.surface` is present but is not a surface label. A surface
    /// stamps its own label, so this only happens on a hand-built
    /// envelope — it proves nothing and authorizes nothing.
    Unrecognized(String),
    /// No `_upeg.surface` at all: the call carries no caller identity.
    /// Only a hand-built envelope reaches this — a chain step inherits
    /// the call's `_upeg` block (`upeg_runtime::inherit_call_context`),
    /// so a nested chain sees the surface the outermost caller was
    /// stamped with rather than nothing at all.
    Absent,
}

impl CallingSurface {
    fn describe(&self) -> String {
        match self {
            Self::Known(surface) => format!("surface `{}`", surface.label()),
            Self::Unrecognized(raw) => format!("unknown surface `{raw}`"),
            Self::Absent => "a call with no `_upeg.surface` identity".to_string(),
        }
    }
}

/// Outcome of the approval gate for one step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StepApproval {
    /// The caller may approve here, and did.
    Approved,
    /// The caller may approve here but did not: the barrier stands and a
    /// person can lift it from where they already are.
    NotRequested,
    /// This caller has no authority to approve anything, anywhere: this
    /// host authenticated it as an agent, or could not identify it at
    /// all. The surface it arrived on is not the reason and widening
    /// `approval_surfaces` would not change it.
    DeniedForPrincipal {
        principal: Principal,
        /// What to do instead — see [`ApprovalSurfaces::hint`].
        hint: String,
    },
    /// No approval from this caller can be honored, whether or not one
    /// was sent.
    DeniedForSurface {
        caller: CallingSurface,
        allowed: String,
        /// What to do instead — see [`ApprovalSurfaces::hint`].
        hint: String,
    },
}

impl StepApproval {
    /// The failure text for a step that did not clear the barrier, or
    /// `None` when it did.
    pub(crate) fn rejection(&self, step_key: &str) -> Option<ApprovalRejection> {
        match self {
            Self::Approved => None,
            Self::NotRequested => Some(ApprovalRejection {
                code: APPROVAL_REQUIRED_ERROR_CODE,
                message: format!(
                    "step `{step_key}` requires approval — re-send the call with \
                     `approve = true` or `_upeg.approvedSteps = [\"{step_key}\"]`"
                ),
            }),
            Self::DeniedForPrincipal { principal, hint } => Some(ApprovalRejection {
                code: APPROVAL_DENIED_FOR_PRINCIPAL_ERROR_CODE,
                message: format!(
                    "step `{step_key}` requires approval, and a `{role}` principal cannot give \
                     it — this host authenticated the caller as `{role}`, not as its operator, \
                     and an approval is the operator taking responsibility for this side \
                     effect: {hint}. Neither re-sending `approve` nor widening \
                     `approval_surfaces` will change the answer.",
                    role = principal.role.label()
                ),
            }),
            Self::DeniedForSurface {
                caller,
                allowed,
                hint,
            } => Some(ApprovalRejection {
                code: APPROVAL_DENIED_FOR_SURFACE_ERROR_CODE,
                message: format!(
                    "step `{step_key}` requires approval, and {caller} cannot give it — \
                     this chain honors approvals only from {allowed}: {hint}. Sending \
                     `approve` from here will not change the answer.",
                    caller = caller.describe()
                ),
            }),
        }
    }
}

/// A typed refusal of a gated step, carried out to the canonical
/// failure envelope.
pub(crate) struct ApprovalRejection {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

/// The gated step was never approved by anyone.
pub(crate) const APPROVAL_REQUIRED_ERROR_CODE: &str = "approval_required";

/// An approval was possible to send but impossible to honor: the calling
/// surface is not one this chain accepts approvals from.
pub(crate) const APPROVAL_DENIED_FOR_SURFACE_ERROR_CODE: &str = "approval_denied_for_surface";

/// An approval was possible to send but impossible to honor for a reason
/// no surface can fix: the caller authenticated as a program
/// ([`upeg_core::PrincipalRole::Agent`]), or as nobody at all. Naming its
/// surface in `approval_surfaces` would not change the answer.
pub(crate) const APPROVAL_DENIED_FOR_PRINCIPAL_ERROR_CODE: &str = "approval_denied_for_principal";

/// The principal the runtime stamped on this call, if any.
///
/// `None` means the envelope carries no principal block at all — a
/// hand-built call that never went through
/// [`upeg_runtime::apply_execution_context`]. That is not authority, so
/// it does not open the gate; it simply falls through to the surface
/// check, which is what decided such calls before principals existed.
fn calling_principal(args: &Value) -> Option<Principal> {
    args.get(EXECUTION_CONTEXT_ARG)
        .and_then(|context| context.get(EXECUTION_CONTEXT_PRINCIPAL))
        .and_then(Principal::from_json)
}

fn calling_surface(args: &Value) -> CallingSurface {
    let Some(raw) = args
        .get(EXECUTION_CONTEXT_ARG)
        .and_then(|context| context.get(EXECUTION_CONTEXT_SURFACE))
        .and_then(Value::as_str)
    else {
        return CallingSurface::Absent;
    };
    Surface::parse(raw).map_or_else(
        || CallingSurface::Unrecognized(raw.to_string()),
        CallingSurface::Known,
    )
}

/// Did the caller ask for this step to run? Unchanged arg shape: the
/// `approve` boolean approves the whole call, `_upeg.approvedSteps`
/// names step keys or tool ids.
fn approval_requested(args: &Value, step_key: &str, tool: &str) -> bool {
    if args.get(APPROVE_ARG).and_then(Value::as_bool) == Some(true) {
        return true;
    }
    args.get(EXECUTION_CONTEXT_ARG)
        .and_then(|context| context.get(EXECUTION_CONTEXT_APPROVED_STEPS))
        .and_then(Value::as_array)
        .is_some_and(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .any(|approved| approved == step_key || approved == tool)
        })
}

/// Reserved call argument that approves every gated step in one chain
/// call. Mirrored by `upeg-cli`'s `ReservedInputs` so the CLI accepts it
/// on Chain tools without the tool declaring it as an input.
const APPROVE_ARG: &str = "approve";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const STEP_KEY: &str = "gate";
    const STEP_TOOL: &str = "text.uppercase";
    const CHAIN_ID: &str = "dev.precommit";

    fn default_authorization() -> ApprovalSurfaces {
        ApprovalSurfaces::parse(CHAIN_ID, None).expect("the undeclared default is valid")
    }

    fn verdict(args: &Value) -> StepApproval {
        default_authorization().authorize(args, STEP_KEY, STEP_TOOL)
    }

    #[test]
    fn approval_from_call_without_surface_is_rejected() {
        // What a nested chain's inner step sees: `_upeg` was not
        // propagated so the caller has no identity. An approval without
        // identity is not an approval.
        let outcome = verdict(&json!({ "approve": true }));
        assert!(matches!(
            outcome,
            StepApproval::DeniedForSurface {
                caller: CallingSurface::Absent,
                ..
            }
        ));
    }

    #[test]
    fn approval_with_unknown_surface_label_is_rejected() {
        let outcome =
            verdict(&json!({ "approve": true, "_upeg": { "surface": "carrier-pigeon" } }));
        let StepApproval::DeniedForSurface { caller, .. } = outcome else {
            panic!("an unknown surface must be denied");
        };
        assert_eq!(
            caller,
            CallingSurface::Unrecognized("carrier-pigeon".to_string())
        );
    }

    #[test]
    fn approved_steps_may_name_step_id_or_tool_id() {
        for approved in [STEP_KEY, STEP_TOOL] {
            let args = json!({ "_upeg": { "surface": "cli", "approvedSteps": [approved] } });
            assert_eq!(
                verdict(&args),
                StepApproval::Approved,
                "approved={approved}"
            );
        }
    }

    #[test]
    fn default_approval_surfaces_are_the_three_attended_surfaces() {
        let default = default_authorization();
        for surface in [Surface::Cli, Surface::Tui, Surface::Desktop] {
            assert!(default.honors(surface), "{}", surface.label());
        }
        for surface in [Surface::Mcp, Surface::Http, Surface::Pwa, Surface::Ext] {
            assert!(
                !default.honors(surface),
                "a program-filled envelope is not a default: {}",
                surface.label()
            );
        }
    }

    #[test]
    fn default_approval_surfaces_all_have_operator_principal() {
        // The rationale for the default: for these three the OS user
        // account itself is the caller, so they are operators without
        // any token. If one were not, the principal gate would block
        // its own default.
        for surface in DEFAULT_APPROVAL_SURFACES {
            assert!(
                Principal::for_surface(*surface).may_approve(),
                "{}",
                surface.label()
            );
        }
    }

    #[test]
    fn agent_principal_approval_denied_even_on_authorized_surface() {
        let args = json!({
            "approve": true,
            "_upeg": {
                "surface": "cli",
                "principal": { "role": "agent", "surface": "cli" },
            },
        });

        let outcome = verdict(&args);

        let StepApproval::DeniedForPrincipal { principal, .. } = outcome else {
            panic!("an agent principal must be denied regardless of surface: {outcome:?}");
        };
        assert_eq!(principal.role, upeg_core::PrincipalRole::Agent);
        let rejection = verdict(&args)
            .rejection(STEP_KEY)
            .expect("a denial carries a message");
        assert_eq!(rejection.code, APPROVAL_DENIED_FOR_PRINCIPAL_ERROR_CODE);
        assert!(rejection.message.contains("agent"), "{}", rejection.message);
    }

    #[test]
    fn local_principal_may_approve_when_chain_authorizes_surface() {
        // stdio MCP is a program the OS user launched — it passes the
        // principal gate, and the surface gate decides where it may
        // approve.
        let mcp_only = ApprovalSurfaces::parse(CHAIN_ID, Some(&["mcp".to_string()]))
            .expect("an mcp-only declaration is valid");
        let args = json!({
            "approve": true,
            "_upeg": {
                "surface": "mcp",
                "principal": { "role": "local", "surface": "mcp" },
            },
        });

        assert_eq!(
            mcp_only.authorize(&args, STEP_KEY, STEP_TOOL),
            StepApproval::Approved
        );
        // The same principal still hits the surface gate under the
        // default authorization (cli/tui/desktop) — the two gates do
        // not stand in for each other.
        assert!(matches!(
            verdict(&args),
            StepApproval::DeniedForSurface { .. }
        ));
    }

    #[test]
    fn agent_principal_cannot_approve_even_on_authorized_surface() {
        let mcp_only = ApprovalSurfaces::parse(CHAIN_ID, Some(&["mcp".to_string()]))
            .expect("an mcp-only declaration is valid");
        let args = json!({
            "approve": true,
            "_upeg": {
                "surface": "mcp",
                "principal": { "role": "agent", "surface": "mcp" },
            },
        });

        let rejection = mcp_only
            .authorize(&args, STEP_KEY, STEP_TOOL)
            .rejection(STEP_KEY)
            .expect("a denial carries a message");

        assert_eq!(rejection.code, APPROVAL_DENIED_FOR_PRINCIPAL_ERROR_CODE);
    }

    #[test]
    fn operator_principal_may_approve_on_authorized_surface() {
        for surface in DEFAULT_APPROVAL_SURFACES {
            let label = surface.label();
            let args = json!({
                "approve": true,
                "_upeg": {
                    "surface": label,
                    "principal": { "role": "operator", "surface": label },
                },
            });
            assert_eq!(verdict(&args), StepApproval::Approved, "{label}");
        }
    }

    #[test]
    fn operator_principal_denied_on_unauthorized_surface() {
        // The two gates are independent: a passing principal still
        // leaves the surface gate.
        let args = json!({
            "approve": true,
            "_upeg": {
                "surface": "http",
                "principal": { "role": "operator", "surface": "http" },
            },
        });

        let rejection = verdict(&args)
            .rejection(STEP_KEY)
            .expect("a denial carries a message");

        assert_eq!(rejection.code, APPROVAL_DENIED_FOR_SURFACE_ERROR_CODE);
    }

    #[test]
    fn extension_surface_never_approves_even_when_a_hand_built_policy_names_it() {
        let ext_only = ApprovalSurfaces {
            chain: CHAIN_ID.to_string(),
            surfaces: vec![Surface::Ext],
        };
        let args = json!({
            "approve": true,
            "_upeg": {
                "surface": "ext",
                "principal": { "role": "operator", "surface": "ext" },
            },
        });

        let rejection = ext_only
            .authorize(&args, STEP_KEY, STEP_TOOL)
            .rejection(STEP_KEY)
            .expect("the extension has no approval gesture");

        assert_eq!(rejection.code, APPROVAL_DENIED_FOR_SURFACE_ERROR_CODE);
    }

    #[test]
    fn call_without_principal_block_is_judged_by_surface_gate_only() {
        // No principal is not authority, it is missing information — it
        // neither opens the gate nor skips the surface check.
        let args = json!({ "approve": true, "_upeg": { "surface": "cli" } });
        assert_eq!(verdict(&args), StepApproval::Approved);

        let args = json!({ "approve": true, "_upeg": { "surface": "mcp" } });
        assert!(matches!(
            verdict(&args),
            StepApproval::DeniedForSurface { .. }
        ));
    }

    #[test]
    fn registered_policy_exposes_all_chain_authorized_surfaces() {
        let surfaces = ApprovalSurfaces::parse(CHAIN_ID, Some(&["mcp".to_string()]))
            .expect("an mcp-only declaration is valid");

        let policy = surfaces.policy();

        assert!(policy.requires_approval());
        assert!(policy.honors(Surface::Mcp));
        assert!(!policy.honors(Surface::Cli));
    }

    #[test]
    fn cli_honoring_chain_rejection_names_the_runnable_command() {
        let outcome = verdict(&json!({ "approve": true, "_upeg": { "surface": "mcp" } }));
        let message = outcome
            .rejection(STEP_KEY)
            .expect("a denial carries a message")
            .message;
        assert!(
            message.contains(&format!("upeg call {CHAIN_ID} -a approve=true")),
            "{message}"
        );
    }

    #[test]
    fn non_cli_chain_rejection_does_not_invent_a_command() {
        let surfaces = ApprovalSurfaces::parse(CHAIN_ID, Some(&["mcp".to_string()]))
            .expect("an mcp-only declaration is valid");
        let outcome = surfaces.authorize(
            &json!({ "approve": true, "_upeg": { "surface": "cli" } }),
            STEP_KEY,
            STEP_TOOL,
        );
        let message = outcome
            .rejection(STEP_KEY)
            .expect("a denial carries a message")
            .message;
        assert!(!message.contains("upeg call"), "{message}");
        assert!(message.contains("mcp"), "{message}");
    }
}
