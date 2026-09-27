//! Project-scoped board identity: namespaces and store keys.
//!
//! A `.upeg/project.toml` declaration may add project boards with a
//! top-level `[[boards]]` array. Those boards exist only while that
//! manifest is detected, and their pins must never bleed into another
//! project — two checkouts of the same repository are two different
//! projects as far as the pegboard store is concerned.
//!
//! The rule that makes both properties fall out is *one* naming
//! convention, expressed here so no surface can spell it differently:
//!
//! ```text
//!   global board   →  store key = "dev"
//!   project board  →  store key = "project:<namespace>:<id>"
//! ```
//!
//! `<namespace>` is a stable digest of the project root's absolute path
//! ([`ProjectBoardNamespace::for_project_root`]); `<id>` is the id the
//! manifest declared. Surfaces never show the store key — the *visible*
//! board reference stays the bare declared id (see
//! `upeg_sources::pegboard`), because [`crate::BoardKey`] rejects the
//! separator and therefore cannot be confused with a store key.

use std::path::Path;

use crate::board_key::{BoardKey, PROJECT_BOARD_KEY_SEPARATOR};

/// Leading segment marking a store key as project-scoped.
pub const PROJECT_BOARD_KEY_PREFIX: &str = "project";

/// FNV-1a 64-bit offset basis. Chosen over `DefaultHasher` because the
/// digest is *persisted* in board store keys: it must be identical on
/// every machine and across compiler versions, which `SipHasher13`'s
/// keys do not guarantee.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Digest width in hex characters (`u64` → 16 nibbles).
const NAMESPACE_HEX_WIDTH: usize = 16;

/// Stable identity of the Project Manifest a board belongs to.
///
/// Derived from the project root's path, so moving a project directory
/// starts a fresh set of project-board placements — an accepted
/// trade-off, documented in `upeg_sources::project`'s module docs:
/// path is the only identity upeg can read before parsing the config,
/// and a project-declared id would collide between unrelated
/// checkouts.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectBoardNamespace(String);

impl ProjectBoardNamespace {
    /// Digest `path` into a namespace. The path is used verbatim — the
    /// caller is responsible for handing over an absolute path (the
    /// loader canonicalizes the project root before it gets here), because
    /// two spellings of the same file must not produce two namespaces.
    #[must_use]
    pub fn for_project_root(path: &Path) -> Self {
        Self(fnv1a64_hex(path.to_string_lossy().as_bytes()))
    }

    /// Rehydrate a namespace read back out of a store key.
    #[must_use]
    pub(crate) fn from_digest(digest: &str) -> Self {
        Self(digest.to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProjectBoardNamespace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn fnv1a64_hex(bytes: &[u8]) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:0NAMESPACE_HEX_WIDTH$x}")
}

/// The key a board row is stored under in the pegboard store.
///
/// Distinct from [`BoardKey`] on purpose: `BoardKey` is what a *user*
/// types and what surfaces display, while this is what SQLite holds.
/// Keeping them apart is what lets a project board be visible as plain
/// `upeg-dev` while persisting as `project:<ns>:upeg-dev`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BoardStoreKey(String);

impl BoardStoreKey {
    /// Store key of a global (built-in or user-created) board: the
    /// visible key verbatim.
    #[must_use]
    pub fn global(key: &BoardKey) -> Self {
        Self(key.as_str().to_string())
    }

    /// Store key of a board declared by the manifest identified by
    /// `namespace`.
    #[must_use]
    pub fn project(namespace: &ProjectBoardNamespace, id: &BoardKey) -> Self {
        Self(format!(
            "{PROJECT_BOARD_KEY_PREFIX}{PROJECT_BOARD_KEY_SEPARATOR}{namespace}{PROJECT_BOARD_KEY_SEPARATOR}{id}",
        ))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Classify a key read back out of the store.
    ///
    /// A row whose key *looks* project-scoped but whose id is not a
    /// valid [`BoardKey`] is [`StoredBoardKey::Unusable`] rather than an
    /// error: the store is shared with other devices, and one corrupt
    /// row must not make the whole board list unreadable — it is simply
    /// invisible.
    #[must_use]
    pub fn classify(raw: &str) -> StoredBoardKey {
        let Some(rest) = raw.strip_prefix(PROJECT_BOARD_KEY_PREFIX) else {
            return global_or_unusable(raw);
        };
        let Some(rest) = rest.strip_prefix(PROJECT_BOARD_KEY_SEPARATOR) else {
            return global_or_unusable(raw);
        };
        let Some((digest, id)) = rest.split_once(PROJECT_BOARD_KEY_SEPARATOR) else {
            return StoredBoardKey::Unusable;
        };
        if digest.is_empty() {
            return StoredBoardKey::Unusable;
        }
        match BoardKey::parse(id) {
            Ok(id) => StoredBoardKey::Project {
                namespace: ProjectBoardNamespace::from_digest(digest),
                id,
            },
            Err(_) => StoredBoardKey::Unusable,
        }
    }
}

fn global_or_unusable(raw: &str) -> StoredBoardKey {
    BoardKey::parse(raw).map_or(StoredBoardKey::Unusable, StoredBoardKey::Global)
}

impl std::fmt::Display for BoardStoreKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a stored board key turned out to be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoredBoardKey {
    /// A built-in or user-created board, visible in every project.
    Global(BoardKey),
    /// A board declared by one Project Manifest, visible only while
    /// that manifest is detected.
    Project {
        namespace: ProjectBoardNamespace,
        id: BoardKey,
    },
    /// Neither — a malformed row. Invisible to every surface.
    Unusable,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(raw: &str) -> BoardKey {
        BoardKey::parse(raw).expect("valid key")
    }

    #[test]
    fn namespace_is_stable_for_the_same_path() {
        let a = ProjectBoardNamespace::for_project_root(Path::new("/home/u/proj"));
        let b = ProjectBoardNamespace::for_project_root(Path::new("/home/u/proj"));
        assert_eq!(a, b);
        assert_eq!(a.as_str().len(), NAMESPACE_HEX_WIDTH);
    }

    #[test]
    fn namespace_differs_between_projects() {
        let a = ProjectBoardNamespace::for_project_root(Path::new("/home/u/a"));
        let b = ProjectBoardNamespace::for_project_root(Path::new("/home/u/b"));
        assert_ne!(a, b);
    }

    #[test]
    fn project_store_key_round_trips() {
        let namespace = ProjectBoardNamespace::for_project_root(Path::new("/p"));
        let stored = BoardStoreKey::project(&namespace, &key("upeg-dev"));

        assert_eq!(
            BoardStoreKey::classify(stored.as_str()),
            StoredBoardKey::Project {
                namespace,
                id: key("upeg-dev"),
            }
        );
    }

    #[test]
    fn global_store_key_classifies_verbatim() {
        assert_eq!(
            BoardStoreKey::classify("dev"),
            StoredBoardKey::Global(key("dev"))
        );
        assert_eq!(
            BoardStoreKey::global(&key("dev")).as_str(),
            "dev",
            "global boards are stored without a prefix"
        );
    }

    #[test]
    fn broken_store_keys_classify_as_unusable() {
        for raw in [
            "",
            "   ",
            "project:",
            "project::dev",
            "project:ns:",
            "project:ns: dev",
        ] {
            assert_eq!(
                BoardStoreKey::classify(raw),
                StoredBoardKey::Unusable,
                "{raw:?} must be unusable"
            );
        }
    }

    #[test]
    fn plain_key_starting_with_project_is_global() {
        assert_eq!(
            BoardStoreKey::classify("projects"),
            StoredBoardKey::Global(key("projects")),
            "the prefix must include the separator to be a project board"
        );
    }
}
