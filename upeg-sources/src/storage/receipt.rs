use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Journal, Plan, Result, atomic_write, digest, exists, mode, refuse};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReceiptBinding {
    pub path: PathBuf,
    pub hash: String,
    pub mode: u32,
    pub source_toolkit: PathBuf,
    pub target_toolkit: PathBuf,
}

fn toolkit_entry(plan: &Plan) -> Option<&super::Entry> {
    plan.entries
        .iter()
        .find(|entry| entry.source == Path::new("toolkits/ecosystem.toml"))
}

pub(super) fn binding(plan: &Plan) -> Result<Option<ReceiptBinding>> {
    let Some(entry) = toolkit_entry(plan) else {
        return Ok(None);
    };
    let prefix = plan.ecosystem_prefix.as_ref().ok_or_else(|| refuse("ecosystem.toml relocation requires --ecosystem-prefix PREFIX to prove installer receipt ownership"))?;
    let path = prefix.join("share/ecosystem/install.json");
    if !exists(&path)? {
        return Err(refuse(
            "--ecosystem-prefix has no share/ecosystem/install.json receipt",
        ));
    }
    let bytes = fs::read(&path)?;
    let receipt: Value = serde_json::from_slice(&bytes)?;
    let source_toolkit = plan.source_root.join(&entry.source);
    let target_toolkit = plan.target_root.join(&entry.destination);
    let connections = receipt.get("connections");
    let connected = match connections {
        Some(Value::Array(connections)) => {
            connections.iter().any(|connection| connection == "upeg")
        }
        _ => false,
    };
    let old_key = source_toolkit
        .to_str()
        .ok_or_else(|| refuse("toolkit path is not UTF-8"))?;
    let new_key = target_toolkit
        .to_str()
        .ok_or_else(|| refuse("toolkit path is not UTF-8"))?;
    if receipt["managed_by"].as_str() != Some("ecosystem-installer-v3") {
        return Err(refuse(
            "unrecognized ecosystem receipt; update the ecosystem installer and retry",
        ));
    }
    if receipt["prefix"].as_str() != prefix.to_str()
        || !connected
        || receipt["upeg_toolkit"].as_str() != Some(old_key)
        || receipt["files"][old_key]["sha256"].as_str() != Some(entry.hash.as_str())
        || receipt["files"][old_key]["mode"].as_u64() != Some(u64::from(entry.mode))
        || receipt["files"].get(new_key).is_some()
    {
        return Err(refuse(
            "ecosystem receipt ownership/fingerprint mismatch; fix the selected --ecosystem-prefix without overwriting edits",
        ));
    }
    Ok(Some(ReceiptBinding {
        path,
        hash: digest(&bytes),
        mode: mode(&prefix.join("share/ecosystem/install.json"))?,
        source_toolkit,
        target_toolkit,
    }))
}

pub(super) fn lock(plan: &Plan) -> Result<Option<File>> {
    if plan.receipt.is_none() {
        return Ok(None);
    }
    #[cfg(not(unix))]
    {
        return Err(refuse(
            "ecosystem receipt migration requires the installer's Unix flock implementation on this platform",
        ));
    }
    #[cfg(unix)]
    {
        let prefix = plan
            .ecosystem_prefix
            .as_ref()
            .ok_or_else(|| refuse("missing ecosystem prefix"))?;
        let prefix = prefix
            .to_str()
            .ok_or_else(|| refuse("ecosystem prefix is not UTF-8"))?;
        let hash = digest(prefix.as_bytes());
        let temporary = ["TMPDIR", "TEMP", "TMP"]
            .iter()
            .find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
            .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
        let temporary = upeg_core::paths::normalized_root(&temporary)?;
        if !temporary.is_dir() {
            return Err(refuse(
                "installer temporary directory unavailable; set TMPDIR to an absolute existing directory",
            ));
        }
        let path = temporary.join(format!("ecosystem-install-{}.lock", &hash[..24]));
        upeg_core::paths::check_storage_path(&path)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;
        file.try_lock()
            .map_err(|_| refuse("ecosystem installation is busy"))?;
        Ok(Some(file))
    }
}

pub(super) fn read_before(plan: &Plan) -> Result<Option<Vec<u8>>> {
    let Some(binding) = &plan.receipt else {
        return Ok(None);
    };
    if self::binding(plan)?.as_ref() != Some(binding) {
        return Err(refuse("ecosystem receipt changed since planning"));
    }
    Ok(Some(fs::read(&binding.path)?))
}

pub(super) fn after_bytes(plan: &Plan) -> Result<Option<Vec<u8>>> {
    let Some(binding) = &plan.receipt else {
        return Ok(None);
    };
    let Some(bytes) = read_before(plan)? else {
        return Err(refuse("receipt is missing"));
    };
    let mut value: Value = serde_json::from_slice(&bytes)?;
    let source = binding
        .source_toolkit
        .to_str()
        .ok_or_else(|| refuse("source path is not UTF-8"))?;
    let target = binding
        .target_toolkit
        .to_str()
        .ok_or_else(|| refuse("target path is not UTF-8"))?;
    let files = value["files"]
        .as_object_mut()
        .ok_or_else(|| refuse("receipt.files is not an object"))?;
    let fingerprint = files
        .remove(source)
        .ok_or_else(|| refuse("receipt toolkit entry missing"))?;
    files.insert(target.into(), fingerprint);
    value["upeg_toolkit"] = Value::String(target.into());
    Ok(Some(serde_json::to_vec_pretty(&value)?))
}

fn current(journal: &Journal) -> Result<Option<Vec<u8>>> {
    let Some(binding) = &journal.plan.receipt else {
        return Ok(None);
    };
    upeg_core::paths::check_storage_path(&binding.path)?;
    if mode(&binding.path)? != binding.mode {
        return Err(refuse("receipt mode changed"));
    }
    Ok(Some(fs::read(&binding.path)?))
}

pub(super) fn activate(journal: &Journal) -> Result<()> {
    let Some(binding) = &journal.plan.receipt else {
        return Ok(());
    };
    let current = current(journal)?;
    if current == journal.receipt_after {
        return Ok(());
    }
    if current != journal.receipt_before {
        return Err(refuse("ecosystem receipt changed before activation"));
    }
    let bytes = journal
        .receipt_after
        .as_ref()
        .ok_or_else(|| refuse("receipt backup incomplete"))?;
    atomic_write(&binding.path, bytes, binding.mode)
}

pub(super) fn check_after(journal: &Journal) -> Result<()> {
    if current(journal)? != journal.receipt_after {
        return Err(refuse("ecosystem receipt changed after activation"));
    }
    Ok(())
}

pub(super) fn check_rollback(journal: &Journal, recovering: bool) -> Result<()> {
    let current = current(journal)?;
    if current != journal.receipt_after && !(recovering && current == journal.receipt_before) {
        return Err(refuse(
            "ecosystem receipt changed; rollback will not overwrite edits",
        ));
    }
    Ok(())
}

pub(super) fn rollback(journal: &Journal) -> Result<()> {
    let Some(binding) = &journal.plan.receipt else {
        return Ok(());
    };
    check_rollback(journal, true)?;
    if current(journal)? == journal.receipt_before {
        return Ok(());
    }
    atomic_write(
        &binding.path,
        journal
            .receipt_before
            .as_ref()
            .ok_or_else(|| refuse("receipt backup incomplete"))?,
        binding.mode,
    )
}
