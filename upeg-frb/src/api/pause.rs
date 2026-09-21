//! Host pause FRB surface.
//!
//! Pause is the one host control the desktop still owns: a process-local
//! flag that makes the running host answer `401 Paused` to every request
//! except `/healthz`. The tray "Pause / Resume host" item flips it and
//! the status bar renders it as a chip.

/// Sealed enum mirror of the host's pause flag — no `String`
/// discriminator crosses the bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum PausedStateDto {
    Paused,
    Running,
}

impl PausedStateDto {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) const fn from_paused(paused: bool) -> Self {
        if paused { Self::Paused } else { Self::Running }
    }
}

/// Wrapper struct around [`PausedStateDto`]. FRB v2 cannot return a
/// bare non-opaque enum through `Result<T, FrbError>` cleanly (the
/// generated Dart binding requires a struct-shaped success arm), so
/// the snapshot is the typed envelope the bridge sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PausedStateSnapshotDto {
    pub state: PausedStateDto,
}

/// Flip the host pause flag and return the resulting state. Wraps
/// `upeg_cli::toggle_paused`. The state is a process-local
/// `AtomicBool` (see `upeg_cli::infrastructure::pause`) — no I/O,
/// so this call is infallible.
#[cfg(not(target_arch = "wasm32"))]
#[flutter_rust_bridge::frb(sync)]
pub fn toggle_paused() -> PausedStateSnapshotDto {
    let paused = upeg_cli::toggle_paused();
    PausedStateSnapshotDto {
        state: PausedStateDto::from_paused(paused),
    }
}

#[cfg(target_arch = "wasm32")]
#[flutter_rust_bridge::frb(sync)]
pub fn toggle_paused() -> PausedStateSnapshotDto {
    PausedStateSnapshotDto {
        state: PausedStateDto::Running,
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn paused_state_mirrors_the_boolean_flag() {
        assert_eq!(PausedStateDto::from_paused(true), PausedStateDto::Paused);
        assert_eq!(PausedStateDto::from_paused(false), PausedStateDto::Running);
    }

    #[test]
    fn toggle_paused_flips_state_on_consecutive_calls() {
        let _guard = crate::api::test_support::host_lock().lock().unwrap();
        // Paused state is a process-local AtomicBool (see
        // upeg_cli::infrastructure::pause). Two consecutive flips
        // must alternate. Force a clean starting value so this test
        // is independent of order with other tests touching the
        // same global.
        upeg_cli::set_paused(false);
        let first = toggle_paused().state;
        let second = toggle_paused().state;
        assert_ne!(first, second, "two flips must differ");
        upeg_cli::set_paused(false);
    }
}
