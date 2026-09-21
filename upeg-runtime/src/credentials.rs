//! Manifest-declared credential names per tool.
//!
//! TOML manifests declare `credentials = [{ name = "..." }, …]`; the
//! loader registers the logical names here so presentation surfaces
//! (FRB `ToolDto.credential_name`, future doctor output) can point the
//! user at the exact `upeg credential add <name>` command. Secret
//! values never flow through this registry — names only.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn credential_names_lock() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static NAMES: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    NAMES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record the manifest-level credential names for `tool_id`. An empty
/// list clears the record (tools without credentials stay absent).
pub fn set_tool_credential_names(tool_id: &str, names: Vec<String>) {
    let Ok(mut guard) = credential_names_lock().lock() else {
        return;
    };
    if names.is_empty() {
        guard.remove(tool_id);
    } else {
        guard.insert(tool_id.to_string(), names);
    }
}

/// Manifest-declared credential names for `tool_id`, in declaration
/// order. Empty when the tool declares none.
pub fn tool_credential_names(tool_id: &str) -> Vec<String> {
    credential_names_lock()
        .lock()
        .ok()
        .and_then(|guard| guard.get(tool_id).cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_names_are_looked_up_in_registration_order() {
        let id = "test.credentials.weather";
        set_tool_credential_names(id, vec!["weather_api_key".into(), "backup_key".into()]);
        assert_eq!(
            tool_credential_names(id),
            vec!["weather_api_key".to_string(), "backup_key".to_string()]
        );
        set_tool_credential_names(id, Vec::new());
        assert!(tool_credential_names(id).is_empty());
    }

    #[test]
    fn unregistered_tool_credential_names_are_empty() {
        assert!(tool_credential_names("test.credentials.none").is_empty());
    }
}
