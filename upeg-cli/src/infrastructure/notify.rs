//! OS-level notifications. PRD §5.9.
//!
//! Best-effort wrapper around `notify_rust`. The actual fire function
//! is module-private; the only public path is [`fire_if`], which
//! centralizes the enabled-check so call sites have no gate to forget
//! and no module outside this one can reach `notify_rust` directly.

/// Pure boolean parser for the historic `UPEG_NOTIFY` env vocabulary.
/// Public so the CLI argument-resolution path can call this at startup
/// without re-implementing the truthy-set.
#[must_use]
pub fn enabled_from(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

/// Fire a tool-outcome notification iff `enabled`. The gate lives
/// here, not at the call site, so a future dispatch path cannot
/// accidentally fire without threading its capability flag through.
pub(crate) fn fire_if(enabled: bool, tool_id: &str, success: bool, summary: &str) {
    if enabled {
        fire(tool_id, success, summary);
    }
}

fn fire(tool_id: &str, success: bool, summary: &str) {
    let title = if success {
        format!("upeg · {tool_id}")
    } else {
        format!("upeg · {tool_id} (error)")
    };
    let body = truncate(summary, 240);
    let _ = notify_rust::Notification::new()
        .appname("upeg")
        .summary(&title)
        .body(&body)
        .show();
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_flag_recognizes_truthy_values() {
        for v in ["1", "true", "TRUE", "yes", "YES", " 1 ", "  true "] {
            assert!(enabled_from(Some(v)), "{v:?} should enable");
        }
    }

    #[test]
    fn enable_flag_rejects_other_values_and_absence() {
        for v in ["0", "false", "no", "", "on", "any-other-string"] {
            assert!(!enabled_from(Some(v)), "{v:?} should not enable");
        }
        assert!(!enabled_from(None));
    }

    #[test]
    fn truncate_within_budget_returns_the_original() {
        assert_eq!(truncate("hi", 10), "hi");
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn truncate_at_exact_budget_changes_nothing() {
        assert_eq!(truncate("abcde", 5), "abcde");
    }

    #[test]
    fn truncate_replaces_overflow_with_an_ellipsis() {
        let out = truncate("0123456789abc", 6);
        // chars are 1-byte ascii here; ellipsis is one char.
        assert_eq!(out.chars().count(), 6);
        assert!(out.ends_with('…'));
        assert!(out.starts_with("01234"));
    }

    #[test]
    fn truncate_handles_multibyte_characters() {
        // each emoji is 1 char but several bytes — we must count chars,
        // not bytes, when deciding whether to truncate.
        let out = truncate("🐱🐶🦊🐰🐻", 3);
        assert_eq!(out.chars().count(), 3);
        assert!(out.ends_with('…'));
    }
}
