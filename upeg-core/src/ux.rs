//! Surface-neutral UI vocabulary shared by human-facing surfaces.
//!
//! Keep this module narrow: labels and lifecycle wording that must not drift
//! between the TUI, Desktop/PWA, and extension surfaces.

/// Status label for a completed Tool dispatch.
pub const fn result_status_label(is_error: bool) -> &'static str {
    if is_error { "ERROR" } else { "OK" }
}
