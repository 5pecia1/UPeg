//! Where a Toolkit manifest was loaded from.
//!
//! The External invoker needs two different things from the manifest's
//! location, and they are *not* the same thing:
//!
//!   * a base directory to resolve a relative `cwd = "…"` against, which
//!     every file-backed manifest can provide; and
//!   * an implicit working directory for the spawned child, which only a
//!     Project Manifest can provide — a tool declared in
//!     `~/.upeg/toolkits/dev.toml` has no business running in
//!     `~/.upeg/toolkits`, while a tool declared in a repo's `upeg.toml`
//!     should run at the repo root no matter which subdirectory the
//!     caller happens to be in.
//!
//! That second point is a *boundary*, not merely a fallback: a caller's
//! `_upeg.cwd` may move a Project Manifest tool to a directory inside
//! the project (a workspace member, a subpackage), but one pointing
//! outside the project is ignored in favour of the manifest directory.
//! A toolkit-directory tool has no project to belong to, so there the
//! caller's directory wins outright. The rule is enforced in
//! `dispatcher::external::invoker`'s `WorkingDirectoryPolicy`.
//!
//! Modeling that as one enum (rather than passing a bare `PathBuf` plus
//! a bool) keeps the distinction impossible to mix up at a call site.

use std::path::{Path, PathBuf};

/// The manifest file a tool was declared in, classified by which loader
/// read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ManifestOrigin {
    /// A `*.toml` file under the toolkits directory
    /// (`~/.upeg/toolkits`, or `$UPEG_TOOLKITS_DIR`).
    ToolkitDirectory { directory: PathBuf },
    /// A Project Manifest (`upeg.toml`) merged from a project root.
    ProjectManifest { directory: PathBuf },
}

impl ManifestOrigin {
    /// Classify a toolkits-directory manifest by its file path.
    pub(crate) fn toolkit_file(path: &Path) -> Option<Self> {
        Some(Self::ToolkitDirectory {
            directory: manifest_directory(path)?,
        })
    }

    /// Classify a Project Manifest by its file path.
    pub(crate) fn project_manifest(path: &Path) -> Option<Self> {
        Some(Self::ProjectManifest {
            directory: manifest_directory(path)?,
        })
    }

    /// Directory a relative `cwd` declaration resolves against.
    pub(crate) fn base_directory(&self) -> &Path {
        match self {
            Self::ToolkitDirectory { directory } | Self::ProjectManifest { directory } => directory,
        }
    }

    /// Directory the child process runs in when the tool declares no
    /// `cwd`, and the boundary a caller-supplied working directory may
    /// not escape. `None` for toolkit-directory manifests, whose tools
    /// inherit the process working directory and accept any caller
    /// directory.
    pub(crate) fn implicit_working_directory(&self) -> Option<&Path> {
        match self {
            Self::ToolkitDirectory { .. } => None,
            Self::ProjectManifest { directory } => Some(directory),
        }
    }
}

/// Absolute directory holding `path`.
///
/// Relative manifest paths are made absolute against the process
/// working directory *at load time*, so a later `chdir` (or a daemon
/// serving requests from elsewhere) can't retarget an already-loaded
/// tool.
fn manifest_directory(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    absolute.parent().map(Path::to_path_buf)
}
