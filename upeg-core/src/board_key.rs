//! Validated board key newtype.
//!
//! Board keys are user-facing identifiers for pegboard tabs (`dev`,
//! `trading`, …). Every layer that carries "which board is this call
//! scoped to" uses [`BoardKey`] instead of a raw `String` so an empty
//! or padded key cannot flow into execution-context annotation, board
//! gating, or store lookups.
//!
//! A board key also may not contain [`PROJECT_BOARD_KEY_SEPARATOR`].
//! That character separates the three segments of a project board's
//! *store* key ([`crate::BoardStoreKey`]), so reserving it here is what
//! makes "what the user typed" and "what SQLite holds" two disjoint
//! languages: no user input can ever be mistaken for another project's
//! namespaced row.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Reserved in board keys: separates the segments of a project board's
/// store key (`project:<namespace>:<id>`).
pub const PROJECT_BOARD_KEY_SEPARATOR: char = ':';

/// Validated, trimmed, non-empty board key.
#[derive(Clone, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct BoardKey(String);

impl BoardKey {
    /// Parse a board key. Leading/trailing whitespace is rejected (keys
    /// are stored exactly), empty/whitespace-only input is rejected, and
    /// so is [`PROJECT_BOARD_KEY_SEPARATOR`] anywhere in the key.
    pub fn parse(s: &str) -> Result<Self, BoardKeyError> {
        if s.trim().is_empty() {
            return Err(BoardKeyError::Empty);
        }
        if s != s.trim() {
            return Err(BoardKeyError::Padded { key: s.to_string() });
        }
        if s.contains(PROJECT_BOARD_KEY_SEPARATOR) {
            return Err(BoardKeyError::Reserved { key: s.to_string() });
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BoardKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for BoardKey {
    type Err = BoardKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl AsRef<str> for BoardKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(feature = "serde")]
impl Serialize for BoardKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for BoardKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum BoardKeyError {
    #[error("board key must be non-empty")]
    Empty,
    #[error("board key `{key}` must be canonical and unpadded")]
    Padded { key: String },
    #[error(
        "board key `{key}` must not contain `{PROJECT_BOARD_KEY_SEPARATOR}` (reserved for project board store keys)"
    )]
    Reserved { key: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_key는_정상_키를_보존한다() {
        let key = BoardKey::parse("dev").expect("유효 키");
        assert_eq!(key.as_str(), "dev");
        assert_eq!(key.to_string(), "dev");
    }

    #[test]
    fn board_key는_빈_키를_거부한다() {
        assert_eq!(BoardKey::parse(""), Err(BoardKeyError::Empty));
        assert_eq!(BoardKey::parse("   "), Err(BoardKeyError::Empty));
    }

    #[test]
    fn board_key는_예약된_구분자를_거부한다() {
        assert_eq!(
            BoardKey::parse("project:abc:dev"),
            Err(BoardKeyError::Reserved {
                key: "project:abc:dev".into()
            })
        );
    }

    #[test]
    fn board_key는_패딩된_키를_거부한다() {
        assert_eq!(
            BoardKey::parse(" dev "),
            Err(BoardKeyError::Padded {
                key: " dev ".into()
            })
        );
    }
}
