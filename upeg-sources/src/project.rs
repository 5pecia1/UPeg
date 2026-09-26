//! Project Manifest support (`upeg.toml`).
//!
//! PRD v2.1 makes project-local tools first-class: every CLI/HTTP run should
//! walk from the current directory upward, find the nearest `upeg.toml`, and
//! merge that Toolkit manifest into the same runtime registry as
//! `~/.upeg/toolkits/*.toml`.
//!
//! This module owns the shared I/O parts (`detect_*`, `load_*`,
//! `board_context_with_project`) so CLI, Desktop, and future surfaces do not
//! duplicate project lookup policy.
//!
//! # Security: the upward walk is bounded by `$HOME`
//!
//! A project manifest can declare `invoker = "External"` tools that run
//! arbitrary commands with no consent prompt (docs/architecture.md#security-absolutes).
//! Detection therefore never walks above `$HOME`: an ancestor `upeg.toml`
//! outside the user's home directory (e.g. a world-writable `/tmp/x/upeg.toml`
//! picked up just because the user `cd`ed into `/tmp/x/anything`) must never
//! be auto-loaded. See `SearchScope` and [`detect_project_manifest_from`].
//! Concretely: cwd itself is always checked; inside `$HOME` the walk climbs
//! only through ancestors inside `$HOME` (`$HOME` included); outside `$HOME`
//! no ancestors are walked and `$HOME` itself is the single fallback check;
//! with no `$HOME` only cwd is checked.
//!
//! # `UPEG_PROJECT_MANIFEST_PATH` — detection override
//!
//! | Value | Behavior |
//! |---|---|
//! | unset / empty | `Detect` — the walk above |
//! | `off` (case-insensitive) | `Disabled` — never loads |
//! | absolute path | `Explicit` — that path only; a missing file means "no manifest", no fallback to detection |
//! | relative path | invalid — a one-line stderr warning, falls back to `Detect` |
//!
//! (`UPEG_PROJECT_MANIFEST`, no `_PATH`, is a different variable — an
//! *output* injected into `External` children; never interchangeable.)
//!
//! # Consent notice
//!
//! "Which file is trusted" and the load tally are about the same file,
//! so they merge into one stderr line per process (upeg-cli's
//! `project_manifest_summary_line`): detected + tools → `upeg: loaded
//! project manifest <path> (N tool(s), M failed)`; detected + none →
//! without the tally; env-override + tools → `upeg: loaded N project
//! tool(s) from <path> (M failed)`; env-override + none → silence (the
//! path was already named knowingly). `--quiet` suppresses it.
//!
//! # Precedence
//!
//! Built-in static Tools are immutable; `~/.upeg/toolkits` runtime
//! Toolkits load first; the project `upeg.toml` loads next and may
//! *replace* earlier runtime metadata/dispatchers under the same id;
//! shadowing a built-in id is rejected.
//!
//! # Project boards — `[[boards]]`
//!
//! A project manifest may declare boards; a Tool's `boards = [...]`
//! refers to them, and a board exists only while the manifest is
//! detected. `[[boards]]` in `~/.upeg/toolkits/*.toml` fails that whole
//! file's registration (`BoardsOutsideProjectManifest`) — a global
//! Toolkit has no project to scope to. Declaration rules: `id` is
//! canonical and non-empty, cannot contain `:` (the store-key namespace
//! separator), cannot shadow a built-in board
//! (`upeg_core::BUILTIN_BOARDS`), and cannot repeat inside one manifest.
//!
//! A project board is enumerated on every surface only while detected —
//! outside the repository it is "unknown board" (HTTP 404). Its rows
//! persist under `project:<manifest-path-digest>:<board-id>` — a stable
//! 64-bit digest of the manifest's absolute path
//! (`upeg_core::ProjectBoardNamespace`) — so same-id boards in different
//! projects or checkouts never share pins, and moving the directory
//! loses them (the path is the only identity readable before parsing).
//! Store sweeps touch only rows the manifest could write; another
//! project's namespace is neither read nor erased. Inside the project a
//! project board shadows a same-id global board (the global's rows are
//! untouched — leaving the project restores them), and a
//! manifest-declared board cannot be deleted.
//!
//! # Injected context, provenance, dispatch location
//!
//! Board-scoped calls inject `_upeg.{board, boardEnv, projectManifest}`
//! (see `upeg_runtime::execution`). Project-manifest Tools carry
//! `source = "project-manifest:<path>"` provenance (`upeg tool list
//! --json`, `/v1/tools`, MCP `tools/list`), which drives the attach
//! decision on CLI/TUI/MCP-proxy: a `project-manifest:*` Tool always
//! runs **in-process** even while a host is up — the host resolved its
//! own `upeg.toml`, or none, and does not know these Tools. The same
//! applies one level up: `upeg board <b> call` on a project-declared
//! board does not auto-attach (the host lacks that board — local is the
//! only path that can succeed). Other Tools attach with the caller's
//! absolute cwd stamped into `_upeg.cwd` so the run happens where the
//! call was made. The `upeg mcp` proxy applies the same split and
//! merges project Tools into the host's `tools/list`, deduped by name.
//!
//! # Diagnostics
//!
//! `upeg doctor` and `upeg host status` report the detected manifest
//! path (`none` when absent) and the override state
//! (`detect`/`off`/`explicit`) in text and JSON, reusing this module's
//! verdict logic so reports cannot diverge from detection.

use std::path::{Path, PathBuf};

use upeg_core::BoardExecutionContext;
use upeg_runtime::board_context;

pub const PROJECT_MANIFEST_FILE: &str = "upeg.toml";

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
/// collapses "found at cwd" and "found via $HOME fallback" into one
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
    /// upward walk never leaves `start`. `home` (if any) is checked
    /// once, separately, as a fallback — never as part of the walk.
    OutsideHome { home_fallback: Option<PathBuf> },
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
/// no fallback.
fn usable_home(home: Option<&Path>) -> Option<&Path> {
    home.filter(|home| home.is_absolute() && home.parent().is_some())
}

impl SearchScope {
    /// `home` is normalized through [`usable_home`] here — the single
    /// choke point every `home` value flows through, so a degenerate
    /// `$HOME` cannot reach the walk or the fallback.
    fn classify(start: &Path, home: Option<&Path>) -> Self {
        match usable_home(home) {
            Some(home) if start.starts_with(home) => Self::WithinHome {
                home: home.to_path_buf(),
            },
            other => Self::OutsideHome {
                home_fallback: other.map(Path::to_path_buf),
            },
        }
    }

    /// Is `dir` still eligible for the upward walk? Only ever true for
    /// `WithinHome`, and only while `dir` remains inside `home`
    /// (`home` itself included).
    fn in_walk(&self, dir: &Path) -> bool {
        match self {
            Self::WithinHome { home } => dir.starts_with(home),
            Self::OutsideHome { .. } => false,
        }
    }

    fn home_fallback(&self) -> Option<&Path> {
        match self {
            Self::WithinHome { home } => Some(home.as_path()),
            Self::OutsideHome { home_fallback } => home_fallback.as_deref(),
        }
    }
}

fn manifest_candidate(dir: &Path) -> Option<PathBuf> {
    let candidate = dir.join(PROJECT_MANIFEST_FILE);
    candidate.is_file().then_some(candidate)
}

/// Walk `start → $HOME-bounded parents → $HOME` and return the first
/// `upeg.toml`. Pure and env-free (`home` is an explicit parameter) so
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

    scope.home_fallback().and_then(manifest_candidate)
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
        ProjectManifestOverride::Explicit(path) => path.is_file().then(|| ProjectManifestLookup {
            path: path.clone(),
            origin: ProjectManifestOrigin::EnvOverride,
        }),
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
    let outcome = upeg_loader::load_and_register_file_verbose(&path);
    Some((path, outcome))
}

pub fn board_context_with_project(board: &str) -> BoardExecutionContext {
    let mut context = board_context(board);
    if context.project_manifest.is_none() {
        context.project_manifest = detect_project_manifest().map(|p| p.display().to_string());
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
/// `upeg.toml` project discovery. Shared by every native surface (CLI,
/// FRB desktop) so board-context resolution cannot drift per surface.
#[derive(Default, Clone, Copy, Debug)]
pub struct FilesystemProjectContext;

impl upeg_runtime::ProjectContext for FilesystemProjectContext {
    fn board_context_with_project(&self, board: &str) -> BoardExecutionContext {
        board_context_with_project(board)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds `root/home` (a stand-in `$HOME`) and returns `(root, home)`
    /// after clearing any leftovers from a previous run.
    fn temp_home_tree(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(name);
        let home = root.join("home");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&home).unwrap();
        (root, home)
    }

    fn write_manifest(dir: &Path, id: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join(PROJECT_MANIFEST_FILE),
            format!(
                r#"id = "{id}"
tools = [{{ id = "echo" }}]"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn project_manifest_detection_walks_up_to_nearest_ancestor() {
        let (root, home) = temp_home_tree("upeg_project_detect_parent");
        let nested = home.join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();
        write_manifest(&home, "project");

        let got = detect_project_manifest_from(&nested, Some(&home)).expect("manifest");
        assert_eq!(got, home.join(PROJECT_MANIFEST_FILE));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_manifest_detection_falls_back_to_home() {
        let root = std::env::temp_dir().join("upeg_project_detect_home_root");
        let cwd = root.join("work/outside");
        let home = root.join("home");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&cwd).unwrap();
        write_manifest(&home, "home");

        let got = detect_project_manifest_from(&cwd, Some(&home)).expect("manifest");
        assert_eq!(got, home.join(PROJECT_MANIFEST_FILE));

        let _ = std::fs::remove_dir_all(&root);
    }

    /// B-4 repro: an ancestor `upeg.toml` sitting OUTSIDE `$HOME` must
    /// never be auto-loaded just because cwd is a descendant of it —
    /// e.g. `cd /tmp/evilroot/sub` must not pick up
    /// `/tmp/evilroot/upeg.toml` when `$HOME` is somewhere else
    /// entirely.
    #[test]
    fn cwd_outside_home_does_not_load_ancestor_manifest() {
        let (root, home) = temp_home_tree("upeg_project_detect_outside_home");
        let evil_root = root.join("evilroot");
        let cwd = evil_root.join("sub");
        std::fs::create_dir_all(&cwd).unwrap();
        write_manifest(&evil_root, "evil"); // ancestor of cwd, outside $HOME
        // $HOME has no manifest of its own.

        let got = detect_project_manifest_from(&cwd, Some(&home));
        assert_eq!(
            got, None,
            "upeg.toml in an ancestor directory outside home must be ignored"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Sibling to the outside-$HOME case: cwd itself is still always
    /// checked even when it sits outside $HOME.
    #[test]
    fn cwd_manifest_is_loaded_even_outside_home() {
        let (root, home) = temp_home_tree("upeg_project_detect_outside_home_cwd_itself");
        let cwd = root.join("elsewhere");
        write_manifest(&cwd, "elsewhere");

        let got = detect_project_manifest_from(&cwd, Some(&home)).expect("manifest");
        assert_eq!(got, cwd.join(PROJECT_MANIFEST_FILE));

        let _ = std::fs::remove_dir_all(&root);
    }

    /// B-4 hole at the env boundary: `HOME=""` (what `var_os` returns
    /// for an exported-but-empty `HOME`) and `HOME=/` both make
    /// `starts_with` true for every path, which would turn the whole
    /// filesystem into "inside $HOME" and re-open the outside-$HOME
    /// ancestor walk. A relative `HOME` is unusable for the same
    /// reason. All three must behave exactly like "no home".
    #[test]
    fn degenerate_home_value_is_treated_as_no_home() {
        let (root, _home) = temp_home_tree("upeg_project_detect_degenerate_home");
        let evil_root = root.join("evilroot");
        let cwd = evil_root.join("sub");
        std::fs::create_dir_all(&cwd).unwrap();
        write_manifest(&evil_root, "evil"); // ancestor of cwd

        for degenerate in ["", "/", "relative/home"] {
            let got = detect_project_manifest_from(&cwd, Some(Path::new(degenerate)));
            assert_eq!(
                got, None,
                "degenerate HOME ({degenerate:?}) must not unlock the ancestor walk"
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn degenerate_home_value_does_not_get_home_fallback_either() {
        let (root, _home) = temp_home_tree("upeg_project_detect_degenerate_home_fallback");
        let cwd = root.join("elsewhere");
        std::fs::create_dir_all(&cwd).unwrap();

        for degenerate in ["", "/", "relative/home"] {
            let scope = SearchScope::classify(&cwd, Some(Path::new(degenerate)));
            assert_eq!(
                scope.home_fallback(),
                None,
                "degenerate HOME ({degenerate:?}) cannot be a fallback candidate"
            );
            assert!(
                !scope.in_walk(&cwd),
                "degenerate HOME must not unlock walking"
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn normal_absolute_home_is_used_verbatim() {
        let (root, home) = temp_home_tree("upeg_project_usable_home");
        assert_eq!(usable_home(Some(&home)), Some(home.as_path()));
        assert_eq!(usable_home(None), None);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn without_home_does_not_walk_ancestors() {
        let root = std::env::temp_dir().join("upeg_project_detect_no_home");
        let nested = root.join("a/b");
        let _ = std::fs::remove_dir_all(&root);
        write_manifest(&root, "root_only");
        std::fs::create_dir_all(&nested).unwrap();

        let got = detect_project_manifest_from(&nested, None);
        assert_eq!(
            got, None,
            "without home it must not walk ancestor directories at all"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn override_parsing_treats_empty_as_detect() {
        assert_eq!(
            parse_project_manifest_override(None),
            ProjectManifestOverride::Detect
        );
        assert_eq!(
            parse_project_manifest_override(Some("")),
            ProjectManifestOverride::Detect
        );
        assert_eq!(
            parse_project_manifest_override(Some("   ")),
            ProjectManifestOverride::Detect
        );
    }

    #[test]
    fn override_parsing_treats_off_case_insensitively_as_disabled() {
        assert_eq!(
            parse_project_manifest_override(Some("off")),
            ProjectManifestOverride::Disabled
        );
        assert_eq!(
            parse_project_manifest_override(Some("OFF")),
            ProjectManifestOverride::Disabled
        );
        assert_eq!(
            parse_project_manifest_override(Some(" Off ")),
            ProjectManifestOverride::Disabled
        );
    }

    #[test]
    fn override_parsing_treats_absolute_path_as_explicit() {
        let absolute = if cfg!(windows) {
            "C:\\proj\\upeg.toml"
        } else {
            "/proj/upeg.toml"
        };
        assert_eq!(
            parse_project_manifest_override(Some(absolute)),
            ProjectManifestOverride::Explicit(PathBuf::from(absolute))
        );
    }

    #[test]
    fn override_parsing_falls_back_to_detect_for_relative_path_and_returns_warning() {
        assert_eq!(
            parse_project_manifest_override(Some("relative/upeg.toml")),
            ProjectManifestOverride::Detect
        );
        let warning = project_manifest_override_relative_path_warning(Some("relative/upeg.toml"));
        assert!(warning.is_some());
        assert!(warning.unwrap().contains(PROJECT_MANIFEST_PATH_ENV));
    }

    #[test]
    fn no_warning_for_non_relative_path() {
        assert_eq!(project_manifest_override_relative_path_warning(None), None);
        assert_eq!(
            project_manifest_override_relative_path_warning(Some("off")),
            None
        );
        let absolute = if cfg!(windows) {
            "C:\\proj\\upeg.toml"
        } else {
            "/proj/upeg.toml"
        };
        assert_eq!(
            project_manifest_override_relative_path_warning(Some(absolute)),
            None
        );
    }

    #[test]
    fn explicit_absolute_path_wins_over_detectable_manifest() {
        let (root, home) = temp_home_tree("upeg_project_resolve_explicit_wins");
        let cwd = home.join("proj");
        write_manifest(&cwd, "detectable");
        let explicit_dir = root.join("explicit");
        write_manifest(&explicit_dir, "explicit");
        let explicit_path = explicit_dir.join(PROJECT_MANIFEST_FILE);

        let over = ProjectManifestOverride::Explicit(explicit_path.clone());
        let got = resolve_project_manifest(&over, &cwd, Some(&home)).expect("manifest");

        assert_eq!(
            got.path, explicit_path,
            "must be the explicit path, not the detected one"
        );
        assert_eq!(got.origin, ProjectManifestOrigin::EnvOverride);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_explicit_path_is_not_replaced_by_detection() {
        let (root, home) = temp_home_tree("upeg_project_resolve_explicit_missing");
        let cwd = home.join("proj");
        write_manifest(&cwd, "detectable");
        let missing = root.join("missing").join(PROJECT_MANIFEST_FILE);

        let over = ProjectManifestOverride::Explicit(missing);
        let got = resolve_project_manifest(&over, &cwd, Some(&home));

        assert_eq!(
            got, None,
            "missing explicit file must yield no manifest instead of falling back to detection"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn off_disables_even_when_manifest_is_detectable() {
        let (root, home) = temp_home_tree("upeg_project_resolve_disabled");
        let cwd = home.join("proj");
        write_manifest(&cwd, "detectable");

        let got = resolve_project_manifest(&ProjectManifestOverride::Disabled, &cwd, Some(&home));

        assert_eq!(got, None);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn detect_override_behaves_like_default_detection() {
        let (root, home) = temp_home_tree("upeg_project_resolve_detect");
        let cwd = home.join("proj");
        write_manifest(&home, "home_level");
        std::fs::create_dir_all(&cwd).unwrap();

        let got = resolve_project_manifest(&ProjectManifestOverride::Detect, &cwd, Some(&home))
            .expect("manifest");

        assert_eq!(got.path, home.join(PROJECT_MANIFEST_FILE));
        assert_eq!(got.origin, ProjectManifestOrigin::Detected);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn override_labels_distinguish_all_three_states() {
        assert_eq!(
            project_manifest_override_label(&ProjectManifestOverride::Detect),
            "detect"
        );
        assert_eq!(
            project_manifest_override_label(&ProjectManifestOverride::Disabled),
            "off"
        );
        assert_eq!(
            project_manifest_override_label(&ProjectManifestOverride::Explicit(PathBuf::from(
                "/x"
            ))),
            "explicit"
        );
    }
}
