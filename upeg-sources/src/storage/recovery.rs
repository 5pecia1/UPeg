use super::*;

fn destination(plan: &Plan, relative: &Path, recovering: bool) -> Result<PathBuf> {
    let active = plan.target_root.join(relative);
    let inactive = journal_dir(plan).join("inactive").join(relative);
    if recovering && plan.source_root == plan.target_root && exists(&inactive)? {
        if exists(&active)? {
            return Err(refuse("new destination content appeared during rollback"));
        }
        return Ok(inactive);
    }
    Ok(active)
}

pub fn rollback(target: &Path, yes: bool, quiesced: bool) -> Result<Value> {
    if !yes || !quiesced {
        return Err(refuse("rollback requires --yes --quiesced"));
    }
    let journal = verify::load_journal(target, true)?;
    // A migration that never reached "active" was never cut over: nothing at
    // the source was touched, so there is nothing to roll back, only our own
    // in-progress/never-activated copies to abort and discard. Route those
    // phases (and their own idempotent-retry terminal states) to `abort`
    // instead of the post-activation rollback below, which assumes the
    // marker/receipt swap already happened.
    if matches!(
        journal.phase.as_str(),
        "prepared" | "copied" | "receipt" | "abort_pending" | "aborted"
    ) {
        return abort(journal, yes, quiesced);
    }
    let mut journal = journal;
    if !matches!(
        journal.phase.as_str(),
        "active" | "rollback_pending" | "rolled_back"
    ) {
        return Err(refuse("finish applying the recorded plan before rollback"));
    }
    let recovering = journal.phase != "active";
    let plan = journal.plan.clone();
    let _leases = locks(&plan)?;
    let _ecosystem_lock = receipt::lock(&plan)?;
    refuse_runtime(&plan.source_root)?;
    refuse_runtime(&plan.target_root)?;
    let _source_writer = sqlite::writer_exclusion(&plan.source_root.join("upeg.db"))?;
    let target_data = destination(&plan, Path::new("data/upeg.db"), recovering)?;
    let _target_writer = sqlite::writer_exclusion(&target_data)?;
    for entry in plan
        .entries
        .iter()
        .filter(|entry| !verify::transient(entry))
    {
        checked_entry(&plan.source_root, &entry.source, entry)?;
        checked_entry(
            Path::new(""),
            &destination(&plan, &entry.destination, recovering)?,
            entry,
        )?;
    }
    let mut current = plan.clone();
    collect_source(&mut current)?;
    if current
        .entries
        .iter()
        .filter(|entry| !verify::transient(entry))
        .ne(plan
            .entries
            .iter()
            .filter(|entry| !verify::transient(entry)))
        || current.unclassified_hashes != plan.unclassified_hashes
    {
        return Err(refuse("source changed since cutover"));
    }
    verify::check_target_inventory(&plan, true)?;
    receipt::check_rollback(&journal, recovering)?;
    journal.phase = "rollback_pending".into();
    save_journal(&journal)?;
    if !paths::inspect_storage(&plan.target_root)?.inactive {
        pending(&plan.target_root, &plan)?;
    }
    if plan.source_root != plan.target_root {
        pending(&plan.source_root, &plan)?;
    }
    receipt::rollback(&journal)?;
    let marker = plan.target_root.join(STORAGE_MARKER);
    if exists(&marker)? {
        fs::remove_file(marker)?;
    }
    if plan.source_root == plan.target_root {
        let inactive = journal_dir(&plan).join("inactive");
        private_dir(&inactive)?;
        for role in ["config", "data", "cache", "runtime"] {
            let path = plan.target_root.join(role);
            if exists(&path)? {
                fs::rename(path, inactive.join(role))?;
            }
        }
        let log = plan.target_root.join("state/upeg-http.log");
        if exists(&log)? {
            fs::rename(log, inactive.join("upeg-http.log"))?;
        }
    } else {
        let marker = plan.source_root.join(STORAGE_MARKER);
        if exists(&marker)? {
            fs::remove_file(marker)?;
        }
    }
    journal.phase = "rolled_back".into();
    save_journal(&journal)?;
    if plan.source_root != plan.target_root {
        atomic_write(
            &plan.target_root.join(MIGRATION_PENDING),
            &serde_json::to_vec(&json!({
                "schema_version":1,"app":"upeg","status":"inactive","target":plan.target_root,"plan_hash":plan.plan_hash,
            }))?,
            0o600,
        )?;
    }
    let pending = plan.source_root.join(MIGRATION_PENDING);
    if exists(&pending)? {
        fs::remove_file(pending)?;
    }
    Ok(
        json!({"schema_version":1,"app":"upeg","status":"rolled_back","source_root":plan.source_root,"preserved_target":journal_dir(&plan)}),
    )
}

/// Abort a migration whose journal never reached "active" (`prepared`,
/// `copied`, or `receipt`), or resume an interrupted abort (`abort_pending`,
/// `aborted`). `copy_entries` never writes to the source, so an abort only
/// needs to: restore the ecosystem receipt if activation touched it, put
/// same-root published copies out of the way (never delete user data —
/// they're our own copies, and are preserved under the journal directory for
/// inspection), drop the pending marker(s), and mark the plan retired so a
/// fresh `plan()` for the same root is not blocked by it (see
/// `state_dir_is_only_retired_migrations`). Idempotent: every step is guarded
/// so a retried abort after a crash converges to the same end state.
fn abort(mut journal: Journal, yes: bool, quiesced: bool) -> Result<Value> {
    if !yes || !quiesced {
        return Err(refuse("rollback requires --yes --quiesced"));
    }
    let plan = journal.plan.clone();
    let _leases = locks(&plan)?;
    let _ecosystem_lock = receipt::lock(&plan)?;
    refuse_runtime(&plan.source_root)?;
    refuse_runtime(&plan.target_root)?;
    // Restores the receipt to receipt_before only if it currently equals
    // receipt_after (i.e. activation actually ran); a no-op if activation
    // never touched it or a previous abort attempt already restored it.
    receipt::rollback(&journal)?;
    journal.phase = "abort_pending".into();
    save_journal(&journal)?;
    if plan.source_root == plan.target_root {
        let aborted = journal_dir(&plan).join("aborted");
        private_dir(&aborted)?;
        for role in ["config", "data", "cache", "runtime"] {
            let path = plan.target_root.join(role);
            let stashed = aborted.join(role);
            if exists(&path)? && !exists(&stashed)? {
                fs::rename(&path, &stashed)?;
            }
        }
        let log = plan.target_root.join("state/upeg-http.log");
        let stashed_log = aborted.join("upeg-http.log");
        if exists(&log)? && !exists(&stashed_log)? {
            fs::rename(&log, &stashed_log)?;
        }
    }
    for root in [&plan.target_root, &plan.source_root] {
        let pending_path = root.join(MIGRATION_PENDING);
        if exists(&pending_path)? {
            fs::remove_file(pending_path)?;
        }
    }
    if plan.source_root != plan.target_root {
        atomic_write(
            &plan.target_root.join(MIGRATION_PENDING),
            &serde_json::to_vec(&json!({
                "schema_version":1,"app":"upeg","status":"inactive","target":plan.target_root,"plan_hash":plan.plan_hash,
            }))?,
            0o600,
        )?;
    }
    journal.phase = "aborted".into();
    save_journal(&journal)?;
    Ok(
        json!({"schema_version":1,"app":"upeg","status":"aborted","source_root":plan.source_root,"preserved_target":journal_dir(&plan)}),
    )
}
