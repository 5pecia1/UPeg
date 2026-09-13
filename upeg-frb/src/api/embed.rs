//! Embed-URL resolution exposed to Flutter.
//!
//! Resolves the static `OutputKind::EmbeddedView { url }` field for an
//! embed-kind tool. Platform routing (in-app webview on desktop, iframe
//! with load-timeout fallback on web) lives entirely in the Dart
//! `WebViewPanel` — Rust no longer carries a "force external" flag.
//!
//! Embed URLs are static strings today (no templating); `args_json` is
//! reserved for a future templated-URL path.

use upeg_core::{
    BindingWait, BindingWaitCondition, BindingWaitOnTimeout, ControlledEmbedSettings,
    ControlledEmbedTriggerAction, ControlledEmbedUserAgent, ControlledEmbedViewport,
    ControlledEmbedViewportPreset,
};
use upeg_runtime::selector_pipeline::ExecutionPlan;
use upeg_runtime::{
    BindingRole, SelectorBinding, controlled_embed_settings_owned_for, embed_url_for,
    selector_bindings_owned_for, set_controlled_embed_settings_owned, set_selector_bindings_owned,
    toolbox_tool,
};

/// Dart-mirrored embed resolution. Platform routing (in-app webview vs.
/// iframe vs. external launcher) is decided by `WebViewPanel` from
/// `kIsWeb` alone; Rust just hands over the URL.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct EmbedResolutionDto {
    pub url: String,
}

/// Resolve the embed URL for a tool. Returns `None` for tools without an
/// `OutputKind::EmbeddedView` field or an explicit `register_embed_url`
/// registration.
///
/// `args_json` is currently unused (no templating yet) but part of the
/// signature for forward compatibility.
#[flutter_rust_bridge::frb(sync)]
pub fn resolve_embed_url(tool_id: String, args_json: String) -> Option<EmbedResolutionDto> {
    let _ = args_json; // reserved for templated URLs (not implemented yet)
    let _meta = toolbox_tool(&tool_id)?;
    let url = embed_url_for(&tool_id)?;
    Some(EmbedResolutionDto {
        url: url.to_string(),
    })
}

/// Open `url` in the user's default browser or browser tab.
#[cfg(not(target_arch = "wasm32"))]
#[flutter_rust_bridge::frb(sync)]
pub fn open_in_browser(url: String) -> Result<(), super::boot::FrbError> {
    open::that(&url).map_err(|e| super::boot::FrbError::Io {
        message: e.to_string(),
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = open)]
    fn window_open(url: &str, target: &str) -> wasm_bindgen::JsValue;
}

#[cfg(target_arch = "wasm32")]
#[flutter_rust_bridge::frb(sync)]
pub fn open_in_browser(url: String) -> Result<(), super::boot::FrbError> {
    const NEW_TAB_TARGET: &str = "_blank";

    let opened = window_open(&url, NEW_TAB_TARGET);
    if opened.is_null() || opened.is_undefined() {
        return Err(super::boot::FrbError::Io {
            message: "browser blocked opening a new tab".to_string(),
        });
    }
    Ok(())
}

/// Role this binding plays in the Controlled Embed pipeline. Mirrors
/// [`upeg_core::BindingRole`] across the FRB boundary so Dart code can
/// `switch` exhaustively on a sealed enum instead of a stringly-typed
/// role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum BindingRoleDto {
    /// Write the corresponding input field value into the DOM target.
    Input,
    /// Click the DOM target to fire the page's action.
    Trigger,
    /// Read the DOM target after the trigger fires and surface it as
    /// an output keyed by `field`.
    Output,
}

impl From<BindingRole> for BindingRoleDto {
    fn from(r: BindingRole) -> Self {
        match r {
            BindingRole::Input => Self::Input,
            BindingRole::Trigger => Self::Trigger,
            BindingRole::Output => Self::Output,
        }
    }
}

impl From<BindingRoleDto> for BindingRole {
    fn from(r: BindingRoleDto) -> Self {
        match r {
            BindingRoleDto::Input => Self::Input,
            BindingRoleDto::Trigger => Self::Trigger,
            BindingRoleDto::Output => Self::Output,
        }
    }
}

/// Single selector binding row exposed to Dart. `role` selects the
/// pipeline phase (Input/Trigger/Output); `field` names the form input
/// or output (unused for Trigger); `selector` is the CSS-style selector
/// the embed adapter uses against the embed page.
#[derive(Clone, Debug)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct SelectorBindingDto {
    pub role: BindingRoleDto,
    pub field: String,
    pub selector: String,
    pub trigger_action: ControlledEmbedTriggerActionDto,
    pub wait: Option<BindingWaitDto>,
}

impl From<SelectorBinding> for SelectorBindingDto {
    fn from(b: SelectorBinding) -> Self {
        Self {
            role: b.role.into(),
            field: b.field,
            selector: b.selector,
            trigger_action: b.trigger_action.into(),
            wait: b.wait.map(BindingWaitDto::from),
        }
    }
}

impl From<SelectorBindingDto> for SelectorBinding {
    fn from(d: SelectorBindingDto) -> Self {
        Self {
            role: d.role.into(),
            field: d.field,
            selector: d.selector,
            trigger_action: d.trigger_action.into(),
            wait: d.wait.map(BindingWait::from),
        }
    }
}

/// Trigger action for the Controlled Embed Trigger binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum ControlledEmbedTriggerActionDto {
    Click,
    Enter,
}

impl From<ControlledEmbedTriggerAction> for ControlledEmbedTriggerActionDto {
    fn from(action: ControlledEmbedTriggerAction) -> Self {
        match action {
            ControlledEmbedTriggerAction::Click => Self::Click,
            ControlledEmbedTriggerAction::Enter => Self::Enter,
        }
    }
}

impl From<ControlledEmbedTriggerActionDto> for ControlledEmbedTriggerAction {
    fn from(action: ControlledEmbedTriggerActionDto) -> Self {
        match action {
            ControlledEmbedTriggerActionDto::Click => Self::Click,
            ControlledEmbedTriggerActionDto::Enter => Self::Enter,
        }
    }
}

/// Wait condition for a binding: whether to wait for the element to
/// exist in the DOM or to become visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum BindingWaitConditionDto {
    Exists,
    Visible,
}

impl From<BindingWaitCondition> for BindingWaitConditionDto {
    fn from(c: BindingWaitCondition) -> Self {
        match c {
            BindingWaitCondition::Exists => Self::Exists,
            BindingWaitCondition::Visible => Self::Visible,
        }
    }
}

impl From<BindingWaitConditionDto> for BindingWaitCondition {
    fn from(d: BindingWaitConditionDto) -> Self {
        match d {
            BindingWaitConditionDto::Exists => Self::Exists,
            BindingWaitConditionDto::Visible => Self::Visible,
        }
    }
}

/// Behavior when a binding wait times out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum BindingWaitOnTimeoutDto {
    Fail,
    Continue,
}

impl From<BindingWaitOnTimeout> for BindingWaitOnTimeoutDto {
    fn from(o: BindingWaitOnTimeout) -> Self {
        match o {
            BindingWaitOnTimeout::Fail => Self::Fail,
            BindingWaitOnTimeout::Continue => Self::Continue,
        }
    }
}

impl From<BindingWaitOnTimeoutDto> for BindingWaitOnTimeout {
    fn from(d: BindingWaitOnTimeoutDto) -> Self {
        match d {
            BindingWaitOnTimeoutDto::Fail => Self::Fail,
            BindingWaitOnTimeoutDto::Continue => Self::Continue,
        }
    }
}

/// Wait configuration for a single selector binding. `for_selector` is
/// an optional CSS selector to wait for (defaults to the binding's own
/// selector); `condition` selects exists-vs-visible semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct BindingWaitDto {
    pub for_selector: Option<String>,
    pub condition: BindingWaitConditionDto,
    pub timeout_ms: u64,
    pub settle_ms: u64,
    pub on_timeout: BindingWaitOnTimeoutDto,
}

impl From<BindingWait> for BindingWaitDto {
    fn from(w: BindingWait) -> Self {
        Self {
            for_selector: w.for_selector,
            condition: w.condition.into(),
            timeout_ms: w.timeout_ms,
            settle_ms: w.settle_ms,
            on_timeout: w.on_timeout.into(),
        }
    }
}

impl From<BindingWaitDto> for BindingWait {
    fn from(d: BindingWaitDto) -> Self {
        Self {
            for_selector: d.for_selector,
            condition: d.condition.into(),
            timeout_ms: d.timeout_ms,
            settle_ms: d.settle_ms,
            on_timeout: d.on_timeout.into(),
        }
    }
}

/// Browser settings for a Controlled Embed tool.
#[derive(Clone, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ControlledEmbedSettingsDto {
    pub user_agent: Option<ControlledEmbedUserAgentDto>,
    pub viewport: Option<ControlledEmbedViewportDto>,
}

impl From<ControlledEmbedSettings> for ControlledEmbedSettingsDto {
    fn from(settings: ControlledEmbedSettings) -> Self {
        Self {
            user_agent: settings.user_agent.map(ControlledEmbedUserAgentDto::from),
            viewport: settings.viewport.map(ControlledEmbedViewportDto::from),
        }
    }
}

impl From<ControlledEmbedSettingsDto> for ControlledEmbedSettings {
    fn from(settings: ControlledEmbedSettingsDto) -> Self {
        Self {
            user_agent: settings.user_agent.map(ControlledEmbedUserAgent::from),
            viewport: settings.viewport.map(ControlledEmbedViewport::from),
        }
    }
}

/// User-Agent override for a Controlled Embed browser session.
#[derive(Clone, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum ControlledEmbedUserAgentDto {
    Default,
    MobileSafari,
    Custom { value: String },
}

impl From<ControlledEmbedUserAgent> for ControlledEmbedUserAgentDto {
    fn from(user_agent: ControlledEmbedUserAgent) -> Self {
        match user_agent {
            ControlledEmbedUserAgent::Default => Self::Default,
            ControlledEmbedUserAgent::MobileSafari => Self::MobileSafari,
            ControlledEmbedUserAgent::Custom(value) => Self::Custom { value },
        }
    }
}

impl From<ControlledEmbedUserAgentDto> for ControlledEmbedUserAgent {
    fn from(user_agent: ControlledEmbedUserAgentDto) -> Self {
        match user_agent {
            ControlledEmbedUserAgentDto::Default => Self::Default,
            ControlledEmbedUserAgentDto::MobileSafari => Self::MobileSafari,
            ControlledEmbedUserAgentDto::Custom { value } => Self::Custom(value),
        }
    }
}

/// Viewport override for a Controlled Embed browser session.
#[derive(Clone, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum ControlledEmbedViewportDto {
    Preset {
        preset: ControlledEmbedViewportPresetDto,
    },
    Custom {
        width: u16,
        height: u16,
    },
}

impl From<ControlledEmbedViewport> for ControlledEmbedViewportDto {
    fn from(viewport: ControlledEmbedViewport) -> Self {
        match viewport {
            ControlledEmbedViewport::Preset(preset) => Self::Preset {
                preset: preset.into(),
            },
            ControlledEmbedViewport::Custom { width, height } => Self::Custom { width, height },
        }
    }
}

impl From<ControlledEmbedViewportDto> for ControlledEmbedViewport {
    fn from(viewport: ControlledEmbedViewportDto) -> Self {
        match viewport {
            ControlledEmbedViewportDto::Preset { preset } => Self::Preset(preset.into()),
            ControlledEmbedViewportDto::Custom { width, height } => Self::Custom { width, height },
        }
    }
}

/// Named viewport presets for Controlled Embed sessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum ControlledEmbedViewportPresetDto {
    Mobile,
    Tablet,
    Desktop,
}

impl From<ControlledEmbedViewportPreset> for ControlledEmbedViewportPresetDto {
    fn from(preset: ControlledEmbedViewportPreset) -> Self {
        match preset {
            ControlledEmbedViewportPreset::Mobile => Self::Mobile,
            ControlledEmbedViewportPreset::Tablet => Self::Tablet,
            ControlledEmbedViewportPreset::Desktop => Self::Desktop,
        }
    }
}

impl From<ControlledEmbedViewportPresetDto> for ControlledEmbedViewportPreset {
    fn from(preset: ControlledEmbedViewportPresetDto) -> Self {
        match preset {
            ControlledEmbedViewportPresetDto::Mobile => Self::Mobile,
            ControlledEmbedViewportPresetDto::Tablet => Self::Tablet,
            ControlledEmbedViewportPresetDto::Desktop => Self::Desktop,
        }
    }
}

/// Return the persisted selector-binding rows for `tool_id`. Empty
/// vector when the tool has no bindings registered (either through the
/// static `#[tool]` macro path or the owned-string FRB write below).
#[flutter_rust_bridge::frb(sync)]
pub fn selector_bindings_for(tool_id: String) -> Vec<SelectorBindingDto> {
    selector_bindings_owned_for(&tool_id)
        .into_iter()
        .map(SelectorBindingDto::from)
        .collect()
}

/// Persist `bindings` for `tool_id` so subsequent
/// [`selector_bindings_for`] calls see them. The owned-string registry
/// in `upeg-runtime::embed` exists specifically for this path —
/// the static-keyed `set_selector_bindings` requires `&'static str`
/// keys which FRB callers can't synthesise.
#[flutter_rust_bridge::frb(sync)]
pub fn set_selector_bindings(
    tool_id: String,
    bindings: Vec<SelectorBindingDto>,
) -> Result<(), super::boot::FrbError> {
    let typed: Vec<SelectorBinding> = bindings.into_iter().map(SelectorBinding::from).collect();
    set_selector_bindings_owned(tool_id, typed)
        .map_err(|message| super::boot::FrbError::Internal { message })
}

/// Return the runtime Controlled Embed settings for `tool_id`.
#[flutter_rust_bridge::frb(sync)]
pub fn controlled_embed_settings_for(tool_id: String) -> ControlledEmbedSettingsDto {
    controlled_embed_settings_owned_for(&tool_id).into()
}

/// Persist runtime Controlled Embed settings for `tool_id`.
#[flutter_rust_bridge::frb(sync)]
pub fn set_controlled_embed_settings(
    tool_id: String,
    settings: ControlledEmbedSettingsDto,
) -> Result<(), super::boot::FrbError> {
    set_controlled_embed_settings_owned(tool_id, settings.into())
        .map_err(|message| super::boot::FrbError::Internal { message })
}

/// Pre-compiled JavaScript scripts for the Controlled Embed pipeline.
/// Flutter's `ControlledEmbedRunner` feeds these to
/// `WebViewController.runJavaScriptReturningResult` so its DOM-driving
/// behavior matches the headless backend bit-for-bit.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ExecutionScriptsDto {
    /// JS that writes Input bindings into the DOM. Empty when there is
    /// nothing to write.
    pub write: String,
    /// JS that fires the Trigger's click. Empty when no Trigger binding
    /// is declared.
    pub trigger: String,
    /// JS IIFE that returns `JSON.stringify({field: value, …})`. The
    /// caller `JSON.parse`s the returned string into a map. Returns
    /// `"{}"` when there are no Output bindings.
    pub read: String,
    /// `true` when at least one of `write`/`trigger`/`read` is
    /// actionable. Callers can short-circuit on `false`.
    pub is_actionable: bool,
}

/// Build the three pipeline scripts (`write`, `trigger`, `read`) for
/// `bindings` + `inputs`. Pure function — same crate, same code path
/// as the headless `ControlledEmbedBackend` so live-WebView and
/// headless surfaces stay in lockstep.
#[flutter_rust_bridge::frb(sync)]
pub fn build_execution_scripts(
    bindings: Vec<SelectorBindingDto>,
    inputs: Vec<(String, String)>,
) -> ExecutionScriptsDto {
    let typed: Vec<SelectorBinding> = bindings.into_iter().map(SelectorBinding::from).collect();
    let input_pairs: Vec<(&str, &str)> = inputs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let plan = ExecutionPlan::build(&typed, &input_pairs);
    ExecutionScriptsDto {
        write: plan.write_script(),
        trigger: plan.trigger_script(),
        read: plan.read_script(),
        is_actionable: plan.is_actionable(),
    }
}

/// Build the wait script for a single Controlled Embed binding. Empty
/// string means the binding has no wait declaration.
#[flutter_rust_bridge::frb(sync)]
pub fn build_binding_wait_script(binding: SelectorBindingDto) -> String {
    let typed = SelectorBinding::from(binding);
    let plan = ExecutionPlan::build(std::slice::from_ref(&typed), &[]);
    plan.wait_script()
}

#[cfg(test)]
mod tests {
    use super::{
        BindingWaitConditionDto, BindingWaitDto, BindingWaitOnTimeoutDto,
        ControlledEmbedSettingsDto, ControlledEmbedTriggerActionDto, ControlledEmbedUserAgentDto,
        ControlledEmbedViewportDto, ControlledEmbedViewportPresetDto, SelectorBindingDto,
        build_binding_wait_script, controlled_embed_settings_for, selector_bindings_for,
        set_controlled_embed_settings, set_selector_bindings,
    };

    #[test]
    fn selector_bindings_for는_미등록_도구에_대해_빈_벡터를_반환한다() {
        let bindings = selector_bindings_for("frb.test.selector_bindings.empty".into());
        assert!(bindings.is_empty());
    }

    #[test]
    fn set_selector_bindings는_저장한_값을_round_trip한다() {
        let id = "frb.test.selector_bindings.roundtrip";
        let value = vec![SelectorBindingDto {
            role: super::BindingRoleDto::Input,
            field: "name".into(),
            selector: ".user".into(),
            trigger_action: ControlledEmbedTriggerActionDto::Click,
            wait: None,
        }];
        set_selector_bindings(id.into(), value).expect("set");
        let read_back = selector_bindings_for(id.into());
        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].role, super::BindingRoleDto::Input);
        assert_eq!(read_back[0].field, "name");
        assert_eq!(read_back[0].selector, ".user");
        assert_eq!(
            read_back[0].trigger_action,
            ControlledEmbedTriggerActionDto::Click
        );
    }

    #[test]
    fn selector_bindings_for는_enter_trigger_action을_round_trip한다() {
        let id = "frb.test.selector_bindings.enter";
        let value = vec![SelectorBindingDto {
            role: super::BindingRoleDto::Trigger,
            field: String::new(),
            selector: "form".into(),
            trigger_action: ControlledEmbedTriggerActionDto::Enter,
            wait: None,
        }];
        set_selector_bindings(id.into(), value).expect("set");

        let read_back = selector_bindings_for(id.into());

        assert_eq!(read_back.len(), 1);
        assert_eq!(
            read_back[0].trigger_action,
            ControlledEmbedTriggerActionDto::Enter
        );
    }

    // ─── 대기옵션 (BindingWait) round-trip ───────────────────────

    #[test]
    fn wait_controlled_embed_대기옵션은_input_바인딩에서_round_trip한다() {
        let id = "frb.test.wait.input_roundtrip";
        let wait_dto = BindingWaitDto {
            for_selector: Some("#loading-spinner".into()),
            condition: BindingWaitConditionDto::Exists,
            timeout_ms: 3000,
            settle_ms: 100,
            on_timeout: BindingWaitOnTimeoutDto::Fail,
        };
        let value = vec![SelectorBindingDto {
            role: super::BindingRoleDto::Input,
            field: "q".into(),
            selector: "#q".into(),
            trigger_action: ControlledEmbedTriggerActionDto::Click,
            wait: Some(wait_dto),
        }];
        set_selector_bindings(id.into(), value).expect("set");
        let read_back = selector_bindings_for(id.into());
        assert_eq!(read_back.len(), 1);
        let back_wait = read_back[0].wait.as_ref().expect("wait should be present");
        assert_eq!(back_wait.for_selector, Some("#loading-spinner".into()));
        assert_eq!(back_wait.condition, BindingWaitConditionDto::Exists);
        assert_eq!(back_wait.timeout_ms, 3000);
        assert_eq!(back_wait.settle_ms, 100);
        assert_eq!(back_wait.on_timeout, BindingWaitOnTimeoutDto::Fail);
    }

    #[test]
    fn wait_controlled_embed_대기옵션은_visible_조건을_round_trip한다() {
        let id = "frb.test.wait.visible_condition";
        let wait_dto = BindingWaitDto {
            for_selector: Some(".result-card".into()),
            condition: BindingWaitConditionDto::Visible,
            timeout_ms: 5000,
            settle_ms: 200,
            on_timeout: BindingWaitOnTimeoutDto::Continue,
        };
        let value = vec![SelectorBindingDto {
            role: super::BindingRoleDto::Trigger,
            field: String::new(),
            selector: "#submit".into(),
            trigger_action: ControlledEmbedTriggerActionDto::Click,
            wait: Some(wait_dto),
        }];
        set_selector_bindings(id.into(), value).expect("set");
        let read_back = selector_bindings_for(id.into());
        assert_eq!(read_back.len(), 1);
        let back_wait = read_back[0].wait.as_ref().expect("wait should be present");
        assert_eq!(back_wait.condition, BindingWaitConditionDto::Visible);
        assert_eq!(back_wait.on_timeout, BindingWaitOnTimeoutDto::Continue);
    }

    #[test]
    fn wait_controlled_embed_대기옵션이_없으면_none을_반환한다() {
        let id = "frb.test.wait.no_wait";
        let value = vec![SelectorBindingDto {
            role: super::BindingRoleDto::Output,
            field: "result".into(),
            selector: "#result".into(),
            trigger_action: ControlledEmbedTriggerActionDto::Click,
            wait: None,
        }];
        set_selector_bindings(id.into(), value).expect("set");
        let read_back = selector_bindings_for(id.into());
        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].wait, None);
    }

    #[test]
    fn wait_build_binding_wait_script는_단일_대기스크립트를_생성한다() {
        let script = build_binding_wait_script(SelectorBindingDto {
            role: super::BindingRoleDto::Input,
            field: "q".into(),
            selector: "#q".into(),
            trigger_action: ControlledEmbedTriggerActionDto::Click,
            wait: Some(BindingWaitDto {
                for_selector: Some("#ready".into()),
                condition: BindingWaitConditionDto::Exists,
                timeout_ms: 5000,
                settle_ms: 0,
                on_timeout: BindingWaitOnTimeoutDto::Fail,
            }),
        });

        assert!(script.contains("upegWaitReady"));
        assert!(script.contains("upegWaitReady(\"#ready\",\"exists\")"));
    }

    #[test]
    fn wait_build_binding_wait_script는_대기옵션이_없으면_빈_스크립트를_반환한다() {
        let script = build_binding_wait_script(SelectorBindingDto {
            role: super::BindingRoleDto::Input,
            field: "q".into(),
            selector: "#q".into(),
            trigger_action: ControlledEmbedTriggerActionDto::Click,
            wait: None,
        });

        assert!(script.is_empty());
    }

    #[test]
    fn wait_build_binding_wait_script는_for_selector를_우선한다() {
        let script = build_binding_wait_script(SelectorBindingDto {
            role: super::BindingRoleDto::Input,
            field: "q".into(),
            selector: "#binding-selector".into(),
            trigger_action: ControlledEmbedTriggerActionDto::Click,
            wait: Some(BindingWaitDto {
                for_selector: Some("#explicit-wait".into()),
                condition: BindingWaitConditionDto::Visible,
                timeout_ms: 5000,
                settle_ms: 0,
                on_timeout: BindingWaitOnTimeoutDto::Fail,
            }),
        });

        assert!(script.contains("upegWaitReady(\"#explicit-wait\",\"visible\")"));
        assert!(!script.contains("#binding-selector"));
    }

    #[test]
    fn wait_build_binding_wait_script는_for_selector가_없으면_binding_selector를_쓴다() {
        let script = build_binding_wait_script(SelectorBindingDto {
            role: super::BindingRoleDto::Output,
            field: "result".into(),
            selector: "#binding-selector".into(),
            trigger_action: ControlledEmbedTriggerActionDto::Click,
            wait: Some(BindingWaitDto {
                for_selector: None,
                condition: BindingWaitConditionDto::Exists,
                timeout_ms: 5000,
                settle_ms: 0,
                on_timeout: BindingWaitOnTimeoutDto::Fail,
            }),
        });

        assert!(script.contains("upegWaitReady(\"#binding-selector\",\"exists\")"));
    }

    #[test]
    fn wait_build_binding_wait_script는_selector의_따옴표와_백슬래시를_이스케이프한다() {
        let script = build_binding_wait_script(SelectorBindingDto {
            role: super::BindingRoleDto::Input,
            field: "q".into(),
            selector: "input[name=\"q\\\\path\"]".into(),
            trigger_action: ControlledEmbedTriggerActionDto::Click,
            wait: Some(BindingWaitDto {
                for_selector: None,
                condition: BindingWaitConditionDto::Exists,
                timeout_ms: 5000,
                settle_ms: 0,
                on_timeout: BindingWaitOnTimeoutDto::Fail,
            }),
        });

        assert!(script.contains("input[name=\\\"q\\\\\\\\path\\\"]"));
    }

    // ─── Controlled Embed settings tests ─────────────────────────

    #[test]
    fn controlled_embed_settings는_미등록_도구에서_빈_dto를_반환한다() {
        let settings = controlled_embed_settings_for("frb.test.settings.empty".into());

        assert_eq!(settings.user_agent, None);
        assert_eq!(settings.viewport, None);
    }

    #[test]
    fn controlled_embed_settings는_explicit_default_user_agent를_생략과_구분한다() {
        let id = "frb.test.settings.default_user_agent";
        let settings = ControlledEmbedSettingsDto {
            user_agent: Some(ControlledEmbedUserAgentDto::Default),
            viewport: None,
        };

        set_controlled_embed_settings(id.into(), settings).expect("set");
        let read_back = controlled_embed_settings_for(id.into());

        assert_eq!(
            read_back.user_agent,
            Some(ControlledEmbedUserAgentDto::Default)
        );
        assert_eq!(read_back.viewport, None);
    }

    #[test]
    fn controlled_embed_settings는_mobile_user_agent와_preset_viewport를_round_trip한다() {
        let id = "frb.test.settings.mobile_preset";
        let settings = ControlledEmbedSettingsDto {
            user_agent: Some(ControlledEmbedUserAgentDto::MobileSafari),
            viewport: Some(ControlledEmbedViewportDto::Preset {
                preset: ControlledEmbedViewportPresetDto::Mobile,
            }),
        };

        set_controlled_embed_settings(id.into(), settings).expect("set");
        let read_back = controlled_embed_settings_for(id.into());

        assert_eq!(
            read_back.user_agent,
            Some(ControlledEmbedUserAgentDto::MobileSafari)
        );
        assert_eq!(
            read_back.viewport,
            Some(ControlledEmbedViewportDto::Preset {
                preset: ControlledEmbedViewportPresetDto::Mobile,
            })
        );
    }

    #[test]
    fn controlled_embed_settings는_custom_user_agent와_custom_viewport를_round_trip한다() {
        let id = "frb.test.settings.custom_viewport";
        let settings = ControlledEmbedSettingsDto {
            user_agent: Some(ControlledEmbedUserAgentDto::Custom {
                value: "Custom UA/1.0".into(),
            }),
            viewport: Some(ControlledEmbedViewportDto::Custom {
                width: 320,
                height: 640,
            }),
        };

        set_controlled_embed_settings(id.into(), settings).expect("set");
        let read_back = controlled_embed_settings_for(id.into());

        assert_eq!(
            read_back.user_agent,
            Some(ControlledEmbedUserAgentDto::Custom {
                value: "Custom UA/1.0".into(),
            })
        );
        assert_eq!(
            read_back.viewport,
            Some(ControlledEmbedViewportDto::Custom {
                width: 320,
                height: 640,
            })
        );
    }
}
