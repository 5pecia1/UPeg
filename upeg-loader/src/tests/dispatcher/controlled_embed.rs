//! Controlled Embed dispatcher tests. Previously textually included into
//! `tests/dispatcher.rs` via `include!`; now a real child module. `use
//! super::*` inherits the parent test module's helpers (backends, locks,
//! constants, `call_dispatcher*`, `tool_success`/`tool_failure`).
use super::*;

/// The production loader registers metadata before any dispatcher runs.
/// These focused tests build a dispatcher directly, so install that same
/// output contract without requiring unrelated manifest/browser settings.
fn controlled_embed_dispatcher_for(
    parsed: &ToolToml,
) -> Option<impl for<'a> Fn(upeg_runtime::DispatchArgs<'a>) -> upeg_core::ToolResult> {
    let fields = parsed
        .outputs
        .iter()
        .cloned()
        .map(upeg_core::OutputFieldSpec::try_from)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let meta = upeg_runtime::manifest::lower_runtime_tool_manifest(
        upeg_runtime::manifest::RuntimeToolManifest {
            id: parsed.id.clone(),
            toolkit: parsed.toolkit.clone(),
            tags: Vec::new(),
            display_label: None,
            description: None,
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::new(fields).unwrap(),
            primary_output_id: parsed.primary_output_id.clone(),
            pin: upeg_core::PinKind::ControlledEmbed,
            pegboard_units: upeg_core::PegboardUnits::U2T,
            invoker: upeg_core::Invoker::Embed,
            surfaces: upeg_core::ALL_SURFACES.to_vec(),
            boards: Vec::new(),
        },
    )
    .unwrap();
    upeg_runtime::toolbox_add_tool(meta);
    crate::dispatcher::controlled_embed_dispatcher_for(parsed)
}

#[test]
fn controlled_embed_dispatcher는_backend_unavailable을_전용_error_code로_반환한다() {
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        upeg_runtime::controlled_embed::NoopControlledEmbedBackend,
    ));
    let _restore = RestoreNoopControlledEmbedBackend;

    // With the default `NoopControlledEmbedBackend` installed (no
    // `controlled-embed` feature compiled in), the dispatcher
    // exists but every call surfaces a clear "feature disabled"
    // error — no silent "dispatch not implemented" stub.
    let parsed = toml::from_str::<ToolToml>(
        r##"id = "embed.transform"
            toolkit = "embed"
            invoker = "Embed"
            embed_url = "https://example.com/tool"

            [[controlled_embed.bindings]]
              role = "input"
              field = "input"
              selector = "#in"

            [[controlled_embed.bindings]]
              role = "trigger"
              field = ""
              selector = "button"

            [[controlled_embed.bindings]]
              role = "output"
              field = "output"
              selector = "#out""##,
    )
    .unwrap();
    // Register the embed_url so the dispatcher can look it up; the
    // chain doesn't run loader::register_parsed_toolkit here.
    upeg_runtime::register_embed_url("embed.transform", "https://example.com/tool");
    let f = controlled_embed_dispatcher_for(&parsed).expect("controlled embed builds a dispatcher");
    let failure = tool_failure(call_dispatcher_result(
        &f,
        serde_json::json!({"input": "0xff"}),
    ));

    assert_eq!(failure.error.code, CONTROLLED_EMBED_UNAVAILABLE_ERROR_CODE);
    assert!(
        failure
            .error
            .message
            .contains("Controlled Embed feature is disabled"),
        "expected feature-disabled error, got: {}",
        failure.error.message
    );
}

#[test]
fn controlled_embed_dispatcher는_backend_failed를_실행_error_code로_반환한다() {
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = RestoreNoopControlledEmbedBackend;

    let parsed = toml::from_str::<ToolToml>(
        r##"id = "embed.backend_failed"
            toolkit = "embed"
            invoker = "Embed"
            embed_url = "https://example.com/tool"

            [[controlled_embed.bindings]]
              role = "input"
              field = "input"
              selector = "#in"

            [[controlled_embed.bindings]]
              role = "trigger"
              field = ""
              selector = "button"

            [[controlled_embed.bindings]]
              role = "output"
              field = "output"
              selector = "#out""##,
    )
    .unwrap();
    upeg_runtime::register_embed_url("embed.backend_failed", "https://example.com/tool");
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        BackendFailedControlledEmbedBackend,
    ));

    let f = controlled_embed_dispatcher_for(&parsed).expect("controlled embed builds a dispatcher");
    let failure = tool_failure(call_dispatcher_result(
        &f,
        serde_json::json!({"input": "0xff"}),
    ));

    assert_eq!(failure.error.code, CONTROLLED_EMBED_EXECUTION_ERROR_CODE);
    assert!(
        failure.error.message.contains(BACKEND_FAILED_MESSAGE),
        "expected backend failure message, got: {}",
        failure.error.message
    );
}

#[test]
fn controlled_embed_dispatcher는_wait_timeout을_표준_error_code로_반환한다() {
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = RestoreNoopControlledEmbedBackend;

    let parsed = toml::from_str::<ToolToml>(
        r##"id = "embed.wait_timeout"
            toolkit = "embed"
            invoker = "Embed"
            embed_url = "https://example.com/tool"

            [[controlled_embed.bindings]]
              role = "input"
              field = "input"
              selector = "#in"

            [[controlled_embed.bindings]]
              role = "trigger"
              field = ""
              selector = "button"

            [[controlled_embed.bindings]]
              role = "output"
              field = "output"
              selector = "#out""##,
    )
    .unwrap();
    upeg_runtime::register_embed_url("embed.wait_timeout", "https://example.com/tool");
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        WaitTimeoutControlledEmbedBackend,
    ));

    let f = controlled_embed_dispatcher_for(&parsed).expect("controlled embed builds a dispatcher");
    let failure = tool_failure(call_dispatcher_result(
        &f,
        serde_json::json!({"input": "hello"}),
    ));

    assert_eq!(failure.error.code, CONTROLLED_EMBED_WAIT_TIMEOUT_ERROR_CODE);
    assert!(
        failure
            .error
            .message
            .contains(WAIT_TIMEOUT_BINDING_SELECTOR)
            && failure.error.message.contains(WAIT_TIMEOUT_FOR_SELECTOR)
            && failure.error.message.contains(&WAIT_TIMEOUT_MS.to_string()),
        "expected wait timeout details, got: {}",
        failure.error.message
    );
}

#[test]
fn controlled_embed_dispatcher는_large_unicode_output을_정규_output으로_보존한다() {
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = RestoreNoopControlledEmbedBackend;

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "embed.large_unicode"
            toolkit = "embed"
            invoker = "Embed"
            embed_url = "https://example.com/tool"
            primary_output_id = "summary"
            outputs = [{ name = "summary", type = "string", label = "Summary" }]"#,
    )
    .unwrap();
    upeg_runtime::register_embed_url("embed.large_unicode", "https://example.com/tool");
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        LargeUnicodeOutputBackend,
    ));

    let f = controlled_embed_dispatcher_for(&parsed).expect("controlled embed builds a dispatcher");
    let success = tool_success(call_dispatcher_result(
        &f,
        serde_json::json!({"input": "0xff"}),
    ));

    assert_eq!(
        success.primary_output_id.as_deref(),
        Some(LARGE_UNICODE_OUTPUT_ID)
    );
    assert_eq!(success.outputs.len(), 1);
    assert_eq!(success.outputs[0].id, LARGE_UNICODE_OUTPUT_ID);
    assert_eq!(success.outputs[0].label.as_deref(), Some("Summary"));
    assert_eq!(
        success.outputs[0].value,
        OutputValue::String(large_unicode_output_text())
    );
}

struct SettingsCaptureBackend {
    seen: Arc<Mutex<Option<upeg_core::ControlledEmbedSettings>>>,
}

impl upeg_runtime::controlled_embed::ControlledEmbedBackend for SettingsCaptureBackend {
    fn run(
        &self,
        request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
    ) -> Result<
        upeg_runtime::controlled_embed::ControlledEmbedResponse,
        upeg_runtime::controlled_embed::ControlledEmbedError,
    > {
        *self.seen.lock().unwrap() = Some(request.settings);
        Ok(upeg_runtime::controlled_embed::ControlledEmbedResponse {
            outputs: vec![("output".into(), "ok".into())],
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ControlledEmbedChainCall {
    url: String,
    bindings: Vec<(String, String, String, String)>,
    inputs: Vec<(String, String)>,
    settings: upeg_core::ControlledEmbedSettings,
}

struct ChainCaptureBackend {
    seen: Arc<Mutex<Option<ControlledEmbedChainCall>>>,
}

impl upeg_runtime::controlled_embed::ControlledEmbedBackend for ChainCaptureBackend {
    fn run(
        &self,
        request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
    ) -> Result<
        upeg_runtime::controlled_embed::ControlledEmbedResponse,
        upeg_runtime::controlled_embed::ControlledEmbedError,
    > {
        let inputs: Vec<(String, String)> = request
            .inputs
            .iter()
            .map(|(field, value)| ((*field).to_string(), (*value).to_string()))
            .collect();
        let text = inputs
            .iter()
            .find_map(|(field, value)| (field == "text").then_some(value.as_str()))
            .unwrap_or_default()
            .to_uppercase();
        let bindings = request
            .bindings
            .iter()
            .map(|binding| {
                (
                    binding.role.label().to_string(),
                    binding.field.clone(),
                    binding.selector.clone(),
                    binding.trigger_action.label().to_string(),
                )
            })
            .collect();

        *self.seen.lock().unwrap() = Some(ControlledEmbedChainCall {
            url: request.url.to_string(),
            bindings,
            inputs,
            settings: request.settings,
        });

        Ok(upeg_runtime::controlled_embed::ControlledEmbedResponse {
            outputs: vec![("loud".into(), text)],
        })
    }
}

#[test]
fn controlled_embed_dispatcher는_등록된_settings를_request로_전달한다() {
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = RestoreNoopControlledEmbedBackend;

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "embed.settings_request"
            toolkit = "embed"
            invoker = "Embed"
            embed_url = "https://example.com/tool""#,
    )
    .unwrap();
    let expected = upeg_core::ControlledEmbedSettings {
        user_agent: Some(upeg_core::ControlledEmbedUserAgent::MobileSafari),
        viewport: Some(upeg_core::ControlledEmbedViewport::Preset(
            upeg_core::ControlledEmbedViewportPreset::Desktop,
        )),
    };
    upeg_runtime::register_embed_url("embed.settings_request", "https://example.com/tool");
    upeg_runtime::set_controlled_embed_settings("embed.settings_request", expected.clone());

    let seen = Arc::new(Mutex::new(None));
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        SettingsCaptureBackend {
            seen: Arc::clone(&seen),
        },
    ));

    let f = controlled_embed_dispatcher_for(&parsed).expect("controlled embed builds a dispatcher");
    let out = call_dispatcher(&f, serde_json::json!({"input": "hello"})).expect("backend runs");

    assert_eq!(out, r#"{"output":"ok"}"#);
    assert_eq!(*seen.lock().unwrap(), Some(expected));

    // FRB writes to the owned registry after the dispatcher was created.
    let override_settings = upeg_core::ControlledEmbedSettings {
        user_agent: Some(upeg_core::ControlledEmbedUserAgent::Custom(
            "UPeg test".into(),
        )),
        viewport: Some(upeg_core::ControlledEmbedViewport::Preset(
            upeg_core::ControlledEmbedViewportPreset::Mobile,
        )),
    };
    for settings in [
        override_settings,
        upeg_core::ControlledEmbedSettings::default(),
    ] {
        upeg_runtime::set_controlled_embed_settings_owned(parsed.id.clone(), settings.clone())
            .unwrap();
        call_dispatcher(&f, serde_json::json!({"input": "hello"})).expect("backend runs");
        assert_eq!(*seen.lock().unwrap(), Some(settings));
    }
}

#[test]
fn controlled_embed_dispatcher는_선언된_output_spec으로_dom_출력을_정규화한다() {
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = RestoreNoopControlledEmbedBackend;

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "embed.typed_output"
            toolkit = "embed"
            invoker = "Embed"
            embed_url = "https://example.com/tool"
            primary_output_id = "loud"
            outputs = [{ name = "loud", type = "string", label = "Loud" }]"#,
    )
    .unwrap();
    upeg_runtime::register_embed_url("embed.typed_output", "https://example.com/tool");

    let seen = Arc::new(Mutex::new(None));
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(ChainCaptureBackend {
        seen,
    }));

    let f = controlled_embed_dispatcher_for(&parsed).expect("controlled embed builds a dispatcher");
    let success = tool_success(call_dispatcher_result(
        &f,
        serde_json::json!({"text": "upeg"}),
    ));

    assert_eq!(success.primary_output_id.as_deref(), Some("loud"));
    assert_eq!(success.outputs[0].id, "loud");
    assert_eq!(success.outputs[0].label.as_deref(), Some("Loud"));
    assert_eq!(success.outputs[0].value, OutputValue::String("UPEG".into()));
}

#[test]
fn 체인은_컨트롤드_임베드의_내부_바인딩을_모른다() {
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = RestoreNoopControlledEmbedBackend;
    const EMBED_URL: &str = "data:text/html,<!DOCTYPE html><html><body><input id=\"text\" /><button id=\"go\" onclick=\"document.getElementById(%27out%27).textContent=document.getElementById(%27text%27).value.toUpperCase()\">go</button><div id=\"out\"></div></body></html>";
    const CONTROLLED_EMBED_TOOL_TOML: &str = r##"
id = "embed"
tags = ["embed", "demo"]

[[tools]]
id = "shout"
description = "Shout a word through a deterministic data-URI page."
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = 'data:text/html,<!DOCTYPE html><html><body><input id="text" /><button id="go" onclick="document.getElementById(%27out%27).textContent=document.getElementById(%27text%27).value.toUpperCase()">go</button><div id="out"></div></body></html>'

[tools.controlled_embed.browser]
user_agent = "mobile_safari"
viewport = "mobile"

[[tools.controlled_embed.bindings]]
role = "input"
field = "text"
selector = "#text"

[[tools.controlled_embed.bindings]]
role = "trigger"
selector = "#go"

[[tools.controlled_embed.bindings]]
role = "output"
field = "loud"
selector = "#out"
"##;
    const CHAIN_TOOL_TOML: &str = r#"
[[tools]]
id = "chain_shout"
pegboard_units = "U1"
invoker = "Chain"
output = "{{steps.shout.output}}"

[[tools.steps]]
id = "shout"
tool = "embed.shout"
args = '{"text":"{{input.word}}"}'
"#;

    for forbidden in ["controlled_embed", "selector", "browser", "pin"] {
        assert!(
            !CHAIN_TOOL_TOML.contains(forbidden),
            "Chain syntax must not mention ControlledEmbed metadata field `{forbidden}`"
        );
    }

    let dir = std::env::temp_dir().join("upeg_loader_chain_controlled_embed");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("embed_chain.toml"),
        format!("{CONTROLLED_EMBED_TOOL_TOML}\n{CHAIN_TOOL_TOML}"),
    )
    .unwrap();

    let (loaded, failed) = load_and_register_dir(&dir);
    assert_eq!((loaded, failed), (2, 0));

    let seen = Arc::new(Mutex::new(None));
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(ChainCaptureBackend {
        seen: Arc::clone(&seen),
    }));

    let out = upeg_runtime::try_runtime_dispatch(
        "embed.chain_shout",
        &serde_json::json!({"input": {"word": "upeg"}}),
    );
    let out = runtime_success_text(out);
    assert_eq!(out, r#"{"loud":"UPEG"}"#);

    let call = seen
        .lock()
        .unwrap()
        .take()
        .expect("inner ControlledEmbed tool should run through runtime registry");
    assert_eq!(call.url, EMBED_URL);
    assert_eq!(call.inputs, vec![("text".into(), "upeg".into())]);
    assert_eq!(
        call.bindings,
        vec![
            (
                "input".into(),
                "text".into(),
                "#text".into(),
                "click".into()
            ),
            (
                "trigger".into(),
                String::new(),
                "#go".into(),
                "click".into()
            ),
            (
                "output".into(),
                "loud".into(),
                "#out".into(),
                "click".into()
            ),
        ]
    );
    assert_eq!(
        call.settings,
        upeg_core::ControlledEmbedSettings {
            user_agent: Some(upeg_core::ControlledEmbedUserAgent::MobileSafari),
            viewport: Some(upeg_core::ControlledEmbedViewport::Preset(
                upeg_core::ControlledEmbedViewportPreset::Mobile,
            )),
        }
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn headless_dispatch는_owned_binding을_우선한다() {
    // E4 regression: the headless dispatcher must read GUI/FRB-authored
    // (owned) selector bindings, not only the compile-time static ones.
    // Register a static binding and an owned binding for the same tool id
    // with *different* selectors, then assert the backend the dispatcher
    // drives receives the owned selector (owned wins, static is shadowed).
    const OWNED_BINDING_TOOL_ID: &str = "embed.owned_binding";
    const STATIC_OUTPUT_SELECTOR: &str = "#static-output";
    const OWNED_OUTPUT_SELECTOR: &str = "#owned-output";

    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = RestoreNoopControlledEmbedBackend;

    let parsed = toml::from_str::<ToolToml>(
        r#"id = "embed.owned_binding"
            toolkit = "embed"
            invoker = "Embed"
            embed_url = "https://example.com/tool""#,
    )
    .unwrap();
    upeg_runtime::register_embed_url(OWNED_BINDING_TOOL_ID, "https://example.com/tool");

    // Static binding — as if authored through the `#[tool]` macro path.
    upeg_runtime::set_selector_bindings(
        OWNED_BINDING_TOOL_ID,
        vec![upeg_core::SelectorBinding {
            role: upeg_core::BindingRole::Output,
            field: "output".into(),
            selector: STATIC_OUTPUT_SELECTOR.into(),
            trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
            wait: None,
        }],
    );
    // Owned binding overriding it — as if authored in the GUI / through FRB.
    upeg_runtime::set_selector_bindings_owned(
        OWNED_BINDING_TOOL_ID.to_string(),
        vec![upeg_core::SelectorBinding {
            role: upeg_core::BindingRole::Output,
            field: "output".into(),
            selector: OWNED_OUTPUT_SELECTOR.into(),
            trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
            wait: None,
        }],
    )
    .expect("owned binding registered");

    let seen = Arc::new(Mutex::new(None));
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(ChainCaptureBackend {
        seen: Arc::clone(&seen),
    }));

    let f = controlled_embed_dispatcher_for(&parsed).expect("controlled embed builds a dispatcher");
    let _ = call_dispatcher(&f, serde_json::json!({"input": "hi"})).expect("backend runs");

    let call = seen
        .lock()
        .unwrap()
        .take()
        .expect("dispatcher drove the backend");
    let selectors: Vec<&str> = call
        .bindings
        .iter()
        .map(|(_role, _field, selector, _action)| selector.as_str())
        .collect();
    assert!(
        selectors.contains(&OWNED_OUTPUT_SELECTOR),
        "headless dispatcher must see the owned binding, got: {selectors:?}"
    );
    assert!(
        !selectors.contains(&STATIC_OUTPUT_SELECTOR),
        "owned binding must shadow the static one, got: {selectors:?}"
    );
}
