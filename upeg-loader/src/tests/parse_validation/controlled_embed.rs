use super::{parse_fixture_tool, single_tool_toml_str};
use crate::tests::controlled_embed_backend_test_lock;
use crate::{LoadError, load_and_register_dir_verbose};
use std::sync::{Arc, Mutex};
use upeg_core::{
    BindingRole, BindingWait, BindingWaitCondition, BindingWaitOnTimeout,
    ControlledEmbedTriggerAction, ControlledEmbedUserAgent, ControlledEmbedViewport,
    ControlledEmbedViewportPreset, DEFAULT_CONTROLLED_EMBED_WAIT_SETTLE_MS,
    DEFAULT_CONTROLLED_EMBED_WAIT_TIMEOUT_MS, MAX_CONTROLLED_EMBED_WAIT_MS, SelectorBinding,
};

const REGISTRY_INPUT_WAIT_TIMEOUT_MS: u64 = 111;
const REGISTRY_INPUT_WAIT_SETTLE_MS: u64 = 12;
const REGISTRY_TRIGGER_WAIT_TIMEOUT_MS: u64 = 222;
const REGISTRY_TRIGGER_WAIT_SETTLE_MS: u64 = 0;
const REGISTRY_OUTPUT_WAIT_TIMEOUT_MS: u64 = 333;
const REGISTRY_OUTPUT_WAIT_SETTLE_MS: u64 = 34;

struct RestoreControlledEmbedBackend {
    previous: Arc<dyn upeg_runtime::controlled_embed::ControlledEmbedBackend>,
}

impl Drop for RestoreControlledEmbedBackend {
    fn drop(&mut self) {
        upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::clone(&self.previous));
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CapturedControlledEmbedRequest {
    tool_id: String,
    url: String,
    bindings: Vec<SelectorBinding>,
    inputs: Vec<(String, String)>,
}

struct WaitMetadataCaptureBackend {
    seen: Arc<Mutex<Option<CapturedControlledEmbedRequest>>>,
    target_url: &'static str,
    delegate: Arc<dyn upeg_runtime::controlled_embed::ControlledEmbedBackend>,
}

impl upeg_runtime::controlled_embed::ControlledEmbedBackend for WaitMetadataCaptureBackend {
    fn run(
        &self,
        request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
    ) -> Result<
        upeg_runtime::controlled_embed::ControlledEmbedResponse,
        upeg_runtime::controlled_embed::ControlledEmbedError,
    > {
        if request.url != self.target_url {
            return self.delegate.run(request);
        }
        let inputs = request
            .inputs
            .iter()
            .map(|(field, value)| ((*field).to_string(), (*value).to_string()))
            .collect();
        *self.seen.lock().unwrap() = Some(CapturedControlledEmbedRequest {
            tool_id: request.tool_id.to_string(),
            url: request.url.to_string(),
            bindings: request.bindings.to_vec(),
            inputs,
        });
        Ok(upeg_runtime::controlled_embed::ControlledEmbedResponse {
            outputs: vec![("answer".into(), "ok".into())],
        })
    }
}

fn assert_binding_wait(
    binding: &SelectorBinding,
    role: BindingRole,
    field: &str,
    selector: &str,
    expected: BindingWait,
) {
    assert_eq!(binding.role, role);
    assert_eq!(binding.field, field);
    assert_eq!(binding.selector, selector);
    assert_eq!(binding.wait.as_ref(), Some(&expected));
}

#[test]
fn controlled_embed_settings_register_mobile_safari_and_mobile_viewport() {
    let dir = std::env::temp_dir().join("upeg_loader_task4_mobile_settings");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "task4settings.mobile";
    std::fs::write(
        dir.join("mobile.toml"),
        single_tool_toml_str(&format!(
            r##"id = "{id}"
toolkit = "task4settings"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/mobile"
[controlled_embed.browser]
user_agent = "mobile_safari"
viewport = "mobile"

[[controlled_embed.bindings]]
role = "input"
field = "input"
selector = "#q"

[[controlled_embed.bindings]]
role = "trigger"
field = ""
selector = "button"

[[controlled_embed.bindings]]
role = "output"
field = "output"
selector = "#r""##,
        )),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    let settings = upeg_runtime::controlled_embed_settings_for(id);
    assert_eq!(
        settings.user_agent,
        Some(ControlledEmbedUserAgent::MobileSafari)
    );
    assert_eq!(
        settings.viewport,
        Some(ControlledEmbedViewport::Preset(
            ControlledEmbedViewportPreset::Mobile
        ))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn controlled_embed_settings_register_custom_user_agent_and_custom_viewport() {
    let dir = std::env::temp_dir().join("upeg_loader_task4_custom_settings");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "task4settings.custom";
    std::fs::write(
        dir.join("custom.toml"),
        single_tool_toml_str(&format!(
            r##"id = "{id}"
toolkit = "task4settings"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/custom"
[controlled_embed.browser]
user_agent = "custom"
custom_user_agent = "  MyAgent/1.0  "
viewport = "custom"
viewport_width = 1280
viewport_height = 720

[[controlled_embed.bindings]]
role = "input"
field = "input"
selector = "#q"

[[controlled_embed.bindings]]
role = "trigger"
field = ""
selector = "button"

[[controlled_embed.bindings]]
role = "output"
field = "output"
selector = "#r""##,
        )),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    let settings = upeg_runtime::controlled_embed_settings_for(id);
    assert_eq!(
        settings.user_agent,
        Some(ControlledEmbedUserAgent::Custom("MyAgent/1.0".to_string()))
    );
    assert_eq!(
        settings.viewport,
        Some(ControlledEmbedViewport::Custom {
            width: 1280,
            height: 720
        })
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn controlled_embed_settings_register_trigger_action_enter_on_selector_binding() {
    let dir = std::env::temp_dir().join("upeg_loader_task4_enter_action");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "task4settings.enter";
    std::fs::write(
        dir.join("enter.toml"),
        single_tool_toml_str(&format!(
            r##"id = "{id}"
toolkit = "task4settings"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/enter"
controlled_embed = {{ bindings = [
  {{ role = "input", field = "input", selector = "#q" }},
  {{ role = "trigger", field = "", selector = "#q", action = "enter" }},
  {{ role = "output", field = "output", selector = "#r" }},
] }}"##,
        )),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    let bindings = upeg_runtime::selector_bindings_for(id);
    assert_eq!(
        bindings[1].trigger_action,
        ControlledEmbedTriggerAction::Enter
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn controlled_embed_wait_option_registry_preserves_input_trigger_output_waits_through_dispatcher_request()
 {
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let previous_backend = upeg_runtime::controlled_embed::controlled_embed_backend();
    let _restore = RestoreControlledEmbedBackend {
        previous: Arc::clone(&previous_backend),
    };

    const TOOL_ID: &str = "waitregistry.dispatch";
    const EMBED_URL: &str = "https://example.com/wait-registry";
    let raw = single_tool_toml_str(&format!(
        r##"id = "{TOOL_ID}"
toolkit = "waitregistry"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "{EMBED_URL}"
primary_output_id = "answer"
outputs = [{{ name = "answer", type = "string", label = "Answer" }}]
controlled_embed = {{ bindings = [
  {{ role = "input", field = "query", selector = "#query", wait = {{ condition = "visible", timeout_ms = {REGISTRY_INPUT_WAIT_TIMEOUT_MS}, settle_ms = {REGISTRY_INPUT_WAIT_SETTLE_MS}, on_timeout = "continue" }} }},
  {{ role = "trigger", field = "", selector = "#submit", action = "enter", wait = {{ for_selector = "#button-ready", condition = "exists", timeout_ms = {REGISTRY_TRIGGER_WAIT_TIMEOUT_MS}, settle_ms = {REGISTRY_TRIGGER_WAIT_SETTLE_MS}, on_timeout = "fail" }} }},
  {{ role = "output", field = "answer", selector = "#answer", wait = {{ for_selector = "#answer-ready", condition = "visible", timeout_ms = {REGISTRY_OUTPUT_WAIT_TIMEOUT_MS}, settle_ms = {REGISTRY_OUTPUT_WAIT_SETTLE_MS}, on_timeout = "fail" }} }},
] }}"##,
    ));
    let (toolkit, tools) = crate::parse_toolkit_full(&raw).expect("wait manifest parses");
    let outcome = crate::loader::register_parsed_toolkit_for_tests(toolkit, tools)
        .expect("wait manifest registers");
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    assert_eq!(outcome.loaded, vec![TOOL_ID]);

    let registry_bindings = upeg_runtime::selector_bindings_for(TOOL_ID);
    assert_eq!(registry_bindings.len(), 3);
    assert_binding_wait(
        &registry_bindings[0],
        BindingRole::Input,
        "query",
        "#query",
        BindingWait {
            for_selector: Some("#query".into()),
            condition: BindingWaitCondition::Visible,
            timeout_ms: REGISTRY_INPUT_WAIT_TIMEOUT_MS,
            settle_ms: REGISTRY_INPUT_WAIT_SETTLE_MS,
            on_timeout: BindingWaitOnTimeout::Continue,
        },
    );
    assert_binding_wait(
        &registry_bindings[1],
        BindingRole::Trigger,
        "",
        "#submit",
        BindingWait {
            for_selector: Some("#button-ready".into()),
            condition: BindingWaitCondition::Exists,
            timeout_ms: REGISTRY_TRIGGER_WAIT_TIMEOUT_MS,
            settle_ms: REGISTRY_TRIGGER_WAIT_SETTLE_MS,
            on_timeout: BindingWaitOnTimeout::Fail,
        },
    );
    assert_eq!(
        registry_bindings[1].trigger_action,
        ControlledEmbedTriggerAction::Enter
    );
    assert_binding_wait(
        &registry_bindings[2],
        BindingRole::Output,
        "answer",
        "#answer",
        BindingWait {
            for_selector: Some("#answer-ready".into()),
            condition: BindingWaitCondition::Visible,
            timeout_ms: REGISTRY_OUTPUT_WAIT_TIMEOUT_MS,
            settle_ms: REGISTRY_OUTPUT_WAIT_SETTLE_MS,
            on_timeout: BindingWaitOnTimeout::Fail,
        },
    );

    let seen = Arc::new(Mutex::new(None));
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        WaitMetadataCaptureBackend {
            seen: Arc::clone(&seen),
            target_url: EMBED_URL,
            delegate: previous_backend,
        },
    ));

    let result =
        upeg_runtime::try_runtime_dispatch(TOOL_ID, &serde_json::json!({ "query": "upeg" }))
            .expect("registered controlled embed dispatcher");
    match result {
        upeg_core::ToolResult::Success(success) => {
            assert_eq!(success.primary_output_id.as_deref(), Some("answer"));
        }
        upeg_core::ToolResult::Failure(failure) => {
            panic!(
                "dispatcher should reach capture backend: {:?}",
                failure.error
            )
        }
    }

    let captured = seen
        .lock()
        .unwrap()
        .take()
        .expect("dispatcher passes request to backend");
    assert_eq!(captured.tool_id, TOOL_ID);
    assert_eq!(captured.url, EMBED_URL);
    assert_eq!(captured.inputs, vec![("query".into(), "upeg".into())]);
    assert_eq!(captured.bindings, registry_bindings);
}

#[test]
fn controlled_embed_wait_option_parsing_applies_defaults() {
    let dir = std::env::temp_dir().join("upeg_loader_binding_wait_defaults");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "waitdefaults.tool";
    std::fs::write(
        dir.join("wait-defaults.toml"),
        single_tool_toml_str(&format!(
            r##"id = "{id}"
toolkit = "waitdefaults"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/wait-defaults"
controlled_embed = {{ bindings = [
  {{ role = "input", field = "input", selector = "#q", wait = {{}} }},
  {{ role = "trigger", field = "", selector = "button" }},
  {{ role = "output", field = "output", selector = "#r" }},
] }}"##,
        )),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    let bindings = upeg_runtime::selector_bindings_for(id);
    let wait = bindings[0]
        .wait
        .as_ref()
        .expect("wait = {} must create binding wait settings");
    assert_eq!(wait.for_selector.as_deref(), Some("#q"));
    assert_eq!(wait.condition, BindingWaitCondition::Exists);
    assert_eq!(wait.timeout_ms, DEFAULT_CONTROLLED_EMBED_WAIT_TIMEOUT_MS);
    assert_eq!(wait.settle_ms, DEFAULT_CONTROLLED_EMBED_WAIT_SETTLE_MS);
    assert_eq!(wait.on_timeout, BindingWaitOnTimeout::Fail);
    assert!(
        bindings[1].wait.is_none(),
        "wait absence must stay unchanged"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn controlled_embed_wait_option_parsing_preserves_explicit_values() {
    let dir = std::env::temp_dir().join("upeg_loader_binding_wait_explicit");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "waitexplicit.tool";
    std::fs::write(
        dir.join("wait-explicit.toml"),
        single_tool_toml_str(&format!(
            r##"id = "{id}"
toolkit = "waitexplicit"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/wait-explicit"
controlled_embed = {{ bindings = [
  {{ role = "input", field = "input", selector = "#q", wait = {{ for_selector = "#ready", condition = "visible", timeout_ms = 1200, settle_ms = 250, on_timeout = "continue" }} }},
  {{ role = "trigger", field = "", selector = "button" }},
  {{ role = "output", field = "output", selector = "#r" }},
] }}"##,
        )),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    let bindings = upeg_runtime::selector_bindings_for(id);
    let wait = bindings[0]
        .wait
        .as_ref()
        .expect("explicit wait table must create binding wait settings");
    assert_eq!(wait.for_selector.as_deref(), Some("#ready"));
    assert_eq!(wait.condition, BindingWaitCondition::Visible);
    assert_eq!(wait.timeout_ms, 1200);
    assert_eq!(wait.settle_ms, 250);
    assert_eq!(wait.on_timeout, BindingWaitOnTimeout::Continue);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn controlled_embed_wait_option_validation_rejects_unknown_condition() {
    let s = r##"id = "waitinvalid.condition"
toolkit = "waitinvalid"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/invalid"
controlled_embed = { bindings = [
  { role = "input", field = "input", selector = "#q", wait = { condition = "attached" } },
  { role = "trigger", field = "", selector = "button" },
  { role = "output", field = "output", selector = "#r" },
] }"##;

    match parse_fixture_tool(s) {
        Err(LoadError::InvalidBindingWaitCondition {
            position,
            condition,
        }) => {
            assert_eq!(position, 0);
            assert_eq!(condition, "attached");
        }
        other => panic!("unknown wait condition must be rejected, got {other:?}"),
    }
}

#[test]
fn controlled_embed_wait_option_validation_rejects_unknown_on_timeout() {
    let s = r##"id = "waitinvalid.on_timeout"
toolkit = "waitinvalid"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/invalid"
controlled_embed = { bindings = [
  { role = "input", field = "input", selector = "#q", wait = { on_timeout = "skip" } },
  { role = "trigger", field = "", selector = "button" },
  { role = "output", field = "output", selector = "#r" },
] }"##;

    match parse_fixture_tool(s) {
        Err(LoadError::InvalidBindingWaitOnTimeout {
            position,
            on_timeout,
        }) => {
            assert_eq!(position, 0);
            assert_eq!(on_timeout, "skip");
        }
        other => panic!("unknown wait on_timeout must be rejected, got {other:?}"),
    }
}

#[test]
fn controlled_embed_wait_option_validation_rejects_over_max_ms() {
    for field in ["timeout_ms", "settle_ms"] {
        let value = MAX_CONTROLLED_EMBED_WAIT_MS + 1;
        let s = format!(
            r##"id = "waitinvalid.over_max"
toolkit = "waitinvalid"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/invalid"
controlled_embed = {{ bindings = [
  {{ role = "input", field = "input", selector = "#q", wait = {{ {field} = {value} }} }},
  {{ role = "trigger", field = "", selector = "button" }},
  {{ role = "output", field = "output", selector = "#r" }},
] }}"##
        );

        match parse_fixture_tool(&s) {
            Err(LoadError::OverMaxBindingWaitMs {
                position,
                field: got_field,
                value: got_value,
                max,
            }) => {
                assert_eq!(position, 0);
                assert_eq!(got_field, field);
                assert_eq!(got_value, value);
                assert_eq!(max, MAX_CONTROLLED_EMBED_WAIT_MS);
            }
            other => panic!("over-max {field} must be rejected, got {other:?}"),
        }
    }
}

#[test]
fn controlled_embed_wait_option_validation_rejects_unknown_wait_key() {
    let s = r##"id = "waitinvalid.unknown_key"
toolkit = "waitinvalid"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/invalid"
controlled_embed = { bindings = [
  { role = "input", field = "input", selector = "#q", wait = { poll_ms = 10 } },
  { role = "trigger", field = "", selector = "button" },
  { role = "output", field = "output", selector = "#r" },
] }"##;

    match parse_fixture_tool(s) {
        Err(LoadError::UnknownBindingWaitKey { position, key }) => {
            assert_eq!(position, 0);
            assert_eq!(key, "poll_ms");
        }
        other => panic!("unknown wait key must be rejected, got {other:?}"),
    }
}

#[test]
fn controlled_embed_wait_option_validation_rejects_negative_ms_as_toml_type_error() {
    let s = r##"id = "waitinvalid.negative"
toolkit = "waitinvalid"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/invalid"
controlled_embed = { bindings = [
  { role = "input", field = "input", selector = "#q", wait = { timeout_ms = -1 } },
  { role = "trigger", field = "", selector = "button" },
  { role = "output", field = "output", selector = "#r" },
] }"##;

    match parse_fixture_tool(s) {
        Err(LoadError::Toml(error)) => {
            let message = error.to_string();
            assert!(message.contains("timeout_ms"), "{message}");
            assert!(message.contains("u64"), "{message}");
        }
        other => panic!("negative timeout_ms must be rejected as TOML type error, got {other:?}"),
    }
}

#[test]
fn controlled_embed_validation_rejects_missing_custom_user_agent() {
    let s = r#"id = "task4invalid.custom_ua"
toolkit = "task4invalid"
invoker = "External"
command = "echo"
controlled_embed = { browser = { user_agent = "custom" } }"#;

    match parse_fixture_tool(s) {
        Err(LoadError::MissingControlledEmbedCustomUserAgent) => {}
        other => panic!("missing custom_user_agent must be rejected, got {other:?}"),
    }
}

#[test]
fn controlled_embed_validation_rejects_missing_custom_viewport_dimensions() {
    for (body, expected_field) in [
        (
            r#"controlled_embed = { browser = { viewport = "custom", viewport_height = 720 } }"#,
            "viewport_width",
        ),
        (
            r#"controlled_embed = { browser = { viewport = "custom", viewport_width = 1280 } }"#,
            "viewport_height",
        ),
    ] {
        let s = format!(
            r#"id = "task4invalid.viewport_missing"
toolkit = "task4invalid"
invoker = "External"
command = "echo"
{body}"#
        );
        match parse_fixture_tool(&s) {
            Err(LoadError::MissingControlledEmbedViewportDimension { field }) => {
                assert_eq!(field, expected_field);
            }
            other => panic!("missing {expected_field} must be rejected, got {other:?}"),
        }
    }
}

#[test]
fn controlled_embed_validation_rejects_out_of_range_viewport_dimensions() {
    for (body, expected_field, expected_value) in [
        (
            r#"controlled_embed = { browser = { viewport = "custom", viewport_width = 0, viewport_height = 720 } }"#,
            "viewport_width",
            0,
        ),
        (
            r#"controlled_embed = { browser = { viewport = "custom", viewport_width = 1280, viewport_height = 4097 } }"#,
            "viewport_height",
            4097,
        ),
    ] {
        let s = format!(
            r#"id = "task4invalid.viewport_range"
toolkit = "task4invalid"
invoker = "External"
command = "echo"
{body}"#
        );
        match parse_fixture_tool(&s) {
            Err(LoadError::ControlledEmbedViewportDimensionOutOfRange { field, value, .. }) => {
                assert_eq!(field, expected_field);
                assert_eq!(value, expected_value);
            }
            other => panic!("out-of-range {expected_field} must be rejected, got {other:?}"),
        }
    }
}

#[test]
fn controlled_embed_validation_rejects_custom_only_fields_in_other_modes() {
    let cases = [
        (
            r#"controlled_embed = { browser = { custom_user_agent = "UA" } }"#,
            "custom_user_agent",
        ),
        (
            r#"controlled_embed = { browser = { user_agent = "mobile_safari", custom_user_agent = "UA" } }"#,
            "custom_user_agent",
        ),
        (
            r#"controlled_embed = { browser = { viewport = "mobile", viewport_width = 390 } }"#,
            "viewport_width",
        ),
        (
            "controlled_embed = { browser = { viewport_height = 844 } }",
            "viewport_height",
        ),
    ];

    for (body, expected_field) in cases {
        let s = format!(
            r#"id = "task4invalid.unexpected_field"
toolkit = "task4invalid"
invoker = "External"
command = "echo"
{body}"#
        );
        match (expected_field, parse_fixture_tool(&s)) {
            ("custom_user_agent", Err(LoadError::UnexpectedControlledEmbedCustomUserAgent)) => {}
            (field, Err(LoadError::UnexpectedControlledEmbedViewportDimension { field: got })) => {
                assert_eq!(got, field);
            }
            (_, other) => panic!("unexpected {expected_field} must be rejected, got {other:?}"),
        }
    }
}

#[test]
fn controlled_embed_validation_rejects_unknown_user_agent_and_viewport_labels() {
    for (body, expected_field) in [
        (
            r#"controlled_embed = { browser = { user_agent = "desktop_chrome" } }"#,
            "user_agent",
        ),
        (
            r#"controlled_embed = { browser = { viewport = "watch" } }"#,
            "viewport",
        ),
    ] {
        let s = format!(
            r#"id = "task4invalid.unknown_label"
toolkit = "task4invalid"
invoker = "External"
command = "echo"
{body}"#
        );
        match (expected_field, parse_fixture_tool(&s)) {
            ("user_agent", Err(LoadError::UnknownControlledEmbedUserAgent { user_agent })) => {
                assert_eq!(user_agent, "desktop_chrome");
            }
            ("viewport", Err(LoadError::UnknownControlledEmbedViewport { viewport })) => {
                assert_eq!(viewport, "watch");
            }
            (_, other) => panic!("unknown {expected_field} label must be rejected, got {other:?}"),
        }
    }
}

#[test]
fn controlled_embed_validation_rejects_non_trigger_enter_action() {
    let s = r##"id = "task4invalid.non_trigger_enter"
toolkit = "task4invalid"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/invalid"
controlled_embed = { bindings = [
  { role = "input", field = "input", selector = "#q", action = "enter" },
  { role = "trigger", field = "", selector = "button" },
  { role = "output", field = "output", selector = "#r" },
] }"##;

    match parse_fixture_tool(s) {
        Err(LoadError::NonTriggerSelectorBindingAction {
            position, action, ..
        }) => {
            assert_eq!(position, 0);
            assert_eq!(action, "enter");
        }
        other => panic!("non-trigger enter action must be rejected, got {other:?}"),
    }
}

#[test]
fn controlled_embed_validation_rejects_unknown_trigger_action() {
    let s = r##"id = "task4invalid.unknown_action"
toolkit = "task4invalid"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/invalid"
controlled_embed = { bindings = [
  { role = "input", field = "input", selector = "#q" },
  { role = "trigger", field = "", selector = "button", action = "submit" },
  { role = "output", field = "output", selector = "#r" },
] }"##;

    match parse_fixture_tool(s) {
        Err(LoadError::UnknownSelectorBindingAction { position, action }) => {
            assert_eq!(position, 1);
            assert_eq!(action, "submit");
        }
        other => panic!("unknown trigger action must be rejected, got {other:?}"),
    }
}

#[test]
fn controlled_embed_wait_option_parsing_preserves_zero_timeout_and_settle() {
    let dir = std::env::temp_dir().join("upeg_loader_wait_zero_values");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "waitzero.tool";
    std::fs::write(
        dir.join("wait-zero.toml"),
        single_tool_toml_str(
            r##"id = "waitzero.tool"
toolkit = "waitzero"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/wait-zero"
controlled_embed = { bindings = [
  { role = "input", field = "input", selector = "#q", wait = { timeout_ms = 0, settle_ms = 0 } },
  { role = "trigger", field = "", selector = "button" },
  { role = "output", field = "output", selector = "#r" },
] }"##,
        ),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    let bindings = upeg_runtime::selector_bindings_for(id);
    let wait = bindings[0]
        .wait
        .as_ref()
        .expect("wait with 0 values must create binding wait settings");
    assert_eq!(wait.timeout_ms, 0, "timeout_ms = 0 must be preserved");
    assert_eq!(wait.settle_ms, 0, "settle_ms = 0 must be preserved");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn controlled_embed_wait_option_validation_allows_exact_max_ms() {
    let dir = std::env::temp_dir().join("upeg_loader_wait_max_boundary");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "waitmax.tool";
    let max_value = MAX_CONTROLLED_EMBED_WAIT_MS;
    std::fs::write(
        dir.join("wait-max.toml"),
        single_tool_toml_str(&format!(
            r##"id = "waitmax.tool"
toolkit = "waitmax"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/wait-max"
controlled_embed = {{ bindings = [
  {{ role = "input", field = "input", selector = "#q", wait = {{ timeout_ms = {max_value}, settle_ms = {max_value} }} }},
  {{ role = "trigger", field = "", selector = "button" }},
  {{ role = "output", field = "output", selector = "#r" }},
] }}"##,
        )),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    let bindings = upeg_runtime::selector_bindings_for(id);
    let wait = bindings[0]
        .wait
        .as_ref()
        .expect("wait with MAX value must create binding wait settings");
    assert_eq!(
        wait.timeout_ms, max_value,
        "timeout_ms = MAX must be allowed"
    );
    assert_eq!(wait.settle_ms, max_value, "settle_ms = MAX must be allowed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn controlled_embed_wait_option_validation_rejects_empty_for_selector() {
    let s = r##"id = "waitinvalid.empty_for_selector"
toolkit = "waitinvalid"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/invalid"
controlled_embed = { bindings = [
  { role = "input", field = "input", selector = "#q", wait = { for_selector = "   " } },
  { role = "trigger", field = "", selector = "button" },
  { role = "output", field = "output", selector = "#r" },
] }"##;

    match parse_fixture_tool(s) {
        Err(LoadError::InvalidBindingWait { position }) => {
            assert_eq!(position, 0);
        }
        other => panic!("whitespace-only for_selector must be rejected, got {other:?}"),
    }
}

#[test]
fn controlled_embed_wait_option_parsing_registers_trimmed_for_selector() {
    let dir = std::env::temp_dir().join("upeg_loader_wait_for_selector_trim");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "waittrim.tool";
    std::fs::write(
        dir.join("wait-trim.toml"),
        single_tool_toml_str(
            r##"id = "waittrim.tool"
toolkit = "waittrim"
pin = "ControlledEmbed"
invoker = "Embed"
embed_url = "https://example.com/wait-trim"
controlled_embed = { bindings = [
  { role = "input", field = "input", selector = "#q", wait = { for_selector = "  #ready  " } },
  { role = "trigger", field = "", selector = "button" },
  { role = "output", field = "output", selector = "#r" },
] }"##,
        ),
    )
    .unwrap();

    let outcome = load_and_register_dir_verbose(&dir);
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    let bindings = upeg_runtime::selector_bindings_for(id);
    let wait = bindings[0]
        .wait
        .as_ref()
        .expect("wait with whitespace-padded for_selector must create binding wait settings");
    assert_eq!(
        wait.for_selector.as_deref(),
        Some("#ready"),
        "for_selector must be trimmed"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
