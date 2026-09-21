//! Validated tool-argument presets pinned to a placement.
//!
//! An [`ArgsPreset`] is the JSON object of arguments a user saved on a
//! pin. Only JSON objects are accepted, and the reserved execution-context
//! key ([`EXECUTION_CONTEXT_ARG`]) is rejected so a preset can never spoof
//! runtime-injected context. The invariants also hold through
//! deserialization, so corrupted persisted data cannot pass the type.

use crate::manifest::EXECUTION_CONTEXT_ARG;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Validated argument preset. Internally stored as the canonical
/// (compact) JSON text of the object so `Eq`/`Hash` stay derivable.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct ArgsPreset(String);

impl ArgsPreset {
    /// Parse preset text. Only a JSON object without the reserved
    /// execution-context key is accepted.
    pub fn parse(s: &str) -> Result<Self, ArgsPresetError> {
        let value: serde_json::Value =
            serde_json::from_str(s).map_err(|source| ArgsPresetError::InvalidJson {
                message: source.to_string(),
            })?;
        match value {
            serde_json::Value::Object(object) => Self::from_object(object),
            other => Err(ArgsPresetError::NotAnObject {
                actual: json_type_label(&other),
            }),
        }
    }

    /// Build a preset from an already-parsed JSON object.
    pub fn from_object(
        object: serde_json::Map<String, serde_json::Value>,
    ) -> Result<Self, ArgsPresetError> {
        if object.contains_key(EXECUTION_CONTEXT_ARG) {
            return Err(ArgsPresetError::ReservedKey {
                key: EXECUTION_CONTEXT_ARG,
            });
        }
        let canonical =
            serde_json::to_string(&serde_json::Value::Object(object)).map_err(|source| {
                ArgsPresetError::InvalidJson {
                    message: source.to_string(),
                }
            })?;
        Ok(Self(canonical))
    }

    /// Canonical JSON text of the preset object.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The preset as a JSON object. The stored text is an object by
    /// construction; the fallback empty map is unreachable.
    pub fn to_object(&self) -> serde_json::Map<String, serde_json::Value> {
        match serde_json::from_str(&self.0) {
            Ok(serde_json::Value::Object(object)) => object,
            _ => serde_json::Map::new(),
        }
    }
}

impl std::fmt::Display for ArgsPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ArgsPreset {
    type Err = ArgsPresetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(feature = "serde")]
impl Serialize for ArgsPreset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_object().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ArgsPreset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let object = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
        Self::from_object(object).map_err(serde::de::Error::custom)
    }
}

const fn json_type_label(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum ArgsPresetError {
    #[error("args preset must be valid JSON: {message}")]
    InvalidJson { message: String },
    #[error("args preset must be a JSON object, got {actual}")]
    NotAnObject { actual: &'static str },
    #[error("args preset must not contain the reserved execution-context key `{key}`")]
    ReservedKey { key: &'static str },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_preset_keeps_a_json_object_as_a_canonical_string() {
        let preset =
            ArgsPreset::parse(r#" { "city" : "Seoul", "days": 3 } "#).expect("valid object");

        assert_eq!(preset.as_str(), r#"{"city":"Seoul","days":3}"#);
        assert_eq!(preset.to_object().len(), 2);
        assert_eq!(
            preset.to_object().get("city"),
            Some(&serde_json::Value::String("Seoul".to_string()))
        );
    }

    #[test]
    fn args_preset_rejects_non_object_json() {
        const NON_OBJECTS: &[(&str, &str)] = &[
            ("[]", "array"),
            (r#""seoul""#, "string"),
            ("42", "number"),
            ("true", "boolean"),
            ("null", "null"),
        ];

        for (candidate, expected_label) in NON_OBJECTS {
            assert_eq!(
                ArgsPreset::parse(candidate),
                Err(ArgsPresetError::NotAnObject {
                    actual: expected_label
                }),
                "{candidate} is not an object and must be rejected",
            );
        }
    }

    #[test]
    fn args_preset_rejects_invalid_json() {
        assert!(matches!(
            ArgsPreset::parse("{"),
            Err(ArgsPresetError::InvalidJson { .. })
        ));
    }

    #[test]
    fn args_preset_rejects_the_reserved_execution_context_key() {
        let candidate = format!(r#"{{"{EXECUTION_CONTEXT_ARG}": true}}"#);

        assert_eq!(
            ArgsPreset::parse(&candidate),
            Err(ArgsPresetError::ReservedKey {
                key: EXECUTION_CONTEXT_ARG
            })
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn args_preset_serializes_as_a_json_object_and_round_trips() {
        let preset = ArgsPreset::parse(r#"{"city":"Seoul"}"#).expect("valid object");

        let json = serde_json::to_string(&preset).expect("serialize");
        assert_eq!(json, r#"{"city":"Seoul"}"#);

        let back: ArgsPreset = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, preset);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn args_preset_deserialization_rejects_reserved_keys_and_non_objects() {
        let reserved = format!(r#"{{"{EXECUTION_CONTEXT_ARG}": 1}}"#);
        assert!(serde_json::from_str::<ArgsPreset>(&reserved).is_err());
        assert!(serde_json::from_str::<ArgsPreset>("[]").is_err());
    }
}
