//! Load-time validation of `[[tools.triggers]]` declarations.
//!
//! A trigger that only fails once the watch loop is already running is a
//! trigger that fails invisibly: `upeg tool validate` passes, the tool ships,
//! and the operator discovers the mistake as a diagnostic line in a poll loop —
//! or, for a condition on a source that ignores it, never discovers it at all.
//! Every source's condition contract is therefore checked here, at load time.
//!
//! The contract itself is not restated in this module: it is read off
//! [`TriggerSource::condition_requirement`], so the match below is total and a
//! new source cannot be added without deciding what its condition means.

use crate::{LoadError, ToolToml};
use upeg_runtime::{ScheduleCondition, TriggerConditionRequirement, TriggerSource};

pub(crate) fn validate_triggers(parsed: &ToolToml) -> Result<(), LoadError> {
    let Some(triggers) = &parsed.triggers else {
        return Ok(());
    };
    for (position, trigger) in triggers.iter().enumerate() {
        if trigger.source.trim().is_empty() {
            return Err(LoadError::EmptyTriggerSource { position });
        }
        let source = trigger
            .source
            .parse::<TriggerSource>()
            .map_err(|unknown| LoadError::UnknownTriggerSource(unknown.0))?;
        validate_condition(position, source, declared_condition(trigger))?;
    }
    Ok(())
}

/// The condition as the *runtime* will see it: blank is indistinguishable from
/// absent, so both collapse to `None` here rather than at each check site.
fn declared_condition(trigger: &crate::TriggerToml) -> Option<&str> {
    trigger
        .condition
        .as_deref()
        .map(str::trim)
        .filter(|condition| !condition.is_empty())
}

fn validate_condition(
    position: usize,
    source: TriggerSource,
    condition: Option<&str>,
) -> Result<(), LoadError> {
    match source.condition_requirement() {
        TriggerConditionRequirement::Forbidden => {
            if condition.is_some() {
                return Err(LoadError::UnexpectedTriggerCondition {
                    position,
                    trigger_source: source.as_str(),
                });
            }
        }
        // Lowered here so a malformed schedule expression fails at load time
        // rather than degrading to "fire on every poll" once the watch is
        // already running. An absent condition is the documented `now` default.
        TriggerConditionRequirement::ScheduleExpression => {
            ScheduleCondition::parse_optional(condition)
                .map_err(|error| LoadError::InvalidScheduleCondition { position, error })?;
        }
        TriggerConditionRequirement::Required { expectation } => {
            if condition.is_none() {
                return Err(LoadError::MissingTriggerCondition {
                    position,
                    trigger_source: source.as_str(),
                    expectation,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_with_trigger(source: &str, condition: Option<&str>) -> ToolToml {
        ToolToml {
            triggers: Some(vec![crate::TriggerToml {
                source: source.to_string(),
                condition: condition.map(str::to_string),
            }]),
            ..ToolToml::default()
        }
    }

    #[test]
    fn condition_required_source_rejects_missing_condition() {
        for source in [
            TriggerSource::File,
            TriggerSource::Directory,
            TriggerSource::Hotkey,
        ] {
            // Absent and blank must fail alike: a blank condition is what the
            // watch loop sees as absent.
            for condition in [None, Some("   ")] {
                let tool = tool_with_trigger(source.as_str(), condition);
                match validate_triggers(&tool) {
                    Err(LoadError::MissingTriggerCondition {
                        position,
                        trigger_source: reported,
                        expectation,
                    }) => {
                        assert_eq!(position, 0);
                        assert_eq!(reported, source.as_str());
                        assert!(
                            !expectation.is_empty(),
                            "{source:?} must state the expectation"
                        );
                    }
                    other => panic!("{source:?} {condition:?} must be rejected, got {other:?}"),
                }
            }
        }
    }

    #[test]
    fn condition_required_source_passes_with_condition() {
        for (source, condition) in [
            (TriggerSource::File, "/tmp/drop.txt"),
            (TriggerSource::Directory, "/tmp/inbox"),
            (TriggerSource::Hotkey, "ctrl+shift+u"),
        ] {
            let tool = tool_with_trigger(source.as_str(), Some(condition));
            assert!(
                validate_triggers(&tool).is_ok(),
                "{source:?} must pass with a condition"
            );
        }
    }

    #[test]
    fn condition_free_source_rejects_condition() {
        for source in [TriggerSource::Clipboard, TriggerSource::Webhook] {
            let tool = tool_with_trigger(source.as_str(), Some("anything"));
            match validate_triggers(&tool) {
                Err(LoadError::UnexpectedTriggerCondition {
                    position,
                    trigger_source: reported,
                }) => {
                    assert_eq!(position, 0);
                    assert_eq!(reported, source.as_str());
                }
                other => panic!("{source:?} condition must be rejected, got {other:?}"),
            }
        }
    }

    #[test]
    fn condition_free_source_passes_without_condition() {
        for source in [TriggerSource::Clipboard, TriggerSource::Webhook] {
            let tool = tool_with_trigger(source.as_str(), None);
            assert!(
                validate_triggers(&tool).is_ok(),
                "{source:?} must pass without a condition"
            );
        }
    }

    #[test]
    fn schedule_without_condition_passes_as_now() {
        assert!(validate_triggers(&tool_with_trigger("schedule", None)).is_ok());
        assert!(validate_triggers(&tool_with_trigger("schedule", Some("every:30s"))).is_ok());
    }

    #[test]
    fn schedule_rejects_unparsable_condition() {
        match validate_triggers(&tool_with_trigger("schedule", Some("every:soon"))) {
            Err(LoadError::InvalidScheduleCondition { position, error }) => {
                assert_eq!(position, 0);
                assert!(error.to_string().contains("every:soon"), "{error}");
            }
            other => panic!("an unparsable schedule must be rejected, got {other:?}"),
        }
    }

    #[test]
    fn empty_and_unknown_sources_are_rejected() {
        assert!(matches!(
            validate_triggers(&tool_with_trigger("  ", None)),
            Err(LoadError::EmptyTriggerSource { position: 0 })
        ));
        assert!(matches!(
            validate_triggers(&tool_with_trigger("typing", None)),
            Err(LoadError::UnknownTriggerSource(source)) if source == "typing"
        ));
    }

    #[test]
    fn tool_without_triggers_passes() {
        assert!(validate_triggers(&ToolToml::default()).is_ok());
    }
}
