use std::fmt::Write as _;
use std::io;
use std::path::Path;

use serde_json::{Map, Value, json};
use upeg_core::prefs::{Accent, Locale, Theme};
use upeg_runtime::persistence::{io::update_to_path, native_default_tweaks};

use crate::error::CliError;
use crate::surfaces::cli::{SettingKey, SettingsAction};

pub(super) fn run(action: SettingsAction) -> Result<String, CliError> {
    let path = upeg_core::paths::tweaks_path()
        .ok_or_else(|| CliError::tool_failed("settings: config root unavailable"))?;
    match action {
        SettingsAction::Get { key, json } => {
            let record = read(&path).map_err(settings_error)?;
            if let Some(key) = key {
                return render_one(
                    key.name(),
                    record_value(&record, key).map_err(settings_error)?,
                    json,
                );
            }
            let mut values = Vec::new();
            for key in [SettingKey::Locale, SettingKey::Theme, SettingKey::Accent] {
                values.push((
                    key.name(),
                    record_value(&record, key).map_err(settings_error)?,
                ));
            }
            if json {
                return json_line(
                    &values
                        .into_iter()
                        .collect::<std::collections::BTreeMap<_, _>>(),
                );
            }
            let mut output = String::new();
            for (key, value) in values {
                let _ = writeln!(output, "{key}\t{value}");
            }
            Ok(output)
        }
        SettingsAction::Set { key, value, json } => {
            let saved = key.persisted(&value).map_err(settings_error)?;
            update_to_path(&path, |current| {
                let mut record = parse(current)?;
                record.insert(key.name().into(), saved);
                Ok((Some(serde_json::to_string(&record)?), ()))
            })
            .map_err(settings_error)?;
            render_one(key.name(), value.to_ascii_lowercase(), json)
        }
    }
}

fn read(path: &Path) -> io::Result<Map<String, Value>> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(Some(&text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => parse(None),
        Err(error) => Err(error),
    }
}

fn parse(text: Option<&str>) -> io::Result<Map<String, Value>> {
    if let Some(text) = text {
        return Ok(serde_json::from_str(text)?);
    }
    let defaults = native_default_tweaks();
    let value = serde_json::to_value(defaults)?;
    Ok(serde_json::from_value(value)?)
}

fn record_value(record: &Map<String, Value>, key: SettingKey) -> io::Result<String> {
    let value = record
        .get(key.name())
        .and_then(Value::as_str)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing or invalid {}", key.name()),
            )
        })?;
    key.persisted(value)?;
    Ok(value.to_ascii_lowercase())
}

impl SettingKey {
    const fn name(self) -> &'static str {
        match self {
            Self::Locale => "locale",
            Self::Theme => "theme",
            Self::Accent => "accent",
        }
    }

    fn persisted(self, raw: &str) -> io::Result<Value> {
        let value = raw.to_ascii_lowercase();
        let invalid = |choices| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid {} `{raw}`; expected {choices}", self.name()),
            )
        };
        Ok(match self {
            Self::Locale => json!(match value.as_str() {
                "en" => Locale::En,
                "ko" => Locale::Ko,
                _ => return Err(invalid("en or ko")),
            }),
            Self::Theme => json!(match value.as_str() {
                "light" => Theme::Light,
                "dark" => Theme::Dark,
                _ => return Err(invalid("light or dark")),
            }),
            Self::Accent => json!(match value.as_str() {
                "green" => Accent::Green,
                "amber" => Accent::Amber,
                "cyan" => Accent::Cyan,
                "pink" => Accent::Pink,
                _ => return Err(invalid("green, amber, cyan, or pink")),
            }),
        })
    }
}

fn render_one(key: &str, value: String, json: bool) -> Result<String, CliError> {
    if json {
        return json_line(&json!({"key": key, "value": value}));
    }
    Ok(format!("{value}\n"))
}

fn json_line(value: &impl serde::Serialize) -> Result<String, CliError> {
    serde_json::to_string_pretty(value)
        .map(|value| format!("{value}\n"))
        .map_err(settings_error)
}

fn settings_error(error: impl std::fmt::Display) -> CliError {
    CliError::tool_failed(format!("settings: {error}"))
}
