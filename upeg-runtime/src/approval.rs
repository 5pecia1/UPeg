//! Per-tool approval policy: does running this Tool need a person to say
//! yes first, and which surfaces may say it.
//!
//! Only a Chain Tool can have a `requires_approval` step, so this is the
//! same shape as [`crate::credentials`]: a manifest-derived fact the
//! loader records at registration and presentation surfaces read back.
//! It lives beside the registry rather than inside [`ToolMeta`] because
//! it is a *dispatch* property — a static `#[upeg::tool]` function has no
//! chain and therefore no barrier, and giving every `ToolMeta` in the
//! workspace a field it can never set would put the answer in the wrong
//! layer. [`crate::ToolMetaRuntimeExt`] is how a surface asks a
//! `ToolMeta` for it.
//!
//! What a UI does with it: `requires_approval` says "confirm before you
//! dispatch, or the run will stop at the barrier", and
//! [`ToolApprovalPolicy::surfaces`] says whether *this* surface's
//! confirmation would even be honored (`docs/architecture/chain.md`).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use upeg_core::Surface;

/// A Tool's approval barrier as the UI needs to see it, before dispatch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolApprovalPolicy {
    /// At least one step of this Tool is gated behind
    /// `requires_approval`.
    requires_approval: bool,
    /// The effective set of surfaces whose approval this Tool honors —
    /// the manifest's `approval_surfaces` when declared, otherwise the
    /// loader's default. Empty for a Tool with no barrier at all: there
    /// is nothing to approve, so naming approvers would invent a
    /// question nobody asked.
    surfaces: Vec<Surface>,
}

impl ToolApprovalPolicy {
    /// A Tool whose run needs no approval from anyone.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// A gated Tool and the surfaces it honors approvals from.
    #[must_use]
    pub fn gated(surfaces: Vec<Surface>) -> Self {
        Self {
            requires_approval: true,
            surfaces,
        }
    }

    #[must_use]
    pub const fn requires_approval(&self) -> bool {
        self.requires_approval
    }

    #[must_use]
    pub fn surfaces(&self) -> &[Surface] {
        &self.surfaces
    }

    /// May a caller on `surface` lift this Tool's barrier? Always false
    /// when there is no barrier — the honest answer to "may I approve"
    /// when there is nothing to approve.
    #[must_use]
    pub fn honors(&self, surface: Surface) -> bool {
        self.surfaces.contains(&surface)
    }
}

fn policies_lock() -> &'static Mutex<HashMap<String, ToolApprovalPolicy>> {
    static POLICIES: OnceLock<Mutex<HashMap<String, ToolApprovalPolicy>>> = OnceLock::new();
    POLICIES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record `tool_id`'s approval policy. A no-barrier policy clears the
/// record so a Tool re-registered without a gate stops advertising one.
pub fn set_tool_approval_policy(tool_id: &str, policy: ToolApprovalPolicy) {
    let Ok(mut guard) = policies_lock().lock() else {
        return;
    };
    if policy == ToolApprovalPolicy::none() {
        guard.remove(tool_id);
    } else {
        guard.insert(tool_id.to_string(), policy);
    }
}

/// `tool_id`'s approval policy; [`ToolApprovalPolicy::none`] for every
/// Tool that never registered one (static Rust tools, external/HTTP/LLM
/// manifests, and chains with no gated step).
#[must_use]
pub fn tool_approval_policy(tool_id: &str) -> ToolApprovalPolicy {
    policies_lock()
        .lock()
        .ok()
        .and_then(|guard| guard.get(tool_id).cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 등록되지_않은_도구는_승인_장벽이_없다() {
        let policy = tool_approval_policy("test.approval.unregistered");
        assert!(!policy.requires_approval());
        assert!(policy.surfaces().is_empty());
        assert!(!policy.honors(Surface::Cli));
    }

    #[test]
    fn 게이트된_도구는_인가된_표면만_승인할_수_있다고_답한다() {
        let id = "test.approval.registry.gated";
        set_tool_approval_policy(
            id,
            ToolApprovalPolicy::gated(vec![Surface::Cli, Surface::Tui]),
        );

        let policy = tool_approval_policy(id);
        assert!(policy.requires_approval());
        assert!(policy.honors(Surface::Tui));
        assert!(!policy.honors(Surface::Mcp));

        set_tool_approval_policy(id, ToolApprovalPolicy::none());
        assert!(!tool_approval_policy(id).requires_approval());
    }

    #[test]
    fn 장벽이_없는_정책은_아무_표면도_승인자로_지목하지_않는다() {
        let policy = ToolApprovalPolicy::none();
        for surface in upeg_core::ALL_SURFACES {
            assert!(!policy.honors(*surface), "{}", surface.label());
        }
    }
}
