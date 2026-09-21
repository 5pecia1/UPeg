//! Registration replaces, it never accumulates: what a tool id
//! advertises about its approval barrier must describe the manifest that
//! is loaded *now*.
//!
//! `chain_dispatcher_for` publishes [`upeg_runtime::ToolApprovalPolicy`]
//! so a UI can ask "will this stop at a human?" before it dispatches. The
//! publish used to sit past three early returns, so an id that came back
//! as anything other than a stepped Chain kept whatever the previous
//! registration said — a confirm dialog in front of a run nothing gates,
//! and `honors()` naming approvers for a tool with nothing to approve.

use super::*;

/// Ids are process-global (the policy registry is a static map) and
/// these tests run in parallel, so each one owns its own.
fn chain_toml(id: &str, steps: &str) -> ToolToml {
    toml::from_str::<ToolToml>(&format!(
        r#"id = "{id}"
            toolkit = "test"
            invoker = "Chain"
            {steps}
        "#
    ))
    .expect("valid chain fixture")
}

fn gated_chain(id: &str) -> ToolToml {
    chain_toml(
        id,
        r#"[[steps]]
            id = "gate"
            tool = "text.repeat"
            requires_approval = true"#,
    )
}

#[test]
fn gated_chain_publishes_approval_policy() {
    // Control: keeps the tests below from passing on "the policy was
    // never installed in the first place".
    let id = "test.policy.published";
    assert!(chain_dispatcher_for(&gated_chain(id)).is_some());

    let policy = upeg_runtime::tool_approval_policy(id);
    assert!(policy.requires_approval());
    assert!(policy.honors(upeg_core::Surface::Cli));
}

#[test]
fn rebinding_id_to_non_chain_clears_approval_policy() {
    let id = "test.policy.rebound_external";
    assert!(chain_dispatcher_for(&gated_chain(id)).is_some());
    assert!(upeg_runtime::tool_approval_policy(id).requires_approval());

    let external = toml::from_str::<ToolToml>(&format!(
        r#"id = "{id}"
            toolkit = "test"
            invoker = "External"
            command = "true"
        "#
    ))
    .expect("valid External fixture");
    assert!(
        chain_dispatcher_for(&external).is_none(),
        "a non-Chain manifest builds no chain dispatcher"
    );

    let policy = upeg_runtime::tool_approval_policy(id);
    assert!(
        !policy.requires_approval(),
        "a barrier-free tool must not advertise a barrier"
    );
    assert!(policy.surfaces().is_empty());
}

#[test]
fn rebinding_id_to_stepless_chain_clears_approval_policy() {
    // Covers both early returns: `steps` gone, and `steps` an empty
    // array. Neither builds a dispatcher, so neither may leave a
    // barrier behind.
    for (suffix, steps) in [("dropped", ""), ("empty", "steps = []")] {
        let id = format!("test.policy.rebound_{suffix}");
        assert!(chain_dispatcher_for(&gated_chain(&id)).is_some());
        assert!(upeg_runtime::tool_approval_policy(&id).requires_approval());

        assert!(chain_dispatcher_for(&chain_toml(&id, steps)).is_none());
        assert!(
            !upeg_runtime::tool_approval_policy(&id).requires_approval(),
            "{suffix}"
        );
    }
}
