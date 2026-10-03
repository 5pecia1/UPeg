//! Credential reference registry.
//!
//! upeg records only references (`name` → env var or OS keychain item).
//! Secret bytes stay in the user's shell/OS secret mechanism and are never
//! written by this module. Declarative adapters resolve credentials at
//! execution time through the reference backend.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRecord {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
    pub store: String,
    pub env: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keychain_service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keychain_account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct CredentialStore {
    credentials: BTreeMap<String, CredentialRecord>,
}

/// User-supplied fields for adding a credential reference. Borrowed `&str`
/// so callers (CLI args, tests) don't have to allocate. `name` is required;
/// remaining fields are optional — defaults are computed downstream.
#[derive(Debug, Clone, Copy, Default)]
pub struct CredentialReferenceSpec<'a> {
    pub name: &'a str,
    pub value_type: Option<&'a str>,
    pub store: Option<&'a str>,
    pub env: Option<&'a str>,
    pub keychain_service: Option<&'a str>,
    pub keychain_account: Option<&'a str>,
    pub target: Option<&'a str>,
}

pub fn default_credentials_path() -> Option<PathBuf> {
    crate::infrastructure::paths::credentials_path()
}

pub fn default_env_for(name: &str) -> String {
    let normalized: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("UPEG_CREDENTIAL_{}", normalized.trim_matches('_'))
}

pub fn add_reference_with_schema(
    spec: CredentialReferenceSpec<'_>,
) -> std::io::Result<CredentialRecord> {
    let Some(path) = default_credentials_path() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "HOME is not set and UPEG_CREDENTIALS_PATH was not provided",
        ));
    };
    add_reference_with_schema_at(&path, spec)
}

#[cfg(test)]
pub fn add_reference_at(
    path: &Path,
    name: &str,
    env: Option<&str>,
    target: Option<&str>,
) -> std::io::Result<CredentialRecord> {
    add_reference_with_schema_at(
        path,
        CredentialReferenceSpec {
            name,
            env,
            target,
            ..CredentialReferenceSpec::default()
        },
    )
}

pub fn add_reference_with_schema_at(
    path: &Path,
    spec: CredentialReferenceSpec<'_>,
) -> std::io::Result<CredentialRecord> {
    let name = normalize_name(spec.name)?;
    let store = normalize_store(spec.store)?;
    let value_type = spec
        .value_type
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("opaque")
        .to_string();
    let keychain_service = spec
        .keychain_service
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let keychain_account = spec
        .keychain_account
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    if store == "keychain" && (keychain_service.is_none() || keychain_account.is_none()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "keychain credential references require --service and --account",
        ));
    }
    let record = CredentialRecord {
        env: spec
            .env
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map_or_else(|| default_env_for(&name), str::to_string),
        value_type,
        store,
        keychain_service,
        keychain_account,
        target: spec
            .target
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        created_at_ms: now_ms(),
        name: name.clone(),
    };
    let mut store = read_store(path)?;
    store.credentials.insert(name, record.clone());
    write_store(path, &store)?;
    Ok(record)
}

pub fn list_references() -> std::io::Result<Vec<CredentialRecord>> {
    let Some(path) = default_credentials_path() else {
        return Ok(Vec::new());
    };
    list_references_at(&path)
}

pub fn list_references_at(path: &Path) -> std::io::Result<Vec<CredentialRecord>> {
    let store = read_store(path)?;
    Ok(store.credentials.into_values().collect())
}

pub fn credential_status(record: &CredentialRecord) -> &'static str {
    if record.store == "keychain" {
        "referenced"
    } else if std::env::var_os(&record.env).is_some() {
        "set"
    } else {
        "missing"
    }
}

pub fn format_reference(record: &CredentialRecord) -> String {
    let reference = if record.store == "keychain" {
        format!(
            "keychain={}/{}",
            record.keychain_service.as_deref().unwrap_or(""),
            record.keychain_account.as_deref().unwrap_or("")
        )
    } else {
        format!("env={}", record.env)
    };
    format!(
        "{}\t{}\t{}\t{}\t{}{}\n",
        record.name,
        record.value_type,
        record.store,
        reference,
        credential_status(record),
        record
            .target
            .as_ref()
            .map(|target| format!("\ttarget={target}"))
            .unwrap_or_default()
    )
}

pub fn format_references(records: &[CredentialRecord], json: bool) -> String {
    if json {
        let values: Vec<_> = records
            .iter()
            .map(|record| {
                serde_json::json!({
                    "name": record.name,
                    "type": record.value_type,
                    "store": record.store,
                    "env": record.env,
                    "keychainService": record.keychain_service,
                    "keychainAccount": record.keychain_account,
                    "target": record.target,
                    "status": credential_status(record),
                })
            })
            .collect();
        let mut out = serde_json::to_string_pretty(&values).unwrap_or_else(|_| "[]".into());
        out.push('\n');
        return out;
    }
    let mut out = String::new();
    for record in records {
        out.push_str(&format_reference(record));
    }
    out
}

fn normalize_name(name: &str) -> std::io::Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "credential name cannot be empty",
        ));
    }
    Ok(name.to_string())
}

fn normalize_store(store: Option<&str>) -> std::io::Result<String> {
    let store = store
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("env");
    match store {
        "env" | "keychain" => Ok(store.to_string()),
        other => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("credential store must be `env` or `keychain`, got `{other}`"),
        )),
    }
}

fn read_store(path: &Path) -> std::io::Result<CredentialStore> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CredentialStore::default());
        }
        Err(e) => return Err(e),
    };
    serde_json::from_str(&content).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid credential registry: {e}"),
        )
    })
}

fn write_store(path: &Path, store: &CredentialStore) -> std::io::Result<()> {
    let _storage = if path
        .file_name()
        .is_some_and(|name| name == "credentials.json")
    {
        Some(
            upeg_core::paths::StorageLease::for_artifact(path, "config")
                .map_err(std::io::Error::other)?,
        )
    } else {
        None
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(store)?;
    std::fs::write(path, format!("{content}\n"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_credential_stores_only_env_reference() {
        let path = std::env::temp_dir().join("upeg_credentials_ref.json");
        let _ = std::fs::remove_file(&path);
        let record = add_reference_at(&path, "etherscan_api_key", None, Some("Authorization"))
            .expect("add reference");
        assert_eq!(record.env, "UPEG_CREDENTIAL_ETHERSCAN_API_KEY");
        assert_eq!(record.target.as_deref(), Some("Authorization"));

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("secret"));
        assert!(!raw.contains("value"));
        let records = list_references_at(&path).unwrap();
        assert_eq!(records.len(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn keychain_reference_stores_schema_not_secret_value() {
        let path = std::env::temp_dir().join("upeg_credentials_keychain_ref.json");
        let _ = std::fs::remove_file(&path);
        let record = add_reference_with_schema_at(
            &path,
            CredentialReferenceSpec {
                name: "openai",
                value_type: Some("api_key"),
                store: Some("keychain"),
                env: None,
                keychain_service: Some("upeg"),
                keychain_account: Some("openai"),
                target: Some("Authorization"),
            },
        )
        .expect("add keychain reference");
        assert_eq!(record.store, "keychain");
        assert_eq!(record.value_type, "api_key");
        assert_eq!(credential_status(&record), "referenced");

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"store\": \"keychain\""));
        assert!(raw.contains("\"type\": \"api_key\""));
        assert!(!raw.contains("sk-"));
        assert!(!raw.contains("secret-value"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn default_env_normalizes_names() {
        assert_eq!(
            default_env_for("open-ai key"),
            "UPEG_CREDENTIAL_OPEN_AI_KEY"
        );
    }
}
