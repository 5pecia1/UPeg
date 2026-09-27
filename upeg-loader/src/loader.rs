use crate::LoadError;
use crate::ToolToml;
use crate::dispatcher::{
    chain_dispatcher_for, controlled_embed_dispatcher_for, external_dispatcher_for,
    http_dispatcher_for, llm_dispatcher_for, static_dispatcher_for, wasm_dispatcher_for,
};
use crate::manifest_origin::ManifestOrigin;
use crate::parse::controlled_embed::{binding_wait_from_toml, parse_selector_binding_action};
#[cfg(test)]
use crate::parse::parse_toolkit_full;
use crate::parse::{SkippedTool, parse_manifest};
use std::path::Path;
use upeg_core::{
    ControlledEmbedSettings, ControlledEmbedUserAgent, ControlledEmbedViewport,
    ControlledEmbedViewportPreset, ToolMeta, ToolkitMeta,
};
use upeg_runtime::{
    SelectorBinding, TriggerBinding, clear_embed_url, register_embed_url,
    set_controlled_embed_settings, set_selector_bindings, set_tool_credential_names,
    set_trigger_bindings, toolbox_add_single_text_tool_with_dispatcher,
    toolbox_add_tool_with_dispatcher, toolbox_add_toolkit,
};

#[cfg(test)]
pub(crate) type ParsedToolkit = (ToolkitMeta, Vec<(ToolMeta, ToolToml)>);

/// Parse every `*.toml` file under `dir` into Toolkit manifests. Files that fail
/// to parse are returned as `Err` entries so callers can decide whether
/// to log + continue or abort. Reading the directory itself failing is
/// returned as a single `Err`.
#[cfg(test)]
pub(crate) fn load_dir(
    dir: &Path,
) -> Result<Vec<Result<ParsedToolkit, LoadError>>, std::io::Error> {
    let entries = std::fs::read_dir(dir)?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            let result = std::fs::read_to_string(&path)
                .map_err(LoadError::Io)
                .and_then(|s| parse_toolkit_full(&s));
            out.push(result);
        }
    }
    Ok(out)
}

/// Per-file outcome from `load_and_register_dir_verbose`. Every file that
/// matched `*.toml` shows up exactly once — successes by id, failures
/// with the path and a `LoadError` describing the cause.
#[derive(Debug, Default)]
pub struct LoadOutcome {
    pub loaded: Vec<&'static str>,
    pub failed: Vec<(std::path::PathBuf, LoadError)>,
    /// Tools that parsed and validated cleanly and still cannot run on
    /// this machine — see [`SkippedTool`]. Their file is *not* a
    /// failure: everything else in it loaded.
    pub skipped: Vec<SkippedTool>,
}

fn register_parsed_toolkit<S: std::hash::BuildHasher>(
    out: &mut LoadOutcome,
    toolkit: ToolkitMeta,
    tools: Vec<(ToolMeta, ToolToml)>,
    origin: Option<&ManifestOrigin>,
    selected_ids: Option<&std::collections::HashSet<String, S>>,
) -> Result<(), LoadError> {
    if origin.is_some_and(ManifestOrigin::is_project) {
        upeg_runtime::project_scope::set_project_toolkit_meta(toolkit);
    } else {
        toolbox_add_toolkit(toolkit);
    }

    for (meta, parsed) in tools {
        if selected_ids.is_some_and(|ids| !ids.contains(meta.id)) {
            continue;
        }
        // Register runtime metadata and dispatcher from the same id source.
        // Pre-cleanup this moved `meta` into the registry first and only
        // registered a dispatcher if one of the builders matched, which could
        // leave a visible-but-uncallable "ghost" Tool. Parse-time validation
        // should make the final `else` unreachable; keep it as a fail-closed
        // guard for future invoker additions.
        let id_static: &'static str = meta.id;
        let execution_requirements =
            crate::execution_requirements::external_execution_requirements(&parsed, origin);
        let controlled_embed_settings = controlled_embed_settings_from_toml(
            parsed
                .controlled_embed
                .as_ref()
                .and_then(|ce| ce.browser.as_ref()),
        )?;
        set_controlled_embed_settings(id_static, controlled_embed_settings);
        // Capture pin + invoker before moving `meta` into the dispatcher
        // chain — `validate_embed_pairing` further down needs both.
        let meta_pin = meta.pin;
        let meta_invoker = meta.invoker;
        if let Some(d) = chain_dispatcher_for(&parsed) {
            toolbox_add_tool_with_dispatcher(meta, d);
        } else if let Some(d) = external_dispatcher_for(&parsed, origin) {
            toolbox_add_tool_with_dispatcher(meta, d);
        } else if let Some(d) = http_dispatcher_for(&parsed) {
            toolbox_add_tool_with_dispatcher(meta, d);
        } else if let Some(d) = llm_dispatcher_for(&parsed) {
            toolbox_add_tool_with_dispatcher(meta, d);
        } else if let Some(d) = wasm_dispatcher_for(&parsed) {
            toolbox_add_tool_with_dispatcher(meta, d);
        } else if let Some(d) = controlled_embed_dispatcher_for(&parsed) {
            // Controlled Embed (pin=ControlledEmbed, invoker=Embed):
            // dispatcher drives the page through the registered
            // `ControlledEmbedBackend` (chromiumoxide headless when
            // the `controlled-embed` feature is on; NoopBackend
            // otherwise — friendly disabled error).
            toolbox_add_tool_with_dispatcher(meta, d);
        } else if let Some(d) = static_dispatcher_for(&parsed) {
            // Passive Embed: webview is the tool. Headless callers get
            // an explanatory error instead of "dispatch not implemented".
            toolbox_add_single_text_tool_with_dispatcher(meta, d);
        } else {
            return Err(LoadError::MissingDispatcher {
                id: id_static.to_string(),
                invoker: parsed.invoker.unwrap_or_default(),
            });
        }
        // Embed URL is orthogonal to dispatcher: it drives GUI
        // WebView/iframe surfaces, while the Embed dispatcher above returns
        // a capability-explicit error for headless callers.
        //
        // Iter 244: trim before leaking. Pre-iter-244 a padded URL
        // (`embed_url = "https://x.com "`) made it into the iframe
        // src verbatim — browsers tolerate it but the value drifts
        // from what the author wrote in the TOML. Whitespace-only is
        // already rejected by validate_toml's iter-244 check, so the
        // .trim() here can never produce an empty string at runtime.
        if let Some(url) = parsed.embed_url {
            let url_static: &'static str = Box::leak(url.trim().to_string().into_boxed_str());
            register_embed_url(id_static, url_static);
        } else {
            clear_embed_url(id_static);
        }
        // Selector bindings (iter 92). Replace-semantics matches the
        // registry's contract — re-loading the same TOML gives a fresh
        // list, no piling.
        //
        // Iter 243: trim both fields. `field` must match typed input
        // property names exactly (per SelectorBinding's doc-comment),
        // so a TOML entry like `{field = " input ", ...}` would silently
        // never match. CSS selectors don't carry meaningful surrounding
        // whitespace either. iter-197 EmptySelectorBinding already
        // catches all-whitespace; this trim handles the surrounding-
        // whitespace case the same way iter-241 did for id/toolkit fields.
        let bindings: Vec<SelectorBinding> = parsed
            .controlled_embed
            .as_ref()
            .and_then(|ce| ce.bindings.as_ref())
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(position, b)| {
                let role = upeg_core::BindingRole::parse(b.role.trim())
                    .unwrap_or(upeg_core::BindingRole::Input);
                let trigger_action = parse_selector_binding_action(position, b.action.as_deref())?;
                Ok(SelectorBinding {
                    role,
                    field: b.field.trim().to_string(),
                    selector: b.selector.trim().to_string(),
                    trigger_action,
                    wait: binding_wait_from_toml(position, b)?,
                })
            })
            .collect::<Result<_, LoadError>>()?;
        // Embed pairing with bindings — catches `(ControlledEmbed, no
        // Trigger)`, `(Embed, has bindings)`, etc. Static `assert_valid`
        // already covered the pin/invoker pairing without bindings;
        // this second pass folds the binding-shape rules in once the
        // loader has the parsed list.
        upeg_core::validate_embed_pairing(id_static, meta_pin, meta_invoker, &bindings)?;
        set_selector_bindings(id_static, bindings);
        let triggers: Vec<TriggerBinding> = parsed
            .triggers
            .unwrap_or_default()
            .into_iter()
            .map(|trigger| TriggerBinding {
                tool_id: id_static,
                source: trigger.source.trim().to_string(),
                condition: trigger.condition.map(|c| c.trim().to_string()),
            })
            .collect();
        set_trigger_bindings(id_static, triggers);
        // Logical credential names, registered so presentation surfaces
        // (FRB `ToolDto.credential_name`) can point at the exact
        // `upeg credential add <name>` command. Names only — secret
        // resolution stays in the dispatchers.
        let credential_names: Vec<String> = parsed
            .credentials
            .as_deref()
            .into_iter()
            .flatten()
            .map(|credential| credential.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();
        set_tool_credential_names(id_static, credential_names);
        upeg_runtime::execution_requirements::set_tool_execution_requirements(
            id_static,
            execution_requirements,
        );
        out.loaded.push(id_static);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn register_parsed_toolkit_for_tests(
    toolkit: ToolkitMeta,
    tools: Vec<(ToolMeta, ToolToml)>,
) -> Result<LoadOutcome, LoadError> {
    let mut out = LoadOutcome::default();
    register_parsed_toolkit::<std::collections::hash_map::RandomState>(
        &mut out, toolkit, tools, None, None,
    )?;
    Ok(out)
}

fn controlled_embed_settings_from_toml(
    settings: Option<&crate::model::ControlledEmbedBrowserToml>,
) -> Result<ControlledEmbedSettings, LoadError> {
    let Some(settings) = settings else {
        return Ok(ControlledEmbedSettings::default());
    };
    Ok(ControlledEmbedSettings {
        user_agent: controlled_embed_user_agent_from_toml(settings)?,
        viewport: controlled_embed_viewport_from_toml(settings)?,
    })
}

fn controlled_embed_user_agent_from_toml(
    settings: &crate::model::ControlledEmbedBrowserToml,
) -> Result<Option<ControlledEmbedUserAgent>, LoadError> {
    let Some(label) = settings.user_agent.as_deref().map(str::trim) else {
        return Ok(None);
    };
    match ControlledEmbedUserAgent::parse(label) {
        Some(ControlledEmbedUserAgent::Default) => Ok(Some(ControlledEmbedUserAgent::Default)),
        Some(ControlledEmbedUserAgent::MobileSafari) => {
            Ok(Some(ControlledEmbedUserAgent::MobileSafari))
        }
        Some(ControlledEmbedUserAgent::Custom(_)) => {
            let Some(value) = settings.custom_user_agent.as_deref() else {
                return Err(LoadError::MissingControlledEmbedCustomUserAgent);
            };
            let value = value.trim();
            if value.is_empty() {
                return Err(LoadError::EmptyControlledEmbedCustomUserAgent);
            }
            Ok(Some(ControlledEmbedUserAgent::Custom(value.to_string())))
        }
        None => Err(LoadError::UnknownControlledEmbedUserAgent {
            user_agent: label.to_string(),
        }),
    }
}

fn controlled_embed_viewport_from_toml(
    settings: &crate::model::ControlledEmbedBrowserToml,
) -> Result<Option<ControlledEmbedViewport>, LoadError> {
    let Some(label) = settings.viewport.as_deref().map(str::trim) else {
        return Ok(None);
    };
    if label.eq_ignore_ascii_case("custom") {
        let width =
            settings
                .viewport_width
                .ok_or(LoadError::MissingControlledEmbedViewportDimension {
                    field: "viewport_width",
                })?;
        let height =
            settings
                .viewport_height
                .ok_or(LoadError::MissingControlledEmbedViewportDimension {
                    field: "viewport_height",
                })?;
        return Ok(Some(ControlledEmbedViewport::Custom { width, height }));
    }
    let preset = ControlledEmbedViewportPreset::parse(label).ok_or_else(|| {
        LoadError::UnknownControlledEmbedViewport {
            viewport: label.to_string(),
        }
    })?;
    Ok(Some(ControlledEmbedViewport::Preset(preset)))
}

/// Verbose variant of `load_and_register_dir` that surfaces which files
/// failed and why. Reading the directory itself failing returns an empty
/// outcome (best-effort, matches the count-only variant).
pub fn load_and_register_dir_verbose(dir: &Path) -> LoadOutcome {
    let mut out = LoadOutcome::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "toml") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                out.failed.push((path, LoadError::Io(e)));
                continue;
            }
        };
        let parsed = match parse_manifest(&content) {
            Ok(parsed) => parsed,
            Err(e) => {
                out.failed.push((path, e));
                continue;
            }
        };
        out.skipped.extend(parsed.skipped);
        let (toolkit, tools) = (parsed.toolkit, parsed.tools);
        let origin = ManifestOrigin::toolkit_file(&path);
        if let Err(e) = register_parsed_toolkit::<std::collections::hash_map::RandomState>(
            &mut out,
            toolkit,
            tools,
            origin.as_ref(),
            None,
        ) {
            out.failed.push((path, e));
        }
    }
    out
}

/// Inspect one project Toolkit without changing the runtime registry.
pub struct ProjectToolkitInspection {
    pub toolkit_id: String,
    pub tool_ids: Vec<String>,
    pub skipped: Vec<SkippedTool>,
}

pub fn inspect_project_toolkit(path: &Path) -> Result<ProjectToolkitInspection, LoadError> {
    let content = std::fs::read_to_string(path)?;
    let parsed = parse_manifest(&content)?;
    Ok(ProjectToolkitInspection {
        toolkit_id: parsed.toolkit.id.to_string(),
        tool_ids: parsed
            .tools
            .into_iter()
            .map(|(meta, _)| meta.id.to_string())
            .collect(),
        skipped: parsed.skipped,
    })
}

/// Register only the selected tools from a project Toolkit. Relative runtime
/// paths use the project root, not the `.upeg/toolkits` source directory.
pub fn load_project_toolkit_file_verbose<S: std::hash::BuildHasher>(
    path: &Path,
    root: &Path,
    selected_ids: &std::collections::HashSet<String, S>,
) -> LoadOutcome {
    let mut out = LoadOutcome::default();
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            out.failed.push((path.to_path_buf(), LoadError::Io(error)));
            return out;
        }
    };
    let parsed = match parse_manifest(&content) {
        Ok(parsed) => parsed,
        Err(error) => {
            out.failed.push((path.to_path_buf(), error));
            return out;
        }
    };
    out.skipped = parsed.skipped;
    let origin = ManifestOrigin::project_root(root);
    if let Err(error) = register_parsed_toolkit(
        &mut out,
        parsed.toolkit,
        parsed.tools,
        origin.as_ref(),
        Some(selected_ids),
    ) {
        out.failed.push((path.to_path_buf(), error));
    }
    out
}

/// Convenience: parse `dir`, register every successfully-parsed tool with
/// `upeg_core` (including External + chain dispatchers when applicable),
/// and return `(loaded_count, failed_count)`. Best-effort — missing
/// directory is treated as `(0, 0)`. Use `load_and_register_dir_verbose`
/// when you need to know which files failed and why.
#[cfg(test)]
pub(crate) fn load_and_register_dir(dir: &Path) -> (usize, usize) {
    let outcome = load_and_register_dir_verbose(dir);
    (outcome.loaded.len(), outcome.failed.len())
}
