use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use upeg_core::paths::{self, MIGRATION_PENDING, STORAGE_LOCK, STORAGE_MARKER, STORAGE_TMP_PREFIX};

use super::{
    Entry, Journal, Plan, Result, check_markers, checked_entry, exists, inventory, plan_digest,
    receipt, refuse, validate_plan_shape,
};

pub(super) fn active_journal(target: &Path) -> Result<Journal> {
    load_journal(target, false)
}

pub(super) fn load_journal(target: &Path, recovery: bool) -> Result<Journal> {
    let target = paths::normalized_root(target)?;
    let inspected = paths::inspect_storage(&target)?;
    let id = if let Some(marker) = inspected.marker {
        paths::validate_marker(&marker, &target)?;
        marker["migration_id"]
            .as_str()
            .ok_or_else(|| refuse("target has no migration journal"))?
            .to_string()
    } else if recovery && inspected.pending {
        let pending: Value = serde_json::from_slice(&fs::read(target.join(MIGRATION_PENDING))?)?;
        pending["plan_hash"]
            .as_str()
            .ok_or_else(|| refuse("invalid recovery pointer"))?
            .to_string()
    } else {
        return Err(refuse("target has no active marker"));
    };
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(refuse("invalid migration id"));
    }
    let path = target
        .join("state/migrations")
        .join(&id)
        .join("journal.json");
    paths::check_storage_path(&path)?;
    let journal: Journal = serde_json::from_slice(&fs::read(path)?)?;
    if journal.plan.target_root != target
        || journal.plan.plan_hash != id
        || plan_digest(&journal.plan)? != id
    {
        return Err(refuse("invalid migration journal"));
    }
    validate_plan_shape(&journal.plan)?;
    check_markers(&journal)?;
    Ok(journal)
}

pub(super) fn transient(entry: &Entry) -> bool {
    entry.destination.starts_with("cache")
        || entry.destination.starts_with("runtime")
        || entry.destination == Path::new("state/upeg-http.log")
}

pub(super) fn verify_entries(plan: &Plan, ignore_transient: bool) -> Result<()> {
    for entry in &plan.entries {
        if ignore_transient && transient(entry) {
            continue;
        }
        // A source-side mismatch after activation means something (almost
        // always an old, split-v2-unaware binary) is still writing to the
        // pre-migration location; call that out explicitly rather than
        // reporting a generic content-changed error indistinguishable from a
        // target-side problem.
        checked_entry(&plan.source_root, &entry.source, entry).map_err(|error| {
            refuse(format!(
                "source modified after activation: {} ({error}); stop the old binary before continuing",
                plan.source_root.join(&entry.source).display()
            ))
        })?;
        checked_entry(&plan.target_root, &entry.destination, entry)?;
    }
    Ok(())
}

/// Non-fatal probe used by `status()`: which non-transient source entries (if
/// any) no longer match the active migration's recorded plan. An empty
/// result means either there is no active migration, or the source has not
/// been touched since cutover.
pub(super) fn source_drift(plan: &Plan) -> Vec<PathBuf> {
    plan.entries
        .iter()
        .filter(|entry| !transient(entry))
        .filter(|entry| checked_entry(&plan.source_root, &entry.source, entry).is_err())
        .map(|entry| plan.source_root.join(&entry.source))
        .collect()
}

pub fn verify(target: &Path) -> Result<Value> {
    let journal = active_journal(target)?;
    verify_entries(&journal.plan, true)?;
    receipt::check_after(&journal)?;
    Ok(
        json!({"schema_version":1,"app":"upeg","status":"verified","target_root":journal.plan.target_root,"migration_id":journal.plan.plan_hash}),
    )
}

pub(super) fn check_target_inventory(plan: &Plan, ignore_transient: bool) -> Result<()> {
    if plan.source_root != plan.target_root {
        for child in fs::read_dir(&plan.target_root)? {
            let child = child?;
            let name = child.file_name();
            if name.to_string_lossy().starts_with(STORAGE_TMP_PREFIX) {
                continue;
            }
            if ![
                "config",
                "data",
                "state",
                "cache",
                "runtime",
                STORAGE_LOCK,
                STORAGE_MARKER,
                MIGRATION_PENDING,
            ]
            .iter()
            .any(|allowed| name == *allowed)
            {
                return Err(refuse(format!(
                    "new destination content: {}",
                    child.path().display()
                )));
            }
        }
    }
    for role in ["config", "data", "state", "cache", "runtime"] {
        if ignore_transient && matches!(role, "cache" | "runtime") {
            continue;
        }
        let root = plan.target_root.join(role);
        if !exists(&root)? {
            continue;
        }
        if role != "state"
            && !plan
                .entries
                .iter()
                .any(|entry| entry.destination.starts_with(role))
        {
            return Err(refuse(format!(
                "unowned destination directory: {}",
                root.display()
            )));
        }
        let mut files = Vec::new();
        inventory(
            &plan.target_root,
            Path::new(role),
            Path::new(role),
            &mut files,
        )?;
        for file in files {
            let path = &file.source;
            if path == Path::new(role)
                || path.starts_with("state/migrations")
                || (ignore_transient && path == Path::new("state/upeg-http.log"))
                || path == Path::new("data/upeg.db-wal")
                || path == Path::new("data/upeg.db-shm")
            {
                continue;
            }
            if !plan.entries.iter().any(|entry| entry.destination == *path) {
                return Err(refuse(format!(
                    "new destination content: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}
