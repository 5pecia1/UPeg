//! What one poll of the trigger runtime produced, and how each caller renders it.
//!
//! The single-shot `upeg trigger run` and the `--watch` loop want opposite
//! things from the same evaluation. A single shot is a *status report*: the
//! operator asked what every trigger is doing right now, so every row —
//! including the ones that did not fire — is the answer. The watch loop is a
//! *feed*: it re-evaluates every trigger once per second forever, so a row per
//! trigger per poll buries the fires under thousands of `idle` lines.
//!
//! Evaluation therefore yields typed outcomes and the two renderers here decide
//! what to show, instead of the poll deciding by formatting a string.

use super::gates::TriggerKey;
use std::collections::HashSet;

/// TSV column separator, shared with `upeg trigger list`'s row rendering.
pub(super) const FIELD_SEPARATOR: &str = "\t";
/// Row terminator, shared with `upeg trigger list`'s row rendering.
pub(super) const ROW_TERMINATOR: &str = "\n";

/// How a fired dispatch ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FireStatus {
    Ok,
    ToolError,
    NotFound,
}

impl FireStatus {
    /// The wire label printed in the status column.
    const fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::ToolError => "tool_error",
            Self::NotFound => "not_found",
        }
    }
}

/// What one registered trigger did during one poll.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TriggerOutcome {
    /// The condition held and the tool was dispatched.
    Fired {
        status: FireStatus,
        /// Primary output text, when the dispatch produced one.
        text: Option<String>,
    },
    /// The condition did not hold on this poll. Routine in a watch loop —
    /// it is the answer for all but a handful of the polls a trigger sees.
    Idle {
        /// How this source is serviced, for the single-shot status report.
        diagnostic: &'static str,
    },
    /// This trigger cannot fire on this host at all (no platform adapter, an
    /// unknown source, a malformed condition). Unlike [`Self::Idle`] it is a
    /// standing fault, so the watch loop reports it once rather than never.
    Unsupported { diagnostic: String },
}

impl TriggerOutcome {
    /// The status column for this outcome.
    const fn status(&self) -> &'static str {
        match self {
            Self::Fired { status, .. } => status.label(),
            Self::Idle { .. } => "idle",
            Self::Unsupported { .. } => "unsupported",
        }
    }

    /// The trailing detail column, when this outcome has one.
    fn detail(&self) -> Option<&str> {
        match self {
            Self::Fired { text, .. } => text.as_deref(),
            Self::Idle { diagnostic } => Some(diagnostic),
            Self::Unsupported { diagnostic } => Some(diagnostic),
        }
    }
}

/// One trigger's result from one poll, tied to the trigger's identity so the
/// watch loop can tell a repeat fault from a new one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TriggerReport {
    key: TriggerKey,
    outcome: TriggerOutcome,
}

impl TriggerReport {
    pub(crate) const fn new(key: TriggerKey, outcome: TriggerOutcome) -> Self {
        Self { key, outcome }
    }

    /// `tool_id \t source \t status [\t detail]`.
    fn render_into(&self, out: &mut String) {
        out.push_str(&self.key.tool_id);
        out.push_str(FIELD_SEPARATOR);
        out.push_str(&self.key.source);
        out.push_str(FIELD_SEPARATOR);
        out.push_str(self.outcome.status());
        if let Some(detail) = self.outcome.detail() {
            out.push_str(FIELD_SEPARATOR);
            out.push_str(detail);
        }
        out.push_str(ROW_TERMINATOR);
    }
}

/// Render every row, including the idle ones: a single-shot run is a status
/// report over all registered triggers.
pub(crate) fn format_status_report(reports: &[TriggerReport]) -> String {
    let mut out = String::new();
    for report in reports {
        report.render_into(&mut out);
    }
    out
}

/// Renders the watch loop's feed: fires always, standing faults once.
///
/// `idle` is never printed — in a one-second poll loop it is the answer for
/// nearly every row of nearly every poll, and printing it drowns the fires.
/// An `unsupported` trigger is a fault the operator must see, but it is the
/// *same* fault on every poll, so it prints on first occurrence per trigger.
#[derive(Debug, Default)]
pub(crate) struct WatchPrinter {
    reported_unsupported: HashSet<TriggerKey>,
}

impl WatchPrinter {
    pub(crate) fn render(&mut self, reports: &[TriggerReport]) -> String {
        let mut out = String::new();
        for report in reports {
            let show = match &report.outcome {
                TriggerOutcome::Fired { .. } => true,
                TriggerOutcome::Idle { .. } => false,
                TriggerOutcome::Unsupported { .. } => {
                    self.reported_unsupported.insert(report.key.clone())
                }
            };
            if show {
                report.render_into(&mut out);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use upeg_runtime::TriggerBinding;

    fn key(tool_id: &'static str, source: &str) -> TriggerKey {
        TriggerKey::from_binding(&TriggerBinding {
            tool_id,
            source: source.to_string(),
            condition: None,
        })
    }

    fn fired(tool_id: &'static str, source: &str, text: &str) -> TriggerReport {
        TriggerReport::new(
            key(tool_id, source),
            TriggerOutcome::Fired {
                status: FireStatus::Ok,
                text: Some(text.to_string()),
            },
        )
    }

    fn idle(tool_id: &'static str, source: &str) -> TriggerReport {
        TriggerReport::new(
            key(tool_id, source),
            TriggerOutcome::Idle {
                diagnostic: "diagnostic",
            },
        )
    }

    fn unsupported(tool_id: &'static str, source: &str) -> TriggerReport {
        TriggerReport::new(
            key(tool_id, source),
            TriggerOutcome::Unsupported {
                diagnostic: "needs a platform adapter".to_string(),
            },
        )
    }

    #[test]
    fn 단발_보고서는_발화하지_않은_행도_모두_출력한다() {
        let rows = [
            fired("demo.one", "schedule", "ran"),
            idle("demo.two", "file"),
            unsupported("demo.three", "hotkey"),
        ];
        let out = format_status_report(&rows);
        assert_eq!(
            out,
            "demo.one\tschedule\tok\tran\n\
             demo.two\tfile\tidle\tdiagnostic\n\
             demo.three\thotkey\tunsupported\tneeds a platform adapter\n"
        );
    }

    #[test]
    fn 감시_피드는_idle_행을_출력하지_않는다() {
        let mut printer = WatchPrinter::default();
        let out = printer.render(&[
            fired("demo.one", "schedule", "ran"),
            idle("demo.two", "file"),
        ]);
        assert_eq!(out, "demo.one\tschedule\tok\tran\n");
    }

    #[test]
    fn 감시_피드는_unsupported를_트리거별로_한_번만_출력한다() {
        let mut printer = WatchPrinter::default();
        let poll = [unsupported("demo.three", "hotkey")];
        assert_eq!(
            printer.render(&poll),
            "demo.three\thotkey\tunsupported\tneeds a platform adapter\n"
        );
        // The same standing fault on every later poll must stay silent.
        assert_eq!(printer.render(&poll), "");
        assert_eq!(printer.render(&poll), "");
    }

    #[test]
    fn 감시_피드의_unsupported_중복제거는_트리거별로_이뤄진다() {
        let mut printer = WatchPrinter::default();
        assert!(
            !printer
                .render(&[unsupported("demo.one", "hotkey")])
                .is_empty()
        );
        // A *different* trigger's first fault is still news.
        let out = printer.render(&[
            unsupported("demo.one", "hotkey"),
            unsupported("demo.two", "hotkey"),
        ]);
        assert_eq!(
            out,
            "demo.two\thotkey\tunsupported\tneeds a platform adapter\n"
        );
    }

    #[test]
    fn 출력이_없는_발화는_상태_열까지만_출력한다() {
        let row = TriggerReport::new(
            key("demo.one", "schedule"),
            TriggerOutcome::Fired {
                status: FireStatus::ToolError,
                text: None,
            },
        );
        assert_eq!(
            format_status_report(&[row]),
            "demo.one\tschedule\ttool_error\n"
        );
    }
}
