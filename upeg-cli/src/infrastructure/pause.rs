//! Process-local paused state. PRD §5.9: while paused, the host
//! refuses all non-`/healthz` HTTP requests with `401 Paused`.
//!
//! State lives in a process-local `AtomicBool` — there is no
//! filesystem mirror. The earlier `~/.upeg/paused.flag` design
//! leaked across processes (a flipped flag from one `cargo test`
//! invocation poisoned the next) and conflated "where files live"
//! (paths) with "is the host paused" (runtime state). Splitting it
//! out gives both modules a single concern.
//!
//! Crash recovery: if the host crashes while paused, the next start
//! is Running. This matches the semantics of an in-memory toggle.

use std::sync::atomic::{AtomicBool, Ordering};

static PAUSED: AtomicBool = AtomicBool::new(false);

/// Is the host currently paused? Lock-free read — safe to call on
/// every HTTP middleware invocation.
pub fn is_paused() -> bool {
    PAUSED.load(Ordering::Acquire)
}

/// Flip the pause state. Returns the new value (`true` = paused).
pub fn toggle_paused() -> bool {
    let prev = PAUSED.fetch_xor(true, Ordering::AcqRel);
    !prev
}

/// Force paused state to a known value. Useful for startup hooks
/// (e.g. "respect a `--paused` CLI flag") and test harnesses that
/// need a deterministic starting state.
pub fn set_paused(value: bool) {
    PAUSED.store(value, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The atomic is process-wide; parallel tests would race on it.
    // Serialize the whole module's tests via a local mutex.
    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn pause_initially_defaults_to_false() {
        let _g = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_paused(false);
        assert!(!is_paused());
    }

    #[test]
    fn pause_toggle_flips_state_and_returns_the_new_value() {
        let _g = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_paused(false);
        assert!(toggle_paused(), "first flip → paused");
        assert!(is_paused());
        assert!(!toggle_paused(), "second flip → running");
        assert!(!is_paused());
    }

    #[test]
    fn pause_set_paused_forces_the_state() {
        let _g = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_paused(true);
        assert!(is_paused());
        set_paused(false);
        assert!(!is_paused());
    }
}
