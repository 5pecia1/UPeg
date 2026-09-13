//! Discovery file (`server.json`). PRD §5.3.
//!
//! Single source of truth for "who hosts the HTTP server right now":
//! endpoint, mcp endpoint, bearer token, pid, start time. Same-OS-user
//! verification is delegated to filesystem permissions (Unix `0600`,
//! Windows inherits user-only ACL from the config root). No keychain
//! pairing UX.
//!
//! Lifecycle (PRD §5.2 Host Precedence):
//!   - Host calls [`publish`] right after binding an ephemeral port.
//!     `create_new` semantics give mutual exclusion; no separate lock file.
//!   - Any surface calls [`read_reachable`] at startup; presence + a
//!     `GET /healthz` round-trip decides "attach as client" vs "host".
//!   - The returned [`DiscoveryGuard`] removes the file on drop, but
//!     only if it still names the same pid (so a concurrent restart
//!     doesn't accidentally clean up the new host's file).

use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::paths;
use super::process::pid_alive;

/// `/healthz` probe budget — aggressive so a hung host doesn't stall a
/// surface's startup path (PRD §5.3). Connect and read are separate.
///
/// `pub(crate)` because `attach::healthz_json` reads the same route's
/// BODY and must spend the same budget: a `/healthz` read is a probe
/// wherever it happens, and the generic 30s attach deadline (sized for
/// a tool that is actually running) would let a hung host stall `upeg
/// host status` for a hundred times the budget the liveness probe next
/// to it accepts.
pub(crate) const HEALTH_CONNECT_TIMEOUT_MS: u64 = 300;
pub(crate) const HEALTH_READ_TIMEOUT_MS: u64 = 300;
const HTTP_RESPONSE_PREFIX: &[u8; 5] = b"HTTP/";

/// Where a reachable host came from. `Explicit` = a user-invoked
/// `upeg host start` (foreground or `--daemon`). `Embedded` = the
/// desktop's own in-process host — never pid-killed, since that IS the
/// desktop process. Embedders read this back through
/// [`crate::current_host`] to tell "my own embed" apart from a separate
/// host process. A `server.json` with no `origin` key deserializes to
/// the conservative default `Explicit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HostOrigin {
    #[default]
    Explicit,
    Embedded,
}

/// On-disk schema for `server.json`. Stable; clients may parse this
/// JSON directly. PRD §5.3.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerInfo {
    pub endpoint: String,
    pub mcp_endpoint: String,
    pub token: String,
    pub pid: u32,
    pub started_at_ms: u64,
    /// How this host came up (RC-6 teardown policy). `#[serde(default)]`
    /// so a legacy file with no `origin` key parses as `Explicit`.
    #[serde(default)]
    pub origin: HostOrigin,
}

impl ServerInfo {
    pub fn new(endpoint: impl Into<String>, token: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        let mcp = format!("{}/mcp", endpoint.trim_end_matches('/'));
        Self {
            endpoint,
            mcp_endpoint: mcp,
            token: token.into(),
            pid: std::process::id(),
            started_at_ms: now_ms(),
            origin: HostOrigin::default(),
        }
    }

    /// Same as [`Self::new`] but with an explicit [`HostOrigin`] —
    /// used by every bring-up lane that knows how it came up (see
    /// `surfaces::http::serve_inner`).
    pub fn with_origin(
        endpoint: impl Into<String>,
        token: impl Into<String>,
        origin: HostOrigin,
    ) -> Self {
        Self {
            origin,
            ..Self::new(endpoint, token)
        }
    }
}

/// RAII guard returned by [`publish`]. Drops remove the discovery file
/// iff it still names our pid. Wrap in `let _g = publish(...)?;` at
/// the top of any host surface and shutdown cleanup is free.
pub struct DiscoveryGuard {
    path: PathBuf,
    pid: u32,
}

impl Drop for DiscoveryGuard {
    fn drop(&mut self) {
        // Only remove if the file still names our pid. A concurrent
        // restart could already have replaced it — don't pull the rug
        // out from under the new host.
        if let Ok(info) = read_from(&self.path)
            && info.pid == self.pid
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Read the discovery file when present. Doesn't validate liveness —
/// use [`read_reachable`] for the alive-or-stale decision.
pub fn read() -> Option<ServerInfo> {
    let path = paths::server_json_path()?;
    read_from(&path).ok()
}

/// Read + verify the recorded endpoint is actually reachable. When the
/// endpoint does not answer `/healthz`, the discovery file is removed
/// ONLY if the recorded process is no longer alive (see [`should_reap`]);
/// a live-but-transiently-unresponsive host's file is preserved so a
/// still-booting or busy host is never orphaned (PRD §5.3 multi-signal
/// staleness). Returns `None` whenever the host is not reachable now, so
/// callers proceed to "no host" code paths.
pub fn read_reachable() -> Option<ServerInfo> {
    let info = read()?;
    if health_check(&info.endpoint) {
        return Some(info);
    }
    // Not answering. Reap the record ONLY if the process is genuinely
    // gone — never pull the rug from under a live-but-transiently-
    // unresponsive host (PRD §5.3 multi-signal staleness).
    if should_reap(&info, false)
        && let Some(path) = paths::server_json_path()
    {
        let _ = std::fs::remove_file(&path);
    }
    None
}

/// Publish `ServerInfo` atomically. Returns `Err(AlreadyExists)` if a
/// (potentially live) host already published — caller should re-run
/// [`read_reachable`] to decide attach-or-bail.
///
/// Unix: file is chmod'd to `0600`. Windows: relies on the user's
/// `%APPDATA%` ACL being user-only by default.
pub fn publish(info: &ServerInfo) -> std::io::Result<DiscoveryGuard> {
    let path = paths::server_json_path().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::NotFound,
            "config root not available; set HOME or APPDATA",
        )
    })?;
    paths::ensure_config_root()?;

    let content = serde_json::to_string_pretty(info).map_err(|e| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("serialise server.json: {e}"),
        )
    })?;

    // Atomic publish: `create_new` rejects an existing file. Two
    // hosts racing produce one winner and one AlreadyExists error.
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            // Probe: maybe the existing file is stale and we can clean it
            // up before the caller retries.
            if read_reachable().is_none() {
                // `read_reachable` removed the stale file; retry once.
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)?
            } else {
                return Err(e);
            }
        }
        Err(e) => return Err(e),
    };
    file.write_all(content.as_bytes())?;
    file.flush()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = file.metadata()?.permissions();
        perm.set_mode(0o600);
        std::fs::set_permissions(&path, perm)?;
    }

    Ok(DiscoveryGuard {
        path,
        pid: info.pid,
    })
}

/// Force-remove the discovery file. Used by `upeg http stop` after
/// signalling the host process; idempotent.
pub fn clear() -> std::io::Result<()> {
    let Some(path) = paths::server_json_path() else {
        return Ok(());
    };
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn read_from(path: &Path) -> std::io::Result<ServerInfo> {
    let mut file = std::fs::File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    serde_json::from_str(&buf)
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, format!("parse server.json: {e}")))
}

/// Whether an unreachable discovery record is safe to delete. Staleness
/// is multi-signal (PRD §5.3): a record is reaped ONLY when the endpoint
/// is unreachable AND its pid is gone. A live-but-slow / still-booting
/// host is transiently unreachable, not stale — deleting its file would
/// orphan a running daemon (single-source-of-truth invariant).
///
/// Future hardening: also compare `started_at_ms` against the OS process
/// start-time to defend against pid reuse (PRD §5.3). Out of scope here.
fn should_reap(info: &ServerInfo, reachable: bool) -> bool {
    !reachable && !pid_alive(info.pid)
}

/// Format `host:port` as a socket-address string that always parses as
/// [`std::net::SocketAddr`]. IPv6 literals (host contains `':'`) are
/// bracketed — `parse_endpoint` strips the brackets, so rebuilding a bare
/// `::1:port` would fail to parse and make a live IPv6 host look dead.
fn socket_addr_str(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn read_http_response_prefix(reader: &mut impl Read) -> io::Result<bool> {
    let mut prefix = [0_u8; HTTP_RESPONSE_PREFIX.len()];
    reader.read_exact(&mut prefix)?;
    Ok(&prefix == HTTP_RESPONSE_PREFIX)
}

/// Lightweight `/healthz` probe. Doesn't require the bearer token —
/// the route is intentionally unauthenticated so liveness checks don't
/// need to know about the token. Any HTTP response (200 or 401) proves
/// the listener is alive; complete connection failure proves it isn't.
///
/// Times out aggressively (see `HEALTH_CONNECT_TIMEOUT_MS` /
/// `HEALTH_READ_TIMEOUT_MS`) so a hung host doesn't stall every
/// surface's startup path.
fn health_check(endpoint: &str) -> bool {
    let Some((host, port, path_prefix)) = parse_endpoint(endpoint) else {
        return false;
    };
    let authority = socket_addr_str(&host, port);
    let Ok(mut stream) = TcpStream::connect_timeout(
        &match authority.parse() {
            Ok(a) => a,
            Err(_) => return false,
        },
        Duration::from_millis(HEALTH_CONNECT_TIMEOUT_MS),
    ) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(HEALTH_READ_TIMEOUT_MS)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(HEALTH_READ_TIMEOUT_MS)));

    let path = if path_prefix.is_empty() {
        "/healthz".to_string()
    } else {
        format!("{path_prefix}/healthz")
    };
    let req = format!("GET {path} HTTP/1.0\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    matches!(read_http_response_prefix(&mut stream), Ok(true))
}

/// Parse `http://host:port[/prefix]` → `(host, port, prefix)`. Returns
/// `None` for non-http schemes or malformed input. Keeps the probe code
/// in this module dependency-free (no `url` crate needed).
pub(crate) fn parse_endpoint(endpoint: &str) -> Option<(String, u16, String)> {
    let rest = endpoint.strip_prefix("http://")?;
    let (authority, prefix) = match rest.find('/') {
        Some(idx) => (&rest[..idx], rest[idx..].trim_end_matches('/').to_string()),
        None => (rest, String::new()),
    };
    // IPv6 literal: `[::1]:port`
    let (host, port) = if let Some(end) = authority.strip_prefix('[')
        && let Some(close) = end.find(']')
    {
        let host = &end[..close];
        let port_str = end[close + 1..].strip_prefix(':')?;
        (host.to_string(), port_str.parse().ok()?)
    } else {
        let (host, port_str) = authority.rsplit_once(':')?;

        (host.to_string(), port_str.parse().ok()?)
    };
    Some((host, port, prefix))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct InterruptOnce<R> {
        inner: R,
        interrupt_pending: bool,
    }

    impl<R: Read> Read for InterruptOnce<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.interrupt_pending {
                self.interrupt_pending = false;
                return Err(std::io::Error::from(ErrorKind::Interrupted));
            }
            self.inner.read(buf)
        }
    }

    #[test]
    fn 엔드포인트_파싱은_ipv4_ipv6_접두사를_모두_처리한다() {
        assert_eq!(
            parse_endpoint("http://127.0.0.1:49317"),
            Some(("127.0.0.1".into(), 49317, String::new()))
        );
        assert_eq!(
            parse_endpoint("http://[::1]:7000/api"),
            Some(("::1".into(), 7000, "/api".into()))
        );
        assert_eq!(parse_endpoint("https://x:80"), None);
        assert_eq!(parse_endpoint("garbage"), None);
    }

    #[test]
    fn 서버_정보는_mcp_엔드포인트를_채운다() {
        let info = ServerInfo::new("http://127.0.0.1:1234", "tok");
        assert_eq!(info.mcp_endpoint, "http://127.0.0.1:1234/mcp");
        assert_eq!(info.pid, std::process::id());
        assert!(info.started_at_ms > 0);
    }

    #[test]
    fn 상태확인_확인_대상으로_죽은_port는_거짓이다() {
        // Bind a listener, immediately drop, leaving the port unused.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        // Sleep a tick so the kernel surely releases the port.
        std::thread::sleep(Duration::from_millis(20));
        assert!(!health_check(&format!("http://127.0.0.1:{port}")));
    }

    #[test]
    fn http_응답_접두사_읽기는_interrupted와_partial_read를_처리한다() {
        // Given: 첫 read는 signal로 중단되고 HTTP 접두사는 두 chunk로 나뉜다.
        let chunks = std::io::Cursor::new(b"HT").chain(std::io::Cursor::new(b"TP/"));
        let mut reader = InterruptOnce {
            inner: chunks,
            interrupt_pending: true,
        };

        // When: health-check 응답 접두사를 읽는다.
        let result = read_http_response_prefix(&mut reader);

        // Then: Interrupted를 재시도하고 partial read를 합쳐 HTTP를 인식한다.
        assert!(matches!(result, Ok(true)));
    }

    #[test]
    fn discovery_guard_만은_자신의_pid를_제거한다() {
        // Smoke test: a different pid's file must not be deleted on drop.
        let tmp = std::env::temp_dir().join("upeg-discovery-test.json");
        let other = ServerInfo {
            endpoint: "http://127.0.0.1:1".into(),
            mcp_endpoint: "http://127.0.0.1:1/mcp".into(),
            token: "x".into(),
            pid: std::process::id().wrapping_add(7777),
            started_at_ms: 1,
            origin: HostOrigin::default(),
        };
        std::fs::write(&tmp, serde_json::to_string(&other).unwrap()).unwrap();
        let guard = DiscoveryGuard {
            path: tmp.clone(),
            pid: std::process::id(),
        };
        drop(guard);
        assert!(tmp.exists(), "must not delete a file owned by another pid");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn socket_addr_str는_ipv6를_브래킷으로_감싼다() {
        use std::net::SocketAddr;
        // IPv4 stays bare and parses.
        assert_eq!(socket_addr_str("127.0.0.1", 49317), "127.0.0.1:49317");
        assert!(
            socket_addr_str("127.0.0.1", 49317)
                .parse::<SocketAddr>()
                .is_ok()
        );
        // IPv6 literal MUST be bracketed, otherwise SocketAddr::parse fails
        // ("::1:7000" is ambiguous). This is the bug that made every
        // IPv6-bound host look dead.
        assert_eq!(socket_addr_str("::1", 7000), "[::1]:7000");
        assert!(socket_addr_str("::1", 7000).parse::<SocketAddr>().is_ok());
        // The old broken reconstruction must NOT parse — pins the regression.
        assert!("::1:7000".parse::<SocketAddr>().is_err());
    }

    #[test]
    fn should_reap는_live_pid_레코드는_보존한다() {
        // Our own pid is definitely alive; an unreachable-but-live host is
        // transiently unresponsive (starting/busy/IPv6), NOT stale.
        let info = ServerInfo {
            endpoint: "http://127.0.0.1:1".into(),
            mcp_endpoint: "http://127.0.0.1:1/mcp".into(),
            token: "x".into(),
            pid: std::process::id(),
            started_at_ms: 1,
            origin: HostOrigin::default(),
        };
        assert!(!should_reap(&info, false), "live pid must never be reaped");
        assert!(!should_reap(&info, true), "reachable is never reaped");
    }

    #[test]
    fn should_reap는_dead_pid_레코드만_stale로_본다() {
        // 99_999_999 is above the default max pid on Linux and not a real
        // process — the same sentinel instance_lock's tests use.
        let info = ServerInfo {
            endpoint: "http://127.0.0.1:1".into(),
            mcp_endpoint: "http://127.0.0.1:1/mcp".into(),
            token: "x".into(),
            pid: 99_999_999,
            started_at_ms: 1,
            origin: HostOrigin::default(),
        };
        assert!(should_reap(&info, false), "dead pid + unreachable is stale");
        assert!(
            !should_reap(&info, true),
            "a reachable host is never reaped"
        );
    }

    #[test]
    fn server_info는_origin을_라운드트립하고_레거시는_explicit로_기본한다() {
        for origin in [HostOrigin::Explicit, HostOrigin::Embedded] {
            let info = ServerInfo {
                endpoint: "http://127.0.0.1:1".into(),
                mcp_endpoint: "http://127.0.0.1:1/mcp".into(),
                token: "x".into(),
                pid: 1,
                started_at_ms: 1,
                origin,
            };
            let json = serde_json::to_string(&info).expect("serialize");
            let back: ServerInfo = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back.origin, origin, "origin must round-trip through JSON");
        }

        // Legacy `server.json` written before this field existed: no
        // `origin` key at all. Must deserialize, defaulting to the
        // conservative `Explicit` (never auto-killed).
        let legacy = r#"{"endpoint":"http://127.0.0.1:1","mcp_endpoint":"http://127.0.0.1:1/mcp","token":"x","pid":1,"started_at_ms":1}"#;
        let parsed: ServerInfo = serde_json::from_str(legacy).expect("legacy json parses");
        assert_eq!(parsed.origin, HostOrigin::Explicit);
    }
}
