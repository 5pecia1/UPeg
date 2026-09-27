//! Process-global single-instance lock for the Flutter `upeg-desktop`
//! binary. PRD §5.9.
//!
//! Owned by the FRB lifecycle (`api/boot.rs::init_app` / `shutdown`).
//! The [`InstanceLock`] guard lives in a `OnceLock<Mutex<Option<...>>>`
//! so the caller does NOT hold a stack-local guard — `init_app` is a
//! single sync FRB call and the lock has to live for the whole process
//! up until `shutdown` drops it.
//!
//! Format: a one-line PID, atomically created. On start:
//!   1. try `create_new` — success → we own the slot.
//!   2. failure → read existing pid, `kill -0` it.
//!      - alive → refuse to start, return [`AcquireError::AlreadyRunning`].
//!      - dead/missing → remove the stale file and retry.
//!
//! Both the lock path and the liveness probe are borrowed rather than
//! reimplemented: the path from [`upeg_core::paths`], the probe from
//! [`upeg_cli::pid_alive`].

#![cfg(not(target_arch = "wasm32"))]

use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

// Single workspace owner of the process-liveness probe. `upeg-frb`
// already depends on `upeg-cli` because the desktop shell embeds the
// host (see `platform::host_bootstrap`), so the stale-lock check reuses
// that implementation instead of keeping a second copy in sync.
use upeg_cli::pid_alive;

/// On Drop the lock file is removed iff it still names our pid.
pub struct InstanceLock {
    path: PathBuf,
    pid: u32,
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        if let Ok(content) = std::fs::read_to_string(&self.path)
            && content.trim().parse::<u32>().ok() == Some(self.pid)
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Outcome of a single `acquire_at` attempt.
pub enum AcquireOutcome {
    Acquired(InstanceLock),
    AlreadyRunning { pid: u32 },
}

/// `acquire_global` failure modes — distinct from `AcquireOutcome` so
/// `boot::init_app` can map straight onto `FrbError` variants.
pub enum AcquireError {
    AlreadyRunning { pid: u32 },
    Io(std::io::Error),
}

static INSTANCE_LOCK: OnceLock<Mutex<Option<InstanceLock>>> = OnceLock::new();

/// Acquire the process-global lock. Idempotent: if the lock is already
/// held by this process, returns `Ok(())` without touching the
/// filesystem (subsequent `init_app` calls — e.g. after hot-reload in
/// development — must not fail).
pub fn acquire_global() -> Result<(), AcquireError> {
    let guard_cell = INSTANCE_LOCK.get_or_init(|| Mutex::new(None));
    let mut slot = guard_cell
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if slot.is_some() {
        return Ok(());
    }
    match acquire() {
        Ok(AcquireOutcome::Acquired(guard)) => {
            *slot = Some(guard);
            Ok(())
        }
        Ok(AcquireOutcome::AlreadyRunning { pid }) => Err(AcquireError::AlreadyRunning { pid }),
        Err(err) => Err(AcquireError::Io(err)),
    }
}

/// Release the process-global lock. Safe to call multiple times; no-op
/// if the lock has not been acquired.
pub fn drop_global() {
    let Some(cell) = INSTANCE_LOCK.get() else {
        return;
    };
    if let Ok(mut slot) = cell.lock() {
        *slot = None;
    }
}

/// Try to take the desktop instance lock at `~/.upeg/desktop.lock`.
/// Cleans up a stale lock from a crashed prior process.
pub fn acquire() -> std::io::Result<AcquireOutcome> {
    let path = lock_path().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::NotFound,
            "config root unavailable (set HOME/APPDATA)",
        )
    })?;
    acquire_at(path)
}

/// Path-injected variant of [`acquire`] — used by tests so they can
/// exercise the atomic-create / stale-cleanup logic without touching
/// the user's `~/.upeg/desktop.lock`.
pub fn acquire_at(path: PathBuf) -> std::io::Result<AcquireOutcome> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let my_pid = std::process::id();

    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                writeln!(file, "{my_pid}")?;
                return Ok(AcquireOutcome::Acquired(InstanceLock { path, pid: my_pid }));
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let other = read_pid(&path).unwrap_or(0);
                if other != 0 && pid_alive(other) {
                    return Ok(AcquireOutcome::AlreadyRunning { pid: other });
                }
                // Stale lock — remove and retry once.
                let _ = std::fs::remove_file(&path);
            }
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other(
        "could not acquire instance lock after stale cleanup",
    ))
}

/// `<config_root>/desktop.lock`. The path constant lives in
/// `upeg-core::paths` (the workspace's config-root owner), not in a
/// surface crate — see the crate-boundaries gate's module docs
/// (`upeg-core/tests/crate_boundaries.rs`).
fn lock_path() -> Option<PathBuf> {
    upeg_core::paths::desktop_lock_path()
}

fn read_pid(path: &std::path::Path) -> Option<u32> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut s = String::new();
    file.read_to_string(&mut s).ok()?;
    s.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_lock(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "upeg-frb-instance-test-{}-{}",
            std::process::id(),
            suffix
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{suffix}.lock"))
    }

    #[test]
    fn acquire_creates_lock_when_absent() {
        let path = tmp_lock("acquire-fresh");
        let _ = std::fs::remove_file(&path);
        let outcome = acquire_at(path.clone()).expect("acquire");
        match outcome {
            AcquireOutcome::Acquired(lock) => {
                assert!(path.exists(), "lock file must be created");
                drop(lock);
                assert!(!path.exists(), "RAII guard must remove the lock");
            }
            AcquireOutcome::AlreadyRunning { pid } => {
                panic!("fresh path must be acquirable; got AlreadyRunning(pid={pid})")
            }
        }
    }

    #[test]
    fn acquire_cleans_stale_lock_of_dead_pid() {
        let path = tmp_lock("stale");
        let _ = std::fs::write(&path, b"99999999\n");
        let outcome = acquire_at(path.clone()).expect("acquire");
        match outcome {
            AcquireOutcome::Acquired(lock) => {
                assert!(path.exists(), "must re-create after stale cleanup");
                drop(lock);
            }
            AcquireOutcome::AlreadyRunning { pid } => {
                panic!("stale lock must yield Acquired; got AlreadyRunning(pid={pid})")
            }
        }
    }

    #[test]
    fn acquire_is_refused_when_live_pid_owns_lock() {
        let path = tmp_lock("alive");
        let me = std::process::id();
        let _ = std::fs::write(&path, format!("{me}\n"));
        let outcome = acquire_at(path.clone()).expect("acquire");
        match outcome {
            AcquireOutcome::AlreadyRunning { pid } => {
                assert_eq!(pid, me);
            }
            AcquireOutcome::Acquired(_) => {
                panic!("must refuse when a live pid owns the lock");
            }
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn instance_lock_drop_removes_only_own_pid() {
        let path = tmp_lock("drop-other");
        let _ = std::fs::write(&path, "9999999\n");
        let lock = InstanceLock {
            path: path.clone(),
            pid: std::process::id(),
        };
        drop(lock);
        assert!(
            path.exists(),
            "Drop must not remove a lock owned by another pid"
        );
        let _ = std::fs::remove_file(&path);
    }
}
