//! Directory project discovery and activation.
//!
//! `.upeg/` is the marker. `.upeg/project.toml` is optional and holds
//! schema version 1, a display name, boards, and explicit Tool conflict
//! choices. Each `.upeg/toolkits/*.toml` is one Toolkit. A root `upeg.toml`
//! has no project meaning and is never discovered or loaded.
//!
//! Auto-discovery chooses the nearest marker. It walks ancestors only while
//! the starting directory remains inside `$HOME`; outside `$HOME` it checks
//! the starting directory itself. The global config root (`~/.upeg` or
//! `UPEG_HOME`) is excluded from project discovery. An explicit absolute
//! project root selects a project across that boundary.
//!
//! Global Tools remain available. A duplicate full Tool id requires a
//! `global` or `project` choice in `[tool_choices]`; without a choice, both
//! definitions of that id are hidden and dispatch returns a conflict error.
//! Built-in Tools cannot be overridden. Activation and closing restore
//! runtime metadata, dispatchers, sidecars, and the project board scope.
//! Project boards persist in the global database under a namespace based
//! on the canonical project root, so two project roots do not share pins.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use upeg_core::{BoardExecutionContext, ProjectRoot, ProjectToolChoice};
use upeg_runtime::board_context;

pub const PROJECT_MANIFEST_FILE: &str = upeg_core::PROJECT_MARKER_DIR;

/// Input env var that steers project-manifest detection. Parsed into
/// [`ProjectManifestOverride`] by [`ProjectManifestOverride::from_env`].
///
/// **Not** the same variable as `upeg_loader::PROJECT_MANIFEST_ENV`
/// (`UPEG_PROJECT_MANIFEST`, no `_PATH`) — that one is an *output* var
/// upeg injects into `External` tool child processes so a tool can see
/// which manifest it came from. This one is an *input* read only by
/// this module; the two must never be conflated.
pub const PROJECT_MANIFEST_PATH_ENV: &str = "UPEG_PROJECT_MANIFEST_PATH";

/// The `UPEG_PROJECT_MANIFEST_PATH` value (case-insensitive) that
/// disables project-manifest detection entirely.
pub const PROJECT_MANIFEST_OVERRIDE_OFF: &str = "off";

/// How project-manifest detection is configured for this process, per
/// [`PROJECT_MANIFEST_PATH_ENV`]. Typed so `upeg doctor` / `upeg host
/// status` (see `upeg-cli/src/surfaces/cli/doctor.rs`,
/// `upeg-cli/src/infrastructure/lifecycle.rs`) can report exactly the
/// same state detection itself acts on — there is only one source of
/// truth for "is detection on, off, or overridden".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectManifestOverride {
    /// No override (unset/empty env var): walk cwd → $HOME-bounded
    /// parents → $HOME, as [`detect_project_manifest_from`] does.
    Detect,
    /// `UPEG_PROJECT_MANIFEST_PATH=off`: never load a project
    /// manifest, regardless of what sits on disk.
    Disabled,
    /// `UPEG_PROJECT_MANIFEST_PATH=<absolute path>`: use exactly this
    /// path, skipping detection entirely. A missing file resolves to
    /// "no manifest" — it does NOT fall back to [`Self::Detect`].
    Explicit(PathBuf),
}

impl ProjectManifestOverride {
    /// Read and parse [`PROJECT_MANIFEST_PATH_ENV`] from the process
    /// environment. Impure (reads env, may print a one-line stderr
    /// warning for a malformed value) — see [`parse_project_manifest_override`]
    /// for the pure core.
    pub fn from_env() -> Self {
        let raw = std::env::var(PROJECT_MANIFEST_PATH_ENV).ok();
        if let Some(warning) = project_manifest_override_relative_path_warning(raw.as_deref()) {
            eprintln!("{warning}");
        }
        parse_project_manifest_override(raw.as_deref())
    }
}

fn normalize_override_raw(raw: Option<&str>) -> Option<&str> {
    raw.map(str::trim).filter(|s| !s.is_empty())
}

/// Pure parse of `UPEG_PROJECT_MANIFEST_PATH`'s raw value:
///   - unset or empty            → [`ProjectManifestOverride::Detect`]
///   - `"off"` (case-insensitive) → [`ProjectManifestOverride::Disabled`]
///   - an absolute path           → [`ProjectManifestOverride::Explicit`]
///   - a relative path            → treated as unset (falls back to
///     [`ProjectManifestOverride::Detect`]); the caller is responsible
///     for warning about this — see
///     [`project_manifest_override_relative_path_warning`]. Kept out of
///     this function so it stays a plain, side-effect-free
///     `Option<&str> -> ProjectManifestOverride` mapping that's trivial
///     to unit test without stderr capture or env races.
pub fn parse_project_manifest_override(raw: Option<&str>) -> ProjectManifestOverride {
    let Some(raw) = normalize_override_raw(raw) else {
        return ProjectManifestOverride::Detect;
    };
    if raw.eq_ignore_ascii_case(PROJECT_MANIFEST_OVERRIDE_OFF) {
        return ProjectManifestOverride::Disabled;
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        ProjectManifestOverride::Explicit(path)
    } else {
        ProjectManifestOverride::Detect
    }
}

/// `Some(warning)` exactly when `raw` is a non-empty, non-`off`,
/// *relative* path — the one case [`parse_project_manifest_override`]
/// silently falls back to `Detect` for. Deliberate choice: a malformed
/// override degrades to safe default behavior (normal detection)
/// rather than hard-erroring, but the user is told why on stderr.
pub fn project_manifest_override_relative_path_warning(raw: Option<&str>) -> Option<String> {
    let raw = normalize_override_raw(raw)?;
    if raw.eq_ignore_ascii_case(PROJECT_MANIFEST_OVERRIDE_OFF) {
        return None;
    }
    if PathBuf::from(raw).is_absolute() {
        return None;
    }
    Some(format!(
        "upeg: {PROJECT_MANIFEST_PATH_ENV} must be an absolute path or {PROJECT_MANIFEST_OVERRIDE_OFF:?} (got relative path {raw:?}) — falling back to detection"
    ))
}

/// Where a resolved [`ProjectManifestLookup`] came from. Deliberately
/// collapses "found at cwd" and "found via ancestor walk" into one
/// `Detected` bucket — callers that care about the consent notice only
/// need to distinguish "detection found this" from "the user pointed
/// us at this explicitly via `UPEG_PROJECT_MANIFEST_PATH`".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectManifestOrigin {
    /// Found by [`detect_project_manifest_from`].
    Detected,
    /// Named explicitly via `UPEG_PROJECT_MANIFEST_PATH`.
    EnvOverride,
}

/// A resolved project-manifest path plus how it was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectManifestLookup {
    pub path: PathBuf,
    pub origin: ProjectManifestOrigin,
}

/// Search scope for the upward walk in [`detect_project_manifest_from`].
/// Typed instead of a pile of booleans so the "bounded by $HOME" rule
/// has exactly one place it can be gotten wrong.
enum SearchScope {
    /// `start` sits inside `home` (or equals it): ancestors are in
    /// scope while they remain inside `home`, `home` itself included.
    WithinHome { home: PathBuf },
    /// `start` sits outside `home`, or there is no `home` at all: the
    /// upward walk never leaves `start`.
    OutsideHome,
}

/// A `$HOME` value is only usable as a search boundary when it is an
/// absolute path that has a parent. Two env-boundary degenerate cases
/// would otherwise collapse the boundary entirely, because
/// [`Path::starts_with`] is true for *every* path against them:
///   - `HOME=""` — `var_os` returns `Some("")`, not `None`.
///   - `HOME=/` (or any filesystem root) — every absolute path is inside it.
///
/// A relative `HOME` is equally unusable (nothing meaningful to bound
/// against). All three degrade to "no home at all", which
/// [`SearchScope::classify`] already handles safely: no ancestor walk,
/// no ancestor walk.
fn usable_home(home: Option<&Path>) -> Option<&Path> {
    home.filter(|home| home.is_absolute() && home.parent().is_some())
}

impl SearchScope {
    /// `home` is normalized through [`usable_home`] here — the single
    /// choke point every `home` value flows through, so a degenerate
    /// `$HOME` cannot unlock the walk.
    fn classify(start: &Path, home: Option<&Path>) -> Self {
        match usable_home(home) {
            Some(home) if start.starts_with(home) => Self::WithinHome {
                home: home.to_path_buf(),
            },
            _ => Self::OutsideHome,
        }
    }

    /// Is `dir` still eligible for the upward walk? Only ever true for
    /// `WithinHome`, and only while `dir` remains inside `home`
    /// (`home` itself included).
    fn in_walk(&self, dir: &Path) -> bool {
        match self {
            Self::WithinHome { home } => dir.starts_with(home),
            Self::OutsideHome => false,
        }
    }
}

fn manifest_candidate(dir: &Path) -> Option<PathBuf> {
    let candidate = if dir
        .file_name()
        .is_some_and(|name| name == PROJECT_MANIFEST_FILE)
    {
        dir.to_path_buf()
    } else {
        dir.join(PROJECT_MANIFEST_FILE)
    };
    let is_global = upeg_core::paths::global_storage_roots()
        .iter()
        .any(|global| {
            candidate == *global
                || candidate.canonicalize().ok().is_some_and(|path| {
                    global
                        .canonicalize()
                        .ok()
                        .is_some_and(|global| path == global)
                })
        });
    let marked_global = candidate.join(upeg_core::paths::STORAGE_MARKER).exists()
        || candidate.join(upeg_core::paths::MIGRATION_PENDING).exists();
    (candidate.is_dir() && !is_global && !marked_global).then_some(candidate)
}

/// Walk `start → $HOME-bounded parents → $HOME` and return the first
/// `.upeg`. Pure and env-free (`home` is an explicit parameter) so
/// it stays trivially testable — the process-level env reads live in
/// [`detect_project_manifest_lookup`].
///
/// Rules (see the module-level security note):
///   - `start` itself is always checked, even when it sits outside `home`.
///   - Ancestors above `start` are only walked while still inside
///     `home` (`home` itself included) — never above it.
///   - When `start` is outside `home` (or there is no `home`), no
///     ancestor walk happens at all: only `start`, then `home` once as
///     a fallback.
pub fn detect_project_manifest_from(start: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    // `start` is always in scope, regardless of $HOME.
    if let Some(found) = manifest_candidate(&dir) {
        return Some(found);
    }

    let scope = SearchScope::classify(&dir, home);
    if let SearchScope::WithinHome { home } = &scope {
        let mut dir = dir;
        while dir.pop() && scope.in_walk(&dir) {
            if let Some(found) = manifest_candidate(&dir) {
                return Some(found);
            }
            if &dir == home {
                break;
            }
        }
        return None;
    }

    None
}

/// Resolve a project manifest given an already-parsed override — pure
/// and env-free, unlike [`detect_project_manifest_lookup`]. This is
/// what makes "explicit wins over detection" and "off disables
/// detection" testable without touching the process environment.
pub fn resolve_project_manifest(
    over: &ProjectManifestOverride,
    start: &Path,
    home: Option<&Path>,
) -> Option<ProjectManifestLookup> {
    match over {
        ProjectManifestOverride::Disabled => None,
        ProjectManifestOverride::Explicit(path) => {
            let marker = if path
                .file_name()
                .is_some_and(|name| name == PROJECT_MANIFEST_FILE)
            {
                path.clone()
            } else {
                path.join(PROJECT_MANIFEST_FILE)
            };
            marker.is_dir().then_some(ProjectManifestLookup {
                path: marker,
                origin: ProjectManifestOrigin::EnvOverride,
            })
        }
        ProjectManifestOverride::Detect => {
            detect_project_manifest_from(start, home).map(|path| ProjectManifestLookup {
                path,
                origin: ProjectManifestOrigin::Detected,
            })
        }
    }
}

/// Detect from the process current directory, `$HOME`, and
/// [`PROJECT_MANIFEST_PATH_ENV`] — the process-level entry point.
/// Returns both the resolved path and how it was resolved.
pub fn detect_project_manifest_lookup() -> Option<ProjectManifestLookup> {
    if explicit_global() {
        return None;
    }
    let cwd = std::env::current_dir().ok()?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    resolve_project_manifest(&ProjectManifestOverride::from_env(), &cwd, home.as_deref())
}

/// Detect from the process current directory and `$HOME`, honoring
/// [`PROJECT_MANIFEST_PATH_ENV`]. Path-only convenience wrapper around
/// [`detect_project_manifest_lookup`] for callers that don't care how
/// the path was resolved.
pub fn detect_project_manifest() -> Option<PathBuf> {
    detect_project_manifest_lookup().map(|lookup| lookup.path)
}

/// Best-effort load of the current project manifest into the runtime
/// registry. Returns the path and detailed loader outcome when a manifest was
/// present; `None` means no project manifest was found.
pub fn load_detected_project_manifest() -> Option<(PathBuf, upeg_loader::LoadOutcome)> {
    let path = detect_project_manifest()?;
    let root = path.parent()?;
    let activation = activate_project(root).ok()?;
    let outcome = upeg_loader::LoadOutcome {
        loaded: activation.loaded_tool_ids,
        ..Default::default()
    };
    Some((path, outcome))
}

#[derive(Clone, Debug)]
pub struct ProjectToolkitDefinition {
    pub path: PathBuf,
    pub id: String,
    pub tool_ids: Vec<String>,
    pub skipped: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectConflict {
    pub tool_id: String,
    pub global_source: String,
    pub project_source: PathBuf,
    pub choice: Option<ProjectToolChoice>,
}

#[derive(Clone, Debug)]
pub struct ProjectDefinition {
    pub root: PathBuf,
    pub name: String,
    pub boards: Vec<upeg_runtime::pegboard_project::ProjectBoardDecl>,
    pub toolkits: Vec<ProjectToolkitDefinition>,
    pub conflicts: Vec<ProjectConflict>,
    pub tool_choices: std::collections::BTreeMap<String, ProjectToolChoice>,
}

#[derive(Debug)]
pub struct ProjectActivation {
    pub root: PathBuf,
    pub name: String,
    pub loaded_tool_ids: Vec<&'static str>,
    pub failed: Vec<(PathBuf, String)>,
    pub conflicts: Vec<ProjectConflict>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("not a UPeg project: `{}` has no .upeg directory", .0.display())]
    MissingMarker(PathBuf),
    #[error("project I/O failed at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid project declaration at {}: {reason}", path.display())]
    Config { path: PathBuf, reason: String },
    #[error("invalid project Toolkit at {}: {reason}", path.display())]
    Toolkit { path: PathBuf, reason: String },
    #[error("project Toolkit tool `{0}` is declared more than once")]
    DuplicateTool(String),
    #[error("project Toolkit `{0}` is declared more than once")]
    DuplicateToolkit(String),
    #[error("project Tool `{0}` cannot override a built-in Tool")]
    BuiltinTool(String),
    #[error("tool choice `{0}` does not name a project Tool")]
    UnknownChoice(String),
    #[error("tool choice `{0}` requires a duplicate global Tool id")]
    NotAConflict(String),
    #[error("project registration failed: {0}")]
    Activation(String),
    #[error("{0}")]
    Transition(&'static str),
}

#[derive(Clone)]
struct ActiveProject {
    definition: ProjectDefinition,
    tool_snapshots: Vec<upeg_runtime::project_scope::ProjectToolSnapshot>,
    global_sources: HashMap<String, String>,
}

struct RollbackCatalog {
    previous: Option<ActiveProject>,
    current_tools: Vec<upeg_runtime::project_scope::ProjectToolSnapshot>,
    project_toolkits: HashMap<String, &'static upeg_core::ToolkitMeta>,
    scope: Option<upeg_runtime::pegboard_project::ProjectBoardScope>,
    blocked: HashSet<String>,
    root: Option<PathBuf>,
    explicit_global: bool,
}

impl RollbackCatalog {
    fn capture(previous: Option<ActiveProject>) -> Self {
        let current_tools = previous.as_ref().map_or_else(Vec::new, |active| {
            active
                .tool_snapshots
                .iter()
                .map(|snapshot| {
                    upeg_runtime::project_scope::ProjectToolSnapshot::take(snapshot.id())
                })
                .collect()
        });
        let project_toolkits = upeg_runtime::project_scope::take_project_toolkits();
        let scope = upeg_runtime::pegboard_project::project_board_scope();
        let blocked = upeg_runtime::project_scope::blocked_project_tools();
        let root = upeg_runtime::project_scope::active_project_root();
        let explicit_global = explicit_global();
        if let Some(active) = previous.clone() {
            restore_project(active);
        }
        Self {
            previous,
            current_tools,
            project_toolkits,
            scope,
            blocked,
            root,
            explicit_global,
        }
    }

    fn restore(self, slot: &mut Option<ActiveProject>) {
        for snapshot in self.current_tools.into_iter().rev() {
            snapshot.restore();
        }
        upeg_runtime::project_scope::restore_project_toolkits(self.project_toolkits);
        if let Some(scope) = self.scope {
            upeg_runtime::pegboard_project::set_project_board_scope(scope);
        }
        upeg_runtime::project_scope::set_blocked_project_tools(self.blocked);
        upeg_runtime::project_scope::set_active_project_root(self.root);
        EXPLICIT_GLOBAL.store(self.explicit_global, Ordering::Release);
        *slot = self.previous;
    }
}

static ACTIVE_PROJECT: OnceLock<Mutex<Option<ActiveProject>>> = OnceLock::new();
static EXPLICIT_GLOBAL: AtomicBool = AtomicBool::new(false);

fn explicit_global() -> bool {
    EXPLICIT_GLOBAL.load(Ordering::Acquire)
}

fn active_project() -> &'static Mutex<Option<ActiveProject>> {
    ACTIVE_PROJECT.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(crate) fn project_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: Mutex<()> = Mutex::new(());
    let guard = GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    EXPLICIT_GLOBAL.store(false, Ordering::Release);
    guard
}

pub fn current_project_root() -> Option<PathBuf> {
    active_project()
        .lock()
        .ok()?
        .as_ref()
        .map(|active| active.definition.root.clone())
}

pub fn current_project_definition() -> Option<ProjectDefinition> {
    active_project()
        .lock()
        .ok()?
        .as_ref()
        .map(|active| active.definition.clone())
}

pub fn validate_project_root(root: &Path) -> Result<ProjectDefinition, ProjectError> {
    let project =
        ProjectRoot::new(root).ok_or_else(|| ProjectError::MissingMarker(root.to_path_buf()))?;
    let config_path = project.config_path();
    let config = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).map_err(|source| ProjectError::Io {
            path: config_path.clone(),
            source,
        })?;
        upeg_loader::parse_project_config(&content).map_err(|error| ProjectError::Config {
            path: config_path.clone(),
            reason: error.to_string(),
        })?
    } else {
        upeg_loader::ProjectConfig::default()
    };
    let mut files = Vec::new();
    let toolkits_dir = project.toolkits_dir();
    if toolkits_dir.exists() {
        let entries = std::fs::read_dir(&toolkits_dir).map_err(|source| ProjectError::Io {
            path: toolkits_dir.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| ProjectError::Io {
                path: toolkits_dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "toml")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut toolkits = Vec::new();
    let mut toolkit_ids = HashSet::new();
    let mut tool_ids = HashSet::new();
    for path in files {
        let inspected =
            upeg_loader::inspect_project_toolkit(&path).map_err(|error| ProjectError::Toolkit {
                path: path.clone(),
                reason: error.to_string(),
            })?;
        if !toolkit_ids.insert(inspected.toolkit_id.clone()) {
            return Err(ProjectError::DuplicateToolkit(inspected.toolkit_id));
        }
        for id in &inspected.tool_ids {
            if !tool_ids.insert(id.clone()) {
                return Err(ProjectError::DuplicateTool(id.clone()));
            }
            if upeg_runtime::toolbox_is_builtin(id) {
                return Err(ProjectError::BuiltinTool(id.clone()));
            }
        }
        toolkits.push(ProjectToolkitDefinition {
            path,
            id: inspected.toolkit_id,
            tool_ids: inspected.tool_ids,
            skipped: inspected
                .skipped
                .into_iter()
                .map(|entry| format!("{}: {}", entry.id, entry.reason))
                .collect(),
        });
    }
    for id in config.tool_choices.keys() {
        if !tool_ids.contains(id) {
            return Err(ProjectError::UnknownChoice(id.clone()));
        }
    }
    let active_global_sources = active_project()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|active| active.global_sources.clone()))
        .unwrap_or_default();
    let mut conflicts = Vec::new();
    for toolkit in &toolkits {
        for id in &toolkit.tool_ids {
            let provenance = upeg_runtime::tool_provenance(id);
            let is_global = active_global_sources.contains_key(id)
                || (upeg_runtime::toolbox_registered_tool(id).is_some()
                    && !provenance.is_project_manifest());
            if is_global {
                conflicts.push(ProjectConflict {
                    tool_id: id.clone(),
                    global_source: active_global_sources
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| provenance.label()),
                    project_source: toolkit.path.clone(),
                    choice: config.tool_choices.get(id).copied(),
                });
            }
        }
    }
    let name = config.name.unwrap_or_else(|| {
        project
            .as_path()
            .file_name()
            .map(|part| part.to_string_lossy().into_owned())
            .unwrap_or_else(|| project.as_path().display().to_string())
    });
    Ok(ProjectDefinition {
        root: project.as_path().to_path_buf(),
        name,
        boards: config.boards,
        toolkits,
        conflicts,
        tool_choices: config.tool_choices,
    })
}

pub fn activate_project(root: &Path) -> Result<ProjectActivation, ProjectError> {
    let definition = validate_project_root(root)?;
    let _transition = upeg_runtime::project_scope::begin_project_transition()
        .map_err(ProjectError::Transition)?;
    activate_project_under_transition(definition)
}

fn activate_project_under_transition(
    definition: ProjectDefinition,
) -> Result<ProjectActivation, ProjectError> {
    let mut active = active_project()
        .lock()
        .map_err(|_| ProjectError::Transition("project context poisoned"))?;
    let rollback = RollbackCatalog::capture(active.take());
    let conflict_ids: HashSet<&str> = definition
        .conflicts
        .iter()
        .map(|conflict| conflict.tool_id.as_str())
        .collect();
    let mut blocked = HashSet::new();
    let mut selections: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    for toolkit in &definition.toolkits {
        for id in &toolkit.tool_ids {
            let choice = definition.tool_choices.get(id).copied();
            if conflict_ids.contains(id.as_str()) && choice.is_none() {
                blocked.insert(id.clone());
                continue;
            }
            if choice == Some(ProjectToolChoice::Global) && conflict_ids.contains(id.as_str()) {
                continue;
            }
            selections
                .entry(toolkit.path.clone())
                .or_default()
                .insert(id.clone());
        }
    }
    let mut tool_snapshots = Vec::new();
    let mut global_sources = HashMap::new();
    let mut loaded_tool_ids = Vec::new();
    let mut failed = Vec::new();
    for toolkit in &definition.toolkits {
        failed.extend(
            toolkit
                .skipped
                .iter()
                .map(|reason| (toolkit.path.clone(), reason.clone())),
        );
    }
    if !failed.is_empty() {
        let reason = failed
            .iter()
            .map(|(path, reason)| format!("{}: {reason}", path.display()))
            .collect::<Vec<_>>()
            .join("; ");
        rollback.restore(&mut active);
        return Err(ProjectError::Activation(reason));
    }
    for toolkit in &definition.toolkits {
        let Some(ids) = selections.get(&toolkit.path) else {
            continue;
        };
        if ids.is_empty() {
            continue;
        }
        let mut sorted_ids: Vec<_> = ids.iter().collect();
        sorted_ids.sort();
        for id in sorted_ids {
            let static_id: &'static str = Box::leak(id.clone().into_boxed_str());
            let snapshot = upeg_runtime::project_scope::ProjectToolSnapshot::take(static_id);
            if let Some(source) = snapshot.previous_source_label() {
                global_sources.insert(id.clone(), source);
            }
            tool_snapshots.push(snapshot);
        }
        let outcome =
            upeg_loader::load_project_toolkit_file_verbose(&toolkit.path, &definition.root, ids);
        let loaded_count = outcome.loaded.len();
        for id in outcome.loaded {
            upeg_runtime::register_tool_provenance(
                id,
                upeg_runtime::ToolProvenance::ProjectManifest {
                    path: toolkit.path.display().to_string(),
                },
            );
            loaded_tool_ids.push(id);
        }
        failed.extend(
            outcome
                .failed
                .into_iter()
                .map(|(path, error)| (path, error.to_string())),
        );
        failed.extend(outcome.skipped.into_iter().map(|skipped| {
            (
                toolkit.path.clone(),
                format!("{}: {}", skipped.id, skipped.reason),
            )
        }));
        if loaded_count != ids.len() {
            failed.push((toolkit.path.clone(), format!("expected {} selected Tool(s), registered {loaded_count}; source changed during activation", ids.len())));
        }
    }
    if !failed.is_empty() {
        let reason = failed
            .iter()
            .map(|(path, message)| format!("{}: {message}", path.display()))
            .collect::<Vec<_>>()
            .join("; ");
        restore_project(ActiveProject {
            definition,
            tool_snapshots,
            global_sources,
        });
        rollback.restore(&mut active);
        return Err(ProjectError::Activation(reason));
    }
    upeg_runtime::project_scope::set_blocked_project_tools(blocked);
    let config_path = definition.root.join(".upeg/project.toml");
    let mut scope = upeg_runtime::pegboard_project::ProjectBoardScope::for_project(
        &definition.root,
        &config_path,
        definition.boards.clone(),
    );
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        scope = scope.with_loaded_content(content);
    }
    upeg_runtime::pegboard_project::set_project_board_scope(scope);
    upeg_runtime::project_scope::set_active_project_root(Some(definition.root.clone()));
    let result = ProjectActivation {
        root: definition.root.clone(),
        name: definition.name.clone(),
        loaded_tool_ids,
        failed,
        conflicts: definition.conflicts.clone(),
    };
    *active = Some(ActiveProject {
        definition,
        tool_snapshots,
        global_sources,
    });
    EXPLICIT_GLOBAL.store(false, Ordering::Release);
    drop(active);
    crate::source_freshness::record_loaded_sources(&crate::RuntimeSourceConfig::from_env());
    Ok(result)
}

fn restore_project(previous: ActiveProject) {
    upeg_runtime::project_scope::set_blocked_project_tools(HashSet::new());
    upeg_runtime::project_scope::set_active_project_root(None);
    upeg_runtime::pegboard_project::clear_project_board_scope();
    upeg_runtime::project_scope::take_project_toolkits();
    for snapshot in previous.tool_snapshots.into_iter().rev() {
        snapshot.restore();
    }
}

pub fn close_project() -> Result<(), ProjectError> {
    let _transition = upeg_runtime::project_scope::begin_project_transition()
        .map_err(ProjectError::Transition)?;
    let mut active = active_project()
        .lock()
        .map_err(|_| ProjectError::Transition("project context poisoned"))?;
    if let Some(previous) = active.take() {
        restore_project(previous);
    }
    EXPLICIT_GLOBAL.store(true, Ordering::Release);
    drop(active);
    crate::source_freshness::record_loaded_sources(&crate::RuntimeSourceConfig::from_env());
    Ok(())
}

pub fn set_project_tool_choice(
    root: &Path,
    id: &str,
    choice: ProjectToolChoice,
) -> Result<ProjectActivation, ProjectError> {
    let definition = validate_project_root(root)?;
    if !definition
        .toolkits
        .iter()
        .any(|toolkit| toolkit.tool_ids.iter().any(|candidate| candidate == id))
    {
        return Err(ProjectError::UnknownChoice(id.to_string()));
    }
    if !definition
        .conflicts
        .iter()
        .any(|conflict| conflict.tool_id == id)
    {
        return Err(ProjectError::NotAConflict(id.to_string()));
    }
    let _transition = upeg_runtime::project_scope::begin_project_transition()
        .map_err(ProjectError::Transition)?;
    let path = definition.root.join(".upeg/project.toml");
    let original = if path.is_file() {
        Some(std::fs::read(&path).map_err(|source| ProjectError::Io {
            path: path.clone(),
            source,
        })?)
    } else {
        None
    };
    let mut value: toml::Value = if let Some(content) = &original {
        let content = std::str::from_utf8(content).map_err(|error| ProjectError::Config {
            path: path.clone(),
            reason: error.to_string(),
        })?;
        toml::from_str(content).map_err(|error| ProjectError::Config {
            path: path.clone(),
            reason: error.to_string(),
        })?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };
    let table = value.as_table_mut().ok_or_else(|| ProjectError::Config {
        path: path.clone(),
        reason: "expected a TOML table".into(),
    })?;
    table.insert("schema_version".into(), toml::Value::Integer(1));
    let choices = table
        .entry("tool_choices")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let choices = choices.as_table_mut().ok_or_else(|| ProjectError::Config {
        path: path.clone(),
        reason: "tool_choices must be a table".into(),
    })?;
    choices.insert(
        id.to_string(),
        toml::Value::String(
            match choice {
                ProjectToolChoice::Global => "global",
                ProjectToolChoice::Project => "project",
            }
            .into(),
        ),
    );
    let serialized = toml::to_string_pretty(&value).map_err(|error| ProjectError::Config {
        path: path.clone(),
        reason: error.to_string(),
    })?;
    let temporary = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    std::fs::write(&temporary, serialized).map_err(|source| ProjectError::Io {
        path: temporary.clone(),
        source,
    })?;
    std::fs::rename(&temporary, &path).map_err(|source| ProjectError::Io {
        path: path.clone(),
        source,
    })?;
    let result =
        validate_project_root(&definition.root).and_then(activate_project_under_transition);
    if result.is_err() {
        match original {
            Some(bytes) => std::fs::write(&path, bytes),
            None => std::fs::remove_file(&path),
        }
        .map_err(|source| ProjectError::Io { path, source })?;
    }
    result
}

pub fn board_context_with_project(board: &str) -> BoardExecutionContext {
    let mut context = board_context(board);
    if context.project_manifest.is_none() {
        context.project_manifest = current_project_root()
            .map(|root| root.join(PROJECT_MANIFEST_FILE))
            .or_else(detect_project_manifest)
            .map(|marker| {
                let config = marker.join(upeg_core::PROJECT_CONFIG_FILE);
                if config.is_file() { config } else { marker }
            })
            .map(|path| path.display().to_string());
    }
    context
}

/// Project-manifest state for `upeg doctor` / `upeg host status`:
/// what detection actually resolved (if anything), plus the override
/// state that produced it. Both formatters build their one-line report
/// from this so they can never drift from [`detect_project_manifest_lookup`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectManifestStatus {
    pub lookup: Option<ProjectManifestLookup>,
    pub override_state: ProjectManifestOverride,
}

/// Process-level entry point for [`ProjectManifestStatus`]. Reads the
/// environment and cwd, same as [`detect_project_manifest_lookup`], and
/// additionally keeps the override state so a report can say *why*
/// there is (or isn't) a manifest.
pub fn project_manifest_status() -> ProjectManifestStatus {
    if explicit_global() {
        return ProjectManifestStatus {
            lookup: None,
            override_state: ProjectManifestOverride::Disabled,
        };
    }
    let over = ProjectManifestOverride::from_env();
    let cwd = std::env::current_dir().ok();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let lookup = cwd.and_then(|cwd| resolve_project_manifest(&over, &cwd, home.as_deref()));
    ProjectManifestStatus {
        lookup,
        override_state: over,
    }
}

/// Stable short label for [`ProjectManifestOverride`], shared by the
/// `doctor` text/JSON formatters and `host status` text/JSON
/// formatters so the wording can't drift between the two surfaces.
pub const fn project_manifest_override_label(over: &ProjectManifestOverride) -> &'static str {
    match over {
        ProjectManifestOverride::Detect => "detect",
        ProjectManifestOverride::Disabled => "off",
        ProjectManifestOverride::Explicit(_) => "explicit",
    }
}

/// Filesystem-backed implementation of the runtime's
/// [`upeg_runtime::ProjectContext`] port: registry board context plus
/// `.upeg` project discovery. Shared by every native surface (CLI,
/// FRB desktop) so board-context resolution cannot drift per surface.
#[derive(Default, Clone, Copy, Debug)]
pub struct FilesystemProjectContext;

impl upeg_runtime::ProjectContext for FilesystemProjectContext {
    fn board_context_with_project(&self, board: &str) -> BoardExecutionContext {
        board_context_with_project(board)
    }
}

#[cfg(test)]
mod project_context_tests;

#[cfg(test)]
mod discovery_tests;
