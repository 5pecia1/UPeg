//! The `change_rev` contract from the table in the parent module docs: local
//! history appends stay off the counter, and opening a current database never
//! waits for an active writer.

use std::time::{Duration, Instant};

use super::*;
use crate::store::ExecutionLogFilter;
use upeg_core::diagnostics::DiagnosticDraft;

fn diagnostic_draft() -> DiagnosticDraft {
    DiagnosticDraft {
        run_id: "run-1".to_string(),
        occurred_at_ms: 1,
        app_version: "test".to_string(),
        os: "test".to_string(),
        tool_id: Some("tool.fail".to_string()),
        source: "flutter".to_string(),
        cwd: None,
        project: None,
        error_code: "failed".to_string(),
        error_message: "boom".to_string(),
        status: "tool_error".to_string(),
        stdout: String::new(),
        stderr: String::new(),
        debug_context: None,
    }
}

/// A store whose counter is already live (`Some(1)`), so the append under test
/// is compared against a revision that a bump would visibly change.
fn store_with_published_revision() -> (Store, Option<u64>) {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_selection(&PegboardSelection::default())
        .expect("shared-state write");
    let revision = store.change_rev().expect("revision after shared write");
    assert_eq!(revision, Some(1), "a shared-state write announces itself");
    (store, revision)
}

#[test]
fn append_diagnostic_keeps_change_rev() {
    let (mut store, before) = store_with_published_revision();

    store
        .append_diagnostic(diagnostic_draft())
        .expect("append diagnostic");

    assert_eq!(
        store
            .read_diagnostics(None)
            .expect("read diagnostics")
            .len(),
        1,
        "the diagnostic row must be stored"
    );
    assert_eq!(store.change_rev().expect("revision after append"), before);
}

#[test]
fn append_execution_record_keeps_change_rev() {
    let (mut store, before) = store_with_published_revision();

    store
        .append_execution_record(&log_record("rev.contract", 1))
        .expect("append execution record");

    assert_eq!(
        store
            .read_execution_records(&ExecutionLogFilter::default())
            .expect("read execution log")
            .len(),
        1,
        "the execution record must be stored"
    );
    assert_eq!(store.change_rev().expect("revision after append"), before);
}

#[test]
fn current_schema_open_takes_no_write_lock() {
    let dir = temp_store_dir("rev-contract-open");
    let path = store_db_path(&dir);
    drop(Store::open_at(&path).expect("initialize store"));

    // Another connection holds the write lock for the whole second open.
    let writer = rusqlite::Connection::open(&path).expect("writer connection");
    writer
        .execute_batch("BEGIN IMMEDIATE")
        .expect("hold the write lock");

    // An open that queued for the lock would wait out the 5 second busy
    // timeout and then fail.
    let started = Instant::now();
    let reopened = Store::open_at(&path).expect("open while a writer is active");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "opening a current database must not wait for the writer: {:?}",
        started.elapsed()
    );

    drop(reopened);
    writer.execute_batch("ROLLBACK").expect("release the lock");
    let _ = std::fs::remove_dir_all(&dir);
}
