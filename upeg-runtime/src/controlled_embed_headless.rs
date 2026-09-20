//! Headless Controlled Embed backend, gated behind `controlled-embed`.
//!
//! Drives a real Chrome/Chromium/Edge via the Chrome DevTools Protocol
//! (chromiumoxide). The same `selector_pipeline::ExecutionPlan` that
//! the Flutter live-WebView path uses; one source of truth for the
//! generated JavaScript.
//!
//! ## SoC
//! The trait (`ControlledEmbedBackend`) is sync — that lets every
//! surface (CLI dispatcher, MCP/HTTP handlers, even sync test code)
//! invoke a Controlled Embed tool without an async runtime in scope.
//! chromiumoxide is async, so this impl owns a private
//! `tokio::runtime::Runtime` and `block_on`s the whole pipeline. The
//! runtime is created once per backend instance (lazy `OnceCell`) and
//! reused across calls — fork-per-call would be wasteful and a new
//! `Browser::launch` already spins up a fresh chromium process anyway.
//!
//! ## Sandbox + flags
//! Chrome's sandbox stays ON by default. This backend renders
//! third-party pages, which is exactly what the sandbox exists for
//! (security absolutes #14: embeds run in a separate sandbox);
//! `chromiumoxide::BrowserConfig` defaults to `sandbox: true` and
//! [`HEADLESS_LAUNCH_ARGS`] never opts out. The only override is
//! [`CONTROLLED_EMBED_NO_SANDBOX_ENV`] set to `1`, reserved for
//! environments where Chrome cannot sandbox itself at all — CI
//! containers running as root. Never set it on a normal desktop. With
//! it unset there is no weaker retry: a sandboxless environment fails
//! the launch with Chrome's own error.
//! - `--headless=new` — modern Chrome headless mode (paint, JS,
//!   timers all behave like a real session, unlike `--headless=old`).
//! - `--disable-gpu` — headless renderers don't have a GPU context.
//! - `--disable-dev-shm-usage` — write shared memory under `/tmp`, not
//!   the often-tiny `/dev/shm` of a container.
//!
//! ## Settle window
//! The page's "result is ready" signal is page-specific. Without a
//! standard event we wait a small fixed delay (default 500 ms) after
//! the Trigger fires before reading Output selectors. This is the
//! same heuristic the GUI runner uses. Tool manifests can now tune
//! per-binding waits with `controlled_embed.bindings[].wait` fields such
//! as `for_selector`, `condition`, `timeout_ms`, and `settle_ms`.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::OnceLock;
use std::time::Duration;

use chromiumoxide::browser::BrowserConfig;
use chromiumoxide::{Browser, Page};
use futures_util::StreamExt;
use tokio::runtime::Runtime;
use upeg_core::{
    BindingRole, BindingWaitCondition, BindingWaitOnTimeout, ControlledEmbedSettings,
    ControlledEmbedUserAgent, DEFAULT_CONTROLLED_EMBED_WAIT_POLL_MS, SelectorBinding,
};

use crate::controlled_embed::{
    ControlledEmbedBackend, ControlledEmbedError, ControlledEmbedRequest, ControlledEmbedResponse,
    discover_system_browser,
};
use crate::selector_pipeline::ExecutionPlan;

/// Default settle window between Trigger and Output read. Page-
/// specific tuning would belong on the tool manifest if it becomes a
/// real need; for now the fixed window matches the GUI runner.
const DEFAULT_SETTLE_MS: u64 = 500;

/// Flags every headless launch adds on top of chromiumoxide's defaults.
/// Manifest-driven flags (`controlled_embed_launch_args`) come after
/// these. Nothing in this list may weaken Chrome's sandbox — see the
/// module doc.
const HEADLESS_LAUNCH_ARGS: [&str; 3] =
    ["--headless=new", "--disable-gpu", "--disable-dev-shm-usage"];

/// Opt-out env var for Chrome's sandbox. Only the literal `1` counts.
/// Exists for environments where Chrome cannot sandbox itself (CI
/// containers running as root); never set it on a normal desktop — see
/// the module doc.
pub const CONTROLLED_EMBED_NO_SANDBOX_ENV: &str = "UPEG_CONTROLLED_EMBED_NO_SANDBOX";

fn sandbox_opt_out(value: Option<&str>) -> bool {
    value == Some("1")
}

pub const CONTROLLED_EMBED_MOBILE_SAFARI_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 \
Mobile/15E148 Safari/604.1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeadlessOperationKind {
    Write,
    Trigger,
    Read,
}

impl HeadlessOperationKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Trigger => "trigger",
            Self::Read => "read",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HeadlessBindingWait {
    role: BindingRole,
    selector: String,
    for_selector: String,
    condition: BindingWaitCondition,
    timeout_ms: u64,
    settle_ms: u64,
    on_timeout: BindingWaitOnTimeout,
    script: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HeadlessOperation {
    kind: HeadlessOperationKind,
    wait: Option<HeadlessBindingWait>,
    script: String,
}

#[derive(Clone, Debug)]
struct HeadlessExecution {
    executable: PathBuf,
    url: String,
    launch_args: Vec<String>,
    operations: Vec<HeadlessOperation>,
}

trait HeadlessWaitEvaluator {
    fn evaluate_wait<'a>(
        &'a mut self,
        script: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool, ControlledEmbedError>> + Send + 'a>>;
}

struct ChromiumPageWaitEvaluator<'a> {
    page: &'a Page,
}

impl HeadlessWaitEvaluator for ChromiumPageWaitEvaluator<'_> {
    fn evaluate_wait<'a>(
        &'a mut self,
        script: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool, ControlledEmbedError>> + Send + 'a>> {
        Box::pin(async move { evaluate_wait_script(self.page, script).await })
    }
}

fn controlled_embed_launch_args(settings: &ControlledEmbedSettings) -> Vec<String> {
    let mut args = Vec::with_capacity(2);
    if let Some(user_agent) = &settings.user_agent {
        match user_agent {
            ControlledEmbedUserAgent::Default => {}
            ControlledEmbedUserAgent::MobileSafari => args.push(format!(
                "--user-agent={CONTROLLED_EMBED_MOBILE_SAFARI_USER_AGENT}"
            )),
            ControlledEmbedUserAgent::Custom(user_agent) => {
                args.push(format!("--user-agent={user_agent}"));
            }
        }
    }
    if let Some(viewport) = &settings.viewport {
        let (width, height) = viewport.dimensions();
        args.push(format!("--window-size={width},{height}"));
    }
    args
}

/// chromiumoxide-backed Controlled Embed backend. One instance per
/// binary; chromiumoxide launches a fresh browser process per call.
pub struct HeadlessControlledEmbedBackend {
    rt: OnceLock<Runtime>,
}

impl Default for HeadlessControlledEmbedBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadlessControlledEmbedBackend {
    pub fn new() -> Self {
        Self {
            rt: OnceLock::new(),
        }
    }

    fn runtime(&self) -> Result<&Runtime, ControlledEmbedError> {
        if let Some(rt) = self.rt.get() {
            return Ok(rt);
        }
        let rt = Runtime::new()
            .map_err(|e| ControlledEmbedError::BackendFailed(format!("tokio runtime: {e}")))?;
        // Set-if-empty; race-loser drops the duplicate runtime, no
        // double-init reachable from the trait's sync contract.
        Ok(self.rt.get_or_init(|| rt))
    }
}

impl ControlledEmbedBackend for HeadlessControlledEmbedBackend {
    fn run(
        &self,
        request: ControlledEmbedRequest<'_>,
    ) -> Result<ControlledEmbedResponse, ControlledEmbedError> {
        let executable = discover_system_browser()?.executable;
        let pipeline = HeadlessExecution {
            executable,
            url: request.url.to_string(),
            launch_args: controlled_embed_launch_args(&request.settings),
            operations: headless_operations(request.bindings, request.inputs),
        };

        let rt = self.runtime()?;
        block_on_headless(rt, run_headless_pipeline(pipeline))
    }
}

fn block_on_headless<F, T>(rt: &Runtime, future: F) -> Result<T, ControlledEmbedError>
where
    F: Future<Output = Result<T, ControlledEmbedError>> + Send,
    T: Send,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        return std::thread::scope(|scope| {
            scope
                .spawn(move || rt.block_on(future))
                .join()
                .unwrap_or_else(|_| {
                    Err(ControlledEmbedError::BackendFailed(
                        "headless worker thread panicked".into(),
                    ))
                })
        });
    }
    rt.block_on(future)
}

fn headless_operations(
    bindings: &[SelectorBinding],
    inputs: &[(&str, &str)],
) -> Vec<HeadlessOperation> {
    let mut operations = Vec::new();

    for binding in bindings
        .iter()
        .filter(|binding| binding.role == BindingRole::Input)
    {
        let Some(&(_, value)) = inputs
            .iter()
            .find(|(field, _)| *field == binding.field.as_str())
        else {
            continue;
        };
        let binding_inputs = [(binding.field.as_str(), value)];
        let script =
            ExecutionPlan::build(std::slice::from_ref(binding), &binding_inputs).write_script();
        if !script.is_empty() {
            operations.push(HeadlessOperation {
                kind: HeadlessOperationKind::Write,
                wait: headless_wait_for_binding(binding),
                script,
            });
        }
    }

    if let Some(binding) = bindings
        .iter()
        .find(|binding| binding.role == BindingRole::Trigger)
    {
        let script = ExecutionPlan::build(std::slice::from_ref(binding), &[]).trigger_script();
        if !script.is_empty() {
            operations.push(HeadlessOperation {
                kind: HeadlessOperationKind::Trigger,
                wait: headless_wait_for_binding(binding),
                script,
            });
        }
    }

    for binding in bindings
        .iter()
        .filter(|binding| binding.role == BindingRole::Output)
    {
        let script = ExecutionPlan::build(std::slice::from_ref(binding), &[]).read_script();
        operations.push(HeadlessOperation {
            kind: HeadlessOperationKind::Read,
            wait: headless_wait_for_binding(binding),
            script,
        });
    }

    operations
}

fn headless_wait_for_binding(binding: &SelectorBinding) -> Option<HeadlessBindingWait> {
    let wait = binding.wait.as_ref()?;
    let script = ExecutionPlan::build(std::slice::from_ref(binding), &[]).wait_script();
    if script.is_empty() {
        return None;
    }
    Some(HeadlessBindingWait {
        role: binding.role,
        selector: binding.selector.clone(),
        for_selector: wait
            .for_selector
            .clone()
            .unwrap_or_else(|| binding.selector.clone()),
        condition: wait.condition,
        timeout_ms: wait.timeout_ms,
        settle_ms: wait.settle_ms,
        on_timeout: wait.on_timeout,
        script,
    })
}

async fn poll_headless_wait<E: HeadlessWaitEvaluator>(
    wait: &HeadlessBindingWait,
    evaluator: &mut E,
) -> Result<(), ControlledEmbedError> {
    if evaluator.evaluate_wait(&wait.script).await? {
        settle_headless_wait(wait).await;
        return Ok(());
    }

    if wait.timeout_ms > 0 {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(wait.timeout_ms);
        loop {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                break;
            }
            tokio::time::sleep(
                deadline
                    .saturating_duration_since(now)
                    .min(Duration::from_millis(DEFAULT_CONTROLLED_EMBED_WAIT_POLL_MS)),
            )
            .await;
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            if evaluator.evaluate_wait(&wait.script).await? {
                settle_headless_wait(wait).await;
                return Ok(());
            }
        }
    }

    match wait.on_timeout {
        BindingWaitOnTimeout::Continue => Ok(()),
        BindingWaitOnTimeout::Fail => Err(wait_timeout_error(wait)),
    }
}

async fn settle_headless_wait(wait: &HeadlessBindingWait) {
    if wait.settle_ms != 0 {
        tokio::time::sleep(Duration::from_millis(wait.settle_ms)).await;
    }
}

/// DOM-quiescence probe evaluated after a Trigger navigation completes. Once
/// `document.readyState` is `complete` the page is ready for Output reads, so
/// the pipeline can proceed without a blind full-window sleep.
const DOM_SETTLE_SCRIPT: &str = "document.readyState === 'complete'";

/// Post-Trigger settle budget. A trigger binding can tune it through its
/// manifest `wait.settle_ms`; otherwise the fixed [`DEFAULT_SETTLE_MS`] applies.
fn trigger_settle_ms(wait: Option<&HeadlessBindingWait>) -> u64 {
    match wait {
        Some(wait) if wait.settle_ms != 0 => wait.settle_ms,
        _ => DEFAULT_SETTLE_MS,
    }
}

/// Settle after a Trigger fires, event-driven rather than a blind sleep.
///
/// When `navigated` is true (the page completed a navigation within the bounded
/// window) we poll the DOM and return the instant it reports settled, so a fast
/// page pays no fixed delay. The poll is bounded by `settle_ms`, so a
/// never-settling page cannot hang the pipeline. When navigation never settled
/// (slow page) we fall back to a single bounded `settle_ms` wait.
///
/// Evaluate errors during the poll are treated as "not settled yet", not
/// propagated: right after a navigation the previous execution context is
/// destroyed and CDP answers `-32000: Cannot find context with specified id`
/// until the new document's context is ready. A stale context is itself a
/// signal that the page is still settling, so we retry within the same
/// `settle_ms` bound instead of failing the pipeline.
async fn settle_after_trigger<E: HeadlessWaitEvaluator>(
    navigated: bool,
    settle_ms: u64,
    evaluator: &mut E,
) -> Result<(), ControlledEmbedError> {
    if settle_ms == 0 {
        return Ok(());
    }
    if !navigated {
        tokio::time::sleep(Duration::from_millis(settle_ms)).await;
        return Ok(());
    }
    let deadline = tokio::time::Instant::now() + Duration::from_millis(settle_ms);
    loop {
        if evaluator.evaluate_wait(DOM_SETTLE_SCRIPT).await == Ok(true) {
            return Ok(());
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(());
        }
        tokio::time::sleep(
            deadline
                .saturating_duration_since(now)
                .min(Duration::from_millis(DEFAULT_CONTROLLED_EMBED_WAIT_POLL_MS)),
        )
        .await;
    }
}

fn wait_timeout_error(wait: &HeadlessBindingWait) -> ControlledEmbedError {
    ControlledEmbedError::WaitTimeout {
        role: wait.role,
        selector: wait.selector.clone(),
        for_selector: wait.for_selector.clone(),
        condition: wait.condition,
        timeout_ms: wait.timeout_ms,
    }
}

async fn wait_for_headless_binding(
    page: &Page,
    wait: &HeadlessBindingWait,
) -> Result<(), ControlledEmbedError> {
    let mut evaluator = ChromiumPageWaitEvaluator { page };
    poll_headless_wait(wait, &mut evaluator).await
}

async fn evaluate_wait_script(page: &Page, script: &str) -> Result<bool, ControlledEmbedError> {
    let raw = page
        .evaluate(script)
        .await
        .map_err(|e| ControlledEmbedError::BackendFailed(format!("wait: {e}")))?;
    raw.into_value()
        .map_err(|e| ControlledEmbedError::BackendFailed(format!("wait-value: {e}")))
}

async fn run_headless_pipeline(
    pipeline: HeadlessExecution,
) -> Result<ControlledEmbedResponse, ControlledEmbedError> {
    let HeadlessExecution {
        executable,
        url,
        launch_args,
        operations,
    } = pipeline;
    let mut config_builder = BrowserConfig::builder().chrome_executable(executable);
    for arg in HEADLESS_LAUNCH_ARGS {
        config_builder = config_builder.arg(arg);
    }
    if sandbox_opt_out(
        std::env::var(CONTROLLED_EMBED_NO_SANDBOX_ENV)
            .ok()
            .as_deref(),
    ) {
        config_builder = config_builder.arg("--no-sandbox");
    }
    for arg in launch_args {
        config_builder = config_builder.arg(arg);
    }
    let config = config_builder
        .build()
        .map_err(ControlledEmbedError::BackendFailed)?;

    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| ControlledEmbedError::BackendFailed(format!("launch: {e}")))?;

    // chromiumoxide's `Browser` requires the handler stream to be polled or
    // all CDP responses stall. Drain it until the browser closes or errors.
    let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

    let result = run_headless_work(&browser, &url, &operations).await;

    // Best-effort cleanup. Errors here are not propagated — the work result is
    // what the caller actually wants and chrome exits when the launcher drops.
    let _ = browser.close().await;
    let _ = handler_task.await;

    result.map(|outputs| ControlledEmbedResponse { outputs })
}

async fn run_headless_work(
    browser: &Browser,
    url: &str,
    operations: &[HeadlessOperation],
) -> Result<Vec<(String, String)>, ControlledEmbedError> {
    let page = browser
        .new_page(url)
        .await
        .map_err(|e| ControlledEmbedError::BackendFailed(format!("new_page: {e}")))?;
    page.wait_for_navigation()
        .await
        .map_err(|e| ControlledEmbedError::BackendFailed(format!("nav: {e}")))?;

    let mut outputs = Vec::new();
    for operation in operations {
        if let Some(wait) = &operation.wait {
            wait_for_headless_binding(&page, wait).await?;
        }
        match operation.kind {
            HeadlessOperationKind::Write => {
                page.evaluate(operation.script.as_str())
                    .await
                    .map_err(|e| {
                        ControlledEmbedError::BackendFailed(format!(
                            "{}: {e}",
                            operation.kind.label()
                        ))
                    })?;
            }
            HeadlessOperationKind::Trigger => {
                page.evaluate(operation.script.as_str())
                    .await
                    .map_err(|e| {
                        ControlledEmbedError::BackendFailed(format!(
                            "{}: {e}",
                            operation.kind.label()
                        ))
                    })?;
                // Event-driven settle: race the trigger's navigation against a
                // bounded window instead of always sleeping the full delay.
                let navigated = tokio::time::timeout(
                    Duration::from_millis(DEFAULT_SETTLE_MS),
                    page.wait_for_navigation(),
                )
                .await
                .is_ok();
                let settle_ms = trigger_settle_ms(operation.wait.as_ref());
                let mut evaluator = ChromiumPageWaitEvaluator { page: &page };
                settle_after_trigger(navigated, settle_ms, &mut evaluator).await?;
            }
            HeadlessOperationKind::Read => {
                let raw = page
                    .evaluate(operation.script.as_str())
                    .await
                    .map_err(|e| {
                        ControlledEmbedError::BackendFailed(format!(
                            "{}: {e}",
                            operation.kind.label()
                        ))
                    })?;
                let json_str: String = raw.into_value().map_err(|e| {
                    ControlledEmbedError::BackendFailed(format!(
                        "{}-value: {e}",
                        operation.kind.label()
                    ))
                })?;
                let read_outputs: std::collections::HashMap<String, String> =
                    serde_json::from_str(&json_str).map_err(|e| {
                        ControlledEmbedError::BackendFailed(format!(
                            "{}-parse: {e}",
                            operation.kind.label()
                        ))
                    })?;
                outputs.extend(read_outputs);
            }
        }
    }
    Ok(outputs)
}

#[cfg(test)]
mod tests;
