//! Trigger source for a Tool — how the Pin's execution begins.
//!
//! `Source` is a presentation hint for GUI surfaces (desktop, pwa, ext).
//! Non-GUI surfaces (cli, mcp, http, tui) ignore the variant and call the
//! tool function directly. See LEXICON v2.3 §2.

use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Owned trigger source.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "type"))]
pub enum Source {
    #[default]
    UserInput,
    Timer {
        interval: Duration,
    },
    Shortcut {
        keys: String,
    },
    Manual,
    Static,
}

/// Inventory-safe static form emitted by compile-time registration.
///
/// `&'static`-friendly counterpart of [`Source`]. The macro inserts one
/// of these into every tool's `StaticToolMeta`; runtime conversion via
/// [`StaticSource::to_owned_source`] yields the owned form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticSource {
    UserInput,
    Timer { interval_ms: u64 },
    Shortcut { keys: &'static str },
    Manual,
    Static,
}

impl StaticSource {
    pub const fn default_value() -> Self {
        Self::UserInput
    }

    pub fn to_owned_source(self) -> Source {
        match self {
            Self::UserInput => Source::UserInput,
            Self::Timer { interval_ms } => Source::Timer {
                interval: Duration::from_millis(interval_ms),
            },
            Self::Shortcut { keys } => Source::Shortcut {
                keys: keys.to_string(),
            },
            Self::Manual => Source::Manual,
            Self::Static => Source::Static,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::UserInput => "user_input",
            Self::Timer { .. } => "timer",
            Self::Shortcut { .. } => "shortcut",
            Self::Manual => "manual",
            Self::Static => "static",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_default_is_user_input() {
        assert_eq!(Source::default(), Source::UserInput);
        assert_eq!(StaticSource::default_value(), StaticSource::UserInput);
    }

    #[test]
    fn timer_source_converts_milliseconds_to_a_duration() {
        let static_src = StaticSource::Timer {
            interval_ms: 30_000,
        };
        let owned = static_src.to_owned_source();
        assert_eq!(
            owned,
            Source::Timer {
                interval: Duration::from_secs(30)
            }
        );
    }

    #[test]
    fn shortcut_source_moves_the_static_string_to_owned() {
        let static_src = StaticSource::Shortcut {
            keys: "Cmd+Shift+N",
        };
        let owned = static_src.to_owned_source();
        assert_eq!(
            owned,
            Source::Shortcut {
                keys: "Cmd+Shift+N".to_string()
            }
        );
    }

    #[test]
    fn source_labels_are_unique_per_variant() {
        let labels = [
            StaticSource::UserInput.label(),
            StaticSource::Timer { interval_ms: 0 }.label(),
            StaticSource::Shortcut { keys: "x" }.label(),
            StaticSource::Manual.label(),
            StaticSource::Static.label(),
        ];
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), labels.len());
    }

    #[test]
    fn manual_and_static_sources_round_trip_without_data() {
        assert_eq!(StaticSource::Manual.to_owned_source(), Source::Manual);
        assert_eq!(StaticSource::Static.to_owned_source(), Source::Static);
    }
}
