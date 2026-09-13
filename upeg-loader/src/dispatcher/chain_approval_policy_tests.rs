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
fn 체인_toml(id: &str, steps: &str) -> ToolToml {
    toml::from_str::<ToolToml>(&format!(
        r#"id = "{id}"
            toolkit = "test"
            invoker = "Chain"
            {steps}
        "#
    ))
    .expect("유효한 체인 fixture")
}

fn 게이트된_체인(id: &str) -> ToolToml {
    체인_toml(
        id,
        r#"[[steps]]
            id = "gate"
            tool = "text.repeat"
            requires_approval = true"#,
    )
}

#[test]
fn 게이트된_체인은_승인_정책을_공표한다() {
    // 대조군: 아래 테스트들이 "정책이 애초에 실리지 않는다"로도
    // 통과하지 않게 한다.
    let id = "test.policy.published";
    assert!(chain_dispatcher_for(&게이트된_체인(id)).is_some());

    let policy = upeg_runtime::tool_approval_policy(id);
    assert!(policy.requires_approval());
    assert!(policy.honors(upeg_core::Surface::Cli));
}

#[test]
fn 같은_id가_비체인으로_재등록되면_승인_정책이_지워진다() {
    let id = "test.policy.rebound_external";
    assert!(chain_dispatcher_for(&게이트된_체인(id)).is_some());
    assert!(upeg_runtime::tool_approval_policy(id).requires_approval());

    let external = toml::from_str::<ToolToml>(&format!(
        r#"id = "{id}"
            toolkit = "test"
            invoker = "External"
            command = "true"
        "#
    ))
    .expect("유효한 External fixture");
    assert!(
        chain_dispatcher_for(&external).is_none(),
        "Chain이 아닌 매니페스트는 체인 dispatcher를 만들지 않는다"
    );

    let policy = upeg_runtime::tool_approval_policy(id);
    assert!(
        !policy.requires_approval(),
        "장벽이 없는 도구가 장벽을 광고하면 안 된다"
    );
    assert!(policy.surfaces().is_empty());
}

#[test]
fn 같은_id가_step_없는_체인으로_재등록되어도_승인_정책이_지워진다() {
    // `steps`가 사라진 경우와 빈 배열인 경우, 두 이른 반환 모두를
    // 지난다. 어느 쪽도 dispatcher를 만들지 않으므로 어느 쪽도
    // 장벽을 남겨서는 안 된다.
    for (suffix, steps) in [("dropped", ""), ("empty", "steps = []")] {
        let id = format!("test.policy.rebound_{suffix}");
        assert!(chain_dispatcher_for(&게이트된_체인(&id)).is_some());
        assert!(upeg_runtime::tool_approval_policy(&id).requires_approval());

        assert!(chain_dispatcher_for(&체인_toml(&id, steps)).is_none());
        assert!(
            !upeg_runtime::tool_approval_policy(&id).requires_approval(),
            "{suffix}"
        );
    }
}
