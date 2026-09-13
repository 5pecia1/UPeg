//! Right-pane body for a dispatch that is still running.
//!
//! Same shape as `result_body`: the tool id, an arrow, a status word —
//! then whatever the tool has printed so far. The tail is rendered as
//! plain lines rather than parsed output because it *is* raw stream
//! text; the structured `OutputEntry` rows only exist once the final
//! envelope arrives and `View::Result` takes over.

use upeg_core::prefs::Locale;

use crate::i18n::t;

use super::super::model::LiveTail;

/// Separator between the tool id and its status, matching the Result
/// pane's `{tool_id} → {status}` header so the two read as one
/// progression.
const RUNNING_STATUS_ARROW: &str = "→";

/// Blank line between the status header and the output tail.
const RUNNING_BODY_GAP: &str = "\n\n";

pub(super) fn running_body(
    tool_id: &str,
    tail: &LiveTail,
    cancelling: bool,
    locale: Locale,
) -> String {
    let status = if cancelling {
        t(locale, "tui.running.cancelling")
    } else {
        t(locale, "tui.running.status")
    };
    let mut body = format!("{tool_id} {RUNNING_STATUS_ARROW} {status}");
    body.push_str(RUNNING_BODY_GAP);
    if tail.is_empty() {
        body.push_str(t(locale, "tui.running.empty"));
    } else {
        body.push_str(&tail.lines().collect::<Vec<_>>().join("\n"));
    }
    body
}
