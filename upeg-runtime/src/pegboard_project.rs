//! The active project's board declarations, for this process.
//!
//! `.upeg/project.toml` may declare `[[boards]]`. Those boards are visible
//! only while their project is active. One process-global slot holds their
//! scope alongside runtime Tool registration.
//!
//! This module deliberately holds no policy beyond "what did the
//! project declare, and under which namespace". Merging the
//! declarations into the user's pegboard state, hiding other projects'
//! boards, and mapping visible ids onto store keys all belong to
//! `upeg_sources::pegboard`, which is the layer that owns the store.

#![allow(
    clippy::expect_used,
    reason = "lock poisoning means another thread already panicked while holding the project board scope; there is no honest recovery"
)]

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use upeg_core::{BoardGuidance, BoardKey, BoardStoreKey, ProjectBoardNamespace};

static ACTIVE_PROJECT_BOARDS: OnceLock<Mutex<Option<ProjectBoardScope>>> = OnceLock::new();

fn active_lock() -> &'static Mutex<Option<ProjectBoardScope>> {
    ACTIVE_PROJECT_BOARDS.get_or_init(|| Mutex::new(None))
}

/// One `[[boards]]` entry from a project config, already validated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectBoardDecl {
    /// Visible board id — what a user types (`upeg board upeg-dev list`)
    /// and what a tool's `boards = [...]` array names.
    pub id: BoardKey,
    /// Tab title shown by GUI surfaces.
    pub label: String,
    /// Guidance owned by the declaring project config.
    pub guidance: BoardGuidance,
}

impl ProjectBoardDecl {
    #[must_use]
    pub fn new(id: BoardKey, label: String) -> Self {
        Self {
            id,
            label,
            guidance: BoardGuidance::default(),
        }
    }

    #[must_use]
    pub fn with_guidance(mut self, guidance: BoardGuidance) -> Self {
        self.guidance = guidance;
        self
    }
}

/// The boards the active project declares, plus the
/// namespace their store rows are keyed under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectBoardScope {
    manifest_path: PathBuf,
    loaded_content: Option<String>,
    namespace: ProjectBoardNamespace,
    boards: Vec<ProjectBoardDecl>,
}

impl ProjectBoardScope {
    #[must_use]
    pub fn for_project(root: &Path, config_path: &Path, boards: Vec<ProjectBoardDecl>) -> Self {
        Self {
            manifest_path: config_path.to_path_buf(),
            loaded_content: None,
            namespace: ProjectBoardNamespace::for_project_root(root),
            boards,
        }
    }
    /// Build an in-memory scope with `manifest_path` as its identity.
    /// Production activation uses [`Self::for_project`] with the root.
    #[must_use]
    pub fn for_manifest(manifest_path: &Path, boards: Vec<ProjectBoardDecl>) -> Self {
        Self {
            manifest_path: manifest_path.to_path_buf(),
            loaded_content: None,
            namespace: ProjectBoardNamespace::for_project_root(manifest_path),
            boards,
        }
    }

    /// The source file to edit when changing project board guidance.
    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    /// Attach exactly the source bytes the loader parsed for this scope.
    /// In-memory scopes can omit this and do not inspect the filesystem.
    #[must_use]
    pub fn with_loaded_content(mut self, content: String) -> Self {
        self.loaded_content = Some(content);
        self
    }

    /// Whether the loaded source changed or became unreadable. A preview
    /// must reload the runtime before treating such a scope as current.
    #[must_use]
    pub fn source_changed(&self) -> bool {
        let Some(loaded) = self.loaded_content.as_ref() else {
            return false;
        };
        match std::fs::read_to_string(&self.manifest_path) {
            Ok(current) => current != *loaded,
            Err(_) => true,
        }
    }

    #[must_use]
    pub const fn namespace(&self) -> &ProjectBoardNamespace {
        &self.namespace
    }

    #[must_use]
    pub fn boards(&self) -> &[ProjectBoardDecl] {
        &self.boards
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.boards.is_empty()
    }

    /// The declaration for a visible board id, if this project declares it.
    #[must_use]
    pub fn declaration(&self, id: &str) -> Option<&ProjectBoardDecl> {
        self.boards.iter().find(|board| board.id.as_str() == id)
    }

    /// Store key a declared board id persists under. `None` when this
    /// project does not declare `id` — a caller must then treat the
    /// board as global.
    #[must_use]
    pub fn store_key(&self, id: &str) -> Option<BoardStoreKey> {
        self.declaration(id)
            .map(|board| BoardStoreKey::project(&self.namespace, &board.id))
    }
}

/// Replace this process's project board scope. Called once, by the
/// project loader, when a project is activated.
pub fn set_project_board_scope(scope: ProjectBoardScope) {
    *active_lock().lock().expect("project board scope poisoned") = Some(scope);
}

/// Forget the project board scope — no project boards are visible until
/// another manifest registers. Tests use this to return to a clean slate.
pub fn clear_project_board_scope() {
    *active_lock().lock().expect("project board scope poisoned") = None;
}

/// The active scope, or `None` when no Project Manifest declared boards.
#[must_use]
pub fn project_board_scope() -> Option<ProjectBoardScope> {
    let _catalog = crate::project_scope::catalog_read_guard();
    active_lock()
        .lock()
        .expect("project board scope poisoned")
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(id: &str, label: &str) -> ProjectBoardDecl {
        ProjectBoardDecl::new(BoardKey::parse(id).expect("board id"), label.to_string())
    }

    #[test]
    fn scope_wraps_declared_board_store_key_in_namespace() {
        let scope = ProjectBoardScope::for_manifest(
            Path::new("/p/upeg.toml"),
            vec![decl("upeg-dev", "upeg dev")],
        );

        let key = scope.store_key("upeg-dev").expect("declared board");
        assert!(
            key.as_str().starts_with("project:"),
            "a project board carries the namespace prefix: {key}"
        );
        assert!(key.as_str().ends_with(":upeg-dev"));
    }

    #[test]
    fn scope_gives_no_store_key_for_undeclared_board() {
        let scope = ProjectBoardScope::for_manifest(
            Path::new("/p/upeg.toml"),
            vec![decl("upeg-dev", "upeg dev")],
        );

        assert_eq!(scope.store_key("dev"), None);
    }

    #[test]
    fn different_projects_use_different_store_keys_for_same_board_id() {
        let a = ProjectBoardScope::for_manifest(
            Path::new("/a/upeg.toml"),
            vec![decl("shared", "Shared")],
        );
        let b = ProjectBoardScope::for_manifest(
            Path::new("/b/upeg.toml"),
            vec![decl("shared", "Shared")],
        );

        assert_ne!(a.store_key("shared"), b.store_key("shared"));
    }

    #[test]
    fn detects_modification_and_deletion_of_loaded_project_source() {
        let root = std::env::temp_dir().join(format!("upeg-board-source-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create directory");
        let path = root.join("upeg.toml");
        const LOADED: &str = "id = \"project\"\n";
        std::fs::write(&path, LOADED).expect("write source");
        let scope = ProjectBoardScope::for_manifest(&path, Vec::new())
            .with_loaded_content(LOADED.to_string());
        assert!(!scope.source_changed());

        std::fs::write(&path, "id = \"changed\"\n").expect("modify source");
        assert!(scope.source_changed());
        std::fs::remove_file(&path).expect("delete source");
        assert!(scope.source_changed());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scope_without_loaded_source_skips_file_check() {
        let scope = ProjectBoardScope::for_manifest(Path::new("/missing/upeg.toml"), Vec::new());
        assert!(!scope.source_changed());
    }
}
