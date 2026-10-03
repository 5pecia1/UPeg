use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use upeg_core::paths::{
    self, MIGRATION_PENDING, STORAGE_LOCK, STORAGE_MARKER, STORAGE_TMP_PREFIX, StoragePathError,
};

mod receipt;
mod recovery;
mod sqlite;
mod verify;
pub use recovery::rollback;
pub use verify::verify;
#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error(transparent)]
    Path(#[from] StoragePathError),
    #[error("storage migration I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage migration JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage migration SQLite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("storage migration refused: {0}")]
    Refused(String),
}

type Result<T> = std::result::Result<T, StorageError>;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Directory,
    Sqlite,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub kind: EntryKind,
    pub hash: String,
    pub mode: u32,
    pub size: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema_version: u32,
    pub app: String,
    pub source_root: PathBuf,
    pub target_root: PathBuf,
    pub entries: Vec<Entry>,
    pub bindings: BTreeMap<String, Option<PathBuf>>,
    pub excluded_overrides: Vec<String>,
    pub unclassified: Vec<PathBuf>,
    pub unclassified_hashes: BTreeMap<PathBuf, String>,
    pub conflicts: Vec<String>,
    pub ecosystem_prefix: Option<PathBuf>,
    pub receipt: Option<receipt::ReceiptBinding>,
    pub plan_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    plan: Plan,
    phase: String,
    source_marker: Option<Value>,
    target_marker: Option<Value>,
    receipt_before: Option<Vec<u8>>,
    receipt_after: Option<Vec<u8>>,
}

pub(crate) fn database_lease(
    path: &Path,
) -> std::result::Result<Option<paths::StorageLease>, StoragePathError> {
    paths::check_storage_path(path)?;
    if path.file_name().is_none_or(|name| name != "upeg.db") {
        return Ok(None);
    }
    paths::StorageLease::for_artifact(path, "data").map(Some)
}

const ROLES: &[(&str, &str, Option<&str>)] = &[
    (".upeg-tweaks.json", "config/.upeg-tweaks.json", None),
    (
        "credentials.json",
        "config/credentials.json",
        Some("UPEG_CREDENTIALS_PATH"),
    ),
    (
        "mcp-imports",
        "config/mcp-imports",
        Some("UPEG_MCP_IMPORTS_DIR"),
    ),
    ("upeg.db", "data/upeg.db", None),
    ("toolkits", "data/toolkits", Some("UPEG_TOOLKITS_DIR")),
    ("wasm", "data/wasm", Some("UPEG_WASM_DIR")),
    (
        "upeg-http.log",
        "state/upeg-http.log",
        Some("UPEG_HTTP_LOG_PATH"),
    ),
    (
        "toolkit-packs",
        "cache/toolkit-packs",
        Some("UPEG_TOOLKIT_CACHE_DIR"),
    ),
];

fn refuse(message: impl Into<String>) -> StorageError {
    StorageError::Refused(message.into())
}

/// Process environment as a lookup closure, for the public (real) entry
/// points below. Mirrors `upeg_core::paths`'s own `env_path`/lookup-closure
/// split (see `resolve_storage_with_lookup`): production code reads the
/// ambient process environment, while `plan_with_lookup`/`apply_inner` (and
/// the tests that exercise them) can substitute an explicit, injected
/// override set instead of depending on whatever happens to be set in the
/// process that runs them.
fn env_lookup(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn bindings_from_lookup(
    lookup: &impl Fn(&str) -> Option<PathBuf>,
) -> BTreeMap<String, Option<PathBuf>> {
    [
        "UPEG_HOME",
        "HOME",
        "USERPROFILE",
        "HOMEDRIVE",
        "HOMEPATH",
        "APPDATA",
        "UPEG_TOOLKITS_DIR",
        "UPEG_WASM_DIR",
        "UPEG_MCP_IMPORTS_DIR",
        "UPEG_TOOLKIT_CACHE_DIR",
        "UPEG_CREDENTIALS_PATH",
        "UPEG_HTTP_LOG_PATH",
        "UPEG_LOG_PATH",
    ]
    .into_iter()
    .map(|name| (name.into(), lookup(name)))
    .collect()
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn canonical<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    value.sort_all_objects();
    Ok(serde_json::to_vec(&value)?)
}

fn plan_digest(plan: &Plan) -> Result<String> {
    let mut value = serde_json::to_value(plan)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("plan_hash");
    }
    Ok(digest(&canonical(&value)?))
}

fn exists(path: &Path) -> Result<bool> {
    paths::check_storage_path(path)?;
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn mode(path: &Path) -> Result<u32> {
    let metadata = fs::metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        Ok(metadata.permissions().mode() & 0o777)
    }
    #[cfg(not(unix))]
    {
        Ok(if metadata.permissions().readonly() {
            0o444
        } else {
            0o600
        })
    }
}

fn set_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_readonly(mode & 0o200 == 0);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn fingerprint(path: &Path, kind: &EntryKind) -> Result<(String, u64)> {
    paths::check_storage_path(path)?;
    if *kind == EntryKind::Directory {
        return Ok((String::new(), 0));
    }
    if *kind == EntryKind::Sqlite {
        return sqlite::fingerprint(path);
    }
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut size = 0;
    let mut buffer = [0; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        size += count as u64;
    }
    Ok((format!("{:x}", hasher.finalize()), size))
}

fn inventory(
    root: &Path,
    source: &Path,
    destination: &Path,
    entries: &mut Vec<Entry>,
) -> Result<()> {
    let path = root.join(source);
    paths::check_storage_path(&path)?;
    let metadata = fs::metadata(&path)?;
    let kind = if metadata.is_dir() {
        EntryKind::Directory
    } else if metadata.is_file() {
        if source == Path::new("upeg.db") {
            EntryKind::Sqlite
        } else {
            EntryKind::File
        }
    } else {
        return Err(refuse(format!("not a regular file: {}", path.display())));
    };
    let (hash, size) = fingerprint(&path, &kind)?;
    entries.push(Entry {
        source: source.into(),
        destination: destination.into(),
        kind: kind.clone(),
        hash,
        mode: mode(&path)?,
        size,
    });
    if kind == EntryKind::Directory {
        let mut children = fs::read_dir(&path)?.collect::<std::io::Result<Vec<_>>>()?;
        children.sort_by_key(std::fs::DirEntry::file_name);
        for child in children {
            inventory(
                root,
                &source.join(child.file_name()),
                &destination.join(child.file_name()),
                entries,
            )?;
        }
    }
    Ok(())
}

pub fn status() -> Result<Value> {
    let location = paths::resolve_storage()?;
    let inspected = paths::inspect_storage(&location.root)?;
    // Best-effort: if there is an active migration for this root, report any
    // source entries that drifted after cutover (see `source_drift`). This
    // never fails `status()` itself; `verify()` is the strict, failing check.
    let source_modified_after_activation = if location.layout == paths::StorageLayout::SplitV2 {
        verify::active_journal(&location.root)
            .map(|journal| verify::source_drift(&journal.plan))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    Ok(
        json!({"schema_version": 1, "app": "upeg", "root": location.root,
        "layout": location.layout.as_str(), "marker": inspected.marker,
        "config_dir": location.paths.config_dir, "data_dir": location.paths.data_dir,
        "state_dir": location.paths.state_dir, "cache_dir": location.paths.cache_dir,
        "runtime_dir": location.paths.runtime_dir,
        "source_modified_after_activation": source_modified_after_activation}),
    )
}

fn unsafe_containment(source: &Path, target: &Path) -> bool {
    let source_inside_target = source.starts_with(target);
    let target_inside_source = target.starts_with(source);
    source != target && (source_inside_target || target_inside_source)
}

pub fn plan(source: &Path, target: &Path, ecosystem_prefix: Option<&Path>) -> Result<Plan> {
    plan_with_lookup(source, target, ecosystem_prefix, &env_lookup)
}

/// Core of `plan()`, taking the environment/override lookup as a parameter for testability.
/// Production paths (plan(), apply(), and read_plan()) pass &env_lookup to read the actual
/// process environment, while tests inject their own lookups to remain hermetic and ensure
/// consistent override bindings end-to-end.
fn plan_with_lookup(
    source: &Path,
    target: &Path,
    ecosystem_prefix: Option<&Path>,
    lookup: &impl Fn(&str) -> Option<PathBuf>,
) -> Result<Plan> {
    paths::check_storage_path(source)?;
    paths::check_storage_path(target)?;
    if let Some(prefix) = ecosystem_prefix {
        paths::check_storage_path(prefix)?;
    }
    let source = paths::normalized_root(source)?;
    let target = paths::normalized_root(target)?;
    if unsafe_containment(&source, &target) {
        return Err(refuse(
            "source/target containment is unsafe; only identical roots are supported",
        ));
    }
    let source_inventory = paths::inspect_storage(&source)?;
    let target_inventory = paths::inspect_storage(&target)?;
    if source_inventory.marker.is_some()
        || source_inventory.pending
        || target_inventory.marker.is_some()
        || target_inventory.pending
    {
        return Err(refuse(
            "marker or pending migration exists; verify or resume its existing plan",
        ));
    }
    if !exists(&source)? {
        return Err(refuse("source root does not exist"));
    }
    let mut plan = Plan {
        schema_version: 1,
        app: "upeg".into(),
        source_root: source,
        target_root: target,
        entries: Vec::new(),
        bindings: bindings_from_lookup(lookup),
        excluded_overrides: Vec::new(),
        unclassified: Vec::new(),
        unclassified_hashes: BTreeMap::new(),
        conflicts: Vec::new(),
        ecosystem_prefix: ecosystem_prefix.map(paths::normalized_root).transpose()?,
        receipt: None,
        plan_hash: String::new(),
    };
    collect_source(&mut plan)?;
    if plan.source_root == plan.target_root {
        for role in ["config", "data", "state", "cache", "runtime"] {
            let path = plan.target_root.join(role);
            if !exists(&path)? {
                continue;
            }
            if role == "state" && state_dir_is_only_retired_migrations(&path)? {
                continue;
            }
            plan.conflicts
                .push(format!("destination already exists: {role}"));
        }
    } else if target_inventory.populated {
        plan.conflicts
            .push("destination contains unowned content".into());
    }
    plan.receipt = receipt::binding(&plan)?;
    plan.plan_hash = plan_digest(&plan)?;
    Ok(plan)
}

fn collect_source(plan: &mut Plan) -> Result<()> {
    plan.entries.clear();
    plan.excluded_overrides.clear();
    plan.unclassified.clear();
    plan.unclassified_hashes.clear();
    for (source, destination, override_name) in ROLES {
        if let Some(name) = override_name
            && plan.bindings.get(*name).is_some_and(Option::is_some)
        {
            plan.excluded_overrides.push((*name).into());
            continue;
        }
        if exists(&plan.source_root.join(source))? {
            inventory(
                &plan.source_root,
                Path::new(source),
                Path::new(destination),
                &mut plan.entries,
            )?;
        }
    }
    for child in fs::read_dir(&plan.source_root)? {
        let name = PathBuf::from(child?.file_name());
        if name.to_string_lossy().starts_with(STORAGE_TMP_PREFIX) {
            continue;
        }
        if !paths::LEGACY_ARTIFACTS
            .iter()
            .any(|artifact| name == Path::new(artifact))
            && name != Path::new(STORAGE_LOCK)
            && name != Path::new(STORAGE_MARKER)
            && name != Path::new(MIGRATION_PENDING)
            && !(plan.source_root == plan.target_root
                && ["config", "data", "state", "cache", "runtime"]
                    .iter()
                    .any(|role| name == Path::new(role)))
        {
            let mut contents = Vec::new();
            inventory(&plan.source_root, &name, &name, &mut contents)?;
            plan.unclassified_hashes
                .insert(name.clone(), digest(&canonical(&contents)?));
            plan.unclassified.push(name);
        }
    }
    plan.unclassified.sort();
    Ok(())
}

/// A same-root `state/` directory left behind by a rolled-back or aborted
/// migration must not block a fresh `plan()` at the same root: it holds only
/// our own retired migration history, never user data. Returns `true` only
/// when every entry under `state/` is either the `migrations/` directory
/// itself or a `migrations/<64-hex-id>` journal whose recorded phase is a
/// terminal, non-live one (`rolled_back` or `aborted`).
fn state_dir_is_only_retired_migrations(path: &Path) -> Result<bool> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_name() != std::ffi::OsStr::new("migrations") || !entry.path().is_dir() {
            return Ok(false);
        }
    }
    let migrations_dir = path.join("migrations");
    if !exists(&migrations_dir)? {
        return Ok(true);
    }
    for entry in fs::read_dir(&migrations_dir)? {
        let entry = entry?;
        let id = entry.file_name();
        let id = id.to_string_lossy();
        if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Ok(false);
        }
        let journal_path = migrations_dir.join(id.as_ref()).join("journal.json");
        if !exists(&journal_path)? {
            return Ok(false);
        }
        let journal: Journal = serde_json::from_slice(&fs::read(&journal_path)?)?;
        if !matches!(journal.phase.as_str(), "rolled_back" | "aborted") {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn write_plan(path: &Path, plan: &Plan) -> Result<()> {
    paths::check_storage_path(path)?;
    write_new(path, &serde_json::to_vec_pretty(plan)?, 0o600)
}

pub fn read_plan(path: &Path) -> Result<Plan> {
    paths::check_storage_path(path)?;
    let plan: Plan = serde_json::from_slice(&fs::read(path)?)?;
    validate_plan(&plan, &env_lookup)?;
    Ok(plan)
}

fn validate_plan_shape(plan: &Plan) -> Result<()> {
    if paths::normalized_root(&plan.source_root)? != plan.source_root
        || paths::normalized_root(&plan.target_root)? != plan.target_root
        || unsafe_containment(&plan.source_root, &plan.target_root)
    {
        return Err(refuse("invalid plan roots"));
    }
    for entry in &plan.entries {
        if entry
            .source
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(refuse("invalid relative source path"));
        }
        let allowed = ROLES.iter().any(|(source, destination, _)| {
            entry
                .source
                .strip_prefix(source)
                .is_ok_and(|suffix| entry.destination == Path::new(destination).join(suffix))
        });
        if !allowed {
            return Err(refuse("entry outside migration allowlist"));
        }
    }
    if let Some(receipt) = &plan.receipt {
        let prefix = plan
            .ecosystem_prefix
            .as_ref()
            .ok_or_else(|| refuse("missing receipt prefix"))?;
        if paths::normalized_root(prefix)? != *prefix
            || receipt.path != prefix.join("share/ecosystem/install.json")
            || receipt.source_toolkit != plan.source_root.join("toolkits/ecosystem.toml")
            || receipt.target_toolkit != plan.target_root.join("data/toolkits/ecosystem.toml")
        {
            return Err(refuse("invalid receipt binding paths"));
        }
    }
    Ok(())
}

fn validate_plan(plan: &Plan, lookup: &impl Fn(&str) -> Option<PathBuf>) -> Result<()> {
    validate_plan_shape(plan)?;
    if plan.schema_version != 1 || plan.app != "upeg" || plan_digest(plan)? != plan.plan_hash {
        return Err(refuse("invalid or tampered plan"));
    }
    if plan.bindings != bindings_from_lookup(lookup) {
        return Err(refuse("environment/override bindings changed since plan"));
    }
    if !plan.conflicts.is_empty() {
        return Err(refuse(format!(
            "plan has conflicts: {}",
            plan.conflicts.join("; ")
        )));
    }
    let mut current = plan.clone();
    collect_source(&mut current)?;
    if current.entries != plan.entries
        || current.unclassified != plan.unclassified
        || current.unclassified_hashes != plan.unclassified_hashes
        || current.excluded_overrides != plan.excluded_overrides
    {
        return Err(refuse(
            "source contents changed or plan paths were injected; create a new plan",
        ));
    }
    Ok(())
}

fn journal_dir(plan: &Plan) -> PathBuf {
    plan.target_root
        .join("state/migrations")
        .join(&plan.plan_hash)
}

fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    paths::check_storage_path(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(mode);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    set_mode(path, mode)?;
    Ok(())
}

fn private_dir(path: &Path) -> Result<()> {
    paths::check_storage_path(path)?;
    if exists(path)? {
        return Ok(());
    }
    fs::create_dir_all(path)?;
    set_mode(path, 0o700)
}

fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    paths::check_storage_path(path)?;
    let parent = path.parent().ok_or_else(|| refuse("path has no parent"))?;
    let temporary = parent.join(format!("{STORAGE_TMP_PREFIX}{}", uuid::Uuid::new_v4()));
    write_new(&temporary, bytes, mode)?;
    fs::rename(&temporary, path)?;
    #[cfg(unix)]
    {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

/// Atomically create `path` with `bytes` using hard-link-then-remove, the
/// same crash-safe technique `ensure_split_marker` uses: a partially written
/// temp file never becomes visible at `path`, and a concurrent writer racing
/// to create the same content is tolerated (`AlreadyExists`).
fn pin_new(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    paths::check_storage_path(path)?;
    let parent = path.parent().ok_or_else(|| refuse("path has no parent"))?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let temporary = parent.join(format!(
        "{STORAGE_TMP_PREFIX}pin-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    write_new(&temporary, bytes, mode)?;
    let result = match fs::hard_link(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    };
    let _ = fs::remove_file(&temporary);
    result?;
    Ok(())
}

fn save_journal(journal: &Journal) -> Result<()> {
    atomic_write(
        &journal_dir(&journal.plan).join("journal.json"),
        &serde_json::to_vec(journal)?,
        0o600,
    )
}

fn locks(plan: &Plan) -> Result<Vec<File>> {
    let mut roots = vec![&plan.source_root, &plan.target_root];
    roots.sort();
    roots.dedup();
    roots
        .into_iter()
        .map(|root| paths::lock_storage(root, true).map_err(StorageError::from))
        .collect()
}

fn refuse_runtime(root: &Path) -> Result<()> {
    for relative in [
        "server.json",
        "desktop.lock",
        "runtime/server.json",
        "runtime/desktop.lock",
    ] {
        if exists(&root.join(relative))? {
            return Err(refuse(format!(
                "runtime evidence at {}; stop the old host/desktop and remove its stale discovery/lock only after confirming shutdown",
                root.join(relative).display()
            )));
        }
    }
    Ok(())
}

fn checked_entry(root: &Path, relative: &Path, expected: &Entry) -> Result<()> {
    let path = root.join(relative);
    let metadata = fs::metadata(&path)?;
    if metadata.is_dir() != (expected.kind == EntryKind::Directory)
        || mode(&path)? != expected.mode
        || fingerprint(&path, &expected.kind)? != (expected.hash.clone(), expected.size)
    {
        return Err(refuse(format!(
            "content or mode changed: {}",
            path.display()
        )));
    }
    Ok(())
}

fn copy_entries(journal: &Journal) -> Result<()> {
    let plan = &journal.plan;
    let writable_mode = if cfg!(unix) { 0o700 } else { 0o600 };
    let staging = journal_dir(plan).join("staging");
    private_dir(&staging)?;
    for entry in &plan.entries {
        let source = plan.source_root.join(&entry.source);
        let staged = staging.join(&entry.destination);
        if entry.kind == EntryKind::Directory {
            private_dir(&staged)?;
            // Staging remains writable so nested files can be copied and
            // the disposable tree can be removed after activation.
            set_mode(&staged, writable_mode)?;
        } else if exists(&staged)? {
            checked_entry(&staging, &entry.destination, entry)?;
        } else {
            if let Some(parent) = staged.parent() {
                private_dir(parent)?;
            }
            let temporary = staged.with_extension("copying");
            if exists(&temporary)? {
                fs::remove_file(&temporary)?;
            }
            if entry.kind == EntryKind::Sqlite {
                sqlite::backup(&source, &temporary)?;
            } else {
                write_new(&temporary, &fs::read(&source)?, entry.mode)?;
            }
            set_mode(&temporary, entry.mode)?;
            let (hash, size) = fingerprint(&temporary, &entry.kind)?;
            if hash != entry.hash || size != entry.size {
                return Err(refuse("source changed during copy"));
            }
            fs::rename(&temporary, &staged)?;
        }
    }
    for entry in &plan.entries {
        let destination = plan.target_root.join(&entry.destination);
        if entry.kind == EntryKind::Directory {
            if exists(&destination)? {
                if !fs::metadata(&destination)?.is_dir() {
                    return Err(refuse(format!(
                        "destination is not a directory: {}",
                        destination.display()
                    )));
                }
                let current_mode = mode(&destination)?;
                if current_mode != entry.mode && current_mode != writable_mode {
                    return Err(refuse(format!(
                        "destination directory mode changed: {}",
                        destination.display()
                    )));
                }
            }
            private_dir(&destination)?;
            // An interrupted copy may have left the original read-only mode.
            set_mode(&destination, writable_mode)?;
        } else if exists(&destination)? {
            checked_entry(&plan.target_root, &entry.destination, entry)?;
        } else {
            if let Some(parent) = destination.parent() {
                private_dir(parent)?;
            }
            let temporary = journal_dir(plan)
                .join("publishing")
                .join(&entry.destination);
            if let Some(parent) = temporary.parent() {
                private_dir(parent)?;
            }
            if exists(&temporary)? {
                fs::remove_file(&temporary)?;
            }
            if entry.kind == EntryKind::Sqlite {
                sqlite::backup(&staging.join(&entry.destination), &temporary)?;
                set_mode(&temporary, entry.mode)?;
            } else {
                write_new(
                    &temporary,
                    &fs::read(staging.join(&entry.destination))?,
                    entry.mode,
                )?;
            }
            checked_entry(
                &journal_dir(plan).join("publishing"),
                &entry.destination,
                entry,
            )?;
            fs::hard_link(&temporary, &destination)?;
            fs::remove_file(&temporary)?;
            checked_entry(&plan.target_root, &entry.destination, entry)?;
        }
    }
    // Restore directory modes after all descendants are in place. Parent
    // directories must stay writable until their children are complete.
    for entry in plan.entries.iter().rev() {
        if entry.kind == EntryKind::Directory {
            set_mode(&plan.target_root.join(&entry.destination), entry.mode)?;
        }
    }
    Ok(())
}

fn split_marker(plan: &Plan) -> Value {
    json!({"schema_version":1,"app":"upeg","layout":"split-v2","root":plan.target_root,"migration_id":plan.plan_hash})
}

fn redirect_marker(plan: &Plan) -> Value {
    json!({"schema_version":1,"app":"upeg","layout":"redirect-v2","root":plan.source_root,"target":plan.target_root,"migration_id":plan.plan_hash})
}

fn pending(root: &Path, plan: &Plan) -> Result<()> {
    let path = root.join(MIGRATION_PENDING);
    let expected = json!({"schema_version":1,"app":"upeg","plan_hash":plan.plan_hash,"target":plan.target_root});
    if exists(&path)? {
        if serde_json::from_slice::<Value>(&fs::read(path)?)? != expected {
            return Err(refuse("another migration owns this root"));
        }
    } else {
        pin_new(&path, &serde_json::to_vec(&expected)?, 0o600)?;
    }
    Ok(())
}

fn check_markers(journal: &Journal) -> Result<()> {
    let plan = &journal.plan;
    for (root, original, desired) in [
        (
            &plan.target_root,
            &journal.target_marker,
            split_marker(plan),
        ),
        (
            &plan.source_root,
            &journal.source_marker,
            if plan.source_root == plan.target_root {
                split_marker(plan)
            } else {
                redirect_marker(plan)
            },
        ),
    ] {
        let current = paths::inspect_storage(root)?.marker;
        if current != *original && current.as_ref() != Some(&desired) {
            return Err(refuse("storage marker changed outside migration"));
        }
    }
    Ok(())
}

pub fn apply(plan: &Plan, yes: bool, quiesced: bool) -> Result<Value> {
    apply_inner(plan, yes, quiesced, None, &env_lookup)
}

fn apply_inner(
    plan: &Plan,
    yes: bool,
    quiesced: bool,
    stop: Option<&str>,
    lookup: &impl Fn(&str) -> Option<PathBuf>,
) -> Result<Value> {
    if !yes || !quiesced {
        return Err(refuse(
            "apply requires --yes --quiesced; stop OLD binaries first",
        ));
    }
    validate_plan(plan, lookup)?;
    let _leases = locks(plan)?;
    refuse_runtime(&plan.source_root)?;
    refuse_runtime(&plan.target_root)?;
    let _ecosystem_lock = receipt::lock(plan)?;
    let _database_writer = sqlite::writer_exclusion(&plan.source_root.join("upeg.db"))?;
    validate_plan(plan, lookup)?;
    let directory = journal_dir(plan);
    let journal_path = directory.join("journal.json");
    let mut journal = if exists(&journal_path)? {
        let journal: Journal = serde_json::from_slice(&fs::read(&journal_path)?)?;
        if journal.plan != *plan
            || !matches!(
                journal.phase.as_str(),
                "prepared" | "copied" | "receipt" | "active"
            )
        {
            return Err(refuse(
                "journal does not own this plan or rollback has started",
            ));
        }
        check_markers(&journal)?;
        journal
    } else {
        let current = self::plan_with_lookup(
            &plan.source_root,
            &plan.target_root,
            plan.ecosystem_prefix.as_deref(),
            lookup,
        )?;
        if current != *plan {
            return Err(refuse("source, destination, or receipt changed since plan"));
        }
        private_dir(&directory)?;
        let journal = Journal {
            plan: plan.clone(),
            phase: "prepared".into(),
            source_marker: None,
            target_marker: None,
            receipt_before: receipt::read_before(plan)?,
            receipt_after: receipt::after_bytes(plan)?,
        };
        save_journal(&journal)?;
        journal
    };
    if journal.phase == "active" {
        let result = verify(&plan.target_root)?;
        for root in [&plan.source_root, &plan.target_root] {
            let path = root.join(MIGRATION_PENDING);
            if exists(&path)? {
                pending(root, plan)?;
                fs::remove_file(path)?;
            }
        }
        return Ok(result);
    }
    pending(&plan.target_root, plan)?;
    if plan.source_root != plan.target_root {
        pending(&plan.source_root, plan)?;
    }
    copy_entries(&journal)?;
    validate_plan(plan, lookup)?;
    journal.phase = "copied".into();
    save_journal(&journal)?;
    if stop == Some("copy") {
        return Err(refuse("interrupted after copy"));
    }
    verify::check_target_inventory(plan, false)?;
    receipt::activate(&journal)?;
    journal.phase = "receipt".into();
    save_journal(&journal)?;
    if stop == Some("receipt") {
        return Err(refuse("interrupted after receipt"));
    }
    verify::verify_entries(plan, false)?;
    validate_plan(plan, lookup)?;
    if stop == Some("marker") {
        return Err(refuse("interrupted before marker"));
    }
    atomic_write(
        &plan.target_root.join(STORAGE_MARKER),
        &serde_json::to_vec(&split_marker(plan))?,
        0o600,
    )?;
    if plan.source_root != plan.target_root {
        atomic_write(
            &plan.source_root.join(STORAGE_MARKER),
            &serde_json::to_vec(&redirect_marker(plan))?,
            0o600,
        )?;
    }
    journal.phase = "active".into();
    save_journal(&journal)?;
    fs::remove_file(plan.target_root.join(MIGRATION_PENDING))?;
    if plan.source_root != plan.target_root {
        fs::remove_file(plan.source_root.join(MIGRATION_PENDING))?;
    }
    // Staging copies are UPeg's own disposable working copies (the real,
    // owned files already live at their published destinations); clean them
    // up best-effort now that the migration is active. Failure here must not
    // fail an otherwise-successful activation.
    let _ = fs::remove_dir_all(journal_dir(plan).join("staging"));
    Ok(
        json!({"schema_version":1,"app":"upeg","status":"active","target_root":plan.target_root,"migration_id":plan.plan_hash}),
    )
}
