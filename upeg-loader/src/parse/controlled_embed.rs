//! Controlled Embed manifest validation.
//!
//! Everything that turns the raw `[tool.controlled_embed]` TOML table
//! into a checked contract lives here: the pre-`serde` rejection of
//! retired/flat browser fields and unknown `wait` keys, selector-binding
//! entry validation (role/action/selector/wait), and the browser
//! settings (user agent + viewport) rules. `parse.rs` keeps the generic
//! manifest story; this module keeps the Controlled Embed one.

use crate::model::{ControlledEmbedBrowserToml, SelectorBindingToml};
use crate::{LoadError, ToolToml};
use upeg_core::{
    BindingRole, BindingWait, BindingWaitCondition, BindingWaitOnTimeout,
    CONTROLLED_EMBED_VIEWPORT_MAX, CONTROLLED_EMBED_VIEWPORT_MIN, ControlledEmbedTriggerAction,
    ControlledEmbedUserAgent, ControlledEmbedViewportPreset,
    DEFAULT_CONTROLLED_EMBED_WAIT_SETTLE_MS, DEFAULT_CONTROLLED_EMBED_WAIT_TIMEOUT_MS,
    MAX_CONTROLLED_EMBED_WAIT_MS,
};

pub(super) fn reject_unknown_binding_wait_keys(
    tool_table: &toml::map::Map<String, toml::Value>,
) -> Result<(), LoadError> {
    let Some(bindings) = tool_table
        .get("controlled_embed")
        .and_then(toml::Value::as_table)
        .and_then(|controlled_embed| controlled_embed.get("bindings"))
        .and_then(toml::Value::as_array)
    else {
        return Ok(());
    };

    for (position, binding) in bindings.iter().enumerate() {
        let Some(wait) = binding
            .as_table()
            .and_then(|binding| binding.get("wait"))
            .and_then(toml::Value::as_table)
        else {
            continue;
        };
        for key in wait.keys() {
            if !is_binding_wait_key(key) {
                return Err(LoadError::UnknownBindingWaitKey {
                    position,
                    key: key.clone(),
                });
            }
        }
    }
    Ok(())
}

fn is_binding_wait_key(key: &str) -> bool {
    matches!(
        key,
        "for_selector" | "condition" | "timeout_ms" | "settle_ms" | "on_timeout"
    )
}

pub(super) fn reject_flat_controlled_embed_browser_fields(
    tool_table: &toml::map::Map<String, toml::Value>,
) -> Result<(), LoadError> {
    const CONTROLLED_EMBED_BROWSER_FIELDS: [&str; 5] = [
        "user_agent",
        "custom_user_agent",
        "viewport",
        "viewport_width",
        "viewport_height",
    ];

    let Some(controlled_embed) = tool_table
        .get("controlled_embed")
        .and_then(toml::Value::as_table)
    else {
        return Ok(());
    };

    let has_nested_browser = controlled_embed.contains_key("browser");
    for field in CONTROLLED_EMBED_BROWSER_FIELDS {
        if controlled_embed.contains_key(field) {
            return if has_nested_browser {
                Err(LoadError::ConflictingControlledEmbedBrowser { field })
            } else {
                Err(LoadError::DeprecatedControlledEmbedBrowserField { field })
            };
        }
    }
    Ok(())
}

pub(super) fn validate_selector_bindings(parsed: &ToolToml) -> Result<(), LoadError> {
    let Some(bindings) = controlled_embed_bindings(parsed) else {
        return Ok(());
    };
    validate_selector_binding_entries(bindings)
}

fn controlled_embed_bindings(parsed: &ToolToml) -> Option<&[SelectorBindingToml]> {
    parsed
        .controlled_embed
        .as_ref()
        .and_then(|ce| ce.bindings.as_deref())
}

fn validate_selector_binding_entries(bindings: &[SelectorBindingToml]) -> Result<(), LoadError> {
    for (position, b) in bindings.iter().enumerate() {
        let role =
            BindingRole::parse(b.role.trim()).ok_or_else(|| LoadError::UnknownBindingRole {
                position,
                role: b.role.clone(),
            })?;
        let action = parse_selector_binding_action(position, b.action.as_deref())?;
        if role != BindingRole::Trigger && action != ControlledEmbedTriggerAction::Click {
            return Err(LoadError::NonTriggerSelectorBindingAction {
                position,
                role: b.role.clone(),
                action: b
                    .action
                    .clone()
                    .unwrap_or_else(|| action.label().to_string()),
            });
        }
        if role != BindingRole::Trigger && b.field.trim().is_empty() {
            return Err(LoadError::EmptySelectorBinding {
                position,
                missing: "field",
            });
        }
        if b.selector.trim().is_empty() {
            return Err(LoadError::EmptySelectorBinding {
                position,
                missing: "selector",
            });
        }
        binding_wait_from_toml(position, b)?;
    }
    Ok(())
}

pub(crate) fn parse_selector_binding_action(
    position: usize,
    action: Option<&str>,
) -> Result<ControlledEmbedTriggerAction, LoadError> {
    let label = action.unwrap_or(ControlledEmbedTriggerAction::Click.label());
    ControlledEmbedTriggerAction::parse(label.trim()).ok_or_else(|| {
        LoadError::UnknownSelectorBindingAction {
            position,
            action: label.to_string(),
        }
    })
}

pub(crate) fn binding_wait_from_toml(
    position: usize,
    binding: &SelectorBindingToml,
) -> Result<Option<BindingWait>, LoadError> {
    let Some(wait) = binding.wait.as_ref() else {
        return Ok(None);
    };
    let timeout_ms = binding_wait_ms(position, "timeout_ms", wait.timeout_ms)?
        .unwrap_or(DEFAULT_CONTROLLED_EMBED_WAIT_TIMEOUT_MS);
    let settle_ms = binding_wait_ms(position, "settle_ms", wait.settle_ms)?
        .unwrap_or(DEFAULT_CONTROLLED_EMBED_WAIT_SETTLE_MS);

    let for_selector = wait
        .for_selector
        .as_deref()
        .unwrap_or(&binding.selector)
        .trim()
        .to_string();
    if for_selector.is_empty() {
        return Err(LoadError::InvalidBindingWait { position });
    }

    Ok(Some(BindingWait {
        for_selector: Some(for_selector),
        condition: binding_wait_condition(position, wait.condition.as_deref())?,
        timeout_ms,
        settle_ms,
        on_timeout: binding_wait_on_timeout(position, wait.on_timeout.as_deref())?,
    }))
}

fn binding_wait_condition(
    position: usize,
    condition: Option<&str>,
) -> Result<BindingWaitCondition, LoadError> {
    let label = condition
        .map(str::trim)
        .unwrap_or(BindingWaitCondition::Exists.label());
    BindingWaitCondition::parse(label).ok_or_else(|| LoadError::InvalidBindingWaitCondition {
        position,
        condition: label.to_string(),
    })
}

fn binding_wait_on_timeout(
    position: usize,
    on_timeout: Option<&str>,
) -> Result<BindingWaitOnTimeout, LoadError> {
    let label = on_timeout
        .map(str::trim)
        .unwrap_or(BindingWaitOnTimeout::Fail.label());
    BindingWaitOnTimeout::parse(label).ok_or_else(|| LoadError::InvalidBindingWaitOnTimeout {
        position,
        on_timeout: label.to_string(),
    })
}

fn binding_wait_ms(
    position: usize,
    field: &'static str,
    value: Option<u64>,
) -> Result<Option<u64>, LoadError> {
    if let Some(value) = value
        && value > MAX_CONTROLLED_EMBED_WAIT_MS
    {
        return Err(LoadError::OverMaxBindingWaitMs {
            position,
            field,
            value,
            max: MAX_CONTROLLED_EMBED_WAIT_MS,
        });
    }
    Ok(value)
}

pub(super) fn validate_controlled_embed_settings(parsed: &ToolToml) -> Result<(), LoadError> {
    let Some(settings) = controlled_embed_browser(parsed) else {
        return Ok(());
    };
    validate_controlled_embed_user_agent(settings)?;
    validate_controlled_embed_viewport(settings)
}

fn controlled_embed_browser(parsed: &ToolToml) -> Option<&ControlledEmbedBrowserToml> {
    parsed
        .controlled_embed
        .as_ref()
        .and_then(|ce| ce.browser.as_ref())
}

fn validate_controlled_embed_user_agent(
    settings: &ControlledEmbedBrowserToml,
) -> Result<(), LoadError> {
    let user_agent = settings.user_agent.as_deref().map(str::trim);
    let is_custom = match user_agent {
        None => false,
        Some(label) => match ControlledEmbedUserAgent::parse(label) {
            Some(ControlledEmbedUserAgent::Custom(_)) => true,
            Some(_) => false,
            None => {
                return Err(LoadError::UnknownControlledEmbedUserAgent {
                    user_agent: label.to_string(),
                });
            }
        },
    };

    match (is_custom, settings.custom_user_agent.as_deref()) {
        (true, None) => Err(LoadError::MissingControlledEmbedCustomUserAgent),
        (true, Some(value)) if value.trim().is_empty() => {
            Err(LoadError::EmptyControlledEmbedCustomUserAgent)
        }
        (false, Some(_)) => Err(LoadError::UnexpectedControlledEmbedCustomUserAgent),
        _ => Ok(()),
    }
}

fn validate_controlled_embed_viewport(
    settings: &ControlledEmbedBrowserToml,
) -> Result<(), LoadError> {
    let viewport = settings.viewport.as_deref().map(str::trim);
    let is_custom = match viewport {
        None => false,
        Some(label) if label.eq_ignore_ascii_case("custom") => true,
        Some(label) => {
            if ControlledEmbedViewportPreset::parse(label).is_some() {
                false
            } else {
                return Err(LoadError::UnknownControlledEmbedViewport {
                    viewport: label.to_string(),
                });
            }
        }
    };

    validate_controlled_embed_dimension_presence(
        is_custom,
        "viewport_width",
        settings.viewport_width,
    )?;
    validate_controlled_embed_dimension_presence(
        is_custom,
        "viewport_height",
        settings.viewport_height,
    )
}

fn validate_controlled_embed_dimension_presence(
    is_custom: bool,
    field: &'static str,
    value: Option<u16>,
) -> Result<(), LoadError> {
    match (is_custom, value) {
        (true, None) => Err(LoadError::MissingControlledEmbedViewportDimension { field }),
        (true, Some(value))
            if !(CONTROLLED_EMBED_VIEWPORT_MIN..=CONTROLLED_EMBED_VIEWPORT_MAX)
                .contains(&value) =>
        {
            Err(LoadError::ControlledEmbedViewportDimensionOutOfRange {
                field,
                value,
                min: CONTROLLED_EMBED_VIEWPORT_MIN,
                max: CONTROLLED_EMBED_VIEWPORT_MAX,
            })
        }
        (false, Some(_)) => Err(LoadError::UnexpectedControlledEmbedViewportDimension { field }),
        _ => Ok(()),
    }
}
