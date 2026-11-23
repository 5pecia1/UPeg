//! Private local queue shared by the native HTTP host and desktop tray.
//!
//! The queue contains notification titles and summaries that may include
//! tool output, so it uses a separate owner-only file on Unix. Tool
//! execution logs do not contain these bodies.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};
use upeg_core::paths::{self, StorageLease, StorageLocation};

use super::{BUSY_TIMEOUT, StoreError, epoch_millis_now, schema};

const RETENTION: i64 = 200;
const SCHEMA_VERSION: usize = 1;
const CREATE_QUEUE: &str = "CREATE TABLE notification_results (
    id            TEXT PRIMARY KEY NOT NULL,
    tool_id       TEXT NOT NULL,
    title         TEXT NOT NULL,
    summary       TEXT NOT NULL,
    success       INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL
);
";

/// One retained OS notification summary, with a stable acknowledgement id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationResultRecord {
    pub id: String,
    pub tool_id: String,
    pub title: String,
    pub summary: String,
    pub success: bool,
    pub created_at_ms: i64,
}

/// Queue the title and summary already used by the OS notification.
/// Keep the most recent unread summaries when the queue fills.
pub fn append_notification_result(
    tool_id: &str,
    title: &str,
    summary: &str,
    success: bool,
) -> Result<(), StoreError> {
    let lease = StorageLease::acquire()?;
    append_at(
        &queue_path(lease.location()),
        tool_id,
        title,
        summary,
        success,
    )
}

/// Read the unread marker without creating or migrating storage.
pub fn has_unread_notification_results() -> Result<bool, StoreError> {
    has_unread_at(&queue_path(&paths::resolve_storage()?))
}

/// Read the unread summaries, newest first, without acknowledging them.
pub fn list_notification_results() -> Result<Vec<NotificationResultRecord>, StoreError> {
    list_at(&queue_path(&paths::resolve_storage()?))
}

/// Remove precisely the acknowledged ids, preserving any newer results.
pub fn acknowledge_notification_results(ids: &[String]) -> Result<(), StoreError> {
    if ids.is_empty() {
        return Ok(());
    }
    let lease = StorageLease::acquire()?;
    acknowledge_at(&queue_path(lease.location()), ids)
}

fn queue_path(location: &StorageLocation) -> PathBuf {
    location
        .paths
        .data_dir
        .join(paths::NOTIFICATION_RESULTS_FILE)
}

fn open_at(path: &Path, flags: OpenFlags) -> Result<Option<Connection>, StoreError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() {
        return Err(paths::StoragePathError::Unsafe(path.to_path_buf()).into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if metadata.mode() & 0o077 != 0 || metadata.nlink() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "notification database must be an owner-only file",
            )
            .into());
        }
    }
    paths::check_storage_path(path)?;
    let connection = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NOFOLLOW)?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    Ok(Some(connection))
}

fn create_private_file(path: &Path) -> Result<(), StoreError> {
    paths::check_storage_path(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn append_at(
    path: &Path,
    tool_id: &str,
    title: &str,
    summary: &str,
    success: bool,
) -> Result<(), StoreError> {
    create_private_file(path)?;
    let mut connection = open_at(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    connection.pragma_update(None, "secure_delete", true)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if schema::user_version(&tx)? == 0 {
        tx.execute_batch(CREATE_QUEUE)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    let id = uuid::Uuid::now_v7().to_string();
    tx.execute(
        "INSERT INTO notification_results \
         (id, tool_id, title, summary, success, created_at_ms) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, tool_id, title, summary, success, epoch_millis_now()],
    )?;
    tx.execute(
        "DELETE FROM notification_results WHERE rowid IN (\
         SELECT rowid FROM notification_results ORDER BY rowid DESC LIMIT -1 OFFSET ?1)",
        [RETENTION],
    )?;
    tx.commit()?;
    Ok(())
}

fn has_unread_at(path: &Path) -> Result<bool, StoreError> {
    let Some(connection) = open_at(path, OpenFlags::SQLITE_OPEN_READ_ONLY)? else {
        return Ok(false);
    };
    if schema::user_version(&connection)? == 0 {
        return Ok(false);
    }
    Ok(connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM notification_results)",
        [],
        |row| row.get(0),
    )?)
}

fn list_at(path: &Path) -> Result<Vec<NotificationResultRecord>, StoreError> {
    let Some(connection) = open_at(path, OpenFlags::SQLITE_OPEN_READ_ONLY)? else {
        return Ok(Vec::new());
    };
    if schema::user_version(&connection)? == 0 {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "SELECT id, tool_id, title, summary, success, created_at_ms \
         FROM notification_results ORDER BY rowid DESC",
    )?;
    Ok(statement
        .query_map([], |row| {
            Ok(NotificationResultRecord {
                id: row.get(0)?,
                tool_id: row.get(1)?,
                title: row.get(2)?,
                summary: row.get(3)?,
                success: row.get(4)?,
                created_at_ms: row.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

fn acknowledge_at(path: &Path, ids: &[String]) -> Result<(), StoreError> {
    let Some(mut connection) = open_at(path, OpenFlags::SQLITE_OPEN_READ_WRITE)? else {
        return Ok(());
    };
    if schema::user_version(&connection)? == 0 {
        return Ok(());
    }
    connection.pragma_update(None, "secure_delete", true)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    {
        let mut statement = tx.prepare("DELETE FROM notification_results WHERE id = ?1")?;
        for id in ids {
            statement.execute([id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::temp_store_dir;

    #[test]
    fn unread_poll_does_not_create_storage() {
        let dir = temp_store_dir("notification-poll");
        let path = dir.join(paths::NOTIFICATION_RESULTS_FILE);
        assert!(!has_unread_at(&path).expect("missing queue"));
        assert!(list_at(&path).expect("empty queue").is_empty());
        assert!(!dir.exists());

        create_private_file(&path).expect("create a queue without writing a result");
        assert!(!has_unread_at(&path).expect("uninitialized queue"));
        assert!(list_at(&path).expect("empty queue").is_empty());
        fs::remove_dir_all(dir).expect("clean up database");
    }

    #[test]
    fn acknowledgement_leaves_results_from_another_connection_unread() {
        let dir = temp_store_dir("notification-ack");
        let path = dir.join(paths::NOTIFICATION_RESULTS_FILE);
        append_at(&path, "example", "upeg · example", "success", true)
            .expect("queue the first result");
        let seen = list_at(&path).expect("read snapshot");
        assert!(has_unread_at(&path).expect("still unread"));
        assert_eq!(seen[0].summary, "success");
        assert!(seen[0].success);

        append_at(&path, "example", "upeg · example (error)", "failure", false)
            .expect("queue a second result while viewing the first");
        let ids = vec![seen[0].id.clone()];
        acknowledge_at(&path, &ids).expect("acknowledge");
        acknowledge_at(&path, &ids).expect("retry acknowledgement");

        let unread = list_at(&path).expect("read again");
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].tool_id, "example");
        assert_eq!(unread[0].title, "upeg · example (error)");
        assert_eq!(unread[0].summary, "failure");
        assert!(!unread[0].success);
        acknowledge_at(&path, &[unread[0].id.clone()]).expect("acknowledge all");
        assert!(!has_unread_at(&path).expect("cleared"));
        fs::remove_dir_all(dir).expect("clean up database");
    }

    #[test]
    fn retention_keeps_the_newest_summaries() {
        let dir = temp_store_dir("notification-retention");
        let path = dir.join(paths::NOTIFICATION_RESULTS_FILE);
        for sequence in 0..=RETENTION {
            append_at(
                &path,
                "example",
                "upeg · example",
                &sequence.to_string(),
                true,
            )
            .expect("append");
        }

        let unread = list_at(&path).expect("read results");
        assert_eq!(i64::try_from(unread.len()), Ok(RETENTION));
        assert_eq!(
            unread.first().expect("newest").summary,
            RETENTION.to_string()
        );
        assert_eq!(unread.last().expect("oldest retained").summary, "1");
        fs::remove_dir_all(dir).expect("clean up database");
    }

    #[cfg(unix)]
    #[test]
    fn queue_is_private_and_does_not_follow_symlinks() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let dir = temp_store_dir("notification-private");
        let path = dir.join(paths::NOTIFICATION_RESULTS_FILE);
        create_private_file(&path).expect("create a private queue");
        assert_eq!(
            fs::metadata(&path)
                .expect("queue mode")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let alias = dir.join("alias.db");
        symlink(&path, &alias).expect("create symlink");
        assert!(append_at(&alias, "example", "title", "summary", true).is_err());
        assert!(has_unread_at(&alias).is_err());
        assert!(!has_unread_at(&path).expect("target is untouched"));
        fs::remove_dir_all(dir).expect("clean up database");
    }
}
