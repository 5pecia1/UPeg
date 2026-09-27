//! Native toolkit sidecars: verified install, compatible offline fallback,
//! and dispatch through the existing runtime toolbox.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::get_unwrap,
        reason = "tests use fixture assertions"
    )
)]

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use upeg_core::{Invoker, ToolResult};
use upeg_toolkit_catalog::{
    Catalog, MetadataSnapshot, NATIVE_PACK_TOOLKITS, NativeArtifact, abi_digest, sha256_hex,
};

const MAX_ARTIFACT_BYTES: u64 = 25 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CATALOG_BYTES: usize = 2 * 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = upeg_core::MAX_UNTRUSTED_OUTPUT_WIRE_BYTES;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(120);

#[cfg(any(unix, windows))]
type ManagedChild = command_group::GroupChild;
#[cfg(not(any(unix, windows)))]
type ManagedChild = Child;

#[derive(Debug, thiserror::Error)]
pub enum NativeError {
    #[error("toolkit cache root is unavailable")]
    CacheRoot,
    #[error("native toolkit catalog: {0}")]
    Catalog(String),
    #[error("toolkit {0} has no artifact for {1}")]
    Unsupported(String, String),
    #[error("toolkit download: {0}")]
    Download(String),
    #[error("toolkit checksum or size mismatch")]
    Integrity,
    #[error("toolkit process: {0}")]
    Process(String),
    #[error("toolkit result: {0}")]
    Result(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Receipt {
    app_version: String,
    abi_digest: String,
    toolkit_id: String,
    target: String,
    #[serde(default)]
    source_identity: String,
    artifact: NativeArtifact,
}

static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
static INSTALL_LOCK: Mutex<()> = Mutex::new(());
static CATALOG_CACHE: Mutex<Option<(String, Catalog)>> = Mutex::new(None);
// A local release catalog is copied into the shared cache before installation
// takes `INSTALL_LOCK`. Serialize that copy so concurrent first calls cannot
// rename the same PID-scoped atomic-write temporary file.
static CATALOG_LOAD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug)]
pub enum RegisteredDispatch {
    NotFound,
    Unimplemented,
    Ran(ToolResult),
}

pub fn dispatch_registered(tool_id: &str, args: &serde_json::Value) -> RegisteredDispatch {
    if let Err(message) = register_native_toolkits() {
        return RegisteredDispatch::Ran(upeg_runtime::tool_failure("toolkit_catalog", message));
    }
    if upeg_runtime::toolbox_tool(tool_id).is_none() {
        if upeg_runtime::project_scope::is_project_tool_blocked(tool_id) {
            return RegisteredDispatch::Ran(upeg_runtime::tool_failure(
                "project_tool_conflict",
                format!(
                    "tool `{tool_id}` has both global and project definitions; choose one in .upeg/project.toml"
                ),
            ));
        }
        return RegisteredDispatch::NotFound;
    }
    match upeg_runtime::try_runtime_dispatch(tool_id, args) {
        Some(result) => RegisteredDispatch::Ran(result),
        None => RegisteredDispatch::Unimplemented,
    }
}

fn failure(error: NativeError) -> ToolResult {
    upeg_runtime::tool_failure("toolkit_unavailable", error.to_string())
}

/// Install one dispatcher per generated built-in Function tool. No network or
/// executable is touched until the first invocation of that tool.
pub fn register_native_toolkits() -> Result<(), String> {
    upeg_toolkit_catalog::register_embedded_metadata()?;
    REGISTERED
        .get_or_init(|| {
            let snapshot: MetadataSnapshot =
                serde_json::from_str(upeg_toolkit_catalog::EMBEDDED_METADATA)
                    .map_err(|error| error.to_string())?;
            for toolkit in snapshot.toolkits {
                for tool in toolkit.tools {
                    if tool.invoker != Invoker::Function
                        || !NATIVE_PACK_TOOLKITS.contains(&toolkit.id.as_str())
                    {
                        continue;
                    }
                    let meta = upeg_runtime::toolbox_tool(&tool.id).ok_or_else(|| {
                        format!("generated builtin {} is not registered", tool.id)
                    })?;
                    let id = meta.id;
                    let toolkit_id = meta.toolkit;
                    upeg_runtime::register_runtime_dispatcher(id, move |args| {
                        dispatch_native(toolkit_id, id, args.as_value()).unwrap_or_else(failure)
                    });
                }
            }
            Ok(())
        })
        .clone()
}

fn cache_root() -> Result<PathBuf, NativeError> {
    std::env::var_os("UPEG_TOOLKIT_CACHE_DIR")
        .map(PathBuf::from)
        .or_else(|| upeg_core::paths::config_root().map(|root| root.join("toolkit-packs")))
        .ok_or(NativeError::CacheRoot)
}

fn catalog_url() -> String {
    std::env::var("UPEG_TOOLKIT_CATALOG_URL")
        .ok()
        .or_else(|| option_env!("UPEG_TOOLKIT_CATALOG_URL").map(str::to_owned))
        .unwrap_or_else(|| {
            format!(
                "https://github.com/5pecia1/UPeg/releases/download/v{}/toolkits-{}-catalog.json",
                env!("CARGO_PKG_VERSION"),
                env!("UPEG_NATIVE_TARGET")
            )
        })
}

fn local_source_dir() -> Option<PathBuf> {
    std::env::var_os("UPEG_TOOLKIT_LOCAL_DIR").map(PathBuf::from)
}

fn source_identity_for(source: Option<&Path>) -> String {
    match source {
        Some(path) => {
            let absolute = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
            };
            format!("local:{}", absolute.display())
        }
        None => format!("remote:{}", catalog_url()),
    }
}

fn local_catalog_path(source: &Path) -> PathBuf {
    let nested = source.join("catalog.json");
    if nested.exists() {
        nested
    } else {
        source.join(format!(
            "toolkits-{}-catalog.json",
            env!("UPEG_NATIVE_TARGET")
        ))
    }
}

fn catalog_cache_path(root: &Path, source_identity: &str) -> PathBuf {
    root.join("catalogs")
        .join(format!("{}.json", sha256_hex(source_identity.as_bytes())))
}

fn expected_abi() -> Result<String, NativeError> {
    let snapshot = serde_json::from_str(upeg_toolkit_catalog::EMBEDDED_METADATA)
        .map_err(|error| NativeError::Catalog(error.to_string()))?;
    abi_digest(&snapshot).map_err(|error| NativeError::Catalog(error.to_string()))
}

fn read_url(url: &str, limit: usize) -> Result<Vec<u8>, NativeError> {
    #[cfg(test)]
    let test_loopback = url.starts_with("http://127.0.0.1:");
    #[cfg(not(test))]
    let test_loopback = false;
    if !url.starts_with("https://") && !test_loopback {
        return Err(NativeError::Download("artifact URL must use https".into()));
    }
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build(),
    );
    let mut response = agent
        .get(url)
        .call()
        .map_err(|error| NativeError::Download(error.to_string()))?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| NativeError::Download(error.to_string()))?;
    if bytes.len() > limit {
        return Err(NativeError::Download("download exceeds size limit".into()));
    }
    Ok(bytes)
}

fn parse_catalog(bytes: &[u8], expected_abi: &str) -> Result<Catalog, NativeError> {
    let catalog: Catalog =
        serde_json::from_slice(bytes).map_err(|error| NativeError::Catalog(error.to_string()))?;
    catalog
        .validate(expected_abi)
        .map_err(|error| NativeError::Catalog(error.to_string()))?;
    Ok(catalog)
}

fn load_catalog(root: &Path, expected_abi: &str) -> Result<Catalog, NativeError> {
    if let Some(source) = local_source_dir() {
        return load_catalog_from_local(root, expected_abi, &source);
    }
    let _guard = CATALOG_LOAD_LOCK
        .lock()
        .map_err(|_| NativeError::Catalog("catalog load lock poisoned".into()))?;
    let url = catalog_url();
    let identity = source_identity_for(None);
    if let Ok(guard) = CATALOG_CACHE.lock()
        && let Some((cached_identity, catalog)) = guard.as_ref()
        && cached_identity == &identity
    {
        return Ok(catalog.clone());
    }
    let cached = catalog_cache_path(root, &identity);
    if let Ok(bytes) = fs::read(&cached)
        && let Ok(catalog) = parse_catalog(&bytes, expected_abi)
    {
        if let Ok(mut guard) = CATALOG_CACHE.lock() {
            *guard = Some((identity, catalog.clone()));
        }
        return Ok(catalog);
    }
    let fetched = read_url(&url, MAX_CATALOG_BYTES)
        .and_then(|bytes| parse_catalog(&bytes, expected_abi).map(|catalog| (catalog, bytes)));
    let catalog = match fetched {
        Ok((catalog, bytes)) => {
            atomic_write(&cached, &bytes)
                .map_err(|error| NativeError::Catalog(error.to_string()))?;
            catalog
        }
        Err(download_error) => return Err(download_error),
    };
    if let Ok(mut guard) = CATALOG_CACHE.lock() {
        *guard = Some((identity, catalog.clone()));
    }
    Ok(catalog)
}

fn load_catalog_from_local(
    root: &Path,
    expected_abi: &str,
    source: &Path,
) -> Result<Catalog, NativeError> {
    let _guard = CATALOG_LOAD_LOCK
        .lock()
        .map_err(|_| NativeError::Catalog("catalog load lock poisoned".into()))?;
    let cached = catalog_cache_path(root, &source_identity_for(Some(source)));
    match fs::read(local_catalog_path(source)) {
        Ok(bytes) => {
            let catalog = parse_catalog(&bytes, expected_abi)?;
            atomic_write(&cached, &bytes)
                .map_err(|error| NativeError::Catalog(error.to_string()))?;
            Ok(catalog)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let bytes = fs::read(cached).map_err(|_| NativeError::Catalog(error.to_string()))?;
            parse_catalog(&bytes, expected_abi)
        }
        Err(error) => Err(NativeError::Catalog(error.to_string())),
    }
}

fn read_artifact_source_with_dir(
    id: &str,
    artifact: &NativeArtifact,
    source: Option<&Path>,
) -> Result<Vec<u8>, NativeError> {
    let Some(source) = source else {
        return read_url(&artifact.url, MAX_ARTIFACT_BYTES as usize);
    };
    let filename = artifact
        .url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty() && *name != "." && *name != ".." && !name.contains('\\'))
        .ok_or_else(|| NativeError::Download("artifact URL has invalid filename".into()))?;
    let flat = source.join(filename);
    let path = if flat.exists() {
        flat
    } else {
        source
            .join(id)
            .join(env!("CARGO_PKG_VERSION"))
            .join(filename)
    };
    let bytes = fs::read(&path)
        .map_err(|error| NativeError::Download(format!("{}: {error}", path.display())))?;
    if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(NativeError::Download(
            "local artifact exceeds size limit".into(),
        ));
    }
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = fs::File::create(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    if path.exists() {
        #[cfg(windows)]
        fs::remove_file(path)?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}

fn artifact_dir(
    root: &Path,
    abi: &str,
    source_identity: &str,
    id: &str,
    artifact: &NativeArtifact,
) -> PathBuf {
    root.join(abi)
        .join(sha256_hex(source_identity.as_bytes()))
        .join(id)
        .join(&artifact.sha256)
}

fn artifact_filename(id: &str) -> String {
    let extension = if cfg!(windows) { ".exe" } else { "" };
    format!("upeg-toolkit-{id}{extension}")
}

fn verified_file(path: &Path, artifact: &NativeArtifact) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    bytes.len() as u64 == artifact.expanded_size && sha256_hex(&bytes) == artifact.expanded_sha256
}

fn ensure_executable(path: &Path) -> Result<(), NativeError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| NativeError::Process(error.to_string()))?;
    }
    Ok(())
}

fn write_receipt(
    dir: &Path,
    abi: &str,
    source_identity: &str,
    id: &str,
    artifact: &NativeArtifact,
) -> Result<(), NativeError> {
    let receipt = Receipt {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        abi_digest: abi.to_owned(),
        toolkit_id: id.to_owned(),
        target: env!("UPEG_NATIVE_TARGET").to_owned(),
        source_identity: source_identity.to_owned(),
        artifact: artifact.clone(),
    };
    let path = dir.join("receipt.json");
    if let Ok(bytes) = fs::read(&path)
        && serde_json::from_slice::<Receipt>(&bytes).ok().as_ref() == Some(&receipt)
    {
        return Ok(());
    }
    let bytes =
        serde_json::to_vec(&receipt).map_err(|error| NativeError::Process(error.to_string()))?;
    atomic_write(&path, &bytes).map_err(|error| NativeError::Process(error.to_string()))
}

fn install(
    root: &Path,
    abi: &str,
    id: &str,
    artifact: &NativeArtifact,
) -> Result<PathBuf, NativeError> {
    install_from_source(root, abi, id, artifact, local_source_dir().as_deref())
}

fn install_from_source(
    root: &Path,
    abi: &str,
    id: &str,
    artifact: &NativeArtifact,
    source: Option<&Path>,
) -> Result<PathBuf, NativeError> {
    let _guard = INSTALL_LOCK
        .lock()
        .map_err(|_| NativeError::Process("installer lock poisoned".into()))?;
    if artifact.size > MAX_ARTIFACT_BYTES
        || artifact.expanded_size > MAX_EXPANDED_BYTES
        || artifact.encoding != "gzip"
        || artifact.sha256.len() != 64
        || !artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        || artifact.expanded_sha256.len() != 64
        || !artifact
            .expanded_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(NativeError::Integrity);
    }
    let source_identity = source_identity_for(source);
    let dir = artifact_dir(root, abi, &source_identity, id, artifact);
    let path = dir.join(artifact_filename(id));
    if verified_file(&path, artifact) {
        ensure_executable(&path)?;
        write_receipt(&dir, abi, &source_identity, id, artifact)?;
        return Ok(path);
    }
    let bytes = read_artifact_source_with_dir(id, artifact, source)?;
    if bytes.len() as u64 != artifact.size || sha256_hex(&bytes) != artifact.sha256 {
        return Err(NativeError::Integrity);
    }
    let mut expanded = Vec::new();
    GzDecoder::new(bytes.as_slice())
        .take(MAX_EXPANDED_BYTES + 1)
        .read_to_end(&mut expanded)
        .map_err(|error| NativeError::Download(error.to_string()))?;
    if expanded.len() as u64 != artifact.expanded_size
        || sha256_hex(&expanded) != artifact.expanded_sha256
    {
        return Err(NativeError::Integrity);
    }
    atomic_write(&path, &expanded).map_err(|error| NativeError::Process(error.to_string()))?;
    ensure_executable(&path)?;
    write_receipt(&dir, abi, &source_identity, id, artifact)?;
    Ok(path)
}

fn last_compatible(root: &Path, abi: &str, id: &str) -> Option<PathBuf> {
    last_compatible_from_source(root, abi, id, local_source_dir().as_deref())
}

fn last_compatible_from_source(
    root: &Path,
    abi: &str,
    id: &str,
    source: Option<&Path>,
) -> Option<PathBuf> {
    let source_identity = source_identity_for(source);
    let entries = fs::read_dir(
        root.join(abi)
            .join(sha256_hex(source_identity.as_bytes()))
            .join(id),
    )
    .ok()?;
    for entry in entries.flatten() {
        let dir = entry.path();
        let Ok(bytes) = fs::read(dir.join("receipt.json")) else {
            continue;
        };
        let Ok(receipt) = serde_json::from_slice::<Receipt>(&bytes) else {
            continue;
        };
        if receipt.app_version != env!("CARGO_PKG_VERSION")
            || receipt.abi_digest != abi
            || receipt.toolkit_id != id
            || receipt.target != env!("UPEG_NATIVE_TARGET")
            || receipt.source_identity != source_identity
        {
            continue;
        }
        let path = dir.join(artifact_filename(id));
        if verified_file(&path, &receipt.artifact) {
            return Some(path);
        }
    }
    None
}

fn select_artifact(root: &Path, abi: &str, id: &str) -> Result<PathBuf, NativeError> {
    let desired = load_catalog(root, abi).and_then(|catalog| {
        let toolkit = catalog
            .toolkits
            .iter()
            .find(|toolkit| toolkit.metadata.id == id)
            .ok_or_else(|| NativeError::Catalog(format!("toolkit {id} absent")))?;
        let artifact = toolkit
            .native
            .get(env!("UPEG_NATIVE_TARGET"))
            .ok_or_else(|| {
                NativeError::Unsupported(id.to_owned(), env!("UPEG_NATIVE_TARGET").to_owned())
            })?;
        install(root, abi, id, artifact)
    });
    desired.or_else(|error| last_compatible(root, abi, id).ok_or(error))
}

struct ChildGuard(ManagedChild);

impl ChildGuard {
    fn direct_child(&mut self) -> &mut Child {
        #[cfg(any(unix, windows))]
        {
            self.0.inner()
        }
        #[cfg(not(any(unix, windows)))]
        {
            &mut self.0
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn execute(
    path: &Path,
    abi: &str,
    tool_id: &str,
    args: &serde_json::Value,
) -> Result<ToolResult, NativeError> {
    execute_with_timeout(path, abi, tool_id, args, PROCESS_TIMEOUT)
}

fn execute_with_timeout(
    path: &Path,
    abi: &str,
    tool_id: &str,
    args: &serde_json::Value,
    timeout: Duration,
) -> Result<ToolResult, NativeError> {
    let request = serde_json::json!({"abi_digest": abi, "tool_id": tool_id, "args": args});
    let mut request_bytes =
        serde_json::to_vec(&request).map_err(|error| NativeError::Process(error.to_string()))?;
    request_bytes.push(b'\n');
    let mut command = Command::new(path);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(any(unix, windows))]
    let managed = {
        use command_group::CommandGroup as _;
        command
            .group_spawn()
            .map_err(|error| NativeError::Process(error.to_string()))?
    };
    #[cfg(not(any(unix, windows)))]
    let managed = command
        .spawn()
        .map_err(|error| NativeError::Process(error.to_string()))?;
    let mut child = ChildGuard(managed);
    let mut stdin = child
        .direct_child()
        .stdin
        .take()
        .ok_or_else(|| NativeError::Process("missing child stdin".into()))?;
    let stdout = child
        .direct_child()
        .stdout
        .take()
        .ok_or_else(|| NativeError::Process("missing child stdout".into()))?;
    let (writer_sender, writer_receiver) = std::sync::mpsc::sync_channel(1);
    let _writer = std::thread::spawn(move || {
        let _ = writer_sender.send(stdin.write_all(&request_bytes));
    });
    let (reader_sender, reader_receiver) = std::sync::mpsc::sync_channel(1);
    let _reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        let result = stdout
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut output)
            .map(|_| output);
        let _ = reader_sender.send(result);
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child
            .direct_child()
            .try_wait()
            .map_err(|error| NativeError::Process(error.to_string()))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            return Err(NativeError::Process("sidecar timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    writer_receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| NativeError::Process("sidecar input timed out".into()))?
        .map_err(|error| NativeError::Process(error.to_string()))?;
    let output = reader_receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| NativeError::Process("sidecar output timed out".into()))?
        .map_err(|error| NativeError::Process(error.to_string()))?;
    if output.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(NativeError::Result("result exceeds output budget".into()));
    }
    if !status.success() {
        return Err(NativeError::Process(format!("sidecar exited {status}")));
    }
    serde_json::from_slice(&output).map_err(|error| NativeError::Result(error.to_string()))
}

pub fn dispatch_native(
    toolkit_id: &str,
    tool_id: &str,
    args: &serde_json::Value,
) -> Result<ToolResult, NativeError> {
    let abi = expected_abi()?;
    let root = cache_root()?;
    let path = select_artifact(&root, &abi, toolkit_id)?;
    execute(&path, &abi, tool_id, args)
}

#[cfg(test)]
mod tests;
