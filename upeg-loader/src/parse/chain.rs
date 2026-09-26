//! Load-time validation of the Chain contract that the Chain dispatcher
//! then relies on at runtime.
//!
//! The rules live here, all of which exist so a chain cannot be *written*
//! in a shape the engine would have to resolve silently later:
//!
//! * `approval_surfaces` must name real surfaces, and must name at least
//!   one — otherwise a `requires_approval` step is permanently
//!   unsatisfiable and the manifest says nothing about it.
//! * `approval_surfaces` must also *reach* the tool: a chain exposed only
//!   on surfaces none of which may approve is unsatisfiable for exactly
//!   the same reason, one step further out.
//! * A Chain tool may not declare an output under the id the engine
//!   reserves for its per-step summary, so the summary row can never
//!   shadow one the author declared.

use crate::dispatcher::{
    ApprovalSurfaces, ApprovalSurfacesError, CHAIN_STEPS_OUTPUT_ID, surface_label_list,
};
use crate::{LoadError, ToolToml};
use upeg_core::{ALL_SURFACES, Surface};

/// Manifest value of [`upeg_core::Invoker::Chain`]. `steps` also implies
/// it (`toml_to_meta` fills the field in), so both are accepted as proof
/// that this entry is a chain.
const CHAIN_INVOKER: &str = "Chain";

pub(crate) fn validate_chain_contract(parsed: &ToolToml) -> Result<(), LoadError> {
    let is_chain =
        parsed.invoker.as_deref().map(str::trim) == Some(CHAIN_INVOKER) || parsed.steps.is_some();
    if !is_chain {
        // `approval_surfaces` on a non-chain tool would be inert. Say so
        // rather than accept a declaration that authorizes nothing.
        if parsed.approval_surfaces.is_some() {
            return Err(LoadError::ApprovalSurfacesWithoutChain);
        }
        return Ok(());
    }
    let approval = validate_approval_surfaces(parsed)?;
    validate_approval_reach(parsed, &approval)?;
    validate_reserved_outputs(parsed)
}

fn validate_approval_surfaces(parsed: &ToolToml) -> Result<ApprovalSurfaces, LoadError> {
    ApprovalSurfaces::parse(&parsed.id, parsed.approval_surfaces.as_deref()).map_err(|error| {
        match error {
            ApprovalSurfacesError::Empty => LoadError::EmptyApprovalSurfaces,
            ApprovalSurfacesError::EmptyEntry { position } => {
                LoadError::EmptyInApprovalSurfaces { position }
            }
            ApprovalSurfacesError::Unknown { position, surface } => {
                LoadError::UnknownApprovalSurface { position, surface }
            }
            ApprovalSurfacesError::ExtensionUnsupported { position } => {
                LoadError::ExtensionApprovalUnsupported { position }
            }
        }
    })
}

/// A chain gated behind `requires_approval` must be callable from at
/// least one surface that may approve it.
///
/// `approval_surfaces = ["cli"]` on a tool declared `surfaces = ["mcp"]`
/// loads clean today and then refuses every single call: the only
/// callers that can reach it are the ones whose approval is never
/// honored. That is [`LoadError::EmptyApprovalSurfaces`]'s failure with
/// an extra step of indirection, so it fails the same way — at load,
/// where the author can still fix it.
///
/// Only chains with a gated step are checked: an ungated chain's
/// `approval_surfaces` authorizes nothing that ever runs, and narrowing
/// `surfaces` on it is not a mistake.
fn validate_approval_reach(
    parsed: &ToolToml,
    approval: &ApprovalSurfaces,
) -> Result<(), LoadError> {
    if !declares_approval_step(parsed) {
        return Ok(());
    }
    // A malformed `surfaces` list is `validate_surfaces`'s error to
    // report; reaching a verdict from a half-parsed list would steal it.
    let Some(exposed) = exposed_surfaces(parsed) else {
        return Ok(());
    };
    if exposed.iter().any(|surface| approval.honors(*surface)) {
        return Ok(());
    }
    Err(LoadError::UnreachableApprovalSurfaces {
        approval: approval.labels(),
        exposed: surface_label_list(&exposed),
    })
}

fn declares_approval_step(parsed: &ToolToml) -> bool {
    parsed
        .steps
        .iter()
        .flatten()
        .any(|step| step.requires_approval.unwrap_or(false))
}

/// The surfaces this tool is exposed on, or `None` when the declaration
/// is malformed. A Chain tool with no `surfaces` is on all of them
/// (`parse_runtime_surfaces`).
fn exposed_surfaces(parsed: &ToolToml) -> Option<Vec<Surface>> {
    let Some(declared) = &parsed.surfaces else {
        return Some(ALL_SURFACES.to_vec());
    };
    declared.iter().map(|raw| Surface::parse(raw)).collect()
}

fn validate_reserved_outputs(parsed: &ToolToml) -> Result<(), LoadError> {
    parsed
        .outputs
        .iter()
        .position(|output| output.name.trim() == CHAIN_STEPS_OUTPUT_ID)
        .map_or(Ok(()), |position| {
            Err(LoadError::ReservedChainOutputName {
                position,
                name: CHAIN_STEPS_OUTPUT_ID,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::DEFAULT_APPROVAL_SURFACES;
    use crate::parse_toolkit_full;

    /// Minimal manifest of one Toolkit + one Tool. `body` is spliced
    /// verbatim inside the `[[tools]]` block.
    fn single_tool_manifest(body: &str) -> Result<(), LoadError> {
        let manifest = format!(
            "id = \"chaincontract\"\n\n[[tools]]\nid = \"gate\"\npegboard_units = \"U1\"\n{body}\n"
        );
        parse_toolkit_full(&manifest).map(|_| ())
    }

    fn load_error(body: &str) -> LoadError {
        single_tool_manifest(body).expect_err("this manifest must be rejected")
    }

    const APPROVAL_STEP: &str =
        "[[tools.steps]]\ntool = \"text.uppercase\"\nrequires_approval = true\n";

    #[test]
    fn chain_omitting_approval_surfaces_loads() {
        single_tool_manifest(&format!("invoker = \"Chain\"\n{APPROVAL_STEP}"))
            .expect("omitting the declaration uses the default approval surfaces");
    }

    #[test]
    fn unknown_approval_surfaces_entry_fails_load() {
        let error = load_error(&format!(
            "invoker = \"Chain\"\napproval_surfaces = [\"cli\", \"telepathy\"]\n{APPROVAL_STEP}"
        ));
        assert!(
            matches!(
                &error,
                LoadError::UnknownApprovalSurface { position: 1, surface } if surface == "telepathy"
            ),
            "got {error:?}"
        );
        assert!(
            error
                .to_string()
                .contains("cli/tui/desktop/pwa/ext/mcp/http"),
            "{error}"
        );
    }

    #[test]
    fn extension_approval_surface_fails_load_because_it_has_no_gesture() {
        let err = load_error(&format!(
            "invoker = \"Chain\"\napproval_surfaces = [\"ext\"]\n{APPROVAL_STEP}"
        ));

        assert!(matches!(
            err,
            LoadError::ExtensionApprovalUnsupported { position: 0 }
        ));
    }

    #[test]
    fn empty_approval_surfaces_entry_fails_load() {
        let error = load_error(&format!(
            "invoker = \"Chain\"\napproval_surfaces = [\"  \"]\n{APPROVAL_STEP}"
        ));
        assert!(
            matches!(error, LoadError::EmptyInApprovalSurfaces { position: 0 }),
            "got {error:?}"
        );
    }

    #[test]
    fn approval_surfaces_authorizing_nobody_fails_load() {
        let error = load_error(&format!(
            "invoker = \"Chain\"\napproval_surfaces = []\n{APPROVAL_STEP}"
        ));
        assert!(
            matches!(error, LoadError::EmptyApprovalSurfaces),
            "got {error:?}"
        );
    }

    #[test]
    fn approval_surfaces_on_non_chain_tool_fails_load() {
        let error = load_error(
            "invoker = \"External\"\ncommand = \"true\"\napproval_surfaces = [\"cli\"]\n",
        );
        assert!(
            matches!(error, LoadError::ApprovalSurfacesWithoutChain),
            "got {error:?}"
        );
    }

    #[test]
    fn approval_surfaces_not_overlapping_tool_surfaces_fail_load() {
        // The default approval surfaces are the three attended ones, yet
        // this tool is exposed only on `mcp` — no caller who can approve
        // can reach this tool, so this chain's approval step could
        // never pass.
        let error = load_error(&format!(
            "invoker = \"Chain\"\nsurfaces = [\"mcp\"]\n{APPROVAL_STEP}"
        ));
        assert!(
            matches!(
                &error,
                LoadError::UnreachableApprovalSurfaces { approval, exposed }
                    if *approval == surface_label_list(DEFAULT_APPROVAL_SURFACES)
                        && exposed == "mcp"
            ),
            "got {error:?}"
        );
    }

    #[test]
    fn approval_surfaces_overlapping_tool_surfaces_load() {
        single_tool_manifest(&format!(
            "invoker = \"Chain\"\nsurfaces = [\"mcp\", \"cli\"]\n{APPROVAL_STEP}"
        ))
        .expect("any single approving surface makes the chain runnable");
    }

    #[test]
    fn chain_without_approval_steps_loads_despite_surface_mismatch() {
        // Without an approval barrier `approval_surfaces` gates nothing —
        // narrowing such a chain's surfaces is not a mistake.
        single_tool_manifest(
            "invoker = \"Chain\"\nsurfaces = [\"mcp\"]\n[[tools.steps]]\ntool = \"text.uppercase\"\n",
        )
        .expect("a chain without approval steps is not subject to the reach check");
    }

    #[test]
    fn unknown_surface_reports_own_error_not_reach_check() {
        let error = load_error(&format!(
            "invoker = \"Chain\"\nsurfaces = [\"telepathy\"]\n{APPROVAL_STEP}"
        ));
        assert!(
            matches!(&error, LoadError::UnknownSurface(surface) if surface == "telepathy"),
            "got {error:?}"
        );
    }

    #[test]
    fn chain_declaring_reserved_steps_output_fails_load() {
        let error = load_error(&format!(
            "invoker = \"Chain\"\noutputs = [{{ name = \"steps\", type = \"string\" }}]\nprimary_output_id = \"steps\"\n{APPROVAL_STEP}"
        ));
        assert!(
            matches!(
                error,
                LoadError::ReservedChainOutputName {
                    position: 0,
                    name: "steps"
                }
            ),
            "got {error:?}"
        );
    }
}
