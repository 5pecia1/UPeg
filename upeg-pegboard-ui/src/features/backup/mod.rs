//! Pure `EnvironmentBackup` model — surface-agnostic schema for the
//! upeg desktop backup pipeline.
//!
//! This module is intentionally I/O-free: it owns the on-disk shape
//! (struct + serde derives + `BACKUP_VERSION` const + version guard)
//! and nothing else. The FRB layer in `upeg-frb/src/api/backup.rs`
//! reads/writes storage and wraps these helpers behind the public
//! `export_backup` / `import_backup` calls.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use upeg_core::Placement;
use upeg_core::prefs::Tweaks;

use crate::features::boards::BoardData;

#[cfg(test)]
mod tests;

/// Schema version of the exported JSON. Bumping this drops all old
/// backups — exact-version matching is enforced by [`EnvironmentBackup::validate_version`].
///
/// v4: boards carry descriptions and Markdown instructions. An earlier
/// client would discard this nested guidance, so the version guard keeps
/// the full board context attached to its backup.
pub const BACKUP_VERSION: u32 = 4;

/// Snapshot of every piece of user-customizable desktop state.
///
/// `#[serde(deny_unknown_fields)]` is the exact-schema guard: parsing
/// a backup whose JSON carries unknown fields fails before any storage
/// write occurs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentBackup {
    pub version: u32,
    pub tweaks: Tweaks,
    /// Per-board coordinate placements, mirrored from `layouts::serialize_layouts`.
    pub layouts: BTreeMap<String, Vec<Placement>>,
    /// Memo content keyed by memo id (e.g. "scratch").
    pub memos: BTreeMap<String, String>,
    /// User's board (tab) list.
    pub boards: Vec<BoardData>,
}

impl EnvironmentBackup {
    /// Serialize as pretty JSON for human-readable backups. Output is
    /// stable: `BTreeMap` keeps map keys sorted, and `serde_json`
    /// serialises struct fields in declaration order.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| self.to_json())
    }

    /// Compact (single-line) JSON. Used by tests and reserved as part
    /// of the public API for callers that don't want pretty form.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    /// Parse JSON into an `EnvironmentBackup`. Returns `Err` (with the
    /// underlying `serde_json` error stringified) on malformed JSON, a
    /// missing-required-field, or an unknown-field error. Exact-schema
    /// parsing keeps import failures visible before any storage write.
    pub fn from_json(input: &str) -> Result<Self, String> {
        serde_json::from_str(input).map_err(|e| e.to_string())
    }

    /// Exact-version guard. Older or newer backups carry a different
    /// schema and must be refused before any storage write occurs.
    pub fn validate_version(&self) -> Result<(), String> {
        if self.version == BACKUP_VERSION {
            Ok(())
        } else {
            Err(format!(
                "backup version {} is not supported by this client (expected v{BACKUP_VERSION})",
                self.version,
            ))
        }
    }
}
