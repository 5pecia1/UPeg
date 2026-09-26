//! Chain invoker dispatcher: step ordering, the `{{ }}` expression
//! resolver, and the per-run bookkeeping every chain result carries.
//!
//! The pieces that answer a question of their own live beside it:
//!
//! * [`approval`] — which surfaces may satisfy a `requires_approval`
//!   step, and the typed refusal when the caller's surface may not.
//! * [`output`] — [`OutputAdapter`], the chain's declared-output
//!   contract, shared with the other invokers in `dispatcher.rs`.
//! * [`summary`] — the per-step metadata (`ran`/`skipped`/`failed`/
//!   `denied` + duration) that rides out on every envelope.
//!
//! `render_template` stays here because it is the chain's own
//! expression language, and the other invokers reuse it through the
//! parent module's re-export.
//!
//! # Contract
//!
//! A Chain is a Tool with `invoker = "Chain"` — pinned, listed,
//! filtered, logged, triggered, and dispatched like every other Tool;
//! the engine's complexity stays sealed inside.
//!
//! # Runtime model
//!
//! `steps` declares the node instances and `connections =
//! [{ from, to }]` the directed edges — linear chains, fan-out, and
//! joins all use the same list. A node with no incoming connection
//! receives the chain input. Each step references a Tool id and may
//! declare `args` (a JSON object holding `{{ }}` expressions), `when`
//! (a boolean expression — false skips the step), and
//! `requires_approval` (a barrier judged by [`approval`]).
//!
//! # Expression grammar (deliberately small)
//!
//! * `{{input.key}}` — a top-level call argument.
//! * `{{steps.<id>.output}}` — an earlier step's text output.
//! * `{{steps.<id>.ok}}` — whether an earlier step succeeded.
//! * `{{context.board}}` — the reserved `_upeg` context.
//!
//! A bad reference is a deterministic Tool error, logged without the
//! value. Nothing beyond `{{ }}` substitution is supported.
//!
//! # Execution rules
//!
//! 1. Duplicate node ids, unknown connection endpoints, and cycles are
//!    rejected at load time.
//! 2. Run each ready step whose upstream connections are satisfied.
//! 3. Stop at a failed required step.
//! 4. Each step's outcome rides out in the result envelope — see
//!    [`summary`].
//! 5. An approval step is honored only when the caller's principal and
//!    surface both pass the two authorization gates — see [`approval`]
//!    and `upeg_runtime::approval`.
//! 6. The final output is the last completed step's output unless an
//!    `output` expression is set.
use super::{OUTPUT_CONVERSION_ERROR_CODE, TOOL_ERROR_CODE, credentials};
use crate::ToolToml;
use crate::model::{ChainStepToml, CredentialRefToml, chain_step_key};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use upeg_core::{ToolResult, ToolSuccess};
use upeg_runtime::DispatchArgs;

mod approval;
mod output;
mod summary;

pub(crate) use approval::{
    ApprovalSurfaces, ApprovalSurfacesError, DEFAULT_APPROVAL_SURFACES, surface_label_list,
};
pub(crate) use output::OutputAdapter;
// Referenced only by `chain_output_budget_tests.rs`, which is mounted
// into this module and reaches it as `super::output_value_from_text`.
#[cfg(test)]
pub(crate) use output::output_value_from_text;
pub(crate) use summary::CHAIN_STEPS_OUTPUT_ID;
use summary::{
    StepClock, StepStatus, StepSummaries, UNDISPATCHED_STEP_DURATION_MS, is_engine_summary_row,
};

/// Build a runtime dispatcher closure for a Chain Tool.
/// Returns `None` when the toml didn't declare `steps`.
///
/// Step 1 is invoked with the chain's own args. Step N+1 is invoked with
/// `{"input": <step N output>}`. The first step that errors short-circuits
/// the chain; the resulting message is prefixed with `step \`<id>\`:` so
/// users can see which link failed.
///
/// Resolution goes through `try_runtime_dispatch` only — built-in tools
/// must already be registered there, which `dispatch_tool` does on its
/// first call (via `upeg_tools::dispatch_registered` → `register_all`).
/// In practice every callable surface goes through `dispatch_tool`, so by
/// the time a chain dispatcher fires, the built-ins are present.
/// Iter 171: depth ceiling for chain → chain → ... recursion. Real
/// chains are 2-5 steps; this limit is large enough to never bite a
/// legitimate composition while small enough that a cyclic
/// configuration (`A → B → A`) blows the limit before blowing the
/// thread stack. Tracked in a thread-local so each top-level call
/// gets a fresh budget; nested chain dispatchers (the recursive
/// path) share and decrement the same counter.
pub(crate) const MAX_CHAIN_DEPTH: usize = 32;

thread_local! {
    static CHAIN_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn chain_dispatcher_for(
    parsed: &ToolToml,
) -> Option<impl for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static> {
    // Before any early return, not only on the path that publishes a
    // policy: re-registering an id is a *replace*, and a tool that comes
    // back as `External` — or as a Chain whose gated step was deleted —
    // must stop advertising the barrier the previous registration
    // published. Leaving the stale record would make every UI confirm a
    // run nothing gates, and `ToolApprovalPolicy::honors` would name
    // approvers for a tool with nothing to approve.
    upeg_runtime::set_tool_approval_policy(&parsed.id, upeg_runtime::ToolApprovalPolicy::none());
    if parsed.invoker.as_deref().map(str::trim) != Some("Chain") {
        return None;
    }
    let steps = normalize_chain_steps(parsed)?;
    if steps.is_empty() {
        return None;
    }
    let final_output = parsed.output.clone();
    let credentials = parsed.credentials.clone().unwrap_or_default();
    let output_adapter = OutputAdapter::from_tool(parsed);
    // Held as a `Result` for the same reason `OutputAdapter::fields` is:
    // the loader rejects a bad declaration at parse time, so reaching a
    // dispatcher with one means the manifest never went through the
    // loader (a hand-built `ToolToml` in a test). Failing the call says
    // so instead of silently falling back to a default that would
    // authorize surfaces the author did not name.
    let approval = ApprovalSurfaces::parse(&parsed.id, parsed.approval_surfaces.as_deref())
        .map_err(|e| e.describe());
    // Publish what a UI needs to know *before* it dispatches: whether
    // this chain stops at a human-approval barrier, and which surfaces
    // may lift it. A chain with no gated step publishes
    // `ToolApprovalPolicy::none()`, which is what the clear above
    // already wrote — the same replace-semantics the embed-url and
    // selector-binding registries use when a manifest is reloaded.
    upeg_runtime::set_tool_approval_policy(
        &parsed.id,
        match (steps.iter().any(|step| step.requires_approval), &approval) {
            (true, Ok(approval)) => approval.policy(),
            // A declaration the loader would have rejected authorizes
            // nobody; advertising a surface set here would promise an
            // approver for a call that fails before reaching one.
            (true, Err(_)) | (false, _) => upeg_runtime::ToolApprovalPolicy::none(),
        },
    );

    Some(move |args: DispatchArgs<'_>| -> ToolResult {
        execute_chain(
            &steps,
            final_output.as_deref(),
            &credentials,
            args.as_value(),
            &output_adapter,
            approval.as_ref(),
        )
    })
}

/// A chain run that ended in the canonical failure envelope, carrying the
/// error code the surface should report rather than collapsing every
/// stop into one generic tool error.
///
/// `code` is an owned `String` rather than a `&'static str` because a
/// failing step's own code is one of the codes a chain reports: a nested
/// chain that refuses a gated step must still read as
/// `approval_denied_for_surface` from the outside, not as a generic
/// `tool_error` that tells an agent to try again.
#[derive(Debug)]
pub(super) struct ChainError {
    code: String,
    message: String,
}

impl From<String> for ChainError {
    /// The default for every internal `?`: a template, dependency, or
    /// credential problem is an ordinary tool error.
    fn from(message: String) -> Self {
        Self {
            code: TOOL_ERROR_CODE.to_string(),
            message,
        }
    }
}

use upeg_runtime::{output_value_canonical_wire_text, tool_success_primary_canonical_wire_text};

struct DepthGuard;

impl Drop for DepthGuard {
    fn drop(&mut self) {
        CHAIN_DEPTH.with(|c| c.set(c.get().saturating_sub(1)));
    }
}

fn enter_chain_depth() -> Result<DepthGuard, ChainError> {
    let depth = CHAIN_DEPTH.with(|c| {
        let next = c.get() + 1;
        c.set(next);
        next
    });
    if depth > MAX_CHAIN_DEPTH {
        let _guard = DepthGuard;
        return Err(format!(
            "chain depth exceeded {MAX_CHAIN_DEPTH} — likely a cyclic chain \
             (e.g. tool A's chain contains B whose chain contains A)"
        )
        .into());
    }
    Ok(DepthGuard)
}

/// Run the chain and stamp the step summary onto whichever envelope
/// comes out. Both arms carry the same array so a caller reads step
/// metadata from one place regardless of how the run ended:
/// `outputs[steps]` on success, `error.details.steps` on failure.
fn execute_chain(
    steps: &[RuntimeStep],
    final_output: Option<&str>,
    credentials: &[CredentialRefToml],
    args: &Value,
    output_adapter: &OutputAdapter,
    approval: Result<&ApprovalSurfaces, &String>,
) -> ToolResult {
    let mut state = ChainState::new(steps.len());
    match run_chain(
        &mut state,
        steps,
        final_output,
        credentials,
        args,
        output_adapter,
        approval,
    ) {
        Ok(result) => with_step_summary(result, &state.summaries),
        Err(error) => upeg_runtime::tool_failure_with_details(
            error.code,
            error.message,
            json!({ CHAIN_STEPS_OUTPUT_ID: state.summaries.to_json() }),
        ),
    }
}

fn run_chain(
    state: &mut ChainState,
    steps: &[RuntimeStep],
    final_output: Option<&str>,
    credentials: &[CredentialRefToml],
    args: &Value,
    output_adapter: &OutputAdapter,
    approval: Result<&ApprovalSurfaces, &String>,
) -> Result<ToolResult, ChainError> {
    let _guard = enter_chain_depth()?;
    let approval = approval.map_err(Clone::clone)?;
    let creds = credentials::credential_map(credentials, None)?;

    while !state.remaining.is_empty() {
        let ready = state.ready_steps(steps);
        if ready.is_empty() {
            return Err("chain dependency cycle or missing dependency"
                .to_string()
                .into());
        }
        for job in build_chain_jobs(ready, steps, state, args, &creds, approval)? {
            state.apply_result(dispatch_chain_job(job))?;
        }
    }

    match final_output {
        Some(output) => Ok(output_adapter.text_result(Ok(render_template(
            output,
            args,
            Some(&state.step_outputs),
            &creds,
        )?))),
        None => state.last_step_result(steps, output_adapter),
    }
}

/// Append the step summary to a success, or fold it into a failure's
/// `details`.
///
/// A chain that produced no output row of its own gets `steps` as its
/// primary: an action-only chain still answers "what did you do", and
/// the canonical envelope has no way to carry an output row without a
/// primary id.
fn with_step_summary(result: ToolResult, summaries: &StepSummaries) -> ToolResult {
    match result {
        ToolResult::Success(mut success) => {
            if success
                .outputs
                .iter()
                .any(|entry| entry.id == CHAIN_STEPS_OUTPUT_ID)
            {
                return upeg_runtime::tool_failure(
                    CHAIN_STEP_SUMMARY_CONFLICT_ERROR_CODE,
                    format!(
                        "chain result already carries an output named `{CHAIN_STEPS_OUTPUT_ID}`, \
                         which the chain engine reserves for its per-step summary — declare \
                         `outputs` on the chain to rename the step's own row"
                    ),
                );
            }
            let primary = success
                .primary_output_id
                .unwrap_or_else(|| CHAIN_STEPS_OUTPUT_ID.to_string());
            success.outputs.push(summaries.output_entry());
            ToolSuccess::new(Some(primary), success.outputs)
                .map(ToolResult::Success)
                .unwrap_or_else(|error| {
                    upeg_runtime::tool_failure(OUTPUT_CONVERSION_ERROR_CODE, error.to_string())
                })
        }
        ToolResult::Failure(failure) => upeg_runtime::tool_failure_with_details(
            failure.error.code,
            failure.error.message,
            json!({ CHAIN_STEPS_OUTPUT_ID: summaries.to_json() }),
        ),
    }
}

/// The engine's own `steps` row cannot be added because the final step
/// already produced an output under that id.
const CHAIN_STEP_SUMMARY_CONFLICT_ERROR_CODE: &str = "chain_step_summary_conflict";

/// Drop the step summary an inner chain engine appended, so an outer
/// chain that ends on a nested chain can append its own.
///
/// A chain with no `output` expression returns its final step's success
/// verbatim. When that step is itself a chain, the success already
/// carries the engine-owned `steps` row — and
/// [`with_step_summary`] would refuse the whole run with
/// `chain_step_summary_conflict` for a name the author never wrote.
/// Every chain answers for its OWN steps, so the inner summary is that
/// step's business and folds away here; the outer row records the
/// nested chain as one step, the way the manifest declares it.
///
/// Only a row [`is_engine_summary_row`] recognizes folds. A row merely
/// *named* `steps` still reaches the conflict and is reported.
fn fold_inherited_step_summary(mut success: ToolSuccess) -> Result<ToolSuccess, ChainError> {
    let Some(index) = success.outputs.iter().position(is_engine_summary_row) else {
        return Ok(success);
    };
    success.outputs.remove(index);
    // The inner chain made the engine row its primary only when it had no
    // output of its own (`with_step_summary`), which leaves nothing behind
    // — and a success with no outputs carries no primary id.
    let primary = success
        .primary_output_id
        .filter(|id| id != CHAIN_STEPS_OUTPUT_ID);
    ToolSuccess::new(primary, success.outputs).map_err(|error| ChainError {
        code: OUTPUT_CONVERSION_ERROR_CODE.to_string(),
        message: error.to_string(),
    })
}

struct ChainState {
    completed: BTreeSet<String>,
    skipped: BTreeSet<String>,
    step_outputs: BTreeMap<String, StepState>,
    remaining: BTreeSet<usize>,
    summaries: StepSummaries,
}

impl ChainState {
    fn new(step_count: usize) -> Self {
        Self {
            completed: BTreeSet::new(),
            skipped: BTreeSet::new(),
            step_outputs: BTreeMap::new(),
            remaining: (0..step_count).collect(),
            summaries: StepSummaries::default(),
        }
    }

    fn ready_steps(&self, steps: &[RuntimeStep]) -> Vec<usize> {
        self.remaining
            .iter()
            .copied()
            .filter(|i| {
                steps[*i]
                    .upstream
                    .iter()
                    .all(|dep| self.completed.contains(dep) || self.skipped.contains(dep))
            })
            .collect()
    }

    fn skip(&mut self, step: &RuntimeStep) {
        self.skipped.insert(step.key.clone());
        self.summaries.record(
            &step.key,
            &step.tool,
            StepStatus::Skipped,
            UNDISPATCHED_STEP_DURATION_MS,
        );
        self.step_outputs.insert(
            step.key.clone(),
            StepState {
                success: None,
                ok: false,
                skipped: true,
            },
        );
    }

    fn deny(&mut self, step: &RuntimeStep) {
        self.summaries.record(
            &step.key,
            &step.tool,
            StepStatus::Denied,
            UNDISPATCHED_STEP_DURATION_MS,
        );
    }

    fn apply_result(&mut self, result: ChainJobResult) -> Result<(), ChainError> {
        let status = match &result.result {
            Some(ToolResult::Success(_)) => StepStatus::Ran,
            Some(ToolResult::Failure(_)) | None => StepStatus::Failed,
        };
        self.summaries
            .record(&result.key, &result.tool, status, result.duration_ms);
        match result.result {
            Some(ToolResult::Success(success)) => {
                self.completed.insert(result.key.clone());
                self.step_outputs.insert(
                    result.key,
                    StepState {
                        success: Some(success),
                        ok: true,
                        skipped: false,
                    },
                );
                Ok(())
            }
            // The step's own code rides out with it — a chain is a
            // composition, and the reason the composition stopped is the
            // reason its link stopped.
            Some(ToolResult::Failure(failure)) => Err(ChainError {
                code: failure.error.code,
                message: format!("step `{}`: {}", result.tool, failure.error.message),
            }),
            None => Err(format!("step `{}`: tool not found in registry", result.tool).into()),
        }
    }

    fn last_step_result(
        &mut self,
        steps: &[RuntimeStep],
        output_adapter: &OutputAdapter,
    ) -> Result<ToolResult, ChainError> {
        let Some(step) = steps.last() else {
            return Ok(output_adapter.text_result(Ok(String::new())));
        };
        let state = self
            .step_outputs
            .remove(&step.key)
            .ok_or_else(|| format!("missing final step `{}` output", step.key))?;
        let success = state.success.map(fold_inherited_step_summary).transpose()?;
        match success {
            Some(success) if output_adapter.has_declared_outputs() => {
                Ok(output_adapter.adapt_final_success(success))
            }
            Some(success) => Ok(ToolResult::Success(success)),
            None => Ok(output_adapter.text_result(Ok(String::new()))),
        }
    }
}

struct ChainJob {
    key: String,
    tool: String,
    args: Value,
}

struct ChainJobResult {
    key: String,
    tool: String,
    result: Option<ToolResult>,
    duration_ms: u64,
}

fn build_chain_jobs(
    ready: Vec<usize>,
    steps: &[RuntimeStep],
    state: &mut ChainState,
    args: &Value,
    creds: &BTreeMap<String, String>,
    approval: &ApprovalSurfaces,
) -> Result<Vec<ChainJob>, ChainError> {
    let mut jobs = Vec::new();
    for i in ready {
        state.remaining.remove(&i);
        let step = &steps[i];
        if let Some(when) = &step.when
            && !truthy(&render_template(
                when,
                args,
                Some(&state.step_outputs),
                creds,
            )?)
        {
            state.skip(step);
            continue;
        }
        if step.requires_approval
            && let Some(rejection) = approval
                .authorize(args, &step.key, &step.tool)
                .rejection(&step.key)
        {
            state.deny(step);
            return Err(ChainError {
                code: rejection.code.to_string(),
                message: rejection.message,
            });
        }
        let current = match &step.args {
            Some(template) => render_args_template(template, args, &state.step_outputs, creds)?,
            None => default_step_args(step, args, &state.step_outputs)?,
        };
        jobs.push(ChainJob {
            key: step.key.clone(),
            tool: step.tool.clone(),
            // A step's args are a template's output, and `{{input.*}}`
            // splices caller text into that template — so whatever `_upeg`
            // block came out of it states nothing. The call's own block
            // replaces it: a step can neither forge the calling surface nor
            // lose it, which is what lets a nested chain be approved from
            // the surface the person is actually sitting at.
            args: upeg_runtime::inherit_call_context(current, args),
        });
    }
    Ok(jobs)
}

fn dispatch_chain_job(job: ChainJob) -> ChainJobResult {
    let clock = StepClock::start();
    let result = upeg_runtime::try_runtime_dispatch(&job.tool, &job.args);
    ChainJobResult {
        key: job.key,
        tool: job.tool,
        result,
        duration_ms: clock.elapsed_ms(),
    }
}

fn default_step_args(
    step: &RuntimeStep,
    root: &Value,
    steps: &BTreeMap<String, StepState>,
) -> Result<Value, String> {
    match step.upstream.as_slice() {
        [] => Ok(root.clone()),
        [dep] => Ok(json!({
            "input": steps
                .get(dep)
                .map_or_else(|| Ok(String::new()), StepState::primary_wire_text)?
        })),
        deps => {
            let input = deps
                .last()
                .and_then(|dep| steps.get(dep))
                .map_or_else(|| Ok(String::new()), StepState::primary_wire_text)?;
            let inputs = deps
                .iter()
                .map(|dep| {
                    steps
                        .get(dep)
                        .map_or_else(|| Ok(String::new()), StepState::primary_wire_text)
                        .map(|text| (dep.clone(), Value::String(text)))
                })
                .collect::<Result<serde_json::Map<_, _>, String>>()?;
            Ok(json!({ "input": input, "inputs": inputs }))
        }
    }
}

#[derive(Clone, Debug)]
struct RuntimeStep {
    key: String,
    tool: String,
    args: Option<String>,
    when: Option<String>,
    upstream: Vec<String>,
    requires_approval: bool,
}

#[derive(Debug)]
pub(super) struct StepState {
    success: Option<ToolSuccess>,
    ok: bool,
    skipped: bool,
}

impl StepState {
    fn primary_wire_text(&self) -> Result<String, String> {
        self.success.as_ref().map_or_else(
            || Ok(String::new()),
            |success| {
                tool_success_primary_canonical_wire_text(success).map_err(|error| error.to_string())
            },
        )
    }

    fn output_field_wire_text(&self, id: &str) -> Result<Option<String>, String> {
        self.success
            .as_ref()
            .and_then(|success| success.outputs.iter().find(|entry| entry.id == id))
            .map(|entry| {
                output_value_canonical_wire_text(&entry.value).map_err(|error| error.to_string())
            })
            .transpose()
    }
}

fn normalize_chain_steps(parsed: &ToolToml) -> Option<Vec<RuntimeStep>> {
    parsed.steps.as_ref().map(|steps| {
        let dependencies = connection_dependencies(parsed);
        steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                runtime_step_from_rich(
                    i,
                    step,
                    dependencies.get(&chain_step_key(i, step.id.as_deref())),
                )
            })
            .collect()
    })
}

fn connection_dependencies(parsed: &ToolToml) -> BTreeMap<String, Vec<String>> {
    let mut dependencies = BTreeMap::<String, Vec<String>>::new();
    for connection in parsed.connections.as_deref().unwrap_or_default() {
        dependencies
            .entry(connection.to.trim().to_string())
            .or_default()
            .push(connection.from.trim().to_string());
    }
    dependencies
}

fn runtime_step_from_rich(
    i: usize,
    step: &ChainStepToml,
    upstream: Option<&Vec<String>>,
) -> RuntimeStep {
    RuntimeStep {
        key: chain_step_key(i, step.id.as_deref()),
        tool: step.tool.trim().to_string(),
        args: step.args.clone(),
        when: step.when.clone(),
        upstream: upstream.cloned().unwrap_or_default(),
        requires_approval: step.requires_approval.unwrap_or(false),
    }
}

fn render_args_template(
    template: &str,
    root: &Value,
    steps: &BTreeMap<String, StepState>,
    credentials: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let rendered = render_template(template, root, Some(steps), credentials)?;
    serde_json::from_str(&rendered)
        .or_else(|_| Ok::<Value, serde_json::Error>(json!({ "input": rendered })))
        .map_err(|e| e.to_string())
}

pub(super) fn render_template(
    template: &str,
    root: &Value,
    steps: Option<&BTreeMap<String, StepState>>,
    credentials: &BTreeMap<String, String>,
) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        let (before, after_start) = rest.split_at(start);
        out.push_str(before);
        let after_start = &after_start[2..];
        let Some(end) = after_start.find("}}") else {
            return Err("unclosed expression".into());
        };
        let (expr, after) = after_start.split_at(end);
        out.push_str(&resolve_expr(expr.trim(), root, steps, credentials)?);
        rest = &after[2..];
    }
    out.push_str(rest);
    Ok(out)
}

fn resolve_expr(
    expr: &str,
    root: &Value,
    steps: Option<&BTreeMap<String, StepState>>,
    credentials: &BTreeMap<String, String>,
) -> Result<String, String> {
    if expr == "input" {
        return Ok(value_to_text(root.get("input").unwrap_or(root)));
    }
    if let Some(path) = expr.strip_prefix("input.") {
        return resolve_json_path(root.get("input").unwrap_or(root), path).map(value_to_text);
    }
    if let Some(path) = expr.strip_prefix("context.") {
        return root
            .get(upeg_core::EXECUTION_CONTEXT_ARG)
            .ok_or_else(|| format!("missing context for expression `{expr}`"))
            .and_then(|context| resolve_json_path(context, path).map(value_to_text));
    }
    if let Some(name) = expr.strip_prefix("credential.") {
        return credentials
            .get(name)
            .cloned()
            .ok_or_else(|| format!("missing credential `{name}`"));
    }
    if let Some(path) = expr.strip_prefix("steps.") {
        let mut parts = path.splitn(2, '.');
        let key = parts.next().unwrap_or_default();
        let field = parts.next().unwrap_or("output");
        let Some(step) = steps.and_then(|s| s.get(key)) else {
            return Err(format!("missing step `{key}` for expression `{expr}`"));
        };
        return match field {
            "output" => step.primary_wire_text(),
            "ok" => Ok(step.ok.to_string()),
            "skipped" => Ok(step.skipped.to_string()),
            field => step
                .output_field_wire_text(field)?
                .ok_or_else(|| format!("unknown step field `{field}` in expression `{expr}`")),
        };
    }
    Err(format!("unknown expression `{expr}`"))
}

fn resolve_json_path<'a>(value: &'a Value, path: &str) -> Result<&'a Value, String> {
    let mut current = value;
    for part in path.split('.') {
        current = current
            .get(part)
            .ok_or_else(|| format!("missing `{part}` in expression path `{path}`"))?;
    }
    Ok(current)
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

fn truthy(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "null" | "no" | "off"
    )
}

#[cfg(test)]
#[path = "chain_approval_policy_tests.rs"]
mod approval_policy_tests;

#[cfg(test)]
#[path = "chain_output_budget_tests.rs"]
mod output_budget_tests;

#[cfg(test)]
#[path = "chain_typed_file_tests.rs"]
mod typed_file_tests;
