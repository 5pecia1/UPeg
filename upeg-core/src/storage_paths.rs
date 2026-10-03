use std::path::{Component, Path, PathBuf};

use super::{Platform, UserPaths, env};

pub const STORAGE_MARKER: &str = "storage-layout.json";
pub const MIGRATION_PENDING: &str = ".storage-migration.json";
pub const STORAGE_LOCK: &str = ".storage.lock";
/// Prefix for every control temp file UPeg creates directly at a storage root
/// (marker/pending writes via hard-link-then-rename or rename-into-place).
/// Inventories/scans must ignore names with this prefix so a leftover or
/// in-flight temp file is never mistaken for user content.
pub const STORAGE_TMP_PREFIX: &str = ".storage-tmp-";
pub const LEGACY_ARTIFACTS: &[&str] = &[
    ".upeg-tweaks.json",
    "credentials.json",
    "mcp-imports",
    "upeg.db",
    "upeg.db-wal",
    "upeg.db-shm",
    "toolkits",
    "wasm",
    "upeg-http.log",
    "toolkit-packs",
    "server.json",
    "desktop.lock",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageLayout {
    LegacyV1,
    SplitV2,
}

impl StorageLayout {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LegacyV1 => "legacy-v1",
            Self::SplitV2 => "split-v2",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageLocation {
    pub root: PathBuf,
    pub layout: StorageLayout,
    pub paths: UserPaths,
}

impl StorageLocation {
    pub fn new(root: PathBuf, layout: StorageLayout) -> Self {
        let role = |name| match layout {
            StorageLayout::LegacyV1 => root.clone(),
            StorageLayout::SplitV2 => root.join(name),
        };
        Self {
            paths: UserPaths {
                config_dir: role("config"),
                data_dir: role("data"),
                state_dir: role("state"),
                cache_dir: role("cache"),
                runtime_dir: role("runtime"),
            },
            root,
            layout,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoragePathError {
    #[error("storage root unavailable; set an absolute UPEG_HOME or profile home")]
    Unavailable,
    #[error("unsafe storage path: {0}")]
    Unsafe(PathBuf),
    #[error("invalid storage marker at {0}")]
    Marker(PathBuf),
    #[error("ambiguous populated storage roots: {0} and {1}; use storage plan explicitly")]
    Ambiguous(PathBuf, PathBuf),
    #[error("storage migration pending or inactive at {0}; resume the recorded plan")]
    Pending(PathBuf),
    #[error("storage changed or is in use at {0}; stop native hosts and retry")]
    Busy(PathBuf),
    #[error("storage I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "storage migration journal missing at {0}; the migration_id recorded in storage-layout.json has no matching state/migrations journal"
    )]
    MissingJournal(PathBuf),
}

impl StoragePathError {
    pub fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct StorageInventory {
    pub marker: Option<serde_json::Value>,
    pub legacy: bool,
    pub populated: bool,
    pub pending: bool,
    pub inactive: bool,
    pub split_artifacts: bool,
}

/// Parses JSON the same way `serde_json::from_slice::<Value>` does, except an
/// object literal containing a duplicate key is rejected outright instead of
/// silently keeping the last occurrence (serde_json's default behavior).
/// Applied only to the two identity-bearing files (`storage-layout.json`,
/// `.storage-migration.json`) that decide which physical directory an app
/// treats as its storage root: a hand-edited or corrupted duplicate key there
/// must never be silently resolved one way or the other. This mirrors the
/// stricter reference (Python) marker parser used elsewhere in the ecosystem.
///
/// Needs the optional `serde` dependency directly (for its generic
/// `Visitor`/`MapAccess` machinery, not just the `serde_json::Value` this
/// crate always depends on), so it is only compiled in behind the `serde`
/// feature. Every real consumer (`upeg-cli`, `upeg-sources`, `upeg-runtime`,
/// ...) always builds `upeg-core` with that feature enabled; only a bare
/// `cargo build -p upeg-core` with no features falls back to plain,
/// last-key-wins parsing below, keeping this crate's default build
/// dependency-light as documented on the `serde` feature itself.
///
/// Also gated `not(target_arch = "wasm32")`: its only caller,
/// `inspect_storage`, is itself native-only (storage inspection has no
/// meaning on wasm32, where `resolve_storage` always returns
/// `Unavailable`), so on a wasm32 build this function has no caller at all
/// and must not be compiled in either.
#[cfg(all(feature = "serde", not(target_arch = "wasm32")))]
fn strict_marker_json(bytes: &[u8]) -> serde_json::Result<serde_json::Value> {
    use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};

    struct Seed;
    impl<'de> DeserializeSeed<'de> for Seed {
        type Value = serde_json::Value;
        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(StrictVisitor)
        }
    }

    struct StrictVisitor;
    impl<'de> Visitor<'de> for StrictVisitor {
        type Value = serde_json::Value;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a JSON value")
        }
        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
            Ok(serde_json::Value::Bool(value))
        }
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
            Ok(serde_json::Value::from(value))
        }
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
            Ok(serde_json::Value::from(value))
        }
        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E> {
            Ok(serde_json::Value::from(value))
        }
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
            Ok(serde_json::Value::String(value.to_owned()))
        }
        fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
            Ok(serde_json::Value::String(value))
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(serde_json::Value::Null)
        }
        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(serde_json::Value::Null)
        }
        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(Self)
        }
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element_seed(Seed)? {
                values.push(value);
            }
            Ok(serde_json::Value::Array(values))
        }
        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut object = serde_json::Map::new();
            while let Some(key) = map.next_key::<String>()? {
                let value = map.next_value_seed(Seed)?;
                if object.insert(key.clone(), value).is_some() {
                    return Err(serde::de::Error::custom(format!("duplicate key: {key}")));
                }
            }
            Ok(serde_json::Value::Object(object))
        }
    }

    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = Seed.deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

/// Fallback used only when the crate is built without the `serde` feature
/// (see the doc comment on the feature-gated `strict_marker_json` above):
/// ordinary, last-key-wins parsing. Same `not(target_arch = "wasm32")`
/// reasoning applies: no caller exists on wasm32.
#[cfg(all(not(feature = "serde"), not(target_arch = "wasm32")))]
fn strict_marker_json(bytes: &[u8]) -> serde_json::Result<serde_json::Value> {
    serde_json::from_slice(bytes)
}

pub fn normalized_root(path: &Path) -> Result<PathBuf, StoragePathError> {
    if !path.is_absolute() || path.parent().is_none() {
        return Err(StoragePathError::Unsafe(path.to_path_buf()));
    }
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(StoragePathError::Unsafe(path.to_path_buf()));
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    if normalized.parent().is_none() {
        return Err(StoragePathError::Unsafe(path.to_path_buf()));
    }
    Ok(normalized)
}

fn normalize_for_platform(path: &Path, platform: Platform) -> Result<PathBuf, StoragePathError> {
    if platform == Platform::Windows && !cfg!(windows) {
        let value = path.to_string_lossy().replace('\\', "/");
        let bytes = value.as_bytes();
        if bytes.len() > 3 && bytes[0].is_ascii_alphabetic() && bytes[1..3] == *b":/" {
            if value.split('/').any(|part| part == ".." || part == ".") {
                return Err(StoragePathError::Unsafe(path.to_path_buf()));
            }
            return Ok(value.into());
        }
    }
    normalized_root(path)
}

pub fn profile_home_from_lookup(
    lookup: &impl Fn(&str) -> Option<PathBuf>,
    platform: Platform,
) -> Result<PathBuf, StoragePathError> {
    let home = match platform {
        Platform::Unix => lookup(env::HOME),
        Platform::Windows => lookup("USERPROFILE")
            .or_else(|| {
                let drive = lookup("HOMEDRIVE")?;
                let path = lookup("HOMEPATH")?;
                Some(PathBuf::from(format!(
                    "{}{}",
                    drive.display(),
                    path.display()
                )))
            })
            .or_else(|| lookup(env::HOME)),
    }
    .ok_or(StoragePathError::Unavailable)?;
    let normalized = normalize_for_platform(&home, platform)?;
    Ok(canonicalized_profile_home(normalized, platform))
}

/// `check_storage_path` (and every marker/inventory check built on it) protects
/// the app root and everything under it, not HOME or HOME's ancestors: HOME (or
/// a directory above it) is routinely a symlink on real systems — Fedora
/// Silverblue/Kinoite/Bazzite (`/home -> /var/home`), macOS temp homes
/// (`/var -> /private/var`), FreeBSD (`/home -> /usr/home`). Canonicalize HOME
/// once here via the real filesystem so that ancestry never reaches the guard;
/// everything appended below it (`.upeg`, its role dirs, legacy sources) is
/// still checked as before. This only touches the real filesystem when
/// resolving the *actual* running platform's home — cross-platform path-rule
/// simulation (testing Windows path rules on a non-Windows host or vice versa)
/// never denotes a real, checkable path, so it is left exactly as normalized.
/// A HOME that does not exist yet (fixtures, fresh containers) falls back to
/// the lexically normalized path unchanged, matching the previous behavior.
fn canonicalized_profile_home(path: PathBuf, platform: Platform) -> PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if platform == Platform::current() {
            return std::fs::canonicalize(&path).unwrap_or(path);
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = platform;
    }
    path
}

pub fn resolve_storage_with_lookup(
    lookup: impl Fn(&str) -> Option<PathBuf>,
    platform: Platform,
    inspect: impl Fn(&Path) -> Result<StorageInventory, StoragePathError>,
) -> Result<StorageLocation, StoragePathError> {
    let default = profile_home_from_lookup(&lookup, platform).map(|home| home.join(".upeg"));
    if let Some(explicit) = lookup(env::UPEG_HOME) {
        let root = normalize_for_platform(&explicit, platform)?;
        let layout = if default.as_ref().is_ok_and(|default| *default == root) {
            StorageLayout::SplitV2
        } else {
            StorageLayout::LegacyV1
        };
        return select_root(&root, layout, &inspect);
    }
    let root = default?;
    let inventory = inspect(&root)?;
    if platform == Platform::Windows
        && let Some(appdata) = lookup(env::APPDATA)
    {
        let legacy = normalize_for_platform(&appdata, platform)?.join("upeg");
        if legacy != root {
            let old = inspect(&legacy)?;
            if old.populated || old.marker.is_some() || old.legacy {
                let selected = select_root(&legacy, StorageLayout::LegacyV1, &inspect)?;
                if selected.root == root {
                    return Ok(selected);
                }
                if inventory.inactive {
                    return Ok(selected);
                }
                if inventory.populated || inventory.marker.is_some() || inventory.legacy {
                    return Err(StoragePathError::Ambiguous(root, legacy));
                }
                return Ok(selected);
            }
        }
    }
    select_root(&root, StorageLayout::SplitV2, &inspect)
}

fn select_root(
    root: &Path,
    default: StorageLayout,
    inspect: &impl Fn(&Path) -> Result<StorageInventory, StoragePathError>,
) -> Result<StorageLocation, StoragePathError> {
    let inventory = inspect(root)?;
    // A pending marker whose recorded status is "inactive" means a migration
    // that once targeted this root was rolled back or aborted: it is
    // informational, not a live migration in progress. Treat it the same as
    // no pending marker at all (fall back to normal resolution) on every
    // platform, instead of refusing indefinitely.
    if inventory.pending && !inventory.inactive {
        return Err(StoragePathError::Pending(root.to_path_buf()));
    }
    let Some(marker) = inventory.marker else {
        if inventory.split_artifacts {
            return Err(StoragePathError::Marker(root.into()));
        }
        return Ok(StorageLocation::new(
            root.to_path_buf(),
            if inventory.legacy {
                StorageLayout::LegacyV1
            } else {
                default
            },
        ));
    };
    validate_marker(&marker, root)?;
    match marker["layout"].as_str() {
        Some("split-v2") => Ok(StorageLocation::new(
            root.to_path_buf(),
            StorageLayout::SplitV2,
        )),
        Some("redirect-v2") => {
            let target = marker["target"]
                .as_str()
                .map(PathBuf::from)
                .ok_or_else(|| StoragePathError::Marker(root.to_path_buf()))?;
            if normalized_root(&target)? != target || target == root {
                return Err(StoragePathError::Marker(root.to_path_buf()));
            }
            let target_inventory = inspect(&target)?;
            if target_inventory.pending {
                return Err(StoragePathError::Pending(target));
            }
            let target_marker = target_inventory
                .marker
                .ok_or_else(|| StoragePathError::Marker(target.clone()))?;
            validate_marker(&target_marker, &target)?;
            if target_marker["layout"] != "split-v2" {
                return Err(StoragePathError::Marker(target));
            }
            // The redirect only vouches for a target that carries the same
            // migration_id: a redirect pointing at an unrelated (or later
            // reused) split-v2 root must not be silently followed.
            if marker.get("migration_id") != target_marker.get("migration_id") {
                return Err(StoragePathError::Marker(root.to_path_buf()));
            }
            Ok(StorageLocation::new(target, StorageLayout::SplitV2))
        }
        _ => Err(StoragePathError::Marker(root.to_path_buf())),
    }
}

pub fn validate_marker(marker: &serde_json::Value, root: &Path) -> Result<(), StoragePathError> {
    let valid = marker.as_object().is_some_and(|object| {
        object.keys().all(|key| {
            matches!(
                key.as_str(),
                "schema_version" | "app" | "layout" | "root" | "target" | "migration_id"
            )
        })
    }) && marker["schema_version"] == 1
        && marker["app"] == "upeg"
        && marker["root"].as_str() == root.to_str()
        && marker
            .get("migration_id")
            .is_none_or(|id| id.as_str().is_some_and(|id| !id.is_empty()))
        && match marker["layout"].as_str() {
            Some("split-v2") => marker.get("target").is_none(),
            Some("redirect-v2") => marker["target"].is_string(),
            _ => false,
        };
    if valid {
        Ok(())
    } else {
        Err(StoragePathError::Marker(root.to_path_buf()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn check_storage_path(path: &Path) -> Result<(), StoragePathError> {
    for ancestor in path.ancestors() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                let mut unsafe_path = metadata.file_type().is_symlink();
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt as _;
                    unsafe_path |= metadata.file_attributes() & 0x400 != 0;
                }
                #[cfg(not(windows))]
                {
                    let _ = &mut unsafe_path;
                }
                if unsafe_path {
                    return Err(StoragePathError::Unsafe(ancestor.to_path_buf()));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(StoragePathError::io(ancestor, error)),
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn inspect_storage(root: &Path) -> Result<StorageInventory, StoragePathError> {
    check_storage_path(root)?;
    let mut inventory = StorageInventory::default();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(inventory),
        Err(error) => return Err(StoragePathError::io(root, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| StoragePathError::io(root, error))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == STORAGE_LOCK || name.starts_with(STORAGE_TMP_PREFIX) {
            continue;
        }
        check_storage_path(&entry.path())?;
        inventory.populated = true;
        inventory.legacy |= LEGACY_ARTIFACTS.contains(&name.as_ref());
        inventory.pending |= name == MIGRATION_PENDING;
        if name == MIGRATION_PENDING {
            let bytes =
                std::fs::read(entry.path()).map_err(|error| StoragePathError::io(root, error))?;
            let value =
                strict_marker_json(&bytes).map_err(|_| StoragePathError::Marker(root.into()))?;
            inventory.inactive = value["schema_version"] == 1
                && value["app"] == "upeg"
                && value["status"] == "inactive"
                && value["target"].as_str() == root.to_str();
        }
        if name == STORAGE_MARKER {
            let bytes =
                std::fs::read(entry.path()).map_err(|error| StoragePathError::io(root, error))?;
            inventory.marker = Some(
                strict_marker_json(&bytes)
                    .map_err(|_| StoragePathError::Marker(root.to_path_buf()))?,
            );
        }
    }
    for relative in [
        "config/.upeg-tweaks.json",
        "config/credentials.json",
        "config/mcp-imports",
        "data/upeg.db",
        "data/toolkits",
        "data/wasm",
        "state/upeg-http.log",
        "cache/toolkit-packs",
        "runtime/server.json",
        "runtime/desktop.lock",
    ] {
        let path = root.join(relative);
        check_storage_path(&path)?;
        inventory.split_artifacts |= path
            .try_exists()
            .map_err(|error| StoragePathError::io(&path, error))?;
    }
    Ok(inventory)
}

pub fn resolve_storage() -> Result<StorageLocation, StoragePathError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let explicit = super::env_path(env::UPEG_HOME);
        let absolute = explicit
            .as_ref()
            .map(|root| {
                if root.is_absolute() {
                    Ok(root.clone())
                } else if root.as_os_str().is_empty() {
                    std::env::current_dir()
                } else {
                    std::path::absolute(root)
                }
            })
            .transpose()
            .map_err(|source| StoragePathError::Io {
                path: explicit.clone().unwrap_or_default(),
                source,
            })?;
        if let Some(root) = &absolute {
            check_storage_path(root)?;
        }
        let mut location = resolve_storage_with_lookup(
            |name| {
                if name == env::UPEG_HOME {
                    absolute.clone()
                } else {
                    super::env_path(name)
                }
            },
            Platform::current(),
            inspect_storage,
        )?;
        if location.layout == StorageLayout::LegacyV1
            && let Some(original) = explicit
        {
            location.paths = StorageLocation::new(original, StorageLayout::LegacyV1).paths;
        }
        Ok(location)
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err(StoragePathError::Unavailable)
    }
}

pub fn storage_root() -> Option<PathBuf> {
    resolve_storage().ok().map(|location| location.root)
}

pub fn global_storage_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = storage_root() {
        roots.push(root);
    }
    if let Some(root) = super::env_path(env::UPEG_HOME) {
        roots.push(root);
    }
    if let Ok(home) = profile_home_from_lookup(&super::env_path, Platform::current()) {
        roots.push(home.join(".upeg"));
    }
    if cfg!(windows)
        && let Some(appdata) = super::env_path(env::APPDATA)
    {
        roots.push(appdata.join("upeg"));
    }
    roots
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct StorageLease {
    _file: std::fs::File,
    location: StorageLocation,
}

#[cfg(not(target_arch = "wasm32"))]
impl StorageLease {
    pub const fn location(&self) -> &StorageLocation {
        &self.location
    }

    pub fn acquire() -> Result<Self, StoragePathError> {
        let location = resolve_storage()?;
        let file = lock_storage(&location.root, false)?;
        if resolve_storage()? != location {
            return Err(StoragePathError::Busy(location.root));
        }
        if location.layout == StorageLayout::SplitV2 {
            ensure_split_marker(&location.root)?;
            check_migrated_files(&location.root)?;
        }
        Ok(Self {
            _file: file,
            location,
        })
    }

    pub fn for_location(location: &StorageLocation) -> Result<Self, StoragePathError> {
        let absolute = if location.root.as_os_str().is_empty() {
            std::env::current_dir()
        } else {
            std::path::absolute(&location.root)
        }
        .map_err(|error| StoragePathError::io(&location.root, error))?;
        check_storage_path(&absolute)?;
        let location = StorageLocation::new(normalized_root(&absolute)?, location.layout);
        let file = lock_storage(&location.root, false)?;
        let lease = Self {
            _file: file,
            location,
        };
        lease.validate()?;
        if lease.location.layout == StorageLayout::SplitV2 {
            ensure_split_marker(&lease.location.root)?;
        }
        Ok(lease)
    }

    pub fn validate(&self) -> Result<(), StoragePathError> {
        let current = select_root(&self.location.root, self.location.layout, &inspect_storage)?;
        if current.root != self.location.root || current.layout != self.location.layout {
            return Err(StoragePathError::Busy(self.location.root.clone()));
        }
        if self.location.layout == StorageLayout::SplitV2 {
            check_migrated_files(&self.location.root)?;
        }
        Ok(())
    }

    pub fn for_artifact(path: &Path, role: &str) -> Result<Self, StoragePathError> {
        check_storage_path(path)?;
        let parent = path
            .parent()
            .ok_or_else(|| StoragePathError::Unsafe(path.into()))?;
        let mut split_parent = false;
        if parent.file_name().is_some_and(|name| name == role)
            && let Some(root) = parent.parent()
        {
            for name in [STORAGE_MARKER, MIGRATION_PENDING, STORAGE_LOCK] {
                let candidate = root.join(name);
                check_storage_path(&candidate)?;
                split_parent |= candidate
                    .try_exists()
                    .map_err(|error| StoragePathError::io(&candidate, error))?;
            }
        }
        let (root, layout) = if split_parent {
            (
                parent
                    .parent()
                    .ok_or_else(|| StoragePathError::Unsafe(path.into()))?,
                StorageLayout::SplitV2,
            )
        } else {
            (parent, StorageLayout::LegacyV1)
        };
        Self::for_location(&StorageLocation::new(root.into(), layout))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn check_migrated_files(root: &Path) -> Result<(), StoragePathError> {
    let Some(marker) = inspect_storage(root)?.marker else {
        return Ok(());
    };
    let Some(id) = marker
        .get("migration_id")
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(());
    };
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(StoragePathError::Marker(root.into()));
    }
    let journal = root.join("state/migrations").join(id).join("journal.json");
    check_storage_path(&journal)?;
    if !journal
        .try_exists()
        .map_err(|error| StoragePathError::io(&journal, error))?
    {
        return Err(StoragePathError::MissingJournal(journal));
    }
    let bytes = std::fs::read(&journal).map_err(|error| StoragePathError::io(&journal, error))?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| StoragePathError::Marker(root.into()))?;
    let entries = value["plan"]["entries"]
        .as_array()
        .ok_or_else(|| StoragePathError::Marker(root.into()))?;
    for entry in entries {
        let relative = Path::new(
            entry["destination"]
                .as_str()
                .ok_or_else(|| StoragePathError::Marker(root.into()))?,
        );
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(StoragePathError::Marker(root.into()));
        }
        if relative.starts_with("cache")
            || relative.starts_with("runtime")
            || relative == Path::new("state/upeg-http.log")
        {
            continue;
        }
        let path = root.join(relative);
        check_storage_path(&path)?;
        if !path
            .try_exists()
            .map_err(|error| StoragePathError::io(&path, error))?
        {
            return Err(StoragePathError::Marker(root.into()));
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn lock_storage(root: &Path, exclusive: bool) -> Result<std::fs::File, StoragePathError> {
    check_storage_path(root)?;
    std::fs::create_dir_all(root).map_err(|error| StoragePathError::io(root, error))?;
    let path = root.join(STORAGE_LOCK);
    check_storage_path(&path)?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let file = options
        .open(&path)
        .map_err(|error| StoragePathError::io(&path, error))?;
    let locked = if exclusive {
        file.try_lock()
    } else {
        file.try_lock_shared()
    };
    locked.map_err(|_| StoragePathError::Busy(root.to_path_buf()))?;
    Ok(file)
}

#[cfg(not(target_arch = "wasm32"))]
fn ensure_split_marker(root: &Path) -> Result<(), StoragePathError> {
    use std::io::Write as _;
    let marker = root.join(STORAGE_MARKER);
    if let Some(existing) = inspect_storage(root)?.marker {
        return validate_marker(&existing, root);
    }
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1, "app": "upeg", "layout": "split-v2", "root": root,
    }))
    .map_err(|_| StoragePathError::Marker(root.to_path_buf()))?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let temporary = root.join(format!(
        "{STORAGE_TMP_PREFIX}marker-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        match std::fs::hard_link(&temporary, &marker) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error),
        }
    })();
    let _ = std::fs::remove_file(&temporary);
    result.map_err(|error| StoragePathError::io(&marker, error))?;
    let current = inspect_storage(root)?
        .marker
        .ok_or_else(|| StoragePathError::Marker(root.to_path_buf()))?;
    validate_marker(&current, root)
}
