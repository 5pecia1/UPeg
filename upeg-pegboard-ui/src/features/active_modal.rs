//! Single source of truth for which modal — if any — is currently
//! open across the desktop frame.
//!
//! Pre-extraction the three modals had three independent open signals
//! (`palette: Signal<bool>`, `settings: Signal<bool>`,
//! `focus: Signal<Option<&str>>`), with nothing preventing them from
//! all opening at once. The visible result was drift: SettingsModal
//! advertised `z-index:1100` while the other two used `50` / `60`,
//! and the global keydown's ESC handler had to know about each
//! signal individually — it silently didn't know about `settings`,
//! which is what kicked off this refactor.
//!
//! Unifying behind an enum gives us mutex by type rather than by
//! convention: opening any modal transitions the same signal, so
//! the previous state is structurally cleared rather than left to
//! z-index ordering luck.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    Palette,
    Settings,
    Expanded(&'static str),
    /// Destructive confirm before dropping a board + its layout entry.
    /// Pre-Phase-8 the delete path called `window.confirm`, which was a
    /// no-op fallback on native desktop — clicking `×` would silently
    /// destroy the board. Routing the confirmation through the unified
    /// modal signal makes the surface symmetric across web + native and
    /// matches the TUI's `ConfirmDeleteBoard` view.
    ConfirmDeleteBoard(&'static str),
}

impl ActiveModal {
    pub const fn is_palette(self) -> bool {
        matches!(self, Self::Palette)
    }

    pub const fn is_settings(self) -> bool {
        matches!(self, Self::Settings)
    }

    /// The expanded modal carries the tool id, so its open-check
    /// returns the id (or `None` when not active).
    pub const fn expanded_id(self) -> Option<&'static str> {
        match self {
            Self::Expanded(id) => Some(id),
            _ => None,
        }
    }

    /// Pending board delete target, or `None` when no confirm is up.
    pub const fn confirm_delete_board_key(self) -> Option<&'static str> {
        match self {
            Self::ConfirmDeleteBoard(key) => Some(key),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveModal;

    #[test]
    fn 변형은_술어로_상호_배타성을_보장한다() {
        // The whole point of the enum is that no two modals report
        // "open" at the same time. Asserting this via the predicate
        // surface pins the public API contract callers rely on.
        let cases = [
            ActiveModal::None,
            ActiveModal::Palette,
            ActiveModal::Settings,
            ActiveModal::Expanded("num.hex_to_decimal"),
            ActiveModal::ConfirmDeleteBoard("dev"),
        ];
        for case in cases {
            let flags = [
                case.is_palette(),
                case.is_settings(),
                case.expanded_id().is_some(),
                case.confirm_delete_board_key().is_some(),
            ];
            let open_count = flags.iter().filter(|f| **f).count();
            assert!(
                open_count <= 1,
                "ActiveModal variant {case:?} reported open in {open_count} \
                 predicates — predicates must be mutually exclusive"
            );
        }
    }

    #[test]
    fn 확장된_id는_페이로드를_추출한다() {
        assert_eq!(
            ActiveModal::Expanded("num.hex_to_decimal").expanded_id(),
            Some("num.hex_to_decimal")
        );
        assert_eq!(ActiveModal::None.expanded_id(), None);
        assert_eq!(ActiveModal::Palette.expanded_id(), None);
        assert_eq!(ActiveModal::Settings.expanded_id(), None);
    }
}
