use super::*;
use flate2::{Compression, GzBuilder};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

fn serve_once(body: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let address = listener.local_addr().expect("listener address");
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request).expect("read request");
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .expect("write headers");
        stream.write_all(&body).expect("write body");
    });
    (format!("http://{address}/pack.gz"), thread)
}

fn artifact(url: String, expanded: &[u8]) -> (NativeArtifact, Vec<u8>) {
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    encoder.write_all(expanded).expect("compress fixture");
    let compressed = encoder.finish().expect("finish gzip");
    (
        NativeArtifact {
            url,
            sha256: sha256_hex(&compressed),
            size: compressed.len() as u64,
            encoding: "gzip".into(),
            expanded_sha256: sha256_hex(expanded),
            expanded_size: expanded.len() as u64,
        },
        compressed,
    )
}

/// Held by every test that writes a script and then executes it.
///
/// A child spawned on another test thread starts with a copy of every open
/// descriptor and keeps it until its own exec. When that copy is a script
/// another test still has open for writing, executing the script fails with
/// ETXTBSY ("Text file busy") instead of running it, so these tests must not
/// write and spawn at the same time.
#[cfg(unix)]
fn exec_fixture_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: Mutex<()> = Mutex::new(());
    GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn write_valid_local_catalog(source: &Path) -> (String, Vec<u8>) {
    use upeg_toolkit_catalog::{MetadataSnapshot, SCHEMA_VERSION, ToolkitPackage};

    let snapshot: MetadataSnapshot =
        serde_json::from_str(upeg_toolkit_catalog::EMBEDDED_METADATA).expect("snapshot");
    let abi = abi_digest(&snapshot).expect("digest");
    let mut catalog = Catalog {
        schema_version: SCHEMA_VERSION,
        app_version: snapshot.app_version.clone(),
        abi_digest: abi.clone(),
        catalog_digest: String::new(),
        toolkits: snapshot
            .toolkits
            .into_iter()
            .map(|metadata| ToolkitPackage {
                metadata,
                version: env!("CARGO_PKG_VERSION").to_owned(),
                web: None,
                native: std::collections::BTreeMap::new(),
                requires_host: false,
            })
            .collect(),
    };
    catalog.catalog_digest = catalog.digest_without_self().expect("catalog digest");
    let bytes = serde_json::to_vec(&catalog).expect("catalog JSON");
    fs::write(source.join("catalog.json"), &bytes).expect("stage catalog");
    (abi, bytes)
}

#[cfg(unix)]
#[test]
fn downloads_executes_and_uses_verified_offline_cache() {
    let _exec = exec_fixture_guard();
    let root = tempfile::tempdir().expect("temp cache");
    let result = upeg_core::ToolResult::Success(
        upeg_core::ToolSuccess::new(
            Some("result".into()),
            vec![upeg_core::OutputEntry {
                id: "result".into(),
                label: None,
                kind: upeg_core::OutputKind::String,
                value: upeg_core::OutputValue::String("works".into()),
            }],
        )
        .expect("result envelope"),
    );
    let json = serde_json::to_string(&result).expect("result JSON");
    let script = format!("#!/bin/sh\nIFS= read -r request\nprintf '%s\\n' '{json}'\n");
    let (_, bytes) = artifact(String::new(), script.as_bytes());
    let (url, server) = serve_once(bytes);
    let (expected, _) = artifact(url, script.as_bytes());

    let path = install_from_source(root.path(), "compatible-abi", "num", &expected, None)
        .expect("verified download");
    server.join().expect("server finishes");
    assert_eq!(
        execute(
            &path,
            "compatible-abi",
            "num.hex_to_decimal",
            &serde_json::json!({})
        )
        .expect("sidecar executes"),
        result
    );
    assert_eq!(
        last_compatible_from_source(root.path(), "compatible-abi", "num", None),
        Some(path.clone())
    );
    assert_eq!(
        last_compatible_from_source(root.path(), "different-abi", "num", None),
        None
    );

    fs::write(&path, b"corrupted").expect("corrupt cached executable");
    assert_eq!(
        last_compatible_from_source(root.path(), "compatible-abi", "num", None),
        None
    );
    let (url, retry_server) = serve_once(artifact(String::new(), script.as_bytes()).1);
    let (retry_artifact, _) = artifact(url, script.as_bytes());
    let repaired = install_from_source(root.path(), "compatible-abi", "num", &retry_artifact, None)
        .expect("redownload after corruption");
    retry_server.join().expect("retry server finishes");
    assert!(verified_file(&repaired, &retry_artifact));
}

#[test]
fn rejects_compressed_and_expanded_hash_mismatches() {
    let root = tempfile::tempdir().expect("temp cache");
    let (url, server) = serve_once(b"not a gzip file".to_vec());
    let (expected, _) = artifact(url, b"#!/bin/sh\nexit 0\n");
    assert!(matches!(
        install_from_source(root.path(), "abi", "num", &expected, None),
        Err(NativeError::Integrity)
    ));
    server.join().expect("server finishes");

    let bytes = artifact(String::new(), b"real executable").1;
    let (url, server) = serve_once(bytes);
    let (mut expected, _) = artifact(url, b"real executable");
    expected.expanded_sha256 = sha256_hex(b"different executable");
    assert!(matches!(
        install_from_source(root.path(), "abi", "num", &expected, None),
        Err(NativeError::Integrity)
    ));
    server.join().expect("server finishes");
}

#[test]
fn simultaneous_first_local_catalog_loads_preserve_the_verified_cache() {
    let cache = tempfile::tempdir().expect("cache");
    let source = tempfile::tempdir().expect("local release");
    let (abi, bytes) = write_valid_local_catalog(source.path());
    let cache_path = catalog_cache_path(cache.path(), &source_identity_for(Some(source.path())));
    assert!(!cache_path.exists(), "cache must be cold before the race");

    let workers = 9;
    let start = Arc::new(Barrier::new(workers));
    let handles: Vec<_> = (0..workers)
        .map(|_| {
            let root = cache.path().to_path_buf();
            let source = source.path().to_path_buf();
            let abi = abi.clone();
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                load_catalog_from_local(&root, &abi, &source).map(|catalog| catalog.catalog_digest)
            })
        })
        .collect();

    let expected = parse_catalog(&bytes, &abi)
        .expect("fixture catalog")
        .catalog_digest;
    for handle in handles {
        let actual = handle.join().expect("loader thread").expect("catalog load");
        assert_eq!(
            actual, expected,
            "every simultaneous first load must validate"
        );
    }
    assert_eq!(fs::read(cache_path).expect("cached catalog"), bytes);
}

#[test]
fn local_release_source_installs_and_bad_update_preserves_cached_catalog() {
    use upeg_toolkit_catalog::{MetadataSnapshot, SCHEMA_VERSION, ToolkitPackage};
    let cache = tempfile::tempdir().expect("cache");
    let source = tempfile::tempdir().expect("local release");
    let snapshot: MetadataSnapshot =
        serde_json::from_str(upeg_toolkit_catalog::EMBEDDED_METADATA).expect("snapshot");
    let abi = abi_digest(&snapshot).expect("digest");
    let expanded = b"#!/bin/sh\nexit 0\n";
    let (artifact, compressed) = artifact(
        "https://example.invalid/upeg-toolkit-num.gz".into(),
        expanded,
    );
    fs::write(source.path().join("upeg-toolkit-num.gz"), compressed).expect("stage pack");
    let mut catalog = Catalog {
        schema_version: SCHEMA_VERSION,
        app_version: snapshot.app_version.clone(),
        abi_digest: abi.clone(),
        catalog_digest: String::new(),
        toolkits: snapshot
            .toolkits
            .into_iter()
            .map(|metadata| {
                let mut native = std::collections::BTreeMap::new();
                if metadata.id == "num" {
                    native.insert(env!("UPEG_NATIVE_TARGET").to_owned(), artifact.clone());
                }
                ToolkitPackage {
                    metadata,
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                    web: None,
                    native,
                    requires_host: false,
                }
            })
            .collect(),
    };
    catalog.catalog_digest = catalog.digest_without_self().expect("catalog digest");
    let catalog_bytes = serde_json::to_vec(&catalog).expect("catalog JSON");
    fs::write(source.path().join("catalog.json"), &catalog_bytes).expect("stage catalog");

    let loaded = load_catalog_from_local(cache.path(), &abi, source.path()).expect("local catalog");
    let staged_artifact = loaded
        .toolkits
        .iter()
        .find(|entry| entry.metadata.id == "num")
        .and_then(|entry| entry.native.get(env!("UPEG_NATIVE_TARGET")))
        .expect("num pack");
    let path = install_from_source(
        cache.path(),
        &abi,
        "num",
        staged_artifact,
        Some(source.path()),
    )
    .expect("install local pack with same verifier");
    let windows_runner_path = artifact_dir(
        Path::new(r"C:\Users\RUNNER~1\AppData\Local\Temp\tmp.gR8mDgVlg1"),
        &abi,
        &source_identity_for(Some(source.path())),
        "num",
        staged_artifact,
    )
    .join("upeg-toolkit-num.exe");
    assert!(
        windows_runner_path.to_string_lossy().len() < 260,
        "cached Windows executable must fit the process launch path limit"
    );
    assert_eq!(fs::read(&path).expect("cached executable"), expanded);
    let staged_catalog = source.path().join("catalog.json");
    fs::rename(&staged_catalog, source.path().join("catalog.held"))
        .expect("temporarily remove local source catalog");
    let started = Instant::now();
    let warm = load_catalog_from_local(cache.path(), &abi, source.path())
        .expect("verified local disk catalog without network");
    assert_eq!(warm.catalog_digest, loaded.catalog_digest);
    assert!(started.elapsed() < Duration::from_secs(1));
    fs::rename(source.path().join("catalog.held"), &staged_catalog).expect("restore local catalog");
    assert_ne!(
        catalog_cache_path(cache.path(), "https://release-A/catalog.json"),
        catalog_cache_path(cache.path(), "https://release-B/catalog.json")
    );
    assert_eq!(
        last_compatible_from_source(cache.path(), &abi, "num", None),
        None
    );

    let receipt_path = artifact_dir(
        cache.path(),
        &abi,
        &source_identity_for(Some(source.path())),
        "num",
        staged_artifact,
    )
    .join("receipt.json");
    fs::remove_file(&receipt_path).expect("simulate interrupted receipt write");
    assert!(last_compatible_from_source(cache.path(), &abi, "num", Some(source.path())).is_none());
    install_from_source(
        cache.path(),
        &abi,
        "num",
        staged_artifact,
        Some(source.path()),
    )
    .expect("verified executable repairs missing receipt");
    assert!(receipt_path.exists());

    catalog.toolkits[0].version.push_str("-bad-update");
    fs::write(
        source.path().join("catalog.json"),
        serde_json::to_vec(&catalog).expect("tampered JSON"),
    )
    .expect("stage bad catalog");
    assert!(load_catalog_from_local(cache.path(), &abi, source.path()).is_err());
    assert_eq!(
        fs::read(catalog_cache_path(
            cache.path(),
            &source_identity_for(Some(source.path()))
        ))
        .expect("last good catalog"),
        catalog_bytes
    );
    assert_eq!(
        last_compatible_from_source(cache.path(), &abi, "num", Some(source.path())),
        Some(path)
    );
}

#[cfg(unix)]
#[test]
fn hanging_sidecar_is_killed_at_timeout() {
    use std::os::unix::fs::PermissionsExt;
    let _exec = exec_fixture_guard();
    let root = tempfile::tempdir().expect("temp script");
    let path = root.path().join("hang.sh");
    fs::write(&path, b"#!/bin/sh\nsleep 5\n").expect("write script");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("chmod script");
    let before = Instant::now();
    let result = execute_with_timeout(
        &path,
        "abi",
        "num.test",
        &serde_json::json!({}),
        Duration::from_millis(50),
    );
    assert!(matches!(result, Err(NativeError::Process(message)) if message.contains("timed out")));
    assert!(before.elapsed() < Duration::from_secs(2));
}

#[cfg(unix)]
#[test]
fn descendant_holding_stdout_cannot_outlive_deadline() {
    use std::os::unix::fs::PermissionsExt;
    let _exec = exec_fixture_guard();
    let root = tempfile::tempdir().expect("temp script");
    let path = root.path().join("orphan-pipe.sh");
    fs::write(
        &path,
        b"#!/bin/sh\nsleep 5 &\nIFS= read -r request\nexit 0\n",
    )
    .expect("write script");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("chmod script");
    let before = Instant::now();
    let result = execute_with_timeout(
        &path,
        "abi",
        "num.test",
        &serde_json::json!({}),
        Duration::from_millis(100),
    );
    assert!(
        matches!(result, Err(NativeError::Process(message)) if message.contains("output timed out"))
    );
    assert!(before.elapsed() < Duration::from_secs(2));
}

fn recorded_failure(identity: &str, at: Instant) -> CatalogFailure {
    CatalogFailure {
        identity: identity.to_owned(),
        at,
        message: "http status: 404".to_owned(),
    }
}

#[test]
fn backoff_message_replays_the_failure_inside_the_window() {
    let at = Instant::now();
    let failure = recorded_failure("remote:a", at);
    let just_inside = CATALOG_FAILURE_BACKOFF
        .checked_sub(Duration::from_millis(1))
        .expect("the window is longer than a millisecond");
    let inside = at + just_inside;
    assert_eq!(
        backoff_message(Some(&failure), "remote:a", inside).as_deref(),
        Some("http status: 404")
    );
    assert_eq!(
        backoff_message(None, "remote:a", inside),
        None,
        "nothing recorded, nothing to replay"
    );
}

#[test]
fn backoff_message_ignores_a_failure_of_another_source() {
    let at = Instant::now();
    let failure = recorded_failure("remote:a", at);
    assert_eq!(backoff_message(Some(&failure), "remote:b", at), None);
}

#[test]
fn backoff_message_expires_when_the_window_has_passed() {
    let at = Instant::now();
    let failure = recorded_failure("remote:a", at);
    assert_eq!(
        backoff_message(Some(&failure), "remote:a", at + CATALOG_FAILURE_BACKOFF),
        None
    );
}

/// Loopback server that answers every request with `500` and counts them.
struct FailingServer {
    url: String,
    requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FailingServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        listener
            .set_nonblocking(true)
            .expect("non-blocking listener");
        let url = format!(
            "http://{}/catalog.json",
            listener.local_addr().expect("listener address")
        );
        let requests = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let requests = Arc::clone(&requests);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    };
                    requests.fetch_add(1, Ordering::SeqCst);
                    // Some platforms hand the accepted stream the listener's
                    // non-blocking mode.
                    stream.set_nonblocking(false).expect("blocking stream");
                    let mut request = [0_u8; 4096];
                    let _ = stream.read(&mut request);
                    let _ = stream.write_all(
                        b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
            })
        };
        Self {
            url,
            requests,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for FailingServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[test]
fn remote_catalog_backoff_skips_the_second_request() {
    let cache = tempfile::tempdir().expect("cache");
    let server = FailingServer::start();
    // The failure slot is process-wide. A source nobody else uses keeps this
    // the only test that touches it, and no environment variable changes.
    let identity = format!("remote:{}", server.url);

    let first = load_remote_catalog(cache.path(), "abi", &server.url, &identity)
        .expect_err("the server answers 500");
    assert!(matches!(first, NativeError::Download(_)), "{first}");
    assert_eq!(server.requests.load(Ordering::SeqCst), 1);

    let second = load_remote_catalog(cache.path(), "abi", &server.url, &identity)
        .expect_err("the recorded failure is replayed");
    assert_eq!(
        server.requests.load(Ordering::SeqCst),
        1,
        "no request inside the back-off window"
    );
    assert_eq!(second.to_string(), first.to_string());
}
