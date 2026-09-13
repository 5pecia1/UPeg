//! Shared user preferences (Tweaks, Locale) and their persistence boundary.
//!
//! Surfaces (Flutter desktop/PWA, `upeg-cli`'s TUI) own their own rendering
//! and catalog, but read/write the same `Tweaks` record so a change in one
//! propagates to the others on cold restart. Layer split:
//!
//! - [`locale::Locale`] — pure display-language selector + platform-agnostic
//!   detection helper (`detect_from_str`).
//! - [`tweaks`] — the `Tweaks` struct + serde.
//!
//! Filesystem persistence (path-based read/write and first-run bootstrap) is
//! deliberately *not* here: `upeg-core` stays domain-only. Those live one layer
//! up in `upeg_runtime::persistence`, which depends on this crate.
//!
//! Detection sources (POSIX `$LANG`, browser `navigator.language`, CLI flag)
//! are the caller's responsibility — `prefs` exposes only pure functions so
//! every surface can unit-test detection without platform plumbing.

mod locale;
mod tweaks;

pub use locale::Locale;
#[cfg(feature = "serde")]
pub use tweaks::parse_tweaks;
pub use tweaks::{Accent, Theme, Tweaks};
