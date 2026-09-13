//! Startup source snapshots keep previews consistent with the loaded registry.

use std::collections::{BTreeMap, HashMap, hash_map::DefaultHasher};
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::RuntimeSourceConfig;

#[cfg(test)]
mod tests;

/// The loaded registry no longer represents the source files on disk.
#[derive(Debug, thiserror::Error)]
pub enum LoadedSourcesError {
    #[error("runtime source changed: {}; restart UPeg or reconnect the MCP server to reload its tools", path.display())]
    Changed { path: PathBuf },
    #[error("runtime source unavailable: {}: {source}; restart UPeg after restoring the source", path.display())]
    Unavailable {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Clone, Debug)]
struct FailedRead {
    path: PathBuf,
    kind: std::io::ErrorKind,
    message: String,
}

impl FailedRead {
    fn at(path: &Path, error: std::io::Error) -> Self {
        Self {
            path: path.to_path_buf(),
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

impl From<FailedRead> for LoadedSourcesError {
    fn from(error: FailedRead) -> Self {
        Self::Unavailable {
            path: error.path,
            source: std::io::Error::new(error.kind, error.message),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    /// Includes absent project discovery, so creating upeg.toml is a change.
    paths: Vec<Option<PathBuf>>,
    files: BTreeMap<PathBuf, u64>,
}

type RecordedSnapshot = Arc<Result<Snapshot, FailedRead>>;

fn snapshots() -> &'static Mutex<HashMap<PathBuf, RecordedSnapshot>> {
    static SNAPSHOTS: OnceLock<Mutex<HashMap<PathBuf, RecordedSnapshot>>> = OnceLock::new();
    SNAPSHOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Called only at local runtime startup, before any declarations are loaded.
/// Deferred MCP import loading must keep this original snapshot.
pub(crate) fn record_loaded_sources(config: &RuntimeSourceConfig) {
    if let Some(root) = upeg_core::paths::config_root() {
        record_sources_for_root(&root, config);
    }
}

fn record_sources_for_root(root: &Path, config: &RuntimeSourceConfig) {
    let key = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    let snapshot = Arc::new(capture(config));
    snapshots()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key, snapshot);
}

/// Validate against startup, even before the first Board preview or session.
/// Embedders that have not loaded local sources have no snapshot to validate.
///
/// # Errors
/// Returns a changed source path or an I/O failure without revealing contents.
pub fn validate_loaded_sources() -> Result<(), LoadedSourcesError> {
    let Some(root) = upeg_core::paths::config_root() else {
        return Ok(());
    };
    validate_sources_for_root(&root, &RuntimeSourceConfig::from_env())
}

fn validate_sources_for_root(
    root: &Path,
    config: &RuntimeSourceConfig,
) -> Result<(), LoadedSourcesError> {
    let key = std::path::absolute(root).map_err(|error| FailedRead::at(root, error))?;
    let recorded = snapshots()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&key)
        .cloned();
    let Some(recorded) = recorded else {
        return Ok(());
    };
    let expected = recorded.as_ref().as_ref().map_err(Clone::clone)?;
    let current = capture(config)?;
    for (before, after) in expected.paths.iter().zip(&current.paths) {
        if before != after {
            return Err(LoadedSourcesError::Changed {
                path: after.as_ref().or(before.as_ref()).cloned().unwrap_or(key),
            });
        }
    }
    for path in expected.files.keys().chain(current.files.keys()) {
        if expected.files.get(path) != current.files.get(path) {
            return Err(LoadedSourcesError::Changed { path: path.clone() });
        }
    }
    Ok(())
}

fn capture(config: &RuntimeSourceConfig) -> Result<Snapshot, FailedRead> {
    let project = config
        .project_manifest
        .as_ref()
        .map(|lookup| lookup.path.as_path());
    let mut directories = vec![
        (
            config.toolkits_dir.as_deref(),
            crate::sources::TOOLKIT_FILE_EXTENSION,
        ),
        (
            config.mcp_import_dir.as_deref(),
            crate::sources::MCP_IMPORT_FILE_EXTENSION,
        ),
    ];
    if cfg!(feature = "wasm-plugin") {
        directories.push((
            config.wasm_dir.as_deref(),
            crate::sources::WASM_FILE_EXTENSION,
        ));
    }
    let paths = std::iter::once(project)
        .chain(directories.iter().map(|(path, _)| *path))
        .map(|path| {
            path.map(|path| std::path::absolute(path).map_err(|error| FailedRead::at(path, error)))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut files = BTreeMap::new();
    if let Some(path) = project {
        read_fingerprint(path, &mut files)?;
    }
    for (directory, extension) in directories {
        let Some(directory) = directory else {
            continue;
        };
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(FailedRead::at(directory, error)),
        };
        for entry in entries {
            let path = entry
                .map_err(|error| FailedRead::at(directory, error))?
                .path();
            if path.extension().is_some_and(|value| value == extension) {
                read_fingerprint(&path, &mut files)?;
            }
        }
    }
    Ok(Snapshot { paths, files })
}

fn read_fingerprint(path: &Path, files: &mut BTreeMap<PathBuf, u64>) -> Result<(), FailedRead> {
    let bytes = std::fs::read(path).map_err(|error| FailedRead::at(path, error))?;
    let path = std::path::absolute(path).map_err(|error| FailedRead::at(path, error))?;
    let mut hash = DefaultHasher::new();
    hash.write(&bytes);
    files.insert(path, hash.finish());
    Ok(())
}
