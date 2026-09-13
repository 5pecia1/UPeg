//! Pure change-detection state for the trigger watch loop.
//!
//! The watch loop re-evaluates every registered trigger once per poll. Without
//! memory that means "the condition holds" is indistinguishable from "the
//! condition just became true", and a trigger fires on every single poll. Each
//! gate here holds the minimum cross-poll state needed to answer the second
//! question, and every gate is pure: the clock and the filesystem are passed in
//! by the caller, so the whole layer is testable without sleeping or touching
//! the disk.

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};
use upeg_runtime::{ScheduleCondition, TriggerBinding};

/// Identity of one registered trigger across watch polls.
///
/// A tool may declare several triggers of the same source (two watched paths,
/// two schedules), so the key carries source *and* condition — never the tool id
/// alone.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TriggerKey {
    pub(crate) tool_id: String,
    pub(crate) source: String,
    pub(crate) condition: Option<String>,
}

impl TriggerKey {
    pub(crate) fn from_binding(binding: &TriggerBinding) -> Self {
        Self {
            tool_id: binding.tool_id.to_string(),
            source: binding.source.clone(),
            condition: binding.condition.clone(),
        }
    }
}

/// Per-trigger change detection for `clipboard` triggers.
///
/// The watch loop must fire a clipboard trigger only when the clipboard's
/// contents *change*, and must never fire for the content that already sat in
/// the clipboard when the watch started. Each trigger gets its own baseline and
/// last-seen value, keyed like every other gate: a single shared baseline would
/// let the first clipboard trigger evaluated in a poll consume the change and
/// starve every other clipboard trigger forever.
#[derive(Debug, Default)]
pub(crate) struct ClipboardChangeGate {
    last_seen: HashMap<TriggerKey, String>,
}

impl ClipboardChangeGate {
    /// Records `current` as the newest clipboard value for `key` and returns the
    /// text to fire on, or `None` to suppress this poll (baseline, unchanged, or
    /// empty).
    pub(crate) fn evaluate(&mut self, key: &TriggerKey, current: &str) -> Option<String> {
        let Some(previous) = self.last_seen.insert(key.clone(), current.to_string()) else {
            // First poll of the watch: adopt as baseline, do not fire.
            return None;
        };
        if previous == current {
            return None;
        }
        (!current.is_empty()).then(|| current.to_string())
    }
}

/// Per-trigger firing state for `schedule` triggers.
///
/// [`ScheduleCondition::Now`] fires exactly once — on the poll that starts the
/// watch. [`ScheduleCondition::Every`] fires on that same first poll and then
/// again whenever a full interval has elapsed since the moment it was *due*, so
/// the poll cadence never leaks into the declared schedule.
///
/// # Why the anchor advances by the interval
///
/// The watch notices a due moment only on its next poll, so every fire is a
/// little late. Anchoring the next interval to the *poll* time would fold that
/// lateness into the schedule and let it accumulate: an `every:2s` trigger
/// polled once per second drifts to 3s, then 4s, and so on. The anchor
/// therefore advances to `last_fired + interval` — the moment the fire was due —
/// which keeps the schedule on its original grid.
///
/// # Why a stalled watch does not fire a burst
///
/// If the host slept (or the loop stalled) for many intervals, walking the
/// anchor forward one interval per poll would fire a backlog of catch-up
/// dispatches for moments that have long passed. When the anchor is already more
/// than one interval behind, it is re-anchored to `now` instead: the trigger
/// fires exactly once for the whole gap, then resumes on a fresh grid.
#[derive(Debug, Default)]
pub(crate) struct ScheduleGate {
    /// The moment each trigger was last *due* (not the poll that noticed it).
    last_fired: HashMap<TriggerKey, Instant>,
}

impl ScheduleGate {
    /// Whether `condition` is due at `now`, recording the fire when it is.
    pub(crate) fn is_due(
        &mut self,
        key: &TriggerKey,
        condition: ScheduleCondition,
        now: Instant,
    ) -> bool {
        let Some(last_fired) = self.last_fired.get(key).copied() else {
            // First poll of the watch: both `now` and `every:<d>` fire.
            self.last_fired.insert(key.clone(), now);
            return true;
        };
        let ScheduleCondition::Every(interval) = condition else {
            // `now` has already fired; it never fires again in this watch.
            return false;
        };
        if now.saturating_duration_since(last_fired) < interval {
            return false;
        }
        self.last_fired
            .insert(key.clone(), next_anchor(last_fired, interval, now));
        true
    }
}

/// Where the schedule grid continues after a fire that was due at
/// `last_fired + interval` and noticed at `now`.
fn next_anchor(last_fired: Instant, interval: Duration, now: Instant) -> Instant {
    last_fired
        .checked_add(interval)
        .filter(|due| now.saturating_duration_since(*due) < interval)
        .unwrap_or(now)
}

/// Per-trigger change detection for `file` and `directory` triggers.
///
/// The gate remembers the last observation of the watched path, where `None`
/// means "absent (or not of the declared kind)" and `Some(mtime)` means
/// "present, last modified then". It fires on creation (absent → present) and
/// on modification (a different mtime). Removal is deliberately silent: a
/// disappearing path carries no payload for the tool's `path` argument, so
/// firing on it would dispatch a tool against something that no longer exists.
/// As with the clipboard gate, the first poll is a silent baseline — a path that
/// already exists when the watch starts is not a change.
#[derive(Debug, Default)]
pub(crate) struct PathChangeGate {
    last_seen: HashMap<TriggerKey, Option<SystemTime>>,
}

impl PathChangeGate {
    /// Whether `observed` is a change worth firing on, recording it either way.
    pub(crate) fn is_changed(&mut self, key: &TriggerKey, observed: Option<SystemTime>) -> bool {
        let Some(previous) = self.last_seen.insert(key.clone(), observed) else {
            // First poll of the watch: adopt as baseline, do not fire.
            return false;
        };
        match (previous, observed) {
            // Created, or modified while present.
            (None, Some(_)) => true,
            (Some(before), Some(after)) => before != after,
            // Removed, or still absent.
            (_, None) => false,
        }
    }
}

/// Cross-poll state the watch loop threads through every trigger evaluation.
///
/// Single-shot `upeg trigger run` builds no state at all (`None` at the call
/// site), which is what keeps its "fire whatever holds right now" semantics.
#[derive(Debug, Default)]
pub(crate) struct TriggerWatchState {
    pub(crate) clipboard: ClipboardChangeGate,
    pub(crate) schedule: ScheduleGate,
    pub(crate) path: PathChangeGate,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trigger_key(source: &str, condition: &str) -> TriggerKey {
        TriggerKey::from_binding(&TriggerBinding {
            tool_id: "demo.tool",
            source: source.to_string(),
            condition: Some(condition.to_string()),
        })
    }

    /// Key of a clipboard trigger; two tools may watch the same clipboard.
    fn clipboard_key(tool_id: &'static str) -> TriggerKey {
        TriggerKey::from_binding(&TriggerBinding {
            tool_id,
            source: "clipboard".to_string(),
            condition: None,
        })
    }

    #[test]
    fn 클립보드_트리거는_시작_시점_내용으로_발화하지_않는다() {
        let mut gate = ClipboardChangeGate::default();
        let key = clipboard_key("demo.tool");
        // The clipboard already holds content when the watch starts.
        assert_eq!(gate.evaluate(&key, "hello"), None);
    }

    #[test]
    fn 클립보드_트리거는_값이_변하지_않으면_재발화하지_않는다() {
        let mut gate = ClipboardChangeGate::default();
        let key = clipboard_key("demo.tool");
        // Baseline capture (silent), then a real change fires once.
        assert_eq!(gate.evaluate(&key, "first"), None);
        assert_eq!(gate.evaluate(&key, "second"), Some("second".to_string()));
        // Same value on the next poll must not re-fire.
        assert_eq!(gate.evaluate(&key, "second"), None);
        assert_eq!(gate.evaluate(&key, "second"), None);
    }

    #[test]
    fn 클립보드_트리거는_빈_값으로는_발화하지_않는다() {
        let mut gate = ClipboardChangeGate::default();
        let key = clipboard_key("demo.tool");
        assert_eq!(gate.evaluate(&key, "value"), None); // baseline
        // Cleared clipboard is a change, but empty content never fires.
        assert_eq!(gate.evaluate(&key, ""), None);
        // Re-copying the value after a clear is a change and fires again.
        assert_eq!(gate.evaluate(&key, "value"), Some("value".to_string()));
    }

    #[test]
    fn 클립보드_상태는_트리거별로_분리된다() {
        // A single shared baseline let the first trigger evaluated in a poll
        // consume the change and starved every other clipboard trigger.
        let mut gate = ClipboardChangeGate::default();
        let first = clipboard_key("demo.first");
        let second = clipboard_key("demo.second");
        assert_eq!(gate.evaluate(&first, "before"), None); // baseline
        assert_eq!(gate.evaluate(&second, "before"), None); // baseline
        // One clipboard change must fire *both* bindings.
        assert_eq!(gate.evaluate(&first, "after"), Some("after".to_string()));
        assert_eq!(gate.evaluate(&second, "after"), Some("after".to_string()));
        // And neither re-fires while the value stays put.
        assert_eq!(gate.evaluate(&first, "after"), None);
        assert_eq!(gate.evaluate(&second, "after"), None);
    }

    #[test]
    fn now_스케줄은_감시_시작에_한_번만_발화한다() {
        let mut gate = ScheduleGate::default();
        let key = trigger_key("schedule", "now");
        let start = Instant::now();
        assert!(gate.is_due(&key, ScheduleCondition::Now, start));
        for tick in 1..5 {
            assert!(
                !gate.is_due(
                    &key,
                    ScheduleCondition::Now,
                    start + Duration::from_secs(tick)
                ),
                "{tick}초 폴에서 now가 다시 발화하면 안 된다"
            );
        }
    }

    #[test]
    fn every_스케줄은_주기가_지난_폴에서만_발화한다() {
        let mut gate = ScheduleGate::default();
        let key = trigger_key("schedule", "every:2s");
        let every_2s = ScheduleCondition::Every(Duration::from_secs(2));
        let start = Instant::now();
        // Fires immediately at watch start.
        assert!(gate.is_due(&key, every_2s, start));
        // One second later the interval has not elapsed.
        assert!(!gate.is_due(&key, every_2s, start + Duration::from_secs(1)));
        // Two seconds after the last fire it is due again.
        assert!(gate.is_due(&key, every_2s, start + Duration::from_secs(2)));
        assert!(!gate.is_due(&key, every_2s, start + Duration::from_secs(3)));
        assert!(gate.is_due(&key, every_2s, start + Duration::from_secs(4)));
    }

    #[test]
    fn every_스케줄은_늦게_발화해도_주기가_밀리지_않는다() {
        // The watch notices a due moment only on its next poll. Re-anchoring to
        // that poll would fold the lateness into the schedule and let it
        // accumulate (2s → 3s → 4s → …); the grid must stay where it was.
        let mut gate = ScheduleGate::default();
        let key = trigger_key("schedule", "every:2s");
        let every_2s = ScheduleCondition::Every(Duration::from_secs(2));
        let start = Instant::now();
        assert!(gate.is_due(&key, every_2s, start));
        // Due at +2s, but the poll that notices lands at +3s.
        assert!(gate.is_due(&key, every_2s, start + Duration::from_secs(3)));
        // Next due moment is +4s, not +5s.
        assert!(gate.is_due(&key, every_2s, start + Duration::from_secs(4)));
        assert!(!gate.is_due(&key, every_2s, start + Duration::from_secs(5)));
        assert!(gate.is_due(&key, every_2s, start + Duration::from_secs(6)));
    }

    #[test]
    fn 멈췄던_감시는_밀린_주기를_한_번만_발화한다() {
        // A suspended host must not produce a burst of catch-up dispatches for
        // moments that have long passed.
        let mut gate = ScheduleGate::default();
        let key = trigger_key("schedule", "every:2s");
        let every_2s = ScheduleCondition::Every(Duration::from_secs(2));
        let start = Instant::now();
        assert!(gate.is_due(&key, every_2s, start));
        let after_stall = start + Duration::from_secs(60);
        assert!(gate.is_due(&key, every_2s, after_stall));
        // Re-anchored to the wake-up moment: the backlog is not replayed.
        assert!(!gate.is_due(&key, every_2s, after_stall + Duration::from_secs(1)));
        assert!(gate.is_due(&key, every_2s, after_stall + Duration::from_secs(2)));
    }

    #[test]
    fn 스케줄_상태는_트리거별로_분리된다() {
        let mut gate = ScheduleGate::default();
        let fast = trigger_key("schedule", "every:1s");
        let slow = trigger_key("schedule", "every:10s");
        let start = Instant::now();
        assert!(gate.is_due(
            &fast,
            ScheduleCondition::Every(Duration::from_secs(1)),
            start
        ));
        assert!(gate.is_due(
            &slow,
            ScheduleCondition::Every(Duration::from_secs(10)),
            start
        ));
        let later = start + Duration::from_secs(1);
        assert!(gate.is_due(
            &fast,
            ScheduleCondition::Every(Duration::from_secs(1)),
            later
        ));
        assert!(!gate.is_due(
            &slow,
            ScheduleCondition::Every(Duration::from_secs(10)),
            later
        ));
    }

    #[test]
    fn 경로_트리거는_생성될_때_발화한다() {
        let mut gate = PathChangeGate::default();
        let key = trigger_key("file", "/tmp/demo.txt");
        let created = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        // Absent at watch start (baseline), then created.
        assert!(!gate.is_changed(&key, None));
        assert!(gate.is_changed(&key, Some(created)));
    }

    #[test]
    fn 경로_트리거는_이미_있는_경로로_시작해도_발화하지_않는다() {
        let mut gate = PathChangeGate::default();
        let key = trigger_key("file", "/tmp/demo.txt");
        let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        assert!(!gate.is_changed(&key, Some(mtime)));
        // Same mtime on later polls is not a change.
        assert!(!gate.is_changed(&key, Some(mtime)));
        assert!(!gate.is_changed(&key, Some(mtime)));
    }

    #[test]
    fn 경로_트리거는_mtime이_바뀌면_발화한다() {
        let mut gate = PathChangeGate::default();
        let key = trigger_key("directory", "/tmp/inbox");
        let first = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let second = first + Duration::from_secs(1);
        assert!(!gate.is_changed(&key, Some(first))); // baseline
        assert!(gate.is_changed(&key, Some(second)));
        assert!(!gate.is_changed(&key, Some(second)));
    }

    #[test]
    fn 경로_트리거는_삭제로는_발화하지_않는다() {
        let mut gate = PathChangeGate::default();
        let key = trigger_key("file", "/tmp/demo.txt");
        let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        assert!(!gate.is_changed(&key, Some(mtime))); // baseline
        assert!(!gate.is_changed(&key, None)); // removed: silent
        // Re-creating it is a change and fires again.
        assert!(gate.is_changed(&key, Some(mtime)));
    }

    #[test]
    fn 경로_상태는_트리거별로_분리된다() {
        let mut gate = PathChangeGate::default();
        let one = trigger_key("file", "/tmp/one.txt");
        let two = trigger_key("file", "/tmp/two.txt");
        let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        assert!(!gate.is_changed(&one, None));
        assert!(!gate.is_changed(&two, Some(mtime)));
        // `one` is created; `two` is untouched.
        assert!(gate.is_changed(&one, Some(mtime)));
        assert!(!gate.is_changed(&two, Some(mtime)));
    }
}
