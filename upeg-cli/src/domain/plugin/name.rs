//! Plugin/toolkit name validation for `upeg plugin new`.
//!
//! Two closed checks, both reused rather than hand-rolled:
//!
//!   1. A scaffold-safe identifier check — required because `name` is
//!      embedded literally into a Rust function identifier
//!      (`{name}_hello`) and a Cargo package name by
//!      [`super::scaffold::render`]; this is a plain character-class
//!      check, not a regex.
//!   2. The same canonical Toolkit-id rules `upeg-wasm`'s loader
//!      enforces on every installed plugin ([`upeg_core::ToolId`]) — a
//!      name that fails this would be rejected again the moment the
//!      scaffolded plugin is built and installed, so `plugin new`
//!      catches it earlier using the identical rule rather than a
//!      second hand-rolled one.

use std::fmt;

use upeg_core::ToolId;

/// Why a candidate plugin name was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PluginNameError {
    /// Empty, or contains characters unsafe for both a Rust identifier
    /// and a Cargo package name segment.
    InvalidIdentifier(String),
    /// Passed the identifier check but still fails upeg's canonical
    /// Toolkit-id rules — the same rules `upeg-wasm` enforces at load
    /// time (see `upeg_wasm::LoadError::from(upeg_core::ToolIdError)`).
    Identity(upeg_core::ToolIdError),
}

impl fmt::Display for PluginNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(name) => write!(
                f,
                "plugin name `{name}` must be a lowercase identifier (letters, digits, \
                 underscore, starting with a letter) — it becomes both the Toolkit id and a \
                 Rust function name in the scaffold"
            ),
            Self::Identity(e) => write!(f, "plugin name is not a valid Toolkit id: {e}"),
        }
    }
}

/// Validate `name` as a `upeg plugin new` argument: a scaffold-safe
/// Rust identifier that is also a canonical upeg Toolkit id.
pub(crate) fn validate_plugin_name(name: &str) -> Result<(), PluginNameError> {
    if !is_scaffold_safe_identifier(name) {
        return Err(PluginNameError::InvalidIdentifier(name.to_string()));
    }
    // Probe the exact rule upeg-wasm applies to every installed plugin's
    // toolkit id: a canonical `{toolkit}.{tool}` id whose toolkit
    // component is this name.
    ToolId::parse_canonical_in_toolkit(&format!("{name}.probe"), name)
        .map(|_| ())
        .map_err(PluginNameError::Identity)
}

fn is_scaffold_safe_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercase_snake_case_names_pass() {
        assert!(validate_plugin_name("greet").is_ok());
        assert!(validate_plugin_name("my_plugin_2").is_ok());
    }

    #[test]
    fn empty_name_is_rejected() {
        assert!(matches!(
            validate_plugin_name(""),
            Err(PluginNameError::InvalidIdentifier(_))
        ));
    }

    #[test]
    fn names_with_uppercase_or_hyphens_are_rejected() {
        assert!(matches!(
            validate_plugin_name("My-Plugin"),
            Err(PluginNameError::InvalidIdentifier(_))
        ));
    }

    #[test]
    fn names_starting_with_digit_are_rejected() {
        assert!(matches!(
            validate_plugin_name("1plugin"),
            Err(PluginNameError::InvalidIdentifier(_))
        ));
    }

    #[test]
    fn names_containing_whitespace_are_rejected() {
        assert!(matches!(
            validate_plugin_name("my plugin"),
            Err(PluginNameError::InvalidIdentifier(_))
        ));
    }
}
