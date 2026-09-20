use super::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use upeg_core::{
    BindingRole, BindingWait, BindingWaitCondition, BindingWaitOnTimeout,
    ControlledEmbedTriggerAction, DEFAULT_CONTROLLED_EMBED_WAIT_POLL_MS, SelectorBinding,
};

struct SequenceWaitEvaluator {
    readiness: VecDeque<bool>,
    calls: usize,
    scripts: Vec<String>,
}

impl SequenceWaitEvaluator {
    fn new(readiness: impl IntoIterator<Item = bool>) -> Self {
        Self {
            readiness: readiness.into_iter().collect(),
            calls: 0,
            scripts: Vec::new(),
        }
    }
}

impl HeadlessWaitEvaluator for SequenceWaitEvaluator {
    fn evaluate_wait<'a>(
        &'a mut self,
        script: &'a str,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<bool, ControlledEmbedError>> + Send + 'a>>
    {
        self.calls += 1;
        self.scripts.push(script.to_string());
        let ready = self.readiness.pop_front().unwrap_or(false);
        Box::pin(std::future::ready(Ok(ready)))
    }
}

/// Fake evaluator whose polls yield scripted `Result`s — lets tests exercise
/// evaluate *errors* (e.g. the stale-context `-32000` CDP failure right after
/// a navigation), which [`SequenceWaitEvaluator`] cannot express.
struct ResultSequenceWaitEvaluator {
    results: VecDeque<Result<bool, ControlledEmbedError>>,
    calls: usize,
}

impl ResultSequenceWaitEvaluator {
    fn new(results: impl IntoIterator<Item = Result<bool, ControlledEmbedError>>) -> Self {
        Self {
            results: results.into_iter().collect(),
            calls: 0,
        }
    }
}

impl HeadlessWaitEvaluator for ResultSequenceWaitEvaluator {
    fn evaluate_wait<'a>(
        &'a mut self,
        _script: &'a str,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<bool, ControlledEmbedError>> + Send + 'a>>
    {
        self.calls += 1;
        let result = self.results.pop_front().unwrap_or(Ok(false));
        Box::pin(std::future::ready(result))
    }
}

fn binding_with_wait(
    role: BindingRole,
    field: &str,
    selector: &str,
    wait: BindingWait,
) -> SelectorBinding {
    SelectorBinding {
        role,
        field: field.to_string(),
        selector: selector.to_string(),
        trigger_action: ControlledEmbedTriggerAction::Click,
        wait: Some(wait),
    }
}

fn binding_wait(
    for_selector: Option<&str>,
    condition: BindingWaitCondition,
    timeout_ms: u64,
    settle_ms: u64,
    on_timeout: BindingWaitOnTimeout,
) -> BindingWait {
    BindingWait {
        for_selector: for_selector.map(str::to_string),
        condition,
        timeout_ms,
        settle_ms,
        on_timeout,
    }
}

struct NoopWaker;

impl Wake for NoopWaker {
    fn wake(self: Arc<Self>) {}
}

fn noop_waker() -> Waker {
    Waker::from(Arc::new(NoopWaker))
}

#[test]
fn headless_operations_order_inputs_trigger_outputs_and_attach_each_wait() {
    const INPUT_A_VALUE: &str = "alpha";
    const INPUT_B_VALUE: &str = "beta";
    const SHORT_WAIT_MS: u64 = 10;
    const MEDIUM_WAIT_MS: u64 = 20;
    const LONG_WAIT_MS: u64 = 30;
    const SETTLE_MS: u64 = 5;

    let output = binding_with_wait(
        BindingRole::Output,
        "first_output",
        "#first-output",
        binding_wait(
            Some("#first-ready"),
            BindingWaitCondition::Exists,
            SHORT_WAIT_MS,
            SETTLE_MS,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let input_b = binding_with_wait(
        BindingRole::Input,
        "input_b",
        "#input-b",
        binding_wait(
            Some("#input-b-ready"),
            BindingWaitCondition::Visible,
            MEDIUM_WAIT_MS,
            0,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let trigger = binding_with_wait(
        BindingRole::Trigger,
        "",
        "#submit",
        binding_wait(
            Some("#submit-ready"),
            BindingWaitCondition::Exists,
            LONG_WAIT_MS,
            0,
            BindingWaitOnTimeout::Continue,
        ),
    );
    let input_a = binding_with_wait(
        BindingRole::Input,
        "input_a",
        "#input-a",
        binding_wait(
            Some("#input-a-ready"),
            BindingWaitCondition::Exists,
            SHORT_WAIT_MS,
            SETTLE_MS,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let output2 = binding_with_wait(
        BindingRole::Output,
        "second_output",
        "#second-output",
        binding_wait(
            Some("#second-ready"),
            BindingWaitCondition::Visible,
            MEDIUM_WAIT_MS,
            SETTLE_MS,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let bindings = vec![
        output.clone(),
        input_b.clone(),
        trigger.clone(),
        input_a.clone(),
        output2.clone(),
    ];

    let operations = headless_operations(
        &bindings,
        &[("input_a", INPUT_A_VALUE), ("input_b", INPUT_B_VALUE)],
    );

    let expected = [
        (&input_b, HeadlessOperationKind::Write),
        (&input_a, HeadlessOperationKind::Write),
        (&trigger, HeadlessOperationKind::Trigger),
        (&output, HeadlessOperationKind::Read),
        (&output2, HeadlessOperationKind::Read),
    ];
    assert_eq!(operations.len(), expected.len());
    for (operation, (binding, kind)) in operations.iter().zip(expected) {
        let expected_wait = headless_wait_for_binding(binding).expect("wait exists");

        assert_eq!(operation.kind, kind);
        assert_eq!(operation.wait.as_ref(), Some(&expected_wait));
    }
    assert!(operations[0].script.contains(INPUT_B_VALUE));
    assert!(operations[1].script.contains(INPUT_A_VALUE));
}

#[test]
fn launch_args_add_no_override_for_default_settings() {
    assert!(controlled_embed_launch_args(&ControlledEmbedSettings::default()).is_empty());
}

#[test]
fn launch_args_apply_mobile_safari_and_preset_viewport() {
    let args = controlled_embed_launch_args(&ControlledEmbedSettings {
        user_agent: Some(upeg_core::ControlledEmbedUserAgent::MobileSafari),
        viewport: Some(upeg_core::ControlledEmbedViewport::Preset(
            upeg_core::ControlledEmbedViewportPreset::Mobile,
        )),
    });

    assert_eq!(
        args,
        vec![
            format!("--user-agent={CONTROLLED_EMBED_MOBILE_SAFARI_USER_AGENT}"),
            "--window-size=390,844".to_string(),
        ]
    );
}

#[test]
fn launch_args_apply_custom_user_agent_and_custom_viewport() {
    let args = controlled_embed_launch_args(&ControlledEmbedSettings {
        user_agent: Some(upeg_core::ControlledEmbedUserAgent::Custom(
            "Custom UA".into(),
        )),
        viewport: Some(upeg_core::ControlledEmbedViewport::Custom {
            width: 320,
            height: 640,
        }),
    });

    assert_eq!(
        args,
        vec![
            "--user-agent=Custom UA".to_string(),
            "--window-size=320,640".to_string(),
        ]
    );
}

/// Security absolutes #14: embeds run sandboxed. chromiumoxide keeps
/// Chrome's sandbox on by default, so the only way to lose it is a flag
/// of our own — none of the launch flags may be one.
#[test]
fn headless_launch_flags_never_disable_chromes_sandbox() {
    for arg in HEADLESS_LAUNCH_ARGS {
        assert!(
            !arg.contains("sandbox"),
            "launch flag `{arg}` must not touch Chrome's sandbox"
        );
    }
    assert!(
        HEADLESS_LAUNCH_ARGS.contains(&"--headless=new"),
        "headless mode itself must still be requested"
    );
    assert!(
        !HEADLESS_LAUNCH_ARGS.contains(&"--no-sandbox"),
        "sandbox opt-out must come from the env var, never the flag list"
    );
}

/// The escape hatch is deliberately narrow: only the literal `1`
/// disables the sandbox, so a stray `0`, `true` or empty assignment can
/// never weaken it.
#[test]
fn sandbox_opt_out_only_accepts_literal_1() {
    assert!(!sandbox_opt_out(None));
    assert!(!sandbox_opt_out(Some("0")));
    assert!(!sandbox_opt_out(Some("true")));
    assert!(!sandbox_opt_out(Some("")));
    assert!(sandbox_opt_out(Some("1")));
}

#[test]
fn nested_runtime_blocks_on_a_separate_thread() {
    let outer = Runtime::new().expect("outer runtime");
    let inner = Runtime::new().expect("inner runtime");

    let result = outer.block_on(async {
        block_on_headless(&inner, async {
            Ok::<_, ControlledEmbedError>("nested runtime ok")
        })
    });

    assert_eq!(result.unwrap(), "nested runtime ok");
}

#[test]
fn wait_success_reads_after_the_element_appears() {
    let binding = binding_with_wait(
        BindingRole::Output,
        "result",
        "#result",
        binding_wait(
            Some("#ready"),
            BindingWaitCondition::Exists,
            200,
            0,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let operations = headless_operations(std::slice::from_ref(&binding), &[]);
    assert_eq!(operations.len(), 1);
    assert!(matches!(operations[0].kind, HeadlessOperationKind::Read));
    assert!(operations[0].script.contains("\"#result\""));

    let wait = operations[0].wait.as_ref().expect("read wait exists");
    let mut evaluator = SequenceWaitEvaluator::new([false, true]);
    let rt = Runtime::new().expect("runtime");

    rt.block_on(poll_headless_wait(wait, &mut evaluator))
        .expect("second readiness poll succeeds");

    assert_eq!(evaluator.calls, 2);
    assert_eq!(
        evaluator.scripts,
        vec![wait.script.clone(), wait.script.clone()]
    );
}

#[test]
fn wait_timeout_with_fail_policy_returns_wait_timeout_error() {
    let binding = binding_with_wait(
        BindingRole::Input,
        "query",
        "#query",
        binding_wait(
            Some("#ready"),
            BindingWaitCondition::Visible,
            DEFAULT_CONTROLLED_EMBED_WAIT_POLL_MS,
            0,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let wait = headless_wait_for_binding(&binding).expect("wait exists");
    let mut evaluator = SequenceWaitEvaluator::new([false, false]);
    let rt = Runtime::new().expect("runtime");

    let error = rt
        .block_on(poll_headless_wait(&wait, &mut evaluator))
        .expect_err("wait should time out");

    assert_eq!(error.code(), "wait-timeout");
    assert!(matches!(
        error,
        ControlledEmbedError::WaitTimeout {
            role: BindingRole::Input,
            selector,
            for_selector,
            condition: BindingWaitCondition::Visible,
            timeout_ms: DEFAULT_CONTROLLED_EMBED_WAIT_POLL_MS,
        } if selector == "#query" && for_selector == "#ready"
    ));
    assert!(evaluator.calls >= 1);
}

#[tokio::test]
async fn wait_timeout_of_one_ms_fails_immediately_and_reports_one_ms() {
    const ONE_MILLISECOND_TIMEOUT_MS: u64 = 1;

    let binding = binding_with_wait(
        BindingRole::Input,
        "query",
        "#query",
        binding_wait(
            Some("#ready"),
            BindingWaitCondition::Visible,
            ONE_MILLISECOND_TIMEOUT_MS,
            0,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let wait = headless_wait_for_binding(&binding).expect("wait exists");
    let mut evaluator = SequenceWaitEvaluator::new([false, true]);

    let error = poll_headless_wait(&wait, &mut evaluator)
        .await
        .expect_err("one millisecond timeout should fail before retry");

    assert_eq!(error.code(), "wait-timeout");
    assert!(matches!(
        error,
        ControlledEmbedError::WaitTimeout {
            role: BindingRole::Input,
            selector,
            for_selector,
            condition: BindingWaitCondition::Visible,
            timeout_ms: ONE_MILLISECOND_TIMEOUT_MS,
        } if selector == "#query" && for_selector == "#ready"
    ));
    assert_eq!(evaluator.calls, 1);
    assert_eq!(evaluator.scripts, vec![wait.script.clone()]);
}

#[tokio::test]
async fn wait_timeout_longer_than_poll_interval_retries_then_succeeds() {
    const RETRY_TIMEOUT_MS: u64 = DEFAULT_CONTROLLED_EMBED_WAIT_POLL_MS * 10;

    let binding = binding_with_wait(
        BindingRole::Output,
        "result",
        "#result",
        binding_wait(
            Some("#ready"),
            BindingWaitCondition::Exists,
            RETRY_TIMEOUT_MS,
            0,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let wait = headless_wait_for_binding(&binding).expect("wait exists");
    let mut evaluator = SequenceWaitEvaluator::new([false, true]);

    poll_headless_wait(&wait, &mut evaluator)
        .await
        .expect("second poll succeeds before timeout");

    assert_eq!(evaluator.calls, 2);
    assert_eq!(
        evaluator.scripts,
        vec![wait.script.clone(), wait.script.clone()]
    );
}

#[test]
fn wait_timeout_with_continue_policy_proceeds_with_the_operation() {
    let binding = binding_with_wait(
        BindingRole::Output,
        "result",
        "#missing-result",
        binding_wait(
            Some("#never"),
            BindingWaitCondition::Exists,
            0,
            0,
            BindingWaitOnTimeout::Continue,
        ),
    );
    let operations = headless_operations(std::slice::from_ref(&binding), &[]);
    let wait = operations[0].wait.as_ref().expect("wait exists");
    let mut evaluator = SequenceWaitEvaluator::new([false]);
    let rt = Runtime::new().expect("runtime");

    rt.block_on(poll_headless_wait(wait, &mut evaluator))
        .expect("continue policy does not fail");

    assert_eq!(evaluator.calls, 1);
    assert!(
        operations[0].script.contains("else{o[\"result\"]=''"),
        "output default must come from normal read script, not wait timeout"
    );
}

#[tokio::test]
async fn wait_timeout_with_continue_policy_skips_settle_ms() {
    const IMMEDIATE_TIMEOUT_MS: u64 = 0;
    const SETTLE_MS_THAT_WOULD_BLOCK_IF_USED: u64 = 60_000;

    let binding = binding_with_wait(
        BindingRole::Output,
        "result",
        "#missing-result",
        binding_wait(
            Some("#never"),
            BindingWaitCondition::Exists,
            IMMEDIATE_TIMEOUT_MS,
            SETTLE_MS_THAT_WOULD_BLOCK_IF_USED,
            BindingWaitOnTimeout::Continue,
        ),
    );
    let wait = headless_wait_for_binding(&binding).expect("wait exists");
    let mut evaluator = SequenceWaitEvaluator::new([false]);
    let poll_result = {
        let mut future = Box::pin(poll_headless_wait(&wait, &mut evaluator));
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);

        future.as_mut().poll(&mut context)
    };

    match poll_result {
        Poll::Ready(result) => result.expect("continue policy should complete immediately"),
        Poll::Pending => panic!("continue policy must not await settle_ms on timeout"),
    }
    assert_eq!(evaluator.calls, 1);
    assert_eq!(evaluator.scripts, vec![wait.script.clone()]);
}

#[test]
fn visible_wait_script_rejects_hidden_display_none_and_zero_opacity() {
    let binding = binding_with_wait(
        BindingRole::Trigger,
        "",
        "#submit",
        binding_wait(
            Some("#result"),
            BindingWaitCondition::Visible,
            200,
            0,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let wait = headless_wait_for_binding(&binding).expect("wait exists");

    assert!(wait.script.contains("condition==='visible'"));
    assert!(wait.script.contains("style.visibility!=='hidden'"));
    assert!(wait.script.contains("style.display!=='none'"));
    assert!(wait.script.contains("style.opacity!=='0'"));
}

#[test]
fn wait_success_sleeps_for_settle_ms() {
    const SETTLE_MS: u64 = 30;
    let binding = binding_with_wait(
        BindingRole::Input,
        "query",
        "#query",
        binding_wait(
            Some("#ready"),
            BindingWaitCondition::Exists,
            200,
            SETTLE_MS,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let wait = headless_wait_for_binding(&binding).expect("wait exists");
    let mut evaluator = SequenceWaitEvaluator::new([true]);
    let rt = Runtime::new().expect("runtime");
    let start = std::time::Instant::now();

    rt.block_on(poll_headless_wait(&wait, &mut evaluator))
        .expect("ready immediately");

    assert_eq!(evaluator.calls, 1);
    assert!(start.elapsed() >= Duration::from_millis(SETTLE_MS));
}

#[test]
fn wait_timeout_of_zero_checks_exactly_once() {
    let binding = binding_with_wait(
        BindingRole::Output,
        "result",
        "#result",
        binding_wait(
            Some("#ready"),
            BindingWaitCondition::Exists,
            0,
            0,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let wait = headless_wait_for_binding(&binding).expect("wait exists");
    let mut evaluator = SequenceWaitEvaluator::new([false, true]);
    let rt = Runtime::new().expect("runtime");

    let error = rt
        .block_on(poll_headless_wait(&wait, &mut evaluator))
        .expect_err("zero timeout should not poll again");

    assert_eq!(error.code(), "wait-timeout");
    assert_eq!(evaluator.calls, 1);
}

#[test]
fn trigger_settle_ms_prefers_manifest_wait_and_falls_back_to_default() {
    const TUNED_SETTLE_MS: u64 = 42;
    let tuned = binding_with_wait(
        BindingRole::Trigger,
        "",
        "#submit",
        binding_wait(
            Some("#ready"),
            BindingWaitCondition::Exists,
            200,
            TUNED_SETTLE_MS,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let tuned_wait = headless_wait_for_binding(&tuned).expect("wait exists");
    assert_eq!(trigger_settle_ms(Some(&tuned_wait)), TUNED_SETTLE_MS);

    // No manifest wait, or a zero settle, falls back to the default window.
    assert_eq!(trigger_settle_ms(None), DEFAULT_SETTLE_MS);
    let zero = binding_with_wait(
        BindingRole::Trigger,
        "",
        "#submit",
        binding_wait(
            Some("#ready"),
            BindingWaitCondition::Exists,
            200,
            0,
            BindingWaitOnTimeout::Fail,
        ),
    );
    let zero_wait = headless_wait_for_binding(&zero).expect("wait exists");
    assert_eq!(trigger_settle_ms(Some(&zero_wait)), DEFAULT_SETTLE_MS);
}

#[tokio::test]
async fn trigger_settle_skips_the_full_wait_once_dom_settles_after_navigation() {
    const SETTLE_MS_THAT_WOULD_BLOCK_IF_USED: u64 = 60_000;
    let mut evaluator = SequenceWaitEvaluator::new([true]);
    let start = std::time::Instant::now();

    settle_after_trigger(true, SETTLE_MS_THAT_WOULD_BLOCK_IF_USED, &mut evaluator)
        .await
        .expect("navigated + settled returns immediately");

    assert_eq!(evaluator.calls, 1, "one DOM-settle probe is enough");
    assert_eq!(evaluator.scripts, vec![DOM_SETTLE_SCRIPT.to_string()]);
    assert!(
        start.elapsed() < Duration::from_millis(1_000),
        "settled navigation must not pay the full settle window"
    );
}

#[tokio::test]
async fn trigger_settle_stops_at_the_settle_bound_even_if_dom_never_settles() {
    const BOUNDED_SETTLE_MS: u64 = 20;
    // Readiness queue empties to `false` forever — the DOM never settles.
    let mut evaluator = SequenceWaitEvaluator::new([false]);
    let start = std::time::Instant::now();

    settle_after_trigger(true, BOUNDED_SETTLE_MS, &mut evaluator)
        .await
        .expect("bounded settle completes even when DOM never settles");

    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(BOUNDED_SETTLE_MS));
    assert!(
        elapsed < Duration::from_secs(5),
        "slow page settle must stay bounded by settle_ms"
    );
    assert!(evaluator.calls >= 1, "the DOM-settle probe must run");
}

#[tokio::test]
async fn trigger_settle_retries_stale_context_evaluate_errors_then_succeeds() {
    const SETTLE_MS_THAT_WOULD_BLOCK_IF_USED: u64 = 60_000;
    // Right after a navigation CDP rejects evaluates against the destroyed
    // execution context with -32000. That error must read as "still
    // settling" and retry, not fail the whole pipeline.
    let stale_context_error = ControlledEmbedError::BackendFailed(
        "wait: Error -32000: Cannot find context with specified id".into(),
    );
    let mut evaluator = ResultSequenceWaitEvaluator::new([Err(stale_context_error), Ok(true)]);
    let start = std::time::Instant::now();

    settle_after_trigger(true, SETTLE_MS_THAT_WOULD_BLOCK_IF_USED, &mut evaluator)
        .await
        .expect("stale-context error must be retried, not propagated");

    assert_eq!(
        evaluator.calls, 2,
        "error poll then successful poll — exactly one retry"
    );
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "recovery must happen within poll cadence, not the full settle window"
    );
}

#[tokio::test]
async fn trigger_settle_stops_at_the_settle_bound_even_if_errors_persist() {
    const BOUNDED_SETTLE_MS: u64 = 20;
    // Queue empties to Ok(false) after the first error, but seed errors to
    // prove a permanently broken context cannot hang or fail the settle.
    let always_stale = ControlledEmbedError::BackendFailed(
        "wait: Error -32000: Cannot find context with specified id".into(),
    );
    let mut evaluator =
        ResultSequenceWaitEvaluator::new([Err(always_stale.clone()), Err(always_stale)]);
    let start = std::time::Instant::now();

    settle_after_trigger(true, BOUNDED_SETTLE_MS, &mut evaluator)
        .await
        .expect("persistent evaluate errors settle at the bound, not an Err");

    assert!(start.elapsed() >= Duration::from_millis(BOUNDED_SETTLE_MS));
    assert!(evaluator.calls >= 1);
}

#[tokio::test]
async fn trigger_settle_without_navigation_sleeps_the_bound_without_polling_dom() {
    const BOUNDED_SETTLE_MS: u64 = 20;
    // Even a ready DOM must not short-circuit the fallback constant wait.
    let mut evaluator = SequenceWaitEvaluator::new([true]);
    let start = std::time::Instant::now();

    settle_after_trigger(false, BOUNDED_SETTLE_MS, &mut evaluator)
        .await
        .expect("slow-page fallback completes");

    assert!(start.elapsed() >= Duration::from_millis(BOUNDED_SETTLE_MS));
    assert_eq!(
        evaluator.calls, 0,
        "unsettled navigation uses the constant sleep, not a DOM poll"
    );
}
