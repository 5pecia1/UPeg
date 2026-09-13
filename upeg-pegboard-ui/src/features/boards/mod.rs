//! Board state — the user's pegboard tabs (dev / trading / personal / + …).
//!
//! Boards used to be a hardcoded `&[Board]` const. Iter 24 makes them
//! user-editable: `+ board` prompts for a title; edit-mode shows × per tab.
//! Persisted to shared UI key `upeg.boards.v1` and included in
//! `EnvironmentBackup` exports/imports.
//!
//! `Board` keeps its `&'static str` fields so existing call sites
//! (`BOARDS.iter()`, `Vec<&'static str>` layouts keys) compile unchanged.
//! User-added boards `Box::leak` their key+title — a few dozen bytes per
//! board, never reclaimed; acceptable for the lifetime of a tab.

use serde::{Deserialize, Serialize};
use upeg_core::BoardGuidance;

/// One pegboard tab. `key` is the canonical id (slug) used everywhere; only
/// `title` changes when the user renames.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Board {
    pub key: &'static str,
    pub title: &'static str,
    pub guidance: BoardGuidance,
}

/// Serializable mirror of `Board` — owned strings round-trip through serde.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardData {
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub guidance: BoardGuidance,
}

impl From<&Board> for BoardData {
    fn from(b: &Board) -> Self {
        Self {
            key: b.key.to_string(),
            title: b.title.to_string(),
            guidance: b.guidance.clone(),
        }
    }
}

impl From<BoardData> for Board {
    fn from(d: BoardData) -> Self {
        // Each user-added board leaks ~50 bytes that live for the rest of
        // the process. Negligible at the ~10s of boards a user might create.
        Self {
            key: Box::leak(d.key.into_boxed_str()),
            title: Box::leak(d.title.into_boxed_str()),
            guidance: d.guidance,
        }
    }
}

/// Default boards on a fresh install.
pub fn default_boards() -> Vec<Board> {
    vec![
        Board {
            key: "dev",
            title: "Dev",
            guidance: BoardGuidance::default(),
        },
        Board {
            key: "trading",
            title: "Trading",
            guidance: BoardGuidance::default(),
        },
        Board {
            key: "personal",
            title: "Personal",
            guidance: BoardGuidance::default(),
        },
    ]
}

pub fn serialize_boards(boards: &[Board]) -> String {
    let data: Vec<BoardData> = boards.iter().map(BoardData::from).collect();
    serde_json::to_string(&data).unwrap_or_else(|_| "[]".into())
}

pub fn deserialize_boards(json: &str) -> Option<Vec<Board>> {
    let data: Vec<BoardData> = serde_json::from_str(json).ok()?;
    Some(data.into_iter().map(Board::from).collect())
}

pub fn save_boards(boards: &[Board]) {
    let json = serialize_boards(boards);
    let _ = crate::platform::storage::set_item(crate::platform::storage::BOARDS_KEY, &json);
}

pub fn load_boards() -> Option<Vec<Board>> {
    let raw = crate::platform::storage::get_item(crate::platform::storage::BOARDS_KEY)?;
    deserialize_boards(&raw)
}

/// Add a new board with the given title. Returns the new board's key on
/// success, or `None` if the title is blank or all-whitespace. The key is a
/// slugified form of the title, deduped against existing boards.
pub fn add_board(boards: &mut Vec<Board>, title: &str) -> Option<&'static str> {
    let title_trimmed = title.trim();
    if title_trimmed.is_empty() {
        return None;
    }

    let title_static: &'static str = Box::leak(title_trimmed.to_string().into_boxed_str());
    let base_key = slugify(title_trimmed);
    let key = dedupe_key(&base_key, boards);
    let key_static: &'static str = Box::leak(key.into_boxed_str());

    boards.push(Board {
        key: key_static,
        title: title_static,
        guidance: BoardGuidance::default(),
    });
    Some(key_static)
}

/// Remove a board by key. Returns `true` if a board was removed. Caller is
/// responsible for any side effects (e.g. dropping the layout entry).
pub fn remove_board(boards: &mut Vec<Board>, key: &str) -> bool {
    let len_before = boards.len();
    boards.retain(|b| b.key != key);
    boards.len() < len_before
}

/// Rename a board: replace its `title` (the visible label) while keeping
/// its `key` (the stable id used by layouts). Returns `true` if the board
/// existed and the new title was non-blank. The new title is `Box::leak`ed
/// to satisfy `Board`'s `&'static str` field, mirroring `add_board`.
///
/// Removed the previous `#[cfg(any(target_arch = "wasm32", test))]`
/// gate in Phase 7 — native desktop now has a real inline rename UX,
/// so this function ships unconditionally.
pub fn rename_board(boards: &mut [Board], key: &str, new_title: &str) -> bool {
    let trimmed = new_title.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Some(b) = boards.iter_mut().find(|b| b.key == key) else {
        return false;
    };
    b.title = Box::leak(trimmed.to_string().into_boxed_str());
    true
}

/// Slugify delegate. The single source of truth lives in
/// `upeg_runtime::pegboard::slugify` so a board added in Desktop hashes
/// to the same key as the equivalent title typed in TUI. Re-exporting
/// keeps existing call sites (`crate::features::boards::slugify`) and
/// the local test suite working unchanged.
pub use upeg_runtime::pegboard::slugify;

/// If `base` collides with an existing key, append `-2`, `-3`, ... until unique.
fn dedupe_key(base: &str, boards: &[Board]) -> String {
    let exists = |k: &str| boards.iter().any(|b| b.key == k);
    if !exists(base) {
        return base.to_string();
    }
    let max_attempt = boards.len() + 2;
    for n in 2..=max_attempt {
        let candidate = format!("{base}-{n}");
        if !exists(&candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", max_attempt + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 보드_안내는_유아이_변환과_직렬화에서_유지된다() {
        let json = r##"[{"key":"dev","title":"Dev","guidance":{"description":"개발 작업","instructions":"# 순서\n\n    cargo test\n"}}]"##;
        let boards = deserialize_boards(json).expect("보드 읽기");
        let exported: serde_json::Value =
            serde_json::from_str(&serialize_boards(&boards)).expect("제이슨 읽기");

        assert_eq!(exported[0]["guidance"]["description"], "개발 작업");
        assert_eq!(
            exported[0]["guidance"]["instructions"],
            "# 순서\n\n    cargo test\n"
        );
    }

    #[test]
    fn 기본은_세개_보드를_가진다() {
        let b = default_boards();
        assert_eq!(b.len(), 3);
        let keys: Vec<&str> = b.iter().map(|b| b.key).collect();
        assert_eq!(keys, vec!["dev", "trading", "personal"]);
    }

    #[test]
    fn 왕복을_직렬화한다() {
        let original = default_boards();
        let json = serialize_boards(&original);
        let back = deserialize_boards(&json).expect("parse");
        assert_eq!(back, original);
    }

    #[test]
    fn 역직렬화_쓰레기값은_없음을_반환한다() {
        assert!(deserialize_boards("not json").is_none());
        assert!(deserialize_boards("42").is_none());
    }

    #[test]
    fn 보드_추가는_제목을_slugify하고_중복을_제거한다() {
        let mut boards = default_boards();
        let key = add_board(&mut boards, "My Board!").expect("added");
        assert_eq!(key, "my-board");
        assert_eq!(boards.len(), 4);

        // Duplicate title gets a suffix
        let key2 = add_board(&mut boards, "My Board").expect("added");
        assert_eq!(key2, "my-board-2");
        assert_eq!(boards.len(), 5);
    }

    #[test]
    fn 보드_추가는_빈_제목을_거부한다() {
        let mut boards = default_boards();
        assert!(add_board(&mut boards, "").is_none());
        assert!(add_board(&mut boards, "   ").is_none());
        assert_eq!(boards.len(), 3);
    }

    #[test]
    fn 보드_삭제는_키를_기준으로_동작한다() {
        let mut boards = default_boards();
        assert!(remove_board(&mut boards, "dev"));
        assert_eq!(boards.len(), 2);
        assert!(!remove_board(&mut boards, "nonexistent"));
        assert_eq!(boards.len(), 2);
    }

    #[test]
    fn 보드_이름변경은_제목만_바꾼다() {
        let mut boards = default_boards();
        assert!(rename_board(&mut boards, "dev", "Development"));
        let dev = boards.iter().find(|b| b.key == "dev").unwrap();
        assert_eq!(dev.title, "Development");
        // Key stays the same
        assert!(!rename_board(&mut boards, "nonexistent", "X"));
    }

    #[test]
    fn 보드_이름변경은_빈_제목을_거부한다() {
        let mut boards = default_boards();
        assert!(!rename_board(&mut boards, "dev", ""));
        assert!(!rename_board(&mut boards, "dev", "  "));
        let dev = boards.iter().find(|b| b.key == "dev").unwrap();
        assert_eq!(dev.title, "Dev");
    }

    #[test]
    fn slugify_다양한_입력들을_검증한다() {
        assert_eq!(slugify(""), "board");
        assert_eq!(slugify("   "), "board");
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("UPPER_case"), "upper-case");
        assert_eq!(slugify("special!!!chars"), "special-chars");
        assert_eq!(slugify("trailing---"), "trailing");
        assert_eq!(slugify("already-slug"), "already-slug");
    }

    #[test]
    fn 고유하면_중복제거_키는_기본값을_반환한다() {
        let boards: Vec<Board> = vec![];
        assert_eq!(dedupe_key("foo", &boards), "foo");
    }

    #[test]
    fn 중복제거_키는_충돌_시_접미사를_붙인다() {
        let boards = default_boards();
        assert_eq!(dedupe_key("dev", &boards), "dev-2");
        assert_eq!(dedupe_key("trading", &boards), "trading-2");
    }
}
