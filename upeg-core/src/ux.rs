//! Surface-neutral UI vocabulary shared by human-facing surfaces.
//!
//! Keep this module narrow: labels and lifecycle wording that must not drift
//! between the TUI, Desktop/PWA, and extension surfaces.
//!
//! # Display anatomy
//!
//! 1. Primary label: the tool's `display_label`.
//! 2. Secondary metadata: the raw technical `id` where space permits.
//! 3. Detail/form metadata: toolkit, effective tags, pin kind, invoker,
//!    surfaces, inputs, and embed metadata stay visible through
//!    surface-appropriate layouts.
//!
//! # Result semantics
//!
//! [`result_status_label`] is the shared OK/ERROR vocabulary. TUI Result
//! view lifecycle: `Enter`/`F1`/`Space` re-run the same tool rather than
//! dismissing — the prior result stays on screen until the new outcome
//! replaces it; `Esc`/`q` close back to the tool list; every other key is
//! ignored. `F2` copies with the same fallback precedence the GUI's copy
//! affordance uses: the error message if the run failed, else the primary
//! output's display text, else a canonical JSON dump of all outputs.

/// Status label for a completed Tool dispatch.
pub const fn result_status_label(is_error: bool) -> &'static str {
    if is_error { "ERROR" } else { "OK" }
}
