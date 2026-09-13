//! Per-step metadata carried out of a Chain run.
//!
//! `docs/architecture/chain.md` rule 4 promises that a skipped step is
//! recorded as metadata, but until now `skipped` was dispatcher-internal
//! state readable only from a `{{steps.<id>.skipped}}` expression — a
//! caller that just ran the chain could not see which links executed.
//!
//! The summary rides the canonical envelope, so every surface gets it
//! from the same place with no per-surface plumbing: a success carries an
//! extra [`CHAIN_STEPS_OUTPUT_ID`] output row (`json` kind), a failure
//! carries the same array under `error.details.steps`. Only steps that
//! reached a terminal state appear — a chain that stops early simply has
//! no row for the links it never got to.

use serde_json::{Value, json};
use upeg_core::{OutputEntry, OutputKind, OutputValue};

/// Output row id (and `error.details` key) holding the step summary.
///
/// Reserved on Chain tools: `parse::chain` rejects a chain that declares
/// an output of this name, so the engine row can never shadow a
/// user-declared one.
pub(crate) const CHAIN_STEPS_OUTPUT_ID: &str = "steps";

/// What became of one step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StepStatus {
    /// Dispatched and returned a success.
    Ran,
    /// `when` evaluated false, so the step never dispatched.
    Skipped,
    /// Dispatched and returned a failure (or resolved to no tool at all).
    Failed,
    /// Gated by `requires_approval` and refused: either unapproved, or
    /// approved from a surface this chain does not honor approvals from.
    Denied,
}

impl StepStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Ran => "ran",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::Denied => "denied",
        }
    }

    /// Inverse of [`Self::label`], so [`is_engine_summary_row`] recognizes
    /// exactly the statuses this module emits and nothing else.
    fn parse(label: &str) -> Option<Self> {
        Some(match label {
            "ran" => Self::Ran,
            "skipped" => Self::Skipped,
            "failed" => Self::Failed,
            "denied" => Self::Denied,
            _ => return None,
        })
    }
}

/// Field names of one summary row. Named once so the writer
/// ([`StepSummary::to_json`]) and the reader ([`is_engine_summary_row`])
/// cannot drift apart.
const SUMMARY_ROW_ID: &str = "id";
const SUMMARY_ROW_TOOL: &str = "tool";
const SUMMARY_ROW_STATUS: &str = "status";
const SUMMARY_ROW_DURATION_MS: &str = "duration_ms";

/// Number of fields in a summary row — a row with extra keys is somebody
/// else's `steps`, not this engine's.
const SUMMARY_ROW_FIELD_COUNT: usize = 4;

/// One row of the step summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StepSummary {
    id: String,
    tool: String,
    status: StepStatus,
    duration_ms: u64,
}

impl StepSummary {
    fn to_json(&self) -> Value {
        json!({
            SUMMARY_ROW_ID: self.id,
            SUMMARY_ROW_TOOL: self.tool,
            SUMMARY_ROW_STATUS: self.status.label(),
            SUMMARY_ROW_DURATION_MS: self.duration_ms,
        })
    }
}

/// Was this output row emitted by *this* engine?
///
/// A chain whose last step is another chain inherits that inner run's
/// success verbatim, engine-owned `steps` row and all — and the outer
/// engine then has nowhere to put its own row. The row has to be
/// recognizable to be folded away, and recognizable by shape rather than
/// by name alone: a tool that is not a chain and returns an output it
/// calls `steps` (a recipe's step list, an installer's plan) must still
/// collide loudly instead of being silently dropped.
///
/// The shape is this module's own: `steps`, kind `json`, a non-empty
/// array of `{ id, tool, status, duration_ms }` objects whose `status` is
/// one of [`StepStatus`]'s labels. A successful chain always records at
/// least one row, so "non-empty" costs nothing here and keeps an author's
/// empty `steps` array a conflict.
pub(crate) fn is_engine_summary_row(entry: &OutputEntry) -> bool {
    entry.id == CHAIN_STEPS_OUTPUT_ID
        && matches!(entry.kind, OutputKind::Json)
        && matches!(
            &entry.value,
            OutputValue::Json(Value::Array(rows))
                if !rows.is_empty() && rows.iter().all(is_engine_summary_json_row)
        )
}

fn is_engine_summary_json_row(row: &Value) -> bool {
    let Some(object) = row.as_object() else {
        return false;
    };
    object.len() == SUMMARY_ROW_FIELD_COUNT
        && object.get(SUMMARY_ROW_ID).is_some_and(Value::is_string)
        && object.get(SUMMARY_ROW_TOOL).is_some_and(Value::is_string)
        && object
            .get(SUMMARY_ROW_DURATION_MS)
            .is_some_and(Value::is_u64)
        && object
            .get(SUMMARY_ROW_STATUS)
            .and_then(Value::as_str)
            .and_then(StepStatus::parse)
            .is_some()
}

/// Terminal-state records for one chain run, in the order the steps
/// reached their outcome.
#[derive(Debug, Default)]
pub(crate) struct StepSummaries(Vec<StepSummary>);

impl StepSummaries {
    pub(crate) fn record(&mut self, id: &str, tool: &str, status: StepStatus, duration_ms: u64) {
        self.0.push(StepSummary {
            id: id.to_string(),
            tool: tool.to_string(),
            status,
            duration_ms,
        });
    }

    /// The summary as it appears on the wire — a JSON array, empty when
    /// the chain failed before any step reached an outcome.
    pub(crate) fn to_json(&self) -> Value {
        Value::Array(self.0.iter().map(StepSummary::to_json).collect())
    }

    /// The canonical output row a successful chain appends.
    pub(crate) fn output_entry(&self) -> OutputEntry {
        OutputEntry {
            id: CHAIN_STEPS_OUTPUT_ID.to_string(),
            label: None,
            kind: OutputKind::Json,
            value: OutputValue::Json(self.to_json()),
        }
    }
}

/// Wall-clock stopwatch for one step's dispatch.
///
/// A step's cost is the thing a chain author reaches for first when a
/// composition feels slow, and it is only observable here — the engine
/// is the only party that sees each link separately.
pub(crate) struct StepClock(std::time::Instant);

impl StepClock {
    pub(crate) fn start() -> Self {
        Self(std::time::Instant::now())
    }

    pub(crate) fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// A step that never dispatched took no measurable time; naming the
/// value keeps the zero from reading like a lost measurement.
pub(crate) const UNDISPATCHED_STEP_DURATION_MS: u64 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    fn 요약_행(status: StepStatus) -> OutputEntry {
        let mut summaries = StepSummaries::default();
        summaries.record("gate", "text.repeat", status, 7);
        summaries.output_entry()
    }

    fn json_행(value: Value) -> OutputEntry {
        OutputEntry {
            id: CHAIN_STEPS_OUTPUT_ID.to_string(),
            label: None,
            kind: OutputKind::Json,
            value: OutputValue::Json(value),
        }
    }

    #[test]
    fn 엔진이_만든_steps_행은_엔진_행으로_인식된다() {
        for status in [
            StepStatus::Ran,
            StepStatus::Skipped,
            StepStatus::Failed,
            StepStatus::Denied,
        ] {
            assert!(is_engine_summary_row(&요약_행(status)), "{status:?}");
        }
    }

    #[test]
    fn 이름만_steps인_사용자_출력은_엔진_행이_아니다() {
        // 체인이 아닌 도구가 낸 `steps` — 요리법의 단계 목록 같은 것. 조용히
        // 버려지면 안 되므로 엔진 행으로 인식되어서는 안 된다.
        let 문자열_배열 = json_행(json!(["preheat", "bake"]));
        assert!(!is_engine_summary_row(&문자열_배열));

        let 빈_배열 = json_행(json!([]));
        assert!(!is_engine_summary_row(&빈_배열));

        let 객체 = json_행(json!({ "count": 2 }));
        assert!(!is_engine_summary_row(&객체));

        let 여분_필드 = json_행(json!([{
            SUMMARY_ROW_ID: "gate",
            SUMMARY_ROW_TOOL: "text.repeat",
            SUMMARY_ROW_STATUS: "ran",
            SUMMARY_ROW_DURATION_MS: 7,
            "note": "extra",
        }]));
        assert!(!is_engine_summary_row(&여분_필드));

        let 알수없는_status = json_행(json!([{
            SUMMARY_ROW_ID: "gate",
            SUMMARY_ROW_TOOL: "text.repeat",
            SUMMARY_ROW_STATUS: "pending",
            SUMMARY_ROW_DURATION_MS: 7,
        }]));
        assert!(!is_engine_summary_row(&알수없는_status));
    }

    #[test]
    fn 같은_모양이어도_kind가_json이_아니면_엔진_행이_아니다() {
        let mut entry = 요약_행(StepStatus::Ran);
        entry.kind = OutputKind::String;
        assert!(!is_engine_summary_row(&entry));
    }
}
