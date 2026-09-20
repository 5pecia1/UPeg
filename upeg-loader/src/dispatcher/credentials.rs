use crate::model::CredentialRefToml;
use std::collections::BTreeMap;

pub(super) fn resolve_credentials(
    credentials: &[CredentialRefToml],
) -> Result<Vec<(String, String)>, String> {
    credentials
        .iter()
        .map(|credential| {
            let name = credential.name.trim();
            let required = credential.required.unwrap_or(true);
            let target = credential
                .target
                .as_deref()
                .map(str::trim)
                .filter(|s: &&str| !s.is_empty())
                .unwrap_or(name)
                .to_string();
            let value = match credential
                .store
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("env")
            {
                "env" => {
                    let env_name = credential
                        .env
                        .as_deref()
                        .map(str::trim)
                        .filter(|s: &&str| !s.is_empty())
                        .map_or_else(|| default_credential_env(name), str::to_string);
                    std::env::var(&env_name).map_err(|_| {
                        format!(
                            "credential `{name}` is missing (set environment variable `{env_name}`)"
                        )
                    })
                }
                "keychain" => resolve_keychain_credential(credential),
                other => Err(format!("credential `{name}` uses unknown store `{other}`")),
            };
            match value {
                Ok(value) => Ok((target, value)),
                Err(_) if !required => Ok((target, String::new())),
                Err(err) => Err(err),
            }
        })
        .collect()
}

fn resolve_keychain_credential(credential: &CredentialRefToml) -> Result<String, String> {
    let name = credential.name.trim();
    let service = credential
        .keychain_service
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("credential `{name}` is missing keychain_service"))?;
    let account = credential
        .keychain_account
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("credential `{name}` is missing keychain_account"))?;

    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("security")
            .args(["find-generic-password", "-s", service, "-a", account, "-w"])
            .output()
            .map_err(|e| format!("credential `{name}` keychain lookup failed: {e}"))?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout)
                .trim_end_matches(['\r', '\n'])
                .to_string());
        }
        return Err(format!(
            "credential `{name}` was not found in macOS keychain service `{service}` account `{account}`"
        ));
    }

    #[cfg(target_os = "linux")]
    {
        let out = std::process::Command::new("secret-tool")
            .args(["lookup", "service", service, "account", account])
            .output()
            .map_err(|e| {
                format!(
                    "credential `{name}` keychain lookup requires `secret-tool` on this Linux host: {e}"
                )
            })?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout)
                .trim_end_matches(['\r', '\n'])
                .to_string());
        }
        Err(format!(
            "credential `{name}` was not found in Linux secret service `{service}` account `{account}`"
        ))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(format!(
            "credential `{name}` keychain store is unsupported on this host; use `store = \"env\"` or a platform adapter"
        ))
    }
}

pub(super) fn credential_map(
    credentials: &[CredentialRefToml],
    primary: Option<&str>,
) -> Result<BTreeMap<String, String>, String> {
    let mut map: BTreeMap<String, String> = resolve_credentials(credentials)?.into_iter().collect();
    if let Some(primary) = primary.map(str::trim).filter(|s| !s.is_empty())
        && !map.contains_key(primary)
    {
        let env_name = default_credential_env(primary);
        let value = std::env::var(&env_name).map_err(|_| {
            format!("credential `{primary}` is missing (set environment variable `{env_name}`)")
        })?;
        map.insert(primary.to_string(), value);
    }
    Ok(map)
}

fn default_credential_env(name: &str) -> String {
    let normalized: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("UPEG_CREDENTIAL_{normalized}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_map_reuses_declared_default_before_default_env_lookup() {
        let name = format!("primary_reuse_{}", std::process::id());
        let credentials = [CredentialRefToml {
            name: name.clone(),
            value_type: None,
            store: Some("env".into()),
            env: Some(format!("UPEG_DECLARED_PRIMARY_{}", std::process::id())),
            keychain_service: None,
            keychain_account: None,
            target: None,
            required: Some(false),
        }];

        let map = credential_map(&credentials, Some(&name))
            .expect("declared primary credential must not require default env fallback");
        assert_eq!(map.get(&name).map(String::as_str), Some(""));
    }
}
