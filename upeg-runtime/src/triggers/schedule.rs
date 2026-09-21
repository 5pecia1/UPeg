//! Typed `schedule` trigger conditions.
//!
//! A `schedule` trigger's `condition` field is a tiny expression language with
//! exactly two forms: `now` (fire once when the trigger runtime starts) and
//! `every:<duration>` (fire at start, then once per elapsed interval). Parsing
//! lives here — in the runtime crate both the loader and the CLI watch adapter
//! depend on — so a malformed expression fails at *load* time instead of
//! silently degrading to "fire on every poll" at watch time.
//!
//! The watch's poll cadence ([`WATCH_POLL_INTERVAL`]) is owned here for the
//! same reason: it is the *resolution* of every interval, so the parser can
//! reject a schedule the watch could never honor.

use std::str::FromStr;
use std::time::Duration;

/// Condition that fires exactly once, when the trigger runtime starts.
const NOW_EXPRESSION: &str = "now";
/// Prefix introducing a recurring interval, for example `every:30s`.
const EVERY_PREFIX: &str = "every:";
/// How often the trigger watch loop re-evaluates every registered trigger.
///
/// This is the resolution of every `every:<duration>` schedule: the watch
/// cannot notice an interval elapsing before its next poll. An interval
/// shorter than this is rejected at load time (it would be silently quantized
/// up to the poll cadence), and an interval that is not a whole multiple of it
/// fires on the first poll at or after each due moment. `upeg-cli`'s watch loop
/// derives its sleep from this constant, so the two can never drift.
pub const WATCH_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// A parsed `schedule` trigger condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleCondition {
    /// Fire exactly once, at the moment the trigger runtime starts.
    Now,
    /// Fire immediately at start, then once per elapsed interval.
    Every(Duration),
}

impl ScheduleCondition {
    /// Parse a declared `condition`, treating an absent or blank value as
    /// [`Self::Now`] (the documented default for a bare `schedule` trigger).
    pub fn parse_optional(condition: Option<&str>) -> Result<Self, ScheduleConditionError> {
        match condition.map(str::trim) {
            None | Some("") => Ok(Self::Now),
            Some(expression) => expression.parse(),
        }
    }
}

impl FromStr for ScheduleCondition {
    type Err = ScheduleConditionError;

    fn from_str(expression: &str) -> Result<Self, Self::Err> {
        let expression = expression.trim();
        if expression == NOW_EXPRESSION {
            return Ok(Self::Now);
        }
        let Some(interval) = expression.strip_prefix(EVERY_PREFIX) else {
            return Err(ScheduleConditionError::Unsupported {
                expression: expression.to_string(),
            });
        };
        let interval = humantime::parse_duration(interval.trim()).map_err(|error| {
            ScheduleConditionError::UnparsableInterval {
                expression: expression.to_string(),
                reason: error.to_string(),
            }
        })?;
        if interval < WATCH_POLL_INTERVAL {
            return Err(ScheduleConditionError::IntervalBelowPollResolution {
                expression: expression.to_string(),
                resolution: WATCH_POLL_INTERVAL,
            });
        }
        Ok(Self::Every(interval))
    }
}

/// Why a `schedule` condition could not be lowered to a [`ScheduleCondition`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleConditionError {
    /// Neither `now` nor `every:<duration>`.
    Unsupported { expression: String },
    /// `every:` carrying a duration `humantime` could not read.
    UnparsableInterval { expression: String, reason: String },
    /// `every:0s`, `every:500ms` — an interval finer than the watch's poll
    /// resolution, which the watch could only honor by quantizing it up.
    IntervalBelowPollResolution {
        expression: String,
        resolution: Duration,
    },
}

impl std::fmt::Display for ScheduleConditionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported { expression } => write!(
                f,
                "unsupported schedule condition `{expression}` (use `{NOW_EXPRESSION}` or `{EVERY_PREFIX}<duration>`)"
            ),
            Self::UnparsableInterval { expression, reason } => write!(
                f,
                "unreadable schedule interval in `{expression}`: {reason} (for example `{EVERY_PREFIX}30s`)"
            ),
            Self::IntervalBelowPollResolution {
                expression,
                resolution,
            } => write!(
                f,
                "schedule interval in `{expression}` must be at least {resolution} — the trigger watch polls once per {resolution}, so a finer interval could only be honored by quantizing it up",
                resolution = humantime::format_duration(*resolution)
            ),
        }
    }
}

impl std::error::Error for ScheduleConditionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_condition_parses_as_now() {
        assert_eq!(
            ScheduleCondition::parse_optional(None),
            Ok(ScheduleCondition::Now)
        );
        assert_eq!(
            ScheduleCondition::parse_optional(Some("   ")),
            Ok(ScheduleCondition::Now)
        );
        assert_eq!(
            ScheduleCondition::parse_optional(Some(" now ")),
            Ok(ScheduleCondition::Now)
        );
    }

    #[test]
    fn every_condition_parses_as_humantime_interval() {
        assert_eq!(
            ScheduleCondition::parse_optional(Some("every:30s")),
            Ok(ScheduleCondition::Every(Duration::from_secs(30)))
        );
        assert_eq!(
            ScheduleCondition::parse_optional(Some("every: 1h 30m ")),
            Ok(ScheduleCondition::Every(Duration::from_secs(5400)))
        );
    }

    #[test]
    fn unparsable_interval_is_rejected() {
        let error = ScheduleCondition::parse_optional(Some("every:soon")).unwrap_err();
        assert!(
            matches!(error, ScheduleConditionError::UnparsableInterval { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("every:soon"), "{error}");
    }

    #[test]
    fn zero_second_interval_is_rejected() {
        assert_eq!(
            ScheduleCondition::parse_optional(Some("every:0s")),
            Err(ScheduleConditionError::IntervalBelowPollResolution {
                expression: "every:0s".to_string(),
                resolution: WATCH_POLL_INTERVAL
            })
        );
    }

    #[test]
    fn interval_shorter_than_poll_interval_is_rejected_naming_resolution() {
        // `every:500ms` used to load and then quantize up to the 1s poll in
        // silence; it now names the resolution at load time.
        let error = ScheduleCondition::parse_optional(Some("every:500ms")).unwrap_err();
        assert_eq!(
            error,
            ScheduleConditionError::IntervalBelowPollResolution {
                expression: "every:500ms".to_string(),
                resolution: WATCH_POLL_INTERVAL
            }
        );
        let message = error.to_string();
        assert!(message.contains("every:500ms"), "{message}");
        assert!(message.contains("1s"), "{message}");
    }

    #[test]
    fn interval_equal_to_poll_interval_is_allowed() {
        assert_eq!(
            ScheduleCondition::parse_optional(Some("every:1s")),
            Ok(ScheduleCondition::Every(WATCH_POLL_INTERVAL))
        );
    }

    #[test]
    fn condition_that_is_neither_now_nor_every_is_rejected() {
        let error = ScheduleCondition::parse_optional(Some("cron(* * * * *)")).unwrap_err();
        assert_eq!(
            error,
            ScheduleConditionError::Unsupported {
                expression: "cron(* * * * *)".to_string()
            }
        );
        assert!(error.to_string().contains("every:<duration>"), "{error}");
    }
}
