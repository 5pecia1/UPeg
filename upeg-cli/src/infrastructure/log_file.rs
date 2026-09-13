//! Single-file log appender with size-based rotation. PRD §5.5.
//!
//! Policy is deliberately minimal so daemonized hosts don't depend on
//! external log rotation tooling:
//!
//!   - One active log path (default `~/.upeg/upeg-http.log`).
//!   - At startup, if the file is ≥ [`MAX_BYTES`], rename it to `.1`
//!     (overwriting any prior `.1`) and start fresh.
//!   - During runtime: plain `O_APPEND`. No mid-run rotation — the
//!     resulting size cap is 2 × `MAX_BYTES` until the next start,
//!     which is fine for a debug log.
//!
//! Single rotation slot (`.1`) is enough for "what did the host log
//! since the last restart"; multi-slot retention belongs in operator
//! tooling (logrotate/journald), not in upeg.

use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// 10 MiB. PRD §5.5 / §9 default.
pub const MAX_BYTES: u64 = 10 * 1024 * 1024;

/// Open `path` for append, rotating the existing file to `.1` first
/// if it is at or beyond [`MAX_BYTES`]. Creates parent directories.
pub fn open_rotated(path: &Path) -> std::io::Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if file_size(path).unwrap_or(0) >= MAX_BYTES {
        let rotated = rotated_path(path);
        // Overwrite the prior `.1` deliberately — single-slot retention.
        let _ = std::fs::remove_file(&rotated);
        std::fs::rename(path, &rotated)?;
    }
    OpenOptions::new().create(true).append(true).open(path)
}

/// Read up to `lines` from the tail of `path`. Stable for "show me
/// the last N lines" without pulling a streaming-tail dependency.
pub fn tail(path: &Path, lines: usize) -> std::io::Result<Vec<String>> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let collected: Vec<&str> = content.lines().collect();
    let start = collected.len().saturating_sub(lines);
    Ok(collected[start..]
        .iter()
        .map(std::string::ToString::to_string)
        .collect())
}

fn file_size(path: &Path) -> std::io::Result<u64> {
    match std::fs::metadata(path) {
        Ok(meta) => Ok(meta.len()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
    }
}

fn rotated_path(path: &Path) -> PathBuf {
    let mut out = path.as_os_str().to_owned();
    out.push(".1");
    PathBuf::from(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("upeg-log-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn 누락된이면_fresh를_연다() {
        let path = tmp("fresh.log");
        let _ = std::fs::remove_file(&path);
        let mut f = open_rotated(&path).expect("open");
        writeln!(f, "hello").unwrap();
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn 상한_아래이면_그대로_덧붙인다() {
        let path = tmp("append.log");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, b"first\n").unwrap();
        let mut f = open_rotated(&path).expect("open");
        writeln!(f, "second").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("first"));
        assert!(content.contains("second"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn 에서_상한이면_순환한다를_검증한다() {
        let path = tmp("rotate.log");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(rotated_path(&path));
        // Write a file ≥ MAX_BYTES.
        std::fs::write(&path, vec![b'x'; MAX_BYTES as usize]).unwrap();
        let _ = open_rotated(&path).expect("open");
        assert!(rotated_path(&path).exists(), "must rotate to `.1`");
        assert_eq!(file_size(&path).unwrap(), 0, "new active log starts empty");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(rotated_path(&path));
    }

    #[test]
    fn tail는_마지막_엔_줄들을_반환한다() {
        let path = tmp("tail.log");
        std::fs::write(&path, b"a\nb\nc\nd\ne\n").unwrap();
        let lines = tail(&path, 3).unwrap();
        assert_eq!(lines, vec!["c", "d", "e"]);
        let _ = std::fs::remove_file(&path);
    }
}
