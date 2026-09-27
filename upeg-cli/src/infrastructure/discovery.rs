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
//!   - Any surface calls [`read_reachable`] at startup; the recorded PID
//!     must exist before a `GET /healthz` can validate its endpoint.
//!   - The returned [`DiscoveryGuard`] removes the file on drop, but
//!     only if it still names the same pid (so a concurrent restart
//!     doesn't accidentally clean up the new host's file).

use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::paths;
use super::process::{ProcessProbe, probe_process};

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
    /// Application wall-clock timestamp, not an OS process birth identity.
    /// It cannot rule out PID reuse.
    pub started_at_ms: u64,
    /// How this host came up (RC-6 teardown policy). `#[serde(default)]`
    /// so a legacy file with no `origin` key parses as `Explicit`.
    #[serde(default)]
    pub origin: HostOrigin,
}

/// A host selected from this process's discovery record. Retains the
/// original path and fields so every authenticated attach can recheck
/// them immediately before writing its bearer token.
#[derive(Debug, Clone)]
pub(crate) struct DiscoveredHost {
    info: ServerInfo,
    path: PathBuf,
    #[cfg(test)]
    _test_root: Option<std::sync::Arc<tempfile::TempDir>>,
}

#[derive(Debug)]
enum DiscoveryRefusal {
    Changed,
    ProcessUnavailable,
}

impl std::fmt::Display for DiscoveryRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Changed => write!(
                formatter,
                "host discovery changed; restart the client to discover the current host"
            ),
            Self::ProcessUnavailable => write!(
                formatter,
                "recorded host process is unavailable; restart the client to discover the current host"
            ),
        }
    }
}

impl std::error::Error for DiscoveryRefusal {}

pub(crate) fn is_discovery_refusal(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(<dyn std::error::Error + Send + Sync>::is::<DiscoveryRefusal>)
}

impl std::ops::Deref for DiscoveredHost {
    type Target = ServerInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl DiscoveredHost {
    pub(crate) fn verify(&self) -> io::Result<()> {
        if !record_unchanged(&self.path, &self.info) {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                DiscoveryRefusal::Changed,
            ));
        }
        if probe_process(self.info.pid) != ProcessProbe::Alive {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                DiscoveryRefusal::ProcessUnavailable,
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn for_test(info: ServerInfo) -> Self {
        let root = std::sync::Arc::new(tempfile::tempdir().expect("test discovery root"));
        let path = root.path().join("server.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&info).expect("test discovery JSON"),
        )
        .expect("write test discovery");
        Self {
            info,
            path,
            _test_root: Some(root),
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_for_test(&self, info: &ServerInfo) {
        std::fs::write(
            &self.path,
            serde_json::to_vec(info).expect("replacement JSON"),
        )
        .expect("replace test discovery");
    }
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

/// A host is attachable only when its recorded PID exists and its
/// endpoint answers `/healthz`. A dead PID is reaped only if the file
/// still contains the same record. Unknown process state or a live
/// process with an unresponsive endpoint leaves the file in place.
pub fn read_reachable() -> Option<ServerInfo> {
    let path = paths::server_json_path()?;
    read_reachable_from(&path, probe_process, health_check)
}

pub(crate) fn discover_reachable() -> Option<DiscoveredHost> {
    let path = paths::server_json_path()?;
    let info = read_reachable_from(&path, probe_process, health_check)?;
    Some(DiscoveredHost {
        info,
        path,
        #[cfg(test)]
        _test_root: None,
    })
}

fn read_reachable_from(
    path: &Path,
    probe: impl Fn(u32) -> ProcessProbe,
    health: impl Fn(&str) -> bool,
) -> Option<ServerInfo> {
    let info = read_from(path).ok()?;
    match probe(info.pid) {
        ProcessProbe::Dead => {
            remove_if_unchanged(path, &info);
            None
        }
        ProcessProbe::Unknown => None,
        ProcessProbe::Alive if health(&info.endpoint) && record_unchanged(path, &info) => {
            Some(info)
        }
        ProcessProbe::Alive => None,
    }
}

fn record_unchanged(path: &Path, info: &ServerInfo) -> bool {
    read_from(path).is_ok_and(|current| current == *info)
}

fn remove_if_unchanged(path: &Path, info: &ServerInfo) {
    if record_unchanged(path, info) {
        let _ = std::fs::remove_file(path);
    }
}

/// Publish `ServerInfo` atomically. Returns `Err(AlreadyExists)` if a
/// (potentially live) host already published — caller should re-run
/// [`read_reachable`] to decide attach-or-bail.
///
/// Unix: the file is created with mode `0600` by the same `open(2)` that
/// creates it ([`create_owner_only`]), so the bearer token never sits in
/// a group/other-readable file, not even briefly. Windows: relies on the
/// user's `%APPDATA%` ACL being user-only by default.
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
    let mut file = match create_owner_only(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            // Probe: maybe the existing file is stale and we can clean it
            // up before the caller retries.
            if read_reachable().is_none() {
                // `read_reachable` removed the stale file; retry once.
                create_owner_only(&path)?
            } else {
                return Err(e);
            }
        }
        Err(e) => return Err(e),
    };
    file.write_all(content.as_bytes())?;
    file.flush()?;

    Ok(DiscoveryGuard {
        path,
        pid: info.pid,
    })
}

/// Create the discovery file exclusively, owner-only from the first
/// instant it exists. `create_new` gives the mutual exclusion; on Unix
/// the `0600` mode is applied by the same `open(2)` call (subject to the
/// umask, which can only tighten it), so there is no window in which a
/// wider-readable file holds the bearer token. Windows has no mode bits
/// here — the config root's user-only ACL is inherited instead.
fn create_owner_only(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
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

/// Format `host:port` as a socket-address string that always parses as
/// [`std::net::SocketAddr`]. IPv6 literals (host contains `':'`) are
/// bracketed — `parse_endpoint` strips the brackets, so rebuilding a bare
/// `::1:port` would fail to parse and make a live IPv6 host look dead.
pub(crate) fn socket_addr_str(host: &str, port: u16) -> String {
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

/// Lightweight unauthenticated `/healthz` probe. Any HTTP response
/// shows only that a listener answered; callers must check the recorded
/// process independently before accepting it as the host.
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

    /// Obviously fake credential for round-trip fixtures. Never a real
    /// secret.
    const FAKE_TOKEN: &str = "not-a-real-token";

    fn info_with(pid: u32, origin: HostOrigin) -> ServerInfo {
        ServerInfo {
            endpoint: "http://127.0.0.1:1".into(),
            mcp_endpoint: "http://127.0.0.1:1/mcp".into(),
            token: FAKE_TOKEN.into(),
            pid,
            started_at_ms: 1,
            origin,
        }
    }

    #[test]
    fn endpoint_parsing_handles_ipv4_ipv6_and_path_prefix() {
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
    fn server_info_fills_in_the_mcp_endpoint() {
        let info = ServerInfo::new("http://127.0.0.1:1234", FAKE_TOKEN);
        assert_eq!(info.mcp_endpoint, "http://127.0.0.1:1234/mcp");
        assert_eq!(info.pid, std::process::id());
        assert!(info.started_at_ms > 0);
    }

    #[test]
    fn health_check_against_a_dead_port_is_false() {
        // Bind a listener, immediately drop, leaving the port unused.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        // Sleep a tick so the kernel surely releases the port.
        std::thread::sleep(Duration::from_millis(20));
        assert!(!health_check(&format!("http://127.0.0.1:{port}")));
    }

    #[test]
    fn http_response_prefix_read_survives_interrupted_and_partial_reads() {
        // Given: the first read is interrupted by a signal and the HTTP
        // prefix arrives in two chunks.
        let chunks = std::io::Cursor::new(b"HT").chain(std::io::Cursor::new(b"TP/"));
        let mut reader = InterruptOnce {
            inner: chunks,
            interrupt_pending: true,
        };

        // When: the health-check response prefix is read.
        let result = read_http_response_prefix(&mut reader);

        // Then: Interrupted is retried and the partial reads add up to HTTP.
        assert!(matches!(result, Ok(true)));
    }

    #[test]
    fn discovery_guard_only_removes_a_file_naming_its_own_pid() {
        // Smoke test: a different pid's file must not be deleted on drop.
        let tmp = std::env::temp_dir().join("upeg-discovery-test.json");
        let other = info_with(std::process::id().wrapping_add(7777), HostOrigin::default());
        std::fs::write(&tmp, serde_json::to_string(&other).unwrap()).unwrap();
        let guard = DiscoveryGuard {
            path: tmp.clone(),
            pid: std::process::id(),
        };
        drop(guard);
        assert!(tmp.exists(), "must not delete a file owned by another pid");
        let _ = std::fs::remove_file(&tmp);
    }

    /// The token lives in this file, so it must be owner-only from the
    /// `open(2)` that creates it — a create-then-chmod sequence leaves a
    /// window in which the umask decides who can read the secret.
    #[cfg(unix)]
    #[test]
    fn discovery_file_is_owner_only_from_the_instant_it_is_created() {
        use std::os::unix::fs::PermissionsExt;
        let path =
            std::env::temp_dir().join(format!("upeg-discovery-mode-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let file = create_owner_only(&path).expect("create discovery file");
        let mode = file.metadata().expect("metadata").permissions().mode() & 0o777;

        assert_eq!(
            mode & 0o077,
            0,
            "group/other bits must be clear before anything is written; got {mode:o}"
        );
        assert!(
            create_owner_only(&path).is_err_and(|e| e.kind() == ErrorKind::AlreadyExists),
            "exclusive creation must still reject a second publisher"
        );
        drop(file);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn publish_writes_an_owner_only_file_that_its_guard_removes() {
        crate::test_support::with_seeded_pegboard_home(
            "discovery-publish",
            |_| {},
            || {
                let path = paths::server_json_path().expect("server.json path under UPEG_HOME");
                let info =
                    ServerInfo::with_origin("http://127.0.0.1:1", FAKE_TOKEN, HostOrigin::Explicit);

                let guard = publish(&info).expect("publish into an empty config root");

                let written = read_from(&path).expect("published file parses");
                assert_eq!(written, info);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
                    assert_eq!(
                        mode & 0o077,
                        0,
                        "published file must stay owner-only; got {mode:o}"
                    );
                }
                drop(guard);
                assert!(
                    !path.exists(),
                    "guard drop must remove the file it published"
                );
            },
        );
    }

    #[test]
    fn socket_addr_str_brackets_ipv6_literals() {
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
    fn should_reap_preserves_a_record_with_a_live_pid() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("server.json");
        let info = info_with(std::process::id(), HostOrigin::default());
        std::fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();

        assert!(read_reachable_from(&path, |_| ProcessProbe::Alive, |_| false).is_none());
        assert_eq!(read_from(&path).unwrap(), info);
        assert_eq!(
            read_reachable_from(&path, |_| ProcessProbe::Alive, |_| true),
            Some(info.clone())
        );
        assert_eq!(read_from(&path).unwrap(), info);
    }

    #[test]
    fn should_reap_treats_only_a_dead_pid_record_as_stale() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("server.json");
        let info = info_with(99_999_999, HostOrigin::default());
        std::fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();
        let health_called = std::cell::Cell::new(false);

        let found = read_reachable_from(
            &path,
            |_| ProcessProbe::Dead,
            |_| {
                health_called.set(true);
                true
            },
        );

        assert!(found.is_none());
        assert!(
            !health_called.get(),
            "dead PID must not probe a foreign listener"
        );
        assert!(!path.exists(), "unchanged dead record must be reaped");
    }

    #[test]
    fn dead_record_cannot_attach_to_an_unrelated_http_listener() {
        crate::test_support::with_seeded_pegboard_home(
            "foreign-listener",
            |_| {},
            || {
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                listener.set_nonblocking(true).unwrap();
                let endpoint = format!("http://{}", listener.local_addr().unwrap());
                let mut info = ServerInfo::new(endpoint, FAKE_TOKEN);
                info.pid = 99_999_999;
                let path = paths::server_json_path().unwrap();
                std::fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();

                let authenticated = std::sync::atomic::AtomicUsize::new(0);
                std::thread::scope(|scope| {
                    scope.spawn(|| {
                        let deadline = std::time::Instant::now() + Duration::from_millis(700);
                        while std::time::Instant::now() < deadline {
                            match listener.accept() {
                                Ok((mut stream, _)) => {
                                    stream
                                        .set_read_timeout(Some(Duration::from_millis(200)))
                                        .unwrap();
                                    let mut request = [0_u8; 512];
                                    let read = stream.read(&mut request).unwrap_or(0);
                                    if request[..read].windows(15).any(|s| s == b"Authorization: ")
                                    {
                                        authenticated
                                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                    }
                                    stream
                                        .write_all(
                                            b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}",
                                        )
                                        .unwrap();
                                }
                                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                                    std::thread::sleep(Duration::from_millis(10));
                                }
                                Err(error) => panic!("foreign listener: {error}"),
                            }
                        }
                    });
                    if let Some(host) = super::super::attach::current_host() {
                        let _ = super::super::attach::post_mcp_with_board(&host, "{}", None);
                    }
                    assert!(
                        read_reachable().is_none(),
                        "a foreign listener cannot validate a dead recorded pid"
                    );
                });
                assert_eq!(authenticated.load(std::sync::atomic::Ordering::SeqCst), 0);
                assert!(!path.exists(), "unchanged dead record should be reaped");
            },
        );
    }

    #[test]
    fn unknown_process_does_not_probe_or_reap_the_record() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("server.json");
        let info = info_with(std::process::id(), HostOrigin::Explicit);
        std::fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();

        let found = read_reachable_from(
            &path,
            |_| ProcessProbe::Unknown,
            |_| panic!("unknown process must not contact the endpoint"),
        );

        assert!(found.is_none());
        assert_eq!(read_from(&path).unwrap(), info);
    }

    #[test]
    fn live_process_with_unresponsive_endpoint_keeps_its_record() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("server.json");
        let info = info_with(std::process::id(), HostOrigin::Explicit);
        std::fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();

        let found = read_reachable_from(&path, |_| ProcessProbe::Alive, |_| false);

        assert!(found.is_none());
        assert_eq!(read_from(&path).unwrap(), info);
    }

    #[test]
    fn live_process_timeout_preserves_the_discovery_file() {
        crate::test_support::with_seeded_pegboard_home(
            "live-timeout",
            |_| {},
            || {
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let info = ServerInfo::new(
                    format!("http://{}", listener.local_addr().unwrap()),
                    FAKE_TOKEN,
                );
                let path = paths::server_json_path().unwrap();
                std::fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();

                std::thread::scope(|scope| {
                    scope.spawn(|| {
                        let (mut stream, _) = listener.accept().unwrap();
                        let mut request = [0_u8; 512];
                        let _ = stream.read(&mut request).unwrap();
                        std::thread::sleep(Duration::from_millis(HEALTH_READ_TIMEOUT_MS + 100));
                    });
                    assert!(read_reachable().is_none());
                });

                assert_eq!(read_from(&path).unwrap(), info);
            },
        );
    }

    #[test]
    fn stale_cleanup_and_attach_recheck_preserve_a_replacement_record() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("server.json");
        let old = info_with(99_999_999, HostOrigin::Explicit);
        let mut replacement = info_with(std::process::id(), HostOrigin::Explicit);
        replacement.token = "replacement-token".into();
        std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();

        let found = read_reachable_from(
            &path,
            |_| {
                std::fs::write(&path, serde_json::to_vec(&replacement).unwrap()).unwrap();
                ProcessProbe::Dead
            },
            |_| panic!("dead process must not contact the endpoint"),
        );

        assert!(found.is_none());
        assert_eq!(read_from(&path).unwrap(), replacement);
        let selected = DiscoveredHost {
            info: old,
            path,
            _test_root: None,
        };
        let error = selected.verify().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("discovery changed"));
    }

    #[test]
    fn server_info_round_trips_origin_and_legacy_files_default_to_explicit() {
        for origin in [HostOrigin::Explicit, HostOrigin::Embedded] {
            let info = info_with(1, origin);
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
