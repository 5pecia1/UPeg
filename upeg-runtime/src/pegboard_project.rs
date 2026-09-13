//! The Project Manifest's board declarations, for this process.
//!
//! A Project Manifest may declare `[[boards]]`; those boards exist only
//! while that manifest is detected. "Detected" is a *process* fact — the
//! loader resolves exactly one `upeg.toml` at startup — so the declared
//! boards live in one process-global slot here, next to the other
//! registries the loader fills (`board_context`, trigger bindings,
//! provenance).
//!
//! This module deliberately holds no policy beyond "what did the
//! manifest declare, and under which namespace". Merging the
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

/// One `[[boards]]` entry from a Project Manifest, already validated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectBoardDecl {
    /// Visible board id — what a user types (`upeg board upeg-dev list`)
    /// and what a tool's `boards = [...]` array names.
    pub id: BoardKey,
    /// Tab title shown by GUI surfaces.
    pub label: String,
    /// Guidance owned by the declaring Project Manifest.
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

/// The boards the currently-detected Project Manifest declares, plus the
/// namespace their store rows are keyed under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectBoardScope {
    manifest_path: PathBuf,
    loaded_content: Option<String>,
    namespace: ProjectBoardNamespace,
    boards: Vec<ProjectBoardDecl>,
}

impl ProjectBoardScope {
    /// Build a scope for the manifest at `manifest_path`. The path is
    /// the identity: it is what [`ProjectBoardNamespace`] digests, so
    /// two projects never share placements.
    #[must_use]
    pub fn for_manifest(manifest_path: &Path, boards: Vec<ProjectBoardDecl>) -> Self {
        Self {
            manifest_path: manifest_path.to_path_buf(),
            loaded_content: None,
            namespace: ProjectBoardNamespace::for_manifest_path(manifest_path),
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
/// loader, when a Project Manifest is registered.
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
    active_lock()
        .lock()
        .expect("project board scope poisoned")
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(id: &str, label: &str) -> ProjectBoardDecl {
        ProjectBoardDecl::new(BoardKey::parse(id).expect("보드 id"), label.to_string())
    }

    #[test]
    fn scope는_선언한_보드의_store_key를_네임스페이스로_감싼다() {
        let scope = ProjectBoardScope::for_manifest(
            Path::new("/p/upeg.toml"),
            vec![decl("upeg-dev", "upeg dev")],
        );

        let key = scope.store_key("upeg-dev").expect("선언된 보드");
        assert!(
            key.as_str().starts_with("project:"),
            "프로젝트 보드는 네임스페이스 접두사를 갖는다: {key}"
        );
        assert!(key.as_str().ends_with(":upeg-dev"));
    }

    #[test]
    fn scope는_선언하지_않은_보드에_store_key를_주지_않는다() {
        let scope = ProjectBoardScope::for_manifest(
            Path::new("/p/upeg.toml"),
            vec![decl("upeg-dev", "upeg dev")],
        );

        assert_eq!(scope.store_key("dev"), None);
    }

    #[test]
    fn 서로_다른_프로젝트는_같은_보드_id에_다른_store_key를_쓴다() {
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
    fn 로드한_프로젝트_원본의_변경과_삭제를_감지한다() {
        let root = std::env::temp_dir().join(format!("upeg-board-source-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("디렉터리 생성");
        let path = root.join("upeg.toml");
        const LOADED: &str = "id = \"project\"\n";
        std::fs::write(&path, LOADED).expect("원본 쓰기");
        let scope = ProjectBoardScope::for_manifest(&path, Vec::new())
            .with_loaded_content(LOADED.to_string());
        assert!(!scope.source_changed());

        std::fs::write(&path, "id = \"changed\"\n").expect("원본 변경");
        assert!(scope.source_changed());
        std::fs::remove_file(&path).expect("원본 삭제");
        assert!(scope.source_changed());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn 원본을_로드하지_않은_스코프는_파일_검사를_생략한다() {
        let scope = ProjectBoardScope::for_manifest(Path::new("/missing/upeg.toml"), Vec::new());
        assert!(!scope.source_changed());
    }
}
