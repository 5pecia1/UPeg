//! Which board rows one load/save pass may see and own.
//!
//! The pegboard store holds board rows for *every* project the user has
//! ever pinned into, keyed `project:<namespace>:<id>`
//! ([`upeg_core::BoardStoreKey`]). Exactly one of those namespaces is
//! live at a time — the Project Manifest this process detected — and the
//! rest must be neither visible nor writable.
//!
//! [`BoardVisibility`] is the single object that answers both halves of
//! that, and load and save consult the *same* instance:
//!
//! | question | answer |
//! |---|---|
//! | a stored row — what does the user call it? | [`BoardVisibility::visible_key`] |
//! | a visible board — where does it persist? | [`BoardVisibility::store_key`] |
//!
//! Because tombstoning is decided in *visible* space, a row this pass
//! cannot see can never be tombstoned by it: leaving a project does not
//! disturb that project's pins, and a global board shadowed by a
//! same-id project board keeps its own rows until the project is left.

use upeg_core::{BoardKey, BoardStoreKey, StoredBoardKey};
use upeg_runtime::pegboard_project::{ProjectBoardScope, project_board_scope};

/// The board rows one pegboard load/save pass owns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoardVisibility {
    project: Option<ProjectBoardScope>,
}

impl BoardVisibility {
    /// Visibility for this process: the Project Manifest's boards when
    /// one was detected, global boards only otherwise.
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            project: project_board_scope(),
        }
    }

    /// Global boards only — no project manifest in play.
    #[must_use]
    pub const fn global_only() -> Self {
        Self { project: None }
    }

    /// Visibility for an explicit project scope. Pure counterpart of
    /// [`Self::from_process`], so tests never race on process state.
    #[must_use]
    pub const fn for_project(scope: ProjectBoardScope) -> Self {
        Self {
            project: Some(scope),
        }
    }

    #[must_use]
    pub const fn project(&self) -> Option<&ProjectBoardScope> {
        self.project.as_ref()
    }

    /// Whether the active project declares this visible board id.
    #[must_use]
    pub fn is_project_board(&self, visible: &str) -> bool {
        self.project
            .as_ref()
            .is_some_and(|scope| scope.declaration(visible).is_some())
    }

    /// What a user calls the board stored under `stored`, or `None` when
    /// this pass cannot see the row (another project's namespace, or a
    /// malformed key).
    #[must_use]
    pub fn visible_key(&self, stored: &str) -> Option<String> {
        match BoardStoreKey::classify(stored) {
            StoredBoardKey::Global(key) => Some(key.as_str().to_string()),
            StoredBoardKey::Project { namespace, id } => {
                let scope = self.project.as_ref()?;
                (scope.namespace() == &namespace && scope.declaration(id.as_str()).is_some())
                    .then(|| id.as_str().to_string())
            }
            StoredBoardKey::Unusable => None,
        }
    }

    /// Where a visible board persists. A board id the active project
    /// declares lands in that project's namespace; everything else is a
    /// global row under its own key.
    ///
    /// A visible key that is not a valid [`BoardKey`] cannot be stored
    /// as a project row (it could not have come from a declaration), so
    /// it falls back to a global key verbatim — the store, not this
    /// mapping, is where such a row is sanitized away.
    #[must_use]
    pub fn store_key(&self, visible: &str) -> String {
        let Some(scope) = self.project.as_ref() else {
            return visible.to_string();
        };
        scope
            .store_key(visible)
            .map_or_else(|| visible.to_string(), |key| key.as_str().to_string())
    }

    /// Store key of a project board declaration, for seeding a board row
    /// that has never been saved.
    #[must_use]
    pub fn project_store_key(&self, id: &BoardKey) -> Option<BoardStoreKey> {
        self.project
            .as_ref()
            .and_then(|scope| scope.store_key(id.as_str()))
    }

    /// The visible key of a row this pass *owns* — one it would itself
    /// write — or `None` for every other row.
    ///
    /// This is the single ownership question, and load and save both ask
    /// it, which is what keeps them symmetric. Two rows can share a
    /// visible key when a project board is declared with the same id as
    /// an existing global board; exactly one of them is owned (the
    /// project row, because [`Self::store_key`] would write there), so
    /// the loser is neither read nor swept and simply reappears when the
    /// project is left.
    #[must_use]
    pub fn owns(&self, store_key: &str) -> Option<String> {
        let visible = self.visible_key(store_key)?;
        (self.store_key(&visible) == store_key).then_some(visible)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use upeg_runtime::pegboard_project::ProjectBoardDecl;

    use super::*;

    fn 프로젝트(path: &str, ids: &[&str]) -> ProjectBoardScope {
        ProjectBoardScope::for_manifest(
            Path::new(path),
            ids.iter()
                .map(|id| {
                    ProjectBoardDecl::new(BoardKey::parse(id).expect("보드 id"), (*id).to_string())
                })
                .collect(),
        )
    }

    #[test]
    fn 전역_보드는_어느_스코프에서도_보인다() {
        let scoped = BoardVisibility::for_project(프로젝트("/a/upeg.toml", &["a-board"]));

        assert_eq!(scoped.visible_key("dev").as_deref(), Some("dev"));
        assert_eq!(
            BoardVisibility::global_only().visible_key("dev").as_deref(),
            Some("dev")
        );
    }

    #[test]
    fn 다른_프로젝트의_보드_행은_보이지_않는다() {
        let a = 프로젝트("/a/upeg.toml", &["shared"]);
        let b = BoardVisibility::for_project(프로젝트("/b/upeg.toml", &["shared"]));
        let stored = a.store_key("shared").expect("a의 store key");

        assert_eq!(b.visible_key(stored.as_str()), None);
        assert_eq!(
            BoardVisibility::global_only().visible_key(stored.as_str()),
            None
        );
    }

    #[test]
    fn 선언한_프로젝트_보드는_바로_그_id로_보인다() {
        let scope = 프로젝트("/a/upeg.toml", &["upeg-dev"]);
        let stored = scope.store_key("upeg-dev").expect("store key");
        let visibility = BoardVisibility::for_project(scope);

        assert_eq!(
            visibility.visible_key(stored.as_str()).as_deref(),
            Some("upeg-dev")
        );
        assert_eq!(visibility.store_key("upeg-dev"), stored.as_str());
    }

    #[test]
    fn 선언이_사라진_네임스페이스_행은_보이지_않는다() {
        let old = 프로젝트("/a/upeg.toml", &["retired"]);
        let stored = old.store_key("retired").expect("store key");
        // 같은 매니페스트가 이제 다른 보드만 선언한다.
        let now = BoardVisibility::for_project(프로젝트("/a/upeg.toml", &["current"]));

        assert_eq!(now.visible_key(stored.as_str()), None);
    }

    #[test]
    fn 프로젝트가_선언한_id의_전역_행은_소유되지_않는다() {
        let scope = 프로젝트("/a/upeg.toml", &["ops"]);
        let project_row = scope.store_key("ops").expect("store key");
        let visibility = BoardVisibility::for_project(scope);

        assert_eq!(visibility.visible_key("ops").as_deref(), Some("ops"));
        assert_eq!(
            visibility.owns("ops"),
            None,
            "같은 id를 프로젝트가 선언했으면 전역 행은 이 pass의 것이 아니다"
        );
        assert_eq!(
            visibility.owns(project_row.as_str()).as_deref(),
            Some("ops")
        );
    }

    #[test]
    fn 프로젝트가_없으면_전역_행을_모두_소유한다() {
        let visibility = BoardVisibility::global_only();
        assert_eq!(visibility.owns("dev").as_deref(), Some("dev"));
    }

    #[test]
    fn 선언하지_않은_보드는_전역_키로_저장된다() {
        let visibility = BoardVisibility::for_project(프로젝트("/a/upeg.toml", &["upeg-dev"]));

        assert_eq!(visibility.store_key("dev"), "dev");
        assert!(!visibility.is_project_board("dev"));
        assert!(visibility.is_project_board("upeg-dev"));
    }
}
