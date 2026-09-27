//! Typed identity and conflict choice for directory projects.

use std::path::{Path, PathBuf};

pub const PROJECT_MARKER_DIR: &str = ".upeg";
pub const PROJECT_CONFIG_FILE: &str = "project.toml";
pub const PROJECT_TOOLKITS_DIR: &str = "toolkits";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum ProjectToolChoice {
    Global,
    Project,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProjectRoot(PathBuf);

impl ProjectRoot {
    pub fn new(root: &Path) -> Option<Self> {
        let root = std::fs::canonicalize(root).ok()?;
        root.join(PROJECT_MARKER_DIR).is_dir().then_some(Self(root))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn marker_dir(&self) -> PathBuf {
        self.0.join(PROJECT_MARKER_DIR)
    }

    pub fn config_path(&self) -> PathBuf {
        self.marker_dir().join(PROJECT_CONFIG_FILE)
    }

    pub fn toolkits_dir(&self) -> PathBuf {
        self.marker_dir().join(PROJECT_TOOLKITS_DIR)
    }
}
