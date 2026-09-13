//! The boards a fresh install starts with — as data, not code.
//!
//! Before this table existed, "which boards exist" was a `vec![…]`
//! literal inside `upeg_sources::pegboard::default_boards`, which meant
//! the set was closed: nothing outside that function could extend it.
//! A Project Manifest's `[[boards]]` extends the board list at runtime
//! (see [`crate::BoardStoreKey`]), so the built-in half has to be
//! readable data that other crates can also *validate against* — the
//! loader rejects a project board that would shadow one of these.

/// One board seeded into a fresh pegboard state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltinBoard {
    /// Store key and visible key — built-in boards are global, so the
    /// two are the same string.
    pub key: &'static str,
    /// Tab title.
    pub title: &'static str,
}

/// Boards every install has. Order is the tab order.
pub const BUILTIN_BOARDS: &[BuiltinBoard] = &[
    BuiltinBoard {
        key: "dev",
        title: "Dev",
    },
    BuiltinBoard {
        key: "trading",
        title: "Trading",
    },
    BuiltinBoard {
        key: "personal",
        title: "Personal",
    },
];

/// Whether `key` names a built-in board. Used by the loader to reject a
/// Project Manifest board that would shadow one.
#[must_use]
pub fn is_builtin_board(key: &str) -> bool {
    BUILTIN_BOARDS.iter().any(|board| board.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board_key::BoardKey;

    #[test]
    fn 내장_보드_키는_모두_유효한_board_key다() {
        for board in BUILTIN_BOARDS {
            BoardKey::parse(board.key).unwrap_or_else(|e| panic!("내장 보드 `{}`: {e}", board.key));
            assert!(!board.title.trim().is_empty(), "제목이 비어 있다");
        }
    }

    #[test]
    fn 내장_보드_키는_중복되지_않는다() {
        let mut keys: Vec<&str> = BUILTIN_BOARDS.iter().map(|board| board.key).collect();
        let before = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), before);
    }

    #[test]
    fn is_builtin_board는_표를_그대로_반영한다() {
        assert!(is_builtin_board("dev"));
        assert!(!is_builtin_board("upeg-dev"));
    }
}
