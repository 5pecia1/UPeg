// Mutex poisoning here only happens if another thread panicked while
// holding the registry lock; we'd be in an unrecoverable state already.
// Tool-id canonicalisation works on compile-time `&'static str` keys
// emitted by the `#[tool]` macro, so the parse can't fail at runtime.
#![allow(
    clippy::expect_used,
    reason = "infallible at construction (compile-time static ids) or fatal on failure (poisoned mutex)"
)]

// Re-export the canonical `SelectorBinding` from upeg-core so downstream
// crates can keep importing `upeg_runtime::SelectorBinding` even though
// the type now lives in core. `BindingRole` is the role enum; both ride
// the existing public API.
use upeg_core::ToolId;
#[allow(unused_imports, reason = "pub use re-exports for downstream crates")]
pub use upeg_core::{BindingRole, ControlledEmbedSettings, SelectorBinding};

static EMBED_URLS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<&'static str, &'static str>>,
> = std::sync::OnceLock::new();

fn embed_urls_lock()
-> &'static std::sync::Mutex<std::collections::HashMap<&'static str, &'static str>> {
    EMBED_URLS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn canonical_tool_id(id: &str) {
    ToolId::parse_canonical(id).expect("tool id must be canonical and unpadded");
}

pub fn register_embed_url(id: &'static str, url: &'static str) {
    canonical_tool_id(id);
    embed_urls_lock()
        .lock()
        .expect("embed url registry poisoned")
        .insert(id, url);
}

pub fn clear_embed_url(id: &'static str) {
    canonical_tool_id(id);
    embed_urls_lock()
        .lock()
        .expect("embed url registry poisoned")
        .remove(id);
}

pub fn embed_url_for(id: &str) -> Option<&'static str> {
    // Declarative metadata wins: a static `OutputKind::EmbeddedView { url }`
    // attached to the tool's `ToolMeta` is the canonical embed URL. The
    // runtime side registry below stays available for TOML-loaded tools
    // that haven't migrated to the declarative outputs grammar yet.
    if let Some(meta) = crate::toolbox_tool(id) {
        for field in &meta.output_spec.fields {
            if let upeg_core::OutputKind::EmbeddedView { url } = &field.kind {
                return Some(url.as_str());
            }
        }
    }
    embed_urls_lock()
        .lock()
        .expect("embed url registry poisoned")
        .get(id)
        .copied()
}

type SelectorBindingsMap = std::collections::HashMap<&'static str, Vec<SelectorBinding>>;

static SELECTOR_BINDINGS: std::sync::OnceLock<std::sync::Mutex<SelectorBindingsMap>> =
    std::sync::OnceLock::new();

fn selector_bindings_lock() -> &'static std::sync::Mutex<SelectorBindingsMap> {
    SELECTOR_BINDINGS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

pub fn set_selector_bindings(id: &'static str, bindings: Vec<SelectorBinding>) {
    canonical_tool_id(id);
    selector_bindings_lock()
        .lock()
        .expect("selector bindings registry poisoned")
        .insert(id, bindings);
}

pub fn selector_bindings_for(id: &str) -> Vec<SelectorBinding> {
    selector_bindings_lock()
        .lock()
        .expect("selector bindings registry poisoned")
        .get(id)
        .cloned()
        .unwrap_or_default()
}

type ControlledEmbedSettingsMap = std::collections::HashMap<&'static str, ControlledEmbedSettings>;

static CONTROLLED_EMBED_SETTINGS: std::sync::OnceLock<
    std::sync::Mutex<ControlledEmbedSettingsMap>,
> = std::sync::OnceLock::new();

fn controlled_embed_settings_lock() -> &'static std::sync::Mutex<ControlledEmbedSettingsMap> {
    CONTROLLED_EMBED_SETTINGS
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

pub fn set_controlled_embed_settings(id: &'static str, settings: ControlledEmbedSettings) {
    canonical_tool_id(id);
    controlled_embed_settings_lock()
        .lock()
        .expect("controlled embed settings registry poisoned")
        .insert(id, settings);
}

pub fn controlled_embed_settings_for(id: &str) -> ControlledEmbedSettings {
    controlled_embed_settings_lock()
        .lock()
        .expect("controlled embed settings registry poisoned")
        .get(id)
        .cloned()
        .unwrap_or_default()
}

type OwnedControlledEmbedSettingsMap = std::collections::HashMap<String, ControlledEmbedSettings>;

static OWNED_CONTROLLED_EMBED_SETTINGS: std::sync::OnceLock<
    std::sync::Mutex<OwnedControlledEmbedSettingsMap>,
> = std::sync::OnceLock::new();

fn owned_controlled_embed_settings_lock()
-> &'static std::sync::Mutex<OwnedControlledEmbedSettingsMap> {
    OWNED_CONTROLLED_EMBED_SETTINGS
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Owned-string variant of [`set_controlled_embed_settings`] for FFI / FRB
/// surfaces that cannot manufacture `&'static str` keys at runtime.
pub fn set_controlled_embed_settings_owned(
    id: String,
    settings: ControlledEmbedSettings,
) -> Result<(), String> {
    ToolId::parse_canonical(&id).map_err(|e| e.to_string())?;
    owned_controlled_embed_settings_lock()
        .lock()
        .expect("owned controlled embed settings registry poisoned")
        .insert(id, settings);
    Ok(())
}

/// Owned-string variant of [`controlled_embed_settings_for`]. Falls back to
/// the static-keyed registry when `id` has no owned entry.
pub fn controlled_embed_settings_owned_for(id: &str) -> ControlledEmbedSettings {
    let owned = owned_controlled_embed_settings_lock()
        .lock()
        .expect("owned controlled embed settings registry poisoned")
        .get(id)
        .cloned();
    match owned {
        Some(found) => found,
        None => controlled_embed_settings_for(id),
    }
}

// Owned-string registry, populated through `set_selector_bindings_owned`.
// FFI/FRB callers cannot synthesise `&'static str` keys at runtime, so the
// static-keyed registry above coexists with this owned-keyed one. Lookups
// (`selector_bindings_for`) only see the static map by design — owned
// bindings live in their own table so the static API stays type-pure.
type OwnedSelectorBindingsMap = std::collections::HashMap<String, Vec<SelectorBinding>>;

static OWNED_SELECTOR_BINDINGS: std::sync::OnceLock<std::sync::Mutex<OwnedSelectorBindingsMap>> =
    std::sync::OnceLock::new();

fn owned_selector_bindings_lock() -> &'static std::sync::Mutex<OwnedSelectorBindingsMap> {
    OWNED_SELECTOR_BINDINGS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Owned-string variant of [`set_selector_bindings`] for FFI / FRB
/// surfaces that cannot manufacture `&'static str` keys at runtime.
///
/// Returns an error string if `id` is not a canonical, unpadded tool id
/// (mirrors the static-keyed variant's panic, but degrades to a typed
/// error so the FRB layer can route it through `FrbError::Internal`).
pub fn set_selector_bindings_owned(
    id: String,
    bindings: Vec<SelectorBinding>,
) -> Result<(), String> {
    ToolId::parse_canonical(&id).map_err(|e| e.to_string())?;
    owned_selector_bindings_lock()
        .lock()
        .expect("owned selector bindings registry poisoned")
        .insert(id, bindings);
    Ok(())
}

/// Owned-string variant of [`selector_bindings_for`]. Falls back to the
/// static-keyed registry when `id` has no owned entry so existing tools
/// declared through the static `#[tool]` macro path keep working.
pub fn selector_bindings_owned_for(id: &str) -> Vec<SelectorBinding> {
    // Drop the lock guard before falling back so we don't hold two
    // selector-binding mutexes at once (lint:
    // `if_let_drop_temporary_in_scrutinee`).
    let owned = owned_selector_bindings_lock()
        .lock()
        .expect("owned selector bindings registry poisoned")
        .get(id)
        .cloned();
    match owned {
        Some(found) => found,
        None => selector_bindings_for(id),
    }
}

pub fn selector_bindings_json_for(id: &str) -> Vec<serde_json::Value> {
    selector_bindings_for(id)
        .into_iter()
        .map(|b| {
            serde_json::json!({
                "role": b.role.label(),
                "field": b.field,
                "selector": b.selector,
                "action": b.trigger_action.label(),
            })
        })
        .collect()
}

#[cfg(test)]
mod controlled_embed_settings_tests {
    use super::*;

    #[test]
    fn controlled_embed_settings는_미등록_도구에_기본값을_반환한다() {
        let settings = controlled_embed_settings_for("settings.unregistered_tool");

        assert_eq!(settings, ControlledEmbedSettings::default());
        assert_eq!(settings.user_agent, None);
        assert_eq!(settings.viewport, None);
    }

    #[test]
    fn controlled_embed_settings는_같은_id를_다시_설정하면_교체한다() {
        let first = ControlledEmbedSettings {
            user_agent: Some(upeg_core::ControlledEmbedUserAgent::MobileSafari),
            viewport: Some(upeg_core::ControlledEmbedViewport::Preset(
                upeg_core::ControlledEmbedViewportPreset::Mobile,
            )),
        };
        let second = ControlledEmbedSettings {
            user_agent: Some(upeg_core::ControlledEmbedUserAgent::Custom(
                "Custom UA".into(),
            )),
            viewport: Some(upeg_core::ControlledEmbedViewport::Custom {
                width: 1024,
                height: 768,
            }),
        };

        set_controlled_embed_settings("settings.replace_tool", first);
        set_controlled_embed_settings("settings.replace_tool", second.clone());

        assert_eq!(
            controlled_embed_settings_for("settings.replace_tool"),
            second
        );
    }
}
