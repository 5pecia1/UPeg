//! Runtime/application boundary for upeg.
//!
//! `upeg-core` owns pure domain types. This crate owns mutable application
//! state: runtime Tool/Toolkit registries, executable dispatchers, trigger and
//! embed sidecars, board execution contexts, and surface network-status flags.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::get_unwrap,
        clippy::tests_outside_test_module,
        clippy::print_stdout,
        clippy::unreachable,
        clippy::string_add,
        clippy::manual_let_else,
        reason = "tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
    )
)]

mod approval;
mod cancel;
pub mod controlled_embed;
#[cfg(all(feature = "controlled-embed", not(target_arch = "wasm32")))]
pub mod controlled_embed_headless;
mod credentials;
mod dispatch;
mod embed;
mod execution;
pub mod execution_requirements;
pub mod interface_inventory;
pub mod manifest;
mod output_text;
pub mod pegboard;
pub mod pegboard_project;
#[cfg(not(target_arch = "wasm32"))]
pub mod persistence;
pub mod progress;
pub mod project_scope;
mod provenance;
pub mod readiness;
pub mod search;
pub mod selector_pipeline;
mod toolbox;
mod triggers;

pub use approval::{ToolApprovalPolicy, set_tool_approval_policy, tool_approval_policy};
pub use cancel::{CancellationToken, active_cancellation, with_cancellation};
pub use credentials::{set_tool_credential_names, tool_credential_names};
pub use dispatch::{
    DispatchArgs, DispatchArgsError, INVALID_ARGS_CODE, RuntimeDispatcherRegistration,
    has_runtime_dispatcher, register_runtime_dispatcher, register_runtime_dispatcher_managed,
    register_single_text_runtime_dispatcher, register_single_text_runtime_dispatcher_managed,
    single_text_dispatcher, single_text_result, tool_failure, tool_failure_with_details,
    tool_result_text, try_runtime_dispatch,
};
pub use embed::{
    BindingRole, ControlledEmbedSettings, SelectorBinding, clear_embed_url,
    controlled_embed_settings_for, controlled_embed_settings_owned_for, embed_url_for,
    register_embed_url, selector_bindings_for, selector_bindings_json_for,
    selector_bindings_owned_for, set_controlled_embed_settings,
    set_controlled_embed_settings_owned, set_selector_bindings, set_selector_bindings_owned,
};
pub use execution::{
    ExecutionContext, ProjectContext, RegistryProjectContext, add_surface_context,
    apply_board_context, apply_execution_context, apply_principal_context, apply_surface_context,
    apply_trigger_context, empty_args_preset, inherit_call_context, merge_args_with_preset,
};
pub use output_text::{
    output_value_canonical_wire_text, output_value_text, tool_success_primary_canonical_wire_text,
    tool_success_primary_text,
};
pub use progress::{
    PROGRESS_STREAM_STDERR, PROGRESS_STREAM_STDOUT, ProgressEvent, ProgressReporter, ProgressSink,
    ProgressStream, SharedProgressSink, active_progress_sink, progress_channel, with_progress_sink,
};
pub use provenance::{
    ToolProvenance, clear_tool_provenance, mcp_import_tool_count, register_tool_provenance,
    tool_provenance,
};
pub use search::search_toolbox_tools;
pub use toolbox::{
    ToolMetaRuntimeExt, ToolboxRegistration, ToolboxRegistrationGroup, board_context,
    boards_for_surface, register_board_context, tags_for_surface, tool_json_entries_for_surface,
    toolbox_add_builtin_catalog_tool, toolbox_add_single_text_tool_with_dispatcher,
    toolbox_add_single_text_tool_with_dispatcher_managed, toolbox_add_tool,
    toolbox_add_tool_managed, toolbox_add_tool_with_dispatcher,
    toolbox_add_tool_with_dispatcher_managed, toolbox_add_toolkit, toolbox_has_id,
    toolbox_has_tool_key, toolbox_is_builtin, toolbox_registered_tool, toolbox_tool,
    toolbox_tool_in_toolkit, toolbox_toolkit, toolbox_toolkits, toolbox_tools,
    toolkits_for_surface, tools_for_toolkit_on_surface, tools_list_json_for_surface,
    tools_on_board_for_surface, tools_with_tag_on_surface, validate_toolbox_addition,
};
pub use triggers::{
    FiredTrigger, TRIGGER_SOURCES, TriggerBinding, TriggerConditionRequirement, TriggerSource,
    UnknownTriggerSource, is_valid_trigger_source, manual_fire_trigger_label,
    registered_trigger_bindings,
    schedule::{ScheduleCondition, ScheduleConditionError, WATCH_POLL_INTERVAL},
    set_trigger_bindings, trigger_bindings_for, trigger_bindings_json_for,
    trigger_source_diagnostic, trigger_source_has_builtin_runtime, webhook_trigger_label,
};

/// Environment variable that flips HTTP server bind from loopback-only
/// to any-interface. Lives here so both the runtime [`NetworkStatus`]
/// snapshot and the upeg-cli HTTP surface read the same canonical name.
pub const HTTP_ALLOW_NON_LOOPBACK_ENV: &str = "UPEG_HTTP_ALLOW_NON_LOOPBACK";

static MCP_INTERFACE_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static HTTP_INTERFACE_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NetworkInterface {
    Mcp,
    Http,
}

#[derive(Debug)]
/// Resets a network-interface status flag when dropped.
///
/// Keep this guard alive for the lifetime of the MCP or HTTP listener so
/// [`NetworkStatus::current`] can report active interfaces accurately.
pub struct NetworkStatusGuard {
    interface: NetworkInterface,
}

impl Drop for NetworkStatusGuard {
    fn drop(&mut self) {
        match self.interface {
            NetworkInterface::Mcp => set_mcp_active(false),
            NetworkInterface::Http => set_http_active(false),
        }
    }
}

fn set_mcp_active(active: bool) {
    MCP_INTERFACE_ACTIVE.store(active, std::sync::atomic::Ordering::Relaxed);
}

fn set_http_active(active: bool) {
    HTTP_INTERFACE_ACTIVE.store(active, std::sync::atomic::Ordering::Relaxed);
}

/// Mark the MCP interface as active until the returned guard is dropped.
pub fn mark_mcp_active() -> NetworkStatusGuard {
    set_mcp_active(true);
    NetworkStatusGuard {
        interface: NetworkInterface::Mcp,
    }
}

/// Mark the HTTP interface as active until the returned guard is dropped.
pub fn mark_http_active() -> NetworkStatusGuard {
    set_http_active(true);
    NetworkStatusGuard {
        interface: NetworkInterface::Http,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Snapshot of runtime network-facing surface state.
pub struct NetworkStatus {
    /// Whether the MCP stdio/server interface is currently active.
    pub mcp_active: bool,
    /// Whether the HTTP interface is currently active.
    pub http_active: bool,
    /// Number of configured trigger bindings.
    pub trigger_count: usize,
    /// Whether HTTP may bind beyond loopback.
    pub remote_bind_allowed: bool,
}

impl NetworkStatus {
    /// Read the current network status and pair it with the given trigger count.
    pub fn current(trigger_count: usize) -> Self {
        Self {
            mcp_active: MCP_INTERFACE_ACTIVE.load(std::sync::atomic::Ordering::Relaxed),
            http_active: HTTP_INTERFACE_ACTIVE.load(std::sync::atomic::Ordering::Relaxed),
            trigger_count,
            remote_bind_allowed: std::env::var(HTTP_ALLOW_NON_LOOPBACK_ENV)
                .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true")),
        }
    }

    /// Render a compact human-readable status label for CLI/UI surfaces.
    pub fn label(&self) -> String {
        format!(
            "MCP {} · HTTP {} · Trigger:{} · {}",
            if self.mcp_active { "on" } else { "off" },
            if self.http_active { "on" } else { "off" },
            self.trigger_count,
            if self.remote_bind_allowed {
                "remote-bind allowed"
            } else {
                "loopback-only"
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use upeg_core::{
        ALL_SURFACES, InputSpec, Invoker, PegboardUnits, PinKind, Surface, ToolId, ToolMeta,
    };

    fn local_id_for(id: &'static str, toolkit: &'static str) -> &'static str {
        ToolId::parse_canonical_in_toolkit(id, toolkit)
            .expect("test ToolMeta id must be in canonical form")
            .local()
    }

    fn meta(id: &'static str) -> ToolMeta {
        ToolMeta {
            id,
            toolkit: "test",
            local_id: local_id_for(id, "test"),
            tags: &[],
            display_label: "Test tool",
            description: "",
            input_spec: InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            effect: upeg_core::ToolEffect::Unknown,
            presentation: None,
            source: upeg_core::Source::UserInput,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: ALL_SURFACES,
            boards: &[],
        }
    }

    #[test]
    fn tool_replacement_clears_previous_external_execution_requirements() {
        use crate::execution_requirements::{
            ToolExecutionRequirements, set_tool_execution_requirements, tool_execution_requirements,
        };
        const ID: &str = "test.requirements_replacement";
        toolbox_add_tool(meta(ID));
        set_tool_execution_requirements(
            ID,
            Some(ToolExecutionRequirements {
                command: Some("old-command".into()),
                ..Default::default()
            }),
        );

        toolbox_add_tool(meta(ID));

        assert!(tool_execution_requirements(ID).is_none());
    }

    #[test]
    fn unregistration_clears_execution_requirements_and_preserves_replaced_registration() {
        use crate::execution_requirements::{
            ToolExecutionRequirements, set_tool_execution_requirements, tool_execution_requirements,
        };
        const ID: &str = "test.requirements_unload";
        let old = toolbox_add_tool_managed(meta(ID));
        let current = toolbox_add_tool_managed(meta(ID));
        set_tool_execution_requirements(
            ID,
            Some(ToolExecutionRequirements {
                command: Some("current-command".into()),
                ..Default::default()
            }),
        );
        drop(old);
        assert_eq!(
            tool_execution_requirements(ID)
                .expect("current registration must remain")
                .command
                .as_deref(),
            Some("current-command")
        );

        drop(current);

        assert!(tool_execution_requirements(ID).is_none());
    }

    #[test]
    fn network_status_label_is_explicit_not_placeholder() {
        let status = NetworkStatus {
            mcp_active: false,
            http_active: true,
            trigger_count: 3,
            remote_bind_allowed: false,
        };
        let label = status.label();
        assert_eq!(label, "MCP off · HTTP on · Trigger:3 · loopback-only");
        assert!(!label.contains("bg tasks"));
        assert!(!label.contains("net OK"));
    }

    #[test]
    fn current_network_status_reads_runtime_interface_flags() {
        set_mcp_active(false);
        set_http_active(false);
        let idle = NetworkStatus::current(2);
        assert!(!idle.mcp_active);
        assert!(!idle.http_active);
        assert_eq!(idle.trigger_count, 2);

        {
            let _mcp = mark_mcp_active();
            let _http = mark_http_active();
            let active = NetworkStatus::current(4);
            assert!(active.mcp_active);
            assert!(active.http_active);
            assert!(active.label().contains("MCP on · HTTP on · Trigger:4"));
        }

        let after_drop = NetworkStatus::current(0);
        assert!(!after_drop.mcp_active);
        assert!(!after_drop.http_active);
    }

    #[test]
    fn trigger_sources_have_runtime_diagnostics_for_every_prd_adapter() {
        assert_eq!(
            TRIGGER_SOURCES,
            &[
                "webhook",
                "schedule",
                "file",
                "directory",
                "clipboard",
                "hotkey"
            ]
        );
        for source in TRIGGER_SOURCES {
            assert!(is_valid_trigger_source(source), "{source} must be valid");
            let diagnostic = trigger_source_diagnostic(source);
            assert!(
                !diagnostic.trim().is_empty() && diagnostic != "unknown trigger source",
                "{source} must have a concrete diagnostic"
            );
        }
        for supported in ["webhook", "schedule", "file", "directory", "clipboard"] {
            assert!(
                trigger_source_has_builtin_runtime(supported),
                "{supported} must report built-in runtime support"
            );
        }
        // `hotkey` is the only remaining host-bound source: it needs a platform
        // global-hotkey adapter, so the built-in runtime reports it unsupported.
        assert!(
            !trigger_source_has_builtin_runtime(TriggerSource::Hotkey.as_str()),
            "hotkey requires a host adapter diagnostic"
        );
    }

    #[test]
    fn runtime_tool_registration_replaces_existing_metadata() {
        let id = "test.runtime.dedupe";
        toolbox_add_tool(ToolMeta {
            description: "first",
            ..meta(id)
        });
        toolbox_add_tool(ToolMeta {
            description: "second",
            ..meta(id)
        });

        let count = toolbox_tools().filter(|t| t.id == id).count();
        assert_eq!(count, 1);
        assert_eq!(
            toolbox_tool(id).expect("registered tool").description,
            "second"
        );
    }

    #[test]
    fn managed_runtime_tool_registration_is_removed_on_drop() {
        let id = "test.runtime.managed_drop";
        {
            let _guard = toolbox_add_tool_managed(meta(id));
            assert!(toolbox_tool(id).is_some());
        }
        assert!(toolbox_tool(id).is_none());
    }

    #[test]
    fn managed_runtime_dispatcher_is_removed_on_drop() {
        let id = "test.runtime.managed_dispatcher_drop";
        toolbox_add_tool(meta(id));
        {
            let _guard =
                register_single_text_runtime_dispatcher_managed(id, |_| Ok("ok".to_string()));
            assert_eq!(
                try_runtime_dispatch(id, &serde_json::json!({})).map(tool_result_text),
                Some(Ok("ok".to_string()))
            );
        }
        assert_eq!(try_runtime_dispatch(id, &serde_json::json!({})), None);
    }

    #[test]
    fn stale_managed_guard_does_not_remove_new_registration() {
        let id = "test.runtime.managed_generation";
        let old = toolbox_add_tool_managed(ToolMeta {
            description: "old",
            ..meta(id)
        });
        let _new = toolbox_add_tool_managed(ToolMeta {
            description: "new",
            ..meta(id)
        });
        drop(old);

        assert_eq!(
            toolbox_tool(id)
                .expect("new registration must remain")
                .description,
            "new"
        );
    }

    #[test]
    fn runtime_dispatch_validates_required_schema_args_before_calling_dispatcher() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let id = "test.runtime.required_schema";
        toolbox_add_tool(ToolMeta {
            input_spec: InputSpec::try_from(&serde_json::json!({
                "type": "object",
                "properties": { "input": { "type": "string" } },
                "required": ["input"],
                "additionalProperties": false,
            }))
            .expect("test input spec must parse"),
            invoker: Invoker::External,
            ..meta(id)
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_dispatcher = Arc::clone(&calls);
        register_single_text_runtime_dispatcher(id, move |_| {
            calls_for_dispatcher.fetch_add(1, Ordering::SeqCst);
            Ok("called".into())
        });

        let result = try_runtime_dispatch(id, &serde_json::json!({}));
        assert_eq!(
            result.map(tool_result_text),
            Some(Err("input `input` is required".to_string()))
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn runtime_dispatch_allows_required_boolean_false() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let id = "test.runtime.required_bool_false";
        toolbox_add_tool(ToolMeta {
            input_spec: InputSpec::try_from(&serde_json::json!({
                "type": "object",
                "properties": { "enabled": { "type": "boolean" } },
                "required": ["enabled"],
                "additionalProperties": false,
            }))
            .expect("test input spec must parse"),
            invoker: Invoker::External,
            ..meta(id)
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_dispatcher = Arc::clone(&calls);
        register_single_text_runtime_dispatcher(id, move |_| {
            calls_for_dispatcher.fetch_add(1, Ordering::SeqCst);
            Ok("called".into())
        });

        let result = try_runtime_dispatch(id, &serde_json::json!({ "enabled": false }));
        assert_eq!(result.map(tool_result_text), Some(Ok("called".into())));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn json_object_conversion_round_trips_embed_and_trigger_metadata() {
        let id = "test.runtime.embed_round_trip";
        toolbox_add_tool(ToolMeta {
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            effect: upeg_core::ToolEffect::Unknown,
            presentation: None,
            source: upeg_core::Source::UserInput,
            pin: PinKind::ControlledEmbed,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: Invoker::Embed,
            surfaces: &[Surface::Desktop],
            ..meta(id)
        });
        register_embed_url(id, "https://example.org/runtime");
        set_selector_bindings(
            id,
            vec![SelectorBinding {
                role: upeg_core::BindingRole::Input,
                field: "input".into(),
                selector: ".q".into(),
                trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
                wait: None,
            }],
        );
        set_trigger_bindings(
            id,
            vec![TriggerBinding {
                tool_id: id,
                source: "webhook".into(),
                condition: Some("ok".into()),
            }],
        );

        let v = toolbox_tool(id)
            .expect("tool must be registered")
            .to_json_object("id");
        assert_eq!(v["pegboardUnits"], "U1");
        assert_eq!(
            v["pegboardSpan"],
            serde_json::json!({ "cols": 1, "rows": 1 })
        );
        assert_eq!(v["embedUrl"], "https://example.org/runtime");
        assert_eq!(v["selectorBindings"][0]["field"], "input");
        assert_eq!(v["triggers"][0]["source"], "webhook");
    }

    #[test]
    fn embed_url_for_prefers_url_from_tool_metadata() {
        // PR #18→#19: when ToolMeta.output_spec declares
        // OutputKind::EmbeddedView { url }, that is the canonical embed URL.
        // The runtime-side registry (register_embed_url) must be a fallback
        // only for tools not yet migrated to the declarative outputs grammar,
        // such as the TOML loader. If both carry a value, the metadata URL
        // wins — that way one tool's embed URL can never diverge between the
        // GUI (metadata) and the CLI (registry).
        use upeg_core::{FieldConstraints, OutputFieldSpec, OutputKind, OutputSpec};
        let id = "test.runtime.embed_inventory_wins";
        let url_in_meta = "https://meta.example/declared";
        toolbox_add_tool(ToolMeta {
            output_spec: OutputSpec {
                fields: vec![OutputFieldSpec {
                    name: "view".into(),
                    label: Some("View".into()),
                    description: None,
                    kind: OutputKind::EmbeddedView {
                        url: url_in_meta.into(),
                    },
                    constraints: FieldConstraints::default(),
                }],
            },
            primary_output_id: Some("view"),
            effect: upeg_core::ToolEffect::Unknown,
            presentation: None,
            pin: PinKind::Embed,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Static,
            ..meta(id)
        });
        register_embed_url(id, "https://registry.example/fallback");
        assert_eq!(
            embed_url_for(id),
            Some(url_in_meta),
            "declarative OutputKind::EmbeddedView must win over the runtime registration"
        );
        clear_embed_url(id);
    }

    #[test]
    fn embed_url_for_returns_registry_url_when_only_registered() {
        // When the tool metadata does not declare an EmbeddedView — e.g. the
        // TOML loader only called the sidecar register_embed_url — the
        // registry value is exposed as-is so the fallback path stays valid.
        let id = "test.runtime.embed_registry_fallback";
        toolbox_add_tool(ToolMeta {
            pin: PinKind::Embed,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Static,
            ..meta(id)
        });
        register_embed_url(id, "https://registry.example/only");
        assert_eq!(
            embed_url_for(id),
            Some("https://registry.example/only"),
            "without EmbeddedView in the meta it must fall back to the runtime registry value"
        );
        clear_embed_url(id);
        assert_eq!(
            embed_url_for(id),
            None,
            "after clearing the registry, None must come back (confirming there is no meta fallback)"
        );
    }

    #[test]
    fn embed_url_for_returns_none_for_unregistered_tool() {
        // Regression pin: an unknown id returns None. If the old bug of a
        // "hardcoded URL fallback for unregistered ids" ever comes back, this
        // breaks immediately.
        assert_eq!(
            embed_url_for("test.runtime.unknown_embed"),
            None,
            "an id with neither registry nor meta must be None"
        );
    }
}
