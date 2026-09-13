//! Tool execution context — the synthesis layer every surface routes a
//! `(tool_id, args)` pair through before dispatch.
//!
//! Formerly `upeg-cli/src/domain/execution/context.rs`; lowered into the
//! runtime so non-CLI surfaces (FRB desktop, future embeds) share one
//! context contract instead of re-implementing (or skipping) it.
//!
//! The context is also total over the identity question: every context
//! carries a [`Principal`] (surface-derived by default,
//! [`ExecutionContext::with_principal`] where a listener knows better),
//! and [`apply_execution_context`] stamps it into `_upeg.principal`.
//!
//! The context is total over the board question:
//!   - [`ExecutionContext::Global`] — no board; only the surface label is
//!     annotated.
//!   - [`ExecutionContext::Board`] — a board-scoped call; the pin's
//!     [`ArgsPreset`] is merged (preset = defaults, caller args override)
//!     and the resolved [`BoardExecutionContext`] plus surface label are
//!     annotated.
//!
//! Hexagonal split: the pure context-merging functions live here; the
//! probe that resolves a [`BoardExecutionContext`] (which may walk the
//! filesystem looking for `upeg.toml`) is the [`ProjectContext`] port.
//! Adapters supply the implementation — `upeg-sources` for filesystem
//! surfaces, [`RegistryProjectContext`] for registry-only surfaces.

use serde_json::Value;
use upeg_core::{
    ArgsPreset, BoardExecutionContext, BoardKey, EXECUTION_CONTEXT_APPROVED_STEPS,
    EXECUTION_CONTEXT_ARG, EXECUTION_CONTEXT_CWD, EXECUTION_CONTEXT_PRINCIPAL,
    EXECUTION_CONTEXT_SURFACE, Principal, Surface,
};

/// Resolve the runtime board context, including any project-manifest
/// discovery that requires filesystem walks. Pure context merging
/// (annotating args with board/surface/trigger labels) lives in this
/// module — only the I/O-touching probe goes through this port.
pub trait ProjectContext {
    fn board_context_with_project(&self, board: &str) -> BoardExecutionContext;
}

/// Registry-only [`ProjectContext`]: resolves the board context from the
/// in-process registry ([`crate::board_context`]) without any filesystem
/// walk. Default probe for surfaces without project discovery (wasm,
/// FRB fallback).
#[derive(Default, Clone, Copy, Debug)]
pub struct RegistryProjectContext;

impl ProjectContext for RegistryProjectContext {
    fn board_context_with_project(&self, board: &str) -> BoardExecutionContext {
        crate::board_context(board)
    }
}

/// Where a tool call enters the system. `Board` totalizes the "board면
/// preset 병합" rule: a board-scoped call always carries the pin's
/// preset (possibly empty), so no call path can forget the merge.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ExecutionContext {
    /// Surface-only call — no board scope.
    Global {
        surface: Surface,
        principal: Principal,
    },
    /// Board-scoped call: the pin's saved preset participates as
    /// argument defaults and the board's execution context is annotated.
    Board {
        surface: Surface,
        board: BoardKey,
        preset: ArgsPreset,
        principal: Principal,
    },
}

impl ExecutionContext {
    pub const fn global(surface: Surface) -> Self {
        Self::Global {
            surface,
            principal: Principal::for_surface(surface),
        }
    }

    pub const fn board(surface: Surface, board: BoardKey, preset: ArgsPreset) -> Self {
        Self::Board {
            surface,
            board,
            preset,
            principal: Principal::for_surface(surface),
        }
    }

    /// Narrow the caller identity beyond what the surface alone implies.
    ///
    /// Only a surface that actually authenticates callers has anything to
    /// say here: the HTTP listener knows whether a request carried the
    /// operator token or an agent token, and that distinction is invisible
    /// to [`Principal::for_surface`], which can only see the door. Every
    /// other surface keeps the surface-derived default.
    #[must_use]
    pub fn with_principal(self, principal: Principal) -> Self {
        match self {
            Self::Global { surface, .. } => Self::Global { surface, principal },
            Self::Board {
                surface,
                board,
                preset,
                ..
            } => Self::Board {
                surface,
                board,
                preset,
                principal,
            },
        }
    }

    pub const fn principal(&self) -> Principal {
        match self {
            Self::Global { principal, .. } | Self::Board { principal, .. } => *principal,
        }
    }

    /// Board-or-global constructor for callers holding an optional board
    /// scope. `preset` is only meaningful with a board; a missing preset
    /// on a board-scoped call collapses to the empty preset.
    pub fn for_optional_board(
        surface: Surface,
        board: Option<BoardKey>,
        preset: Option<ArgsPreset>,
    ) -> Self {
        match board {
            Some(board) => Self::board(surface, board, preset.unwrap_or_else(empty_args_preset)),
            None => Self::global(surface),
        }
    }

    pub const fn surface(&self) -> Surface {
        match self {
            Self::Global { surface, .. } | Self::Board { surface, .. } => *surface,
        }
    }

    pub const fn board_key(&self) -> Option<&BoardKey> {
        match self {
            Self::Global { .. } => None,
            Self::Board { board, .. } => Some(board),
        }
    }
}

/// The empty `{}` preset. Infallible by construction.
#[allow(
    clippy::expect_used,
    reason = "the empty JSON object never contains the reserved key; from_object cannot fail"
)]
pub fn empty_args_preset() -> ArgsPreset {
    ArgsPreset::from_object(serde_json::Map::new()).expect("empty args preset is always valid")
}

/// SINGLE merge point for the board preset rule: the placement's
/// `args_preset` supplies defaults, the caller's args override them
/// key-by-key. Non-object caller args are normalized the same way
/// [`merge_execution_context`] normalizes them (`Null` → `{}`, other
/// scalars → `{"input": value}`) so the two merge layers agree.
pub fn merge_args_with_preset(args: Value, preset: &ArgsPreset) -> Value {
    let mut merged = preset.to_object();
    for (key, value) in into_args_object(args) {
        merged.insert(key, value);
    }
    Value::Object(merged)
}

fn into_args_object(args: Value) -> serde_json::Map<String, Value> {
    match args {
        Value::Object(map) => map,
        Value::Null => serde_json::Map::new(),
        other => {
            let mut map = serde_json::Map::new();
            map.insert("input".into(), other);
            map
        }
    }
}

/// Annotate args with the resolved board's execution context. Pure —
/// no I/O. The caller resolves the `BoardExecutionContext` first
/// (through [`ProjectContext::board_context_with_project`] or otherwise).
pub fn apply_board_context(args: Value, context: BoardExecutionContext) -> Value {
    let Some(map) = context.to_json().as_object().cloned() else {
        return args;
    };
    merge_execution_context(args, map, false)
}

/// Replace any caller-supplied `_upeg` block with one carrying only the
/// surface label. Used when no board is active — the surface annotation
/// is the only context the tool sees.
pub fn apply_surface_context(args: Value, surface: &str) -> Value {
    surface_context(args, surface, false)
}

/// Add the surface label to an existing context block, preserving any
/// board context already merged in.
pub fn add_surface_context(args: Value, surface: &str) -> Value {
    surface_context(args, surface, true)
}

fn surface_context(args: Value, surface: &str, preserve_existing: bool) -> Value {
    let mut context = serde_json::Map::new();
    context.insert(EXECUTION_CONTEXT_SURFACE.into(), surface.trim().into());
    merge_execution_context(args, context, preserve_existing)
}

/// Stamp the caller's [`Principal`] into the context block.
///
/// Authoritative-from-surface: the key is wiped out of whatever the
/// caller sent (see [`CALLER_PRESERVED_CONTEXT_KEYS`]) and written here
/// instead, so `_upeg.principal` states what the runtime proved rather
/// than what the caller claims. Preserves the rest of the block —
/// [`apply_execution_context`] calls it after the surface/board
/// annotations are already in place.
pub fn apply_principal_context(args: Value, principal: Principal) -> Value {
    let mut context = serde_json::Map::new();
    context.insert(EXECUTION_CONTEXT_PRINCIPAL.into(), principal.to_json());
    merge_execution_context(args, context, true)
}

/// Add a `trigger` field to the existing context block, preserving
/// any other annotations.
///
/// `trigger` is a [`crate::FiredTrigger`] label (`source`, or
/// `source:condition`) — never a tool id, which told the tool only what it
/// already knew and could not discriminate between a tool's several triggers.
/// Composing the label is that type's job; this function only stamps it.
pub fn apply_trigger_context(args: Value, trigger: &str) -> Value {
    let mut context = serde_json::Map::new();
    context.insert("trigger".into(), trigger.trim().into());
    merge_execution_context(args, context, true)
}

/// `_upeg.approvedSteps` — see [`CALLER_PRESERVED_CONTEXT_KEYS`]. The
/// key itself is owned by `upeg-core` alongside [`EXECUTION_CONTEXT_CWD`]
/// so the wire name cannot drift between the surface that preserves it
/// and the Chain dispatcher that reads it
/// (`upeg-loader/src/dispatcher/chain/approval.rs`).
const CALLER_APPROVED_STEPS_KEY: &str = EXECUTION_CONTEXT_APPROVED_STEPS;

/// The `_upeg` block splits into two kinds of key, and the split matters
/// because a caller must never be able to spoof the first kind:
///
///   - **Authoritative-from-surface**: `board`, `boardEnv`, `surface`,
///     `principal`, `trigger`, `project_manifest`, … — stamped by the
///     surface/runtime
///     itself. A caller-supplied value under one of these names is
///     always discarded by [`merge_execution_context`]'s reserved-block
///     wipe (`preserve_existing = false`) — that wipe is deliberate and
///     stays in place.
///   - **Carried-from-caller**: [`CALLER_APPROVED_STEPS_KEY`] (Chain-step
///     approval, docs/architecture/chain.md — the caller states intent
///     here; whether that intent is honored is decided by the Chain
///     dispatcher against the surface label, which the caller cannot
///     spoof) and
///     [`EXECUTION_CONTEXT_CWD`] (the caller's working directory). These
///     have no surface-side source of truth to stamp FROM, so the wipe
///     would otherwise destroy legitimate caller input. This allow-list
///     is the ONLY thing that survives the wipe.
const CALLER_PRESERVED_CONTEXT_KEYS: [&str; 2] = [CALLER_APPROVED_STEPS_KEY, EXECUTION_CONTEXT_CWD];

fn is_caller_preserved_context_key(key: &str) -> bool {
    CALLER_PRESERVED_CONTEXT_KEYS.contains(&key)
}

fn merge_execution_context(
    args: Value,
    entries: serde_json::Map<String, Value>,
    preserve_existing: bool,
) -> Value {
    let mut object = into_args_object(args);
    let mut context = if preserve_existing {
        object
            .remove(EXECUTION_CONTEXT_ARG)
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    } else {
        // The reserved block is wiped — a caller must never spoof
        // `board`/`boardEnv`/`surface`/`trigger`/... — except for the
        // narrow caller-preserved allow-list (`approvedSteps`, `cwd`),
        // which has no surface-side value to overwrite it with anyway.
        object
            .remove(EXECUTION_CONTEXT_ARG)
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter(|(key, _)| is_caller_preserved_context_key(key))
            .collect()
    };
    for (key, value) in entries {
        context.insert(key, value);
    }
    object.insert(EXECUTION_CONTEXT_ARG.into(), Value::Object(context));
    Value::Object(object)
}

/// Hand a sub-call the execution context of the call that spawned it.
///
/// A program that composes args for a sub-call — the Chain engine
/// building one step's args out of a manifest template — is not a
/// surface. It cannot stamp `_upeg.surface`, and the block its template
/// produced is not identity: `{{input.*}}` splices caller text into that
/// template, so a caller can write any `_upeg` key it likes into a step's
/// args. So the sub-call's own block goes through the same reserved-block
/// wipe every surface applies, and the parent call's block is installed in
/// its place.
///
/// One definition of "authoritative-from-surface"
/// ([`merge_execution_context`]), one answer to "which surface is asking"
/// no matter how deep the call sits. Nesting neither forges the answer nor
/// loses it: a chain step inside a chain still sees the surface, board, and
/// `cwd` the outermost surface stamped.
pub fn inherit_call_context(args: Value, call: &Value) -> Value {
    let entries = call
        .get(EXECUTION_CONTEXT_ARG)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut merged = merge_execution_context(args, entries, false);
    // A call with no context hands down no context. Leaving an empty
    // `_upeg` block behind would put a key in the sub-call's args that
    // nobody wrote — visible to every dispatcher that iterates them.
    if merged
        .get(EXECUTION_CONTEXT_ARG)
        .and_then(Value::as_object)
        .is_some_and(serde_json::Map::is_empty)
        && let Some(object) = merged.as_object_mut()
    {
        object.remove(EXECUTION_CONTEXT_ARG);
    }
    merged
}

/// Apply the execution context to `args`:
///   - `Board`: merge the pin preset (defaults; caller overrides), then
///     annotate the resolved board context and surface label.
///   - `Global`: annotate only the surface label.
///   - always: stamp `_upeg.principal` ([`apply_principal_context`]), so
///     no dispatch path can reach a Chain approval gate with no caller
///     identity at all.
///   - `trigger`: the [`crate::FiredTrigger`] label of the trigger that
///     started this call, appended to the context block when present.
///
/// Pure-with-respect-to-probe: every filesystem touch is funneled
/// through the [`ProjectContext`] trait, so tests can substitute an
/// in-memory probe.
pub fn apply_execution_context<P: ProjectContext + ?Sized>(
    probe: &P,
    args: Value,
    context: &ExecutionContext,
    trigger: Option<&str>,
) -> Value {
    let mut args = match context {
        ExecutionContext::Global { surface, .. } => apply_surface_context(args, surface.label()),
        ExecutionContext::Board {
            surface,
            board,
            preset,
            ..
        } => {
            let args = merge_args_with_preset(args, preset);
            let args = apply_board_context(args, probe.board_context_with_project(board.as_str()));
            add_surface_context(args, surface.label())
        }
    };
    args = apply_principal_context(args, context.principal());
    if let Some(trigger) = trigger.map(str::trim).filter(|trigger| !trigger.is_empty()) {
        args = apply_trigger_context(args, trigger);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct StaticProbe(BoardExecutionContext);

    impl ProjectContext for StaticProbe {
        fn board_context_with_project(&self, _board: &str) -> BoardExecutionContext {
            self.0.clone()
        }
    }

    fn board_key(key: &str) -> BoardKey {
        BoardKey::parse(key).expect("테스트 보드 키")
    }

    #[test]
    fn 표면_맥락_적용은_호출자가_제공한_예약된_맥락을_버린다() {
        let args = serde_json::json!({
            EXECUTION_CONTEXT_ARG: {
                "boardEnv": { "PATH": "/tmp/evil" },
                "board": "spoofed"
            },
            "input": "hello"
        });

        let out = apply_surface_context(args, "http");
        let context = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("runtime context object");
        assert_eq!(context.get("surface"), Some(&Value::String("http".into())));
        assert!(context.get("boardEnv").is_none());
        assert!(context.get("board").is_none());
    }

    #[test]
    fn 표면_맥락_적용은_호출자가_제공한_승인된_단계목록을_보존한다() {
        // E-3/B-2: `_upeg.approvedSteps`는 authoritative-from-surface가
        // 아니라 caller가 들고 오는 값이다 — 예약 맥락을 버리는 와중에도
        // 살아남아야 chain approval이 동작한다.
        let args = serde_json::json!({
            EXECUTION_CONTEXT_ARG: { "approvedSteps": ["step1", "step2"] },
            "input": "hello"
        });

        let out = apply_surface_context(args, "mcp");
        let context = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("runtime context object");
        assert_eq!(context.get("surface"), Some(&Value::String("mcp".into())));
        assert_eq!(
            context.get("approvedSteps"),
            Some(&serde_json::json!(["step1", "step2"]))
        );
    }

    #[test]
    fn 표면_맥락_적용은_호출자가_제공한_cwd를_보존한다() {
        let args = serde_json::json!({
            EXECUTION_CONTEXT_ARG: { "cwd": "/home/user/project" },
        });

        let out = apply_surface_context(args, "cli");
        let context = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("runtime context object");
        assert_eq!(
            context.get("cwd"),
            Some(&Value::String("/home/user/project".into()))
        );
    }

    #[test]
    fn 보드_맥락_적용도_호출자가_제공한_승인된_단계목록을_보존한다() {
        let args = serde_json::json!({
            EXECUTION_CONTEXT_ARG: {
                "approvedSteps": ["gate"],
                "board": "spoofed",
                "boardEnv": { "PATH": "/tmp/evil" }
            },
            "input": "hello"
        });

        let out = apply_board_context(
            args,
            BoardExecutionContext {
                board: "dev".into(),
                env: BTreeMap::new(),
                project_manifest: None,
            },
        );
        let context = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("runtime context object");
        // 진짜 board 주석(dev)이 승인 목록과 함께 살아 있어야 하고,
        // 호출자가 스푸핑하려던 값(spoofed/evil)은 여전히 버려져야 한다.
        assert_eq!(context.get("board"), Some(&Value::String("dev".into())));
        assert_eq!(
            context.get("approvedSteps"),
            Some(&serde_json::json!(["gate"]))
        );
        assert!(
            context.get("boardEnv").and_then(Value::as_object).is_none()
                || context["boardEnv"].as_object().unwrap().is_empty(),
            "호출자가 준 boardEnv 스푸핑은 버려져야 한다: {context:?}"
        );
    }

    #[test]
    fn 추가_표면_맥락은_runtime_보드_맥락을_보존한다() {
        let mut env = BTreeMap::new();
        env.insert("PROFILE".to_string(), "dev".to_string());
        let args = apply_board_context(
            serde_json::json!({ "input": "hello" }),
            BoardExecutionContext {
                board: "dev".into(),
                env,
                project_manifest: Some("/tmp/upeg.toml".into()),
            },
        );

        let out = add_surface_context(args, "cli");
        let context = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("runtime context object");
        assert_eq!(context.get("board"), Some(&Value::String("dev".into())));
        assert_eq!(context.get("surface"), Some(&Value::String("cli".into())));
        assert_eq!(context["boardEnv"]["PROFILE"], "dev");
    }

    #[test]
    fn 하위_호출은_부모_호출의_예약_맥락을_그대로_물려받는다() {
        // Chain step의 args는 매니페스트 템플릿이 만든 새 객체다 —
        // 부모 호출이 각인받은 surface/board가 거기 있을 리 없다.
        let call = serde_json::json!({
            EXECUTION_CONTEXT_ARG: {
                "surface": "cli",
                "board": "dev",
                "cwd": "/workspaces/upeg",
            },
        });

        let out = inherit_call_context(serde_json::json!({ "input": "x" }), &call);

        let context = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("runtime context object");
        assert_eq!(context.get("surface"), Some(&Value::String("cli".into())));
        assert_eq!(context.get("board"), Some(&Value::String("dev".into())));
        assert_eq!(
            context.get("cwd"),
            Some(&Value::String("/workspaces/upeg".into()))
        );
        assert_eq!(out["input"], "x");
    }

    #[test]
    fn 하위_호출이_스스로_적은_surface는_부모의_것으로_덮인다() {
        // 템플릿에 `{{input.*}}`로 스며든 호출자 텍스트가 `_upeg.surface`를
        // 지어내도, 각인은 surface의 몫이지 호출자의 몫이 아니다.
        let call = serde_json::json!({ EXECUTION_CONTEXT_ARG: { "surface": "mcp" } });
        let spoofed = serde_json::json!({
            EXECUTION_CONTEXT_ARG: { "surface": "cli", "board": "spoofed" },
        });

        let out = inherit_call_context(spoofed, &call);

        let context = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("runtime context object");
        assert_eq!(context.get("surface"), Some(&Value::String("mcp".into())));
        assert!(context.get("board").is_none(), "{context:?}");
    }

    #[test]
    fn 예약_맥락이_없는_부모의_하위_호출은_빈_맥락을_받는다() {
        let out = inherit_call_context(
            serde_json::json!({ EXECUTION_CONTEXT_ARG: { "surface": "cli" } }),
            &serde_json::json!({}),
        );

        assert!(
            out.get(EXECUTION_CONTEXT_ARG).is_none(),
            "부모가 신원을 갖지 않으면 하위 호출의 인자에 예약 키 자체가 없다: {out}"
        );
    }

    #[test]
    fn preset_병합은_preset을_기본값으로_호출자_인자가_덮어쓴다() {
        let preset = ArgsPreset::parse(r#"{"city":"Seoul","days":3}"#).expect("유효 preset");

        let merged = merge_args_with_preset(serde_json::json!({ "days": 7, "unit": "C" }), &preset);

        assert_eq!(merged["city"], "Seoul", "preset 기본값 유지");
        assert_eq!(merged["days"], 7, "호출자 인자가 preset을 덮어쓴다");
        assert_eq!(merged["unit"], "C", "호출자 고유 인자 유지");
    }

    #[test]
    fn preset_병합은_null_인자를_preset만으로_채운다() {
        let preset = ArgsPreset::parse(r#"{"input":"0xff"}"#).expect("유효 preset");

        let merged = merge_args_with_preset(Value::Null, &preset);

        assert_eq!(merged["input"], "0xff");
    }

    #[test]
    fn 보드_맥락은_preset_병합과_보드_주석을_모두_적용한다() {
        let preset = ArgsPreset::parse(r#"{"input":"0xff"}"#).expect("유효 preset");
        let context = ExecutionContext::board(Surface::Cli, board_key("dev"), preset);
        let probe = StaticProbe(BoardExecutionContext::new("dev"));

        let out = apply_execution_context(&probe, Value::Null, &context, None);

        assert_eq!(out["input"], "0xff");
        let annotated = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("runtime context object");
        assert_eq!(annotated.get("board"), Some(&Value::String("dev".into())));
        assert_eq!(annotated.get("surface"), Some(&Value::String("cli".into())));
    }

    #[test]
    fn 모든_실행_맥락은_주체를_각인한다() {
        let probe = RegistryProjectContext;
        for surface in upeg_core::ALL_SURFACES {
            let context = ExecutionContext::global(*surface);
            let out = apply_execution_context(&probe, Value::Null, &context, None);
            let stamped = out[EXECUTION_CONTEXT_ARG][EXECUTION_CONTEXT_PRINCIPAL].clone();
            assert_eq!(
                Principal::from_json(&stamped),
                Some(Principal::for_surface(*surface)),
                "{}",
                surface.label()
            );
        }
    }

    #[test]
    fn 호출자가_적어온_주체는_각인_전에_버려진다() {
        // 주체는 authoritative-from-surface다: 호출자가 operator라고
        // 적어와도 표면이 각인한 값으로 덮인다.
        let probe = RegistryProjectContext;
        let spoofed = serde_json::json!({
            EXECUTION_CONTEXT_ARG: {
                EXECUTION_CONTEXT_PRINCIPAL: { "role": "operator", "surface": "cli" },
            },
        });

        let out = apply_execution_context(
            &probe,
            spoofed,
            &ExecutionContext::global(Surface::Http),
            None,
        );

        let stamped =
            Principal::from_json(&out[EXECUTION_CONTEXT_ARG][EXECUTION_CONTEXT_PRINCIPAL])
                .expect("각인된 주체");
        assert_eq!(stamped, Principal::for_surface(Surface::Http));
        assert!(
            !stamped.may_approve(),
            "스푸핑으로 승인 권한을 얻을 수 없다"
        );
    }

    #[test]
    fn 인증한_표면이_알려준_주체가_표면_기본값을_덮는다() {
        // HTTP 리스너만 아는 사실이다: 같은 문으로 들어온 두 호출자를
        // 토큰이 구분한다.
        let probe = RegistryProjectContext;
        let operator = Principal::new(upeg_core::PrincipalRole::Operator, Surface::Cli);
        let context = ExecutionContext::global(Surface::Cli).with_principal(operator);

        let out = apply_execution_context(&probe, Value::Null, &context, None);

        assert_eq!(
            Principal::from_json(&out[EXECUTION_CONTEXT_ARG][EXECUTION_CONTEXT_PRINCIPAL]),
            Some(operator)
        );
        assert_eq!(
            context.surface(),
            Surface::Cli,
            "주체 지정이 표면을 바꾸지 않는다"
        );
    }

    #[test]
    fn 보드_맥락도_주체를_잃지_않는다() {
        let preset = ArgsPreset::parse(r#"{"input":"0xff"}"#).expect("유효 preset");
        let agent = Principal::new(upeg_core::PrincipalRole::Agent, Surface::Http);
        let context =
            ExecutionContext::board(Surface::Http, board_key("dev"), preset).with_principal(agent);
        let probe = StaticProbe(BoardExecutionContext::new("dev"));

        let out = apply_execution_context(&probe, Value::Null, &context, None);

        let annotated = out[EXECUTION_CONTEXT_ARG].clone();
        assert_eq!(annotated["board"], "dev");
        assert_eq!(annotated["surface"], "http");
        assert_eq!(
            Principal::from_json(&annotated[EXECUTION_CONTEXT_PRINCIPAL]),
            Some(agent)
        );
        assert_eq!(out["input"], "0xff", "preset 병합은 그대로다");
    }

    #[test]
    fn 하위_호출은_부모의_주체를_물려받는다() {
        let call = serde_json::json!({
            EXECUTION_CONTEXT_ARG: {
                "surface": "http",
                EXECUTION_CONTEXT_PRINCIPAL: { "role": "agent", "surface": "http" },
            },
        });

        let out = inherit_call_context(serde_json::json!({ "input": "x" }), &call);

        assert_eq!(
            Principal::from_json(&out[EXECUTION_CONTEXT_ARG][EXECUTION_CONTEXT_PRINCIPAL]),
            Some(Principal::new(
                upeg_core::PrincipalRole::Agent,
                Surface::Http
            )),
            "중첩된 체인 step도 바깥 호출자의 권한을 그대로 본다"
        );
    }

    #[test]
    fn 전역_맥락은_표면만_주석하고_trigger를_추가한다() {
        let context = ExecutionContext::global(Surface::Http);
        let probe = RegistryProjectContext;

        let out = apply_execution_context(
            &probe,
            serde_json::json!({ "input": "x" }),
            &context,
            Some("webhook.tool"),
        );

        let annotated = out
            .get(EXECUTION_CONTEXT_ARG)
            .and_then(Value::as_object)
            .expect("runtime context object");
        assert_eq!(
            annotated.get("surface"),
            Some(&Value::String("http".into()))
        );
        assert_eq!(
            annotated.get("trigger"),
            Some(&Value::String("webhook.tool".into()))
        );
        assert!(annotated.get("board").is_none());
    }

    #[test]
    fn 선택적_보드_생성자는_보드가_없으면_전역이_된다() {
        assert_eq!(
            ExecutionContext::for_optional_board(Surface::Tui, None, None),
            ExecutionContext::global(Surface::Tui)
        );
        let scoped =
            ExecutionContext::for_optional_board(Surface::Tui, Some(board_key("dev")), None);
        assert_eq!(scoped.board_key().map(BoardKey::as_str), Some("dev"));
        match scoped {
            ExecutionContext::Board { preset, .. } => {
                assert_eq!(preset, empty_args_preset(), "preset 없음은 빈 preset");
            }
            ExecutionContext::Global { .. } => panic!("보드 지정은 Board 변형이어야 한다"),
        }
    }
}
