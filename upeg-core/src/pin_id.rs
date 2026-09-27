//! Identity of one placement, separate from the tool it displays.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Stable identity of one pin on a board. Legacy pins use their tool id;
/// newly added pins use an opaque generated id.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct PinId(String);

impl PinId {
    pub fn parse(raw: &str) -> Result<Self, PinIdError> {
        if raw.is_empty() || raw.trim() != raw {
            return Err(PinIdError);
        }
        Ok(Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Preserve the identifier of a placement written before pin ids existed.
    pub fn from_legacy_tool_id(tool_id: &str) -> Self {
        Self(tool_id.to_string())
    }
}

impl std::fmt::Display for PinId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for PinId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("pin id must be non-empty and unpadded")]
pub struct PinIdError;
