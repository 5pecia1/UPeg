//! Structural diff between two interface inventory JSON documents.
//!
//! Keyed by `(kind, id)` — schema v3 records one entry per contract, so the
//! surface set is a *value* that changes, not part of the identity.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use upeg_core::interface_inventory::INTERFACE_INVENTORY_SCHEMA_VERSION;

use crate::error::CliError;

/// JSON pointer of the surface set inside one inventory entry.
pub(super) const SURFACES_CHANGE_PATH: &str = "/surfaces";
const CHANGE_KIND_ADDED: &str = "added";
const CHANGE_KIND_REMOVED: &str = "removed";
const CHANGE_KIND_CHANGED: &str = "changed";
/// JSON pointers diffed as a whole value instead of element-by-element.
///
/// The surface set is a set, not an addressable array: reporting
/// `/surfaces/2 removed` is noise where `surfaces: cli, http, tui -> cli,
/// http` is the fact a reviewer needs.
const ATOMIC_DIFF_PATHS: &[&str] = &[SURFACES_CHANGE_PATH];

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InventoryDiff {
    pub(super) added: Vec<Value>,
    pub(super) removed: Vec<Value>,
    pub(super) changed: Vec<ChangedEntry>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ChangedEntry {
    pub(super) key: InventoryKey,
    pub(super) changes: Vec<DeepChange>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DeepChange {
    pub(super) path: String,
    pub(super) kind: String,
    pub(super) before: Value,
    pub(super) after: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct InventoryKey {
    pub(super) kind: String,
    pub(super) id: String,
}

impl InventoryDiff {
    pub(super) fn between(baseline: &Value, current: &Value) -> Result<Self, CliError> {
        let baseline_entries = index_entries(baseline)?;
        let current_entries = index_entries(current)?;

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();

        for (key, current_entry) in &current_entries {
            match baseline_entries.get(key) {
                None => added.push(current_entry.clone()),
                Some(baseline_entry) if baseline_entry != current_entry => {
                    changed.push(ChangedEntry {
                        key: key.clone(),
                        changes: diff_json_values(baseline_entry, current_entry, ""),
                    });
                }
                Some(_) => {}
            }
        }

        for (key, baseline_entry) in &baseline_entries {
            if !current_entries.contains_key(key) {
                removed.push(baseline_entry.clone());
            }
        }

        Ok(Self {
            added,
            removed,
            changed,
        })
    }

    pub(super) fn has_drift(&self) -> bool {
        !(self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty())
    }

    pub(super) fn to_json(&self) -> Value {
        json!({
            "schemaVersion": INTERFACE_INVENTORY_SCHEMA_VERSION,
            "added": self.added,
            "removed": self.removed,
            "changed": self.changed.iter().map(ChangedEntry::to_json).collect::<Vec<_>>(),
        })
    }
}

impl ChangedEntry {
    fn to_json(&self) -> Value {
        json!({
            "key": self.key.to_json(),
            "changes": self.changes.iter().map(DeepChange::to_json).collect::<Vec<_>>(),
        })
    }
}

impl DeepChange {
    fn to_json(&self) -> Value {
        json!({
            "path": self.path,
            "kind": self.kind,
            "before": self.before,
            "after": self.after,
        })
    }
}

pub(super) fn diff_json_values(before: &Value, after: &Value, path: &str) -> Vec<DeepChange> {
    if before == after {
        return Vec::new();
    }
    if ATOMIC_DIFF_PATHS.contains(&path) {
        return vec![DeepChange {
            path: path.to_string(),
            kind: CHANGE_KIND_CHANGED.to_string(),
            before: before.clone(),
            after: after.clone(),
        }];
    }

    match (before, after) {
        (Value::Object(before_object), Value::Object(after_object)) => {
            let mut keys = BTreeSet::new();
            keys.extend(before_object.keys());
            keys.extend(after_object.keys());

            let mut changes = Vec::new();
            for key in keys {
                let child_path = join_json_pointer_path(path, key);
                match (before_object.get(key), after_object.get(key)) {
                    (Some(before_value), Some(after_value)) => {
                        changes.extend(diff_json_values(before_value, after_value, &child_path));
                    }
                    (None, Some(after_value)) => changes.push(DeepChange {
                        path: child_path,
                        kind: CHANGE_KIND_ADDED.to_string(),
                        before: Value::Null,
                        after: after_value.clone(),
                    }),
                    (Some(before_value), None) => changes.push(DeepChange {
                        path: child_path,
                        kind: CHANGE_KIND_REMOVED.to_string(),
                        before: before_value.clone(),
                        after: Value::Null,
                    }),
                    (None, None) => {}
                }
            }
            changes
        }
        (Value::Array(before_array), Value::Array(after_array)) => {
            let mut changes = Vec::new();
            let common_len = before_array.len().min(after_array.len());
            for index in 0..common_len {
                let child_path = join_json_pointer_path(path, &index.to_string());
                changes.extend(diff_json_values(
                    &before_array[index],
                    &after_array[index],
                    &child_path,
                ));
            }
            for (index, after_value) in after_array.iter().enumerate().skip(common_len) {
                changes.push(DeepChange {
                    path: join_json_pointer_path(path, &index.to_string()),
                    kind: CHANGE_KIND_ADDED.to_string(),
                    before: Value::Null,
                    after: after_value.clone(),
                });
            }
            for (index, before_value) in before_array.iter().enumerate().skip(common_len) {
                changes.push(DeepChange {
                    path: join_json_pointer_path(path, &index.to_string()),
                    kind: CHANGE_KIND_REMOVED.to_string(),
                    before: before_value.clone(),
                    after: Value::Null,
                });
            }
            changes
        }
        _ => vec![DeepChange {
            path: path.to_string(),
            kind: CHANGE_KIND_CHANGED.to_string(),
            before: before.clone(),
            after: after.clone(),
        }],
    }
}

fn join_json_pointer_path(path: &str, key: &str) -> String {
    format!("{path}/{}", escape_json_pointer_key(key))
}

fn escape_json_pointer_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

impl InventoryKey {
    fn from_entry(entry: &Value) -> Result<Self, CliError> {
        Ok(Self {
            kind: required_string(entry, "kind")?.to_string(),
            id: required_string(entry, "id")?.to_string(),
        })
    }

    fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "id": self.id,
        })
    }
}

fn index_entries(value: &Value) -> Result<BTreeMap<InventoryKey, Value>, CliError> {
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::tool_failed("interface inventory entries must be an array"))?;
    let mut indexed = BTreeMap::new();
    for entry in entries {
        let key = InventoryKey::from_entry(entry)?;
        if indexed.insert(key.clone(), entry.clone()).is_some() {
            return Err(CliError::tool_failed(format!(
                "duplicate interface inventory entry {}/{}",
                key.kind, key.id
            )));
        }
    }
    Ok(indexed)
}

fn required_string<'a>(entry: &'a Value, field: &str) -> Result<&'a str, CliError> {
    entry.get(field).and_then(Value::as_str).ok_or_else(|| {
        CliError::tool_failed(format!(
            "interface inventory entry is missing string field {field}"
        ))
    })
}
