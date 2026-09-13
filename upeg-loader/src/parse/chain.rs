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

    /// 한 Toolkit + 한 Tool 짜리 최소 매니페스트. `body`는 `[[tools]]`
    /// 블록 안에 그대로 들어간다.
    fn 단일_도구_매니페스트(body: &str) -> Result<(), LoadError> {
        let manifest = format!(
            "id = \"chaincontract\"\n\n[[tools]]\nid = \"gate\"\npegboard_units = \"U1\"\n{body}\n"
        );
        parse_toolkit_full(&manifest).map(|_| ())
    }

    fn 오류(body: &str) -> LoadError {
        단일_도구_매니페스트(body).expect_err("이 매니페스트는 거부되어야 한다")
    }

    const 승인_단계: &str =
        "[[tools.steps]]\ntool = \"text.uppercase\"\nrequires_approval = true\n";

    #[test]
    fn approval_surfaces를_생략한_체인은_로드된다() {
        단일_도구_매니페스트(&format!("invoker = \"Chain\"\n{승인_단계}"))
            .expect("선언을 생략하면 기본 승인 surface가 쓰인다");
    }

    #[test]
    fn 알수없는_approval_surfaces_항목은_로드에서_거부된다() {
        let error = 오류(&format!(
            "invoker = \"Chain\"\napproval_surfaces = [\"cli\", \"telepathy\"]\n{승인_단계}"
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
    fn 빈_approval_surfaces_항목은_로드에서_거부된다() {
        let error = 오류(&format!(
            "invoker = \"Chain\"\napproval_surfaces = [\"  \"]\n{승인_단계}"
        ));
        assert!(
            matches!(error, LoadError::EmptyInApprovalSurfaces { position: 0 }),
            "got {error:?}"
        );
    }

    #[test]
    fn 아무_표면도_인가하지_않는_approval_surfaces는_로드에서_거부된다() {
        let error = 오류(&format!(
            "invoker = \"Chain\"\napproval_surfaces = []\n{승인_단계}"
        ));
        assert!(
            matches!(error, LoadError::EmptyApprovalSurfaces),
            "got {error:?}"
        );
    }

    #[test]
    fn 체인이_아닌_도구의_approval_surfaces는_로드에서_거부된다() {
        let error =
            오류("invoker = \"External\"\ncommand = \"true\"\napproval_surfaces = [\"cli\"]\n");
        assert!(
            matches!(error, LoadError::ApprovalSurfacesWithoutChain),
            "got {error:?}"
        );
    }

    #[test]
    fn 승인_surface가_도구_surface와_겹치지_않으면_로드에서_거부된다() {
        // 기본 승인 surface는 사람이 앉아있는 세 표면인데 도구는
        // `mcp`에만 노출된다 — 승인할 수 있는 호출자가 이 도구에 닿을 수
        // 없으므로 이 체인의 승인 step은 영원히 통과하지 못한다.
        let error = 오류(&format!(
            "invoker = \"Chain\"\nsurfaces = [\"mcp\"]\n{승인_단계}"
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
    fn 승인_surface가_도구_surface와_하나라도_겹치면_로드된다() {
        단일_도구_매니페스트(&format!(
            "invoker = \"Chain\"\nsurfaces = [\"mcp\", \"cli\"]\n{승인_단계}"
        ))
        .expect("승인할 수 있는 surface가 하나라도 있으면 실행 가능한 체인이다");
    }

    #[test]
    fn 승인_step이_없는_체인은_surface가_겹치지_않아도_로드된다() {
        // 승인 장벽이 없으면 `approval_surfaces`는 아무것도 막지 않는다 —
        // 그런 체인의 surface를 좁히는 것은 실수가 아니다.
        단일_도구_매니페스트(
            "invoker = \"Chain\"\nsurfaces = [\"mcp\"]\n[[tools.steps]]\ntool = \"text.uppercase\"\n",
        )
        .expect("승인 step이 없는 체인은 reach 검사 대상이 아니다");
    }

    #[test]
    fn 알수없는_surface는_reach_검사가_아니라_자기_오류로_보고된다() {
        let error = 오류(&format!(
            "invoker = \"Chain\"\nsurfaces = [\"telepathy\"]\n{승인_단계}"
        ));
        assert!(
            matches!(&error, LoadError::UnknownSurface(surface) if surface == "telepathy"),
            "got {error:?}"
        );
    }

    #[test]
    fn 체인이_예약된_steps_출력을_선언하면_로드에서_거부된다() {
        let error = 오류(&format!(
            "invoker = \"Chain\"\noutputs = [{{ name = \"steps\", type = \"string\" }}]\nprimary_output_id = \"steps\"\n{승인_단계}"
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
