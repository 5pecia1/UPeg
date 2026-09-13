//! Recency ranking for `devcontainer`, extracted from the parent to keep every
//! file under the 1000-line workspace budget.
//!
//! Two ranks are read out of each editor's `globalStorage`: an *open* rank from
//! `storage.json`'s backed-up workspaces, and a *recent* rank from the menubar
//! data plus the recently-opened list in `globalStorage/state.vscdb`. Both are
//! keyed by workspace URI and applied to the entries the storage scan produced.
//! Native-only, for the same reason as [`super::storage`].

use std::collections::BTreeMap;
use std::path::Path;

use super::uri::norm_uri;
use super::{
    DevContainerEntry, GLOBAL_STORAGE_DIR, KEY_BACKUP_FOLDERS, KEY_BACKUP_WORKSPACES,
    KEY_BACKUP_WORKSPACES_LIST, KEY_ENTRIES, KEY_EXTERNAL, KEY_FOLDER_URI, KEY_HISTORY_RECENT,
    KEY_MENUBAR_DATA, KEY_URI, KEY_WORKSPACE_URI, STATE_VSCDB_FILE, STORAGE_JSON_FILE,
    storage::open_sqlite_ro,
};

/// Rank of the first-listed workspace in any ranked source; later entries count
/// upward, so a smaller rank means more recent.
const FIRST_RANK: i64 = 0;

/// URI → rank lookup, keyed by both the raw URI as written by the editor and
/// its normalized form.
///
/// The two forms live in separate maps on purpose: sharing one map would let
/// the normalized form of one URI answer a raw lookup for a *different* URI
/// that happens to collide with it. Ordered maps (not hash maps) keep merging
/// deterministic without a sort step.
#[derive(Debug, Default)]
pub(super) struct RankMap {
    raw: BTreeMap<String, i64>,
    normalized: BTreeMap<String, i64>,
}

impl RankMap {
    /// Record `rank` for `uri` under both key forms, keeping the lowest
    /// (first-seen) rank per key. Empty URIs are ignored.
    pub(super) fn add(&mut self, uri: &str, rank: i64) {
        if uri.is_empty() {
            return;
        }
        self.raw.entry(uri.to_string()).or_insert(rank);
        self.normalized.entry(norm_uri(uri)).or_insert(rank);
    }

    /// Fold `other` in, keeping this map's rank wherever a key already exists.
    pub(super) fn merge_missing(&mut self, other: Self) {
        for (key, rank) in other.raw {
            self.raw.entry(key).or_insert(rank);
        }
        for (key, rank) in other.normalized {
            self.normalized.entry(key).or_insert(rank);
        }
    }

    /// The rank recorded for `uri`: an exact match on the URI as the editor
    /// wrote it is authoritative, and the normalized form is only a fallback —
    /// otherwise one URI's normalized key could answer for another URI that
    /// happens to be spelled like it.
    pub(super) fn rank_of(&self, uri: &str) -> Option<i64> {
        self.raw
            .get(uri)
            .or_else(|| self.normalized.get(&norm_uri(uri)))
            .copied()
    }
}

/// Collect every JSON object in `value` in pre-order (objects only). Requires
/// `serde_json`'s `preserve_order` feature (enabled workspace-wide) so the
/// ranks derived from the walk are deterministic.
fn collect_objects<'a>(
    value: &'a serde_json::Value,
    out: &mut Vec<&'a serde_json::Map<String, serde_json::Value>>,
) {
    match value {
        serde_json::Value::Object(map) => {
            out.push(map);
            for child in map.values() {
                collect_objects(child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                collect_objects(child, out);
            }
        }
        _ => {}
    }
}

/// Open (backed-up) and recent (menubar) rank maps from
/// `globalStorage/storage.json`. A missing or unparseable file yields two
/// empty maps.
pub(super) fn load_storage_json_ranks(user_dir: &Path) -> (RankMap, RankMap) {
    let mut open_ranks = RankMap::default();
    let mut recent_ranks = RankMap::default();

    let storage_json = user_dir.join(GLOBAL_STORAGE_DIR).join(STORAGE_JSON_FILE);
    let Ok(text) = std::fs::read_to_string(&storage_json) else {
        return (open_ranks, recent_ranks);
    };
    let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (open_ranks, recent_ranks);
    };

    if let Some(backup) = data.get(KEY_BACKUP_WORKSPACES).filter(|v| v.is_object()) {
        let mut rank = FIRST_RANK;
        for (section, key) in [
            (KEY_BACKUP_FOLDERS, KEY_FOLDER_URI),
            (KEY_BACKUP_WORKSPACES_LIST, KEY_WORKSPACE_URI),
        ] {
            let Some(items) = backup.get(section).and_then(serde_json::Value::as_array) else {
                continue;
            };
            for item in items {
                if let Some(uri) = item.get(key).and_then(serde_json::Value::as_str) {
                    open_ranks.add(uri, rank);
                    rank += 1;
                }
            }
        }
    }

    let mut objects = Vec::new();
    if let Some(menubar) = data.get(KEY_MENUBAR_DATA) {
        collect_objects(menubar, &mut objects);
    }
    let mut rank = FIRST_RANK;
    for object in objects {
        if let Some(external) = object
            .get(KEY_URI)
            .filter(|v| v.is_object())
            .and_then(|uri| uri.get(KEY_EXTERNAL))
            .and_then(serde_json::Value::as_str)
        {
            recent_ranks.add(external, rank);
            rank += 1;
        }
        for key in [KEY_FOLDER_URI, KEY_WORKSPACE_URI] {
            if let Some(uri) = object.get(key).and_then(serde_json::Value::as_str) {
                recent_ranks.add(uri, rank);
                rank += 1;
            }
        }
    }

    (open_ranks, recent_ranks)
}

/// Recent-opened rank map from `globalStorage/state.vscdb`, ranked by the
/// stored list order. Any missing/unreadable layer yields an empty map.
pub(super) fn load_sqlite_recent_ranks(user_dir: &Path) -> RankMap {
    let mut ranks = RankMap::default();
    let db = user_dir.join(GLOBAL_STORAGE_DIR).join(STATE_VSCDB_FILE);
    if !db.exists() {
        return ranks;
    }
    let Some(conn) = open_sqlite_ro(&db) else {
        return ranks;
    };
    let value: Option<String> = conn
        .query_row(
            "select value from ItemTable where key = ?",
            [KEY_HISTORY_RECENT],
            |row| row.get::<_, String>(0),
        )
        .ok();
    let Some(text) = value else {
        return ranks;
    };
    let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) else {
        return ranks;
    };
    let Some(entries) = data.get(KEY_ENTRIES).and_then(serde_json::Value::as_array) else {
        return ranks;
    };
    for (index, item) in entries.iter().enumerate() {
        for key in [KEY_FOLDER_URI, KEY_WORKSPACE_URI] {
            if let Some(uri) = item.get(key).and_then(serde_json::Value::as_str) {
                ranks.add(uri, index as i64);
            }
        }
    }
    ranks
}

/// The open/recent rank maps for one editor's `User` directory: `storage.json`
/// first, with the SQLite recent list filling in whatever it did not cover.
pub(super) fn user_dir_ranks(user_dir: &Path) -> (RankMap, RankMap) {
    let (open, mut recent) = load_storage_json_ranks(user_dir);
    recent.merge_missing(load_sqlite_recent_ranks(user_dir));
    (open, recent)
}

/// Assign each entry's `open_rank`/`recent_rank` from its editor's rank maps.
/// Rank maps are computed once per `User` directory and reused.
pub(super) fn apply_ranks(entries: &mut [DevContainerEntry]) {
    let mut cache = std::collections::HashMap::<String, (RankMap, RankMap)>::new();

    for entry in entries.iter_mut() {
        // `storage_root` is `<user_dir>/workspaceStorage`; the rank sources
        // live one level up, next to it under `globalStorage`.
        let user_dir = Path::new(&entry.storage_root).parent().map_or_else(
            || std::path::PathBuf::from(&entry.storage_root),
            Path::to_path_buf,
        );
        let user_key = user_dir.to_string_lossy().into_owned();

        let (open_ranks, recent_ranks) = cache
            .entry(user_key)
            .or_insert_with(|| user_dir_ranks(&user_dir));

        entry.open_rank = open_ranks.rank_of(&entry.uri);
        entry.recent_rank = recent_ranks.rank_of(&entry.uri);
    }
}

/// Sort entries by [`DevContainerEntry::sort_key`] (open rank, recent rank,
/// then newest mtime).
pub(super) fn sort_entries(entries: &mut [DevContainerEntry]) {
    entries.sort_by(|a, b| {
        let (a_open, a_recent, a_mtime) = a.sort_key();
        let (b_open, b_recent, b_mtime) = b.sort_key();
        a_open
            .cmp(&b_open)
            .then(a_recent.cmp(&b_recent))
            .then(a_mtime.total_cmp(&b_mtime))
    });
}
