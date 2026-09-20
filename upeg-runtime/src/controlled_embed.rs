//! Controlled Embed runtime — driving an external web page through CSS
//! selector bindings.
//!
//! See LEXICON v2.3 §2 and the Pin rename + auto-render design spec.
//!
//! Desktop installs a WebView service backend through flutter_rust_bridge.
//! GUI dispatch and clients of its HTTP host therefore reach the same
//! session. Standalone CLI hosts can install the optional CDP backend;
//! selecting a backend never retries a failed call in another browser.

use std::path::{Path, PathBuf};

pub const CONTROLLED_EMBED_WAIT_TIMEOUT_CODE: &str = "wait-timeout";

/// Outcome of probing the host for a CDP-compatible browser executable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserDiscovery {
    pub kind: BrowserKind,
    pub executable: PathBuf,
}

/// The browser family upeg detected. CDP-compatible only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BrowserKind {
    Chrome,
    Chromium,
    Edge,
}

impl BrowserKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Chromium => "chromium",
            Self::Edge => "edge",
        }
    }
}

/// Errors raised while probing the host for a usable browser.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BrowserDiscoveryError {
    #[error(
        "no Chrome/Chromium/Edge executable found on PATH or in the standard install locations"
    )]
    NotFound,
    #[error("browser executable `{path}` is not executable")]
    NotExecutable { path: PathBuf },
}

/// Trait every Controlled Embed backend implements. Lets the upeg runtime
/// stay generic over the driver (CDP via chromiumoxide today, WebDriver
/// tomorrow, IPC-to-desktop for fast paths).
pub trait ControlledEmbedBackend: Send + Sync {
    fn run(
        &self,
        request: ControlledEmbedRequest<'_>,
    ) -> Result<ControlledEmbedResponse, ControlledEmbedError>;
}

/// Input for one Controlled Embed call. Borrowed against caller-owned
/// `SelectorBinding`s + (field, value) input pairs so the backend
/// implementations don't force allocations.
#[derive(Clone, Debug)]
pub struct ControlledEmbedRequest<'a> {
    /// Canonical tool identity used by a backend that retains sessions.
    pub tool_id: &'a str,
    pub url: &'a str,
    pub bindings: &'a [upeg_core::SelectorBinding],
    pub inputs: &'a [(&'a str, &'a str)],
    pub settings: upeg_core::ControlledEmbedSettings,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlledEmbedResponse {
    pub outputs: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ControlledEmbedError {
    #[error(transparent)]
    Discovery(#[from] BrowserDiscoveryError),
    #[error("Controlled Embed feature is disabled in this build")]
    FeatureDisabled,
    #[error("Controlled Embed backend unavailable: {reason}")]
    Unavailable { reason: String },
    #[error("Controlled Embed execution cancelled")]
    Cancelled,
    #[error("Controlled Embed backend did not respond within {timeout_ms}ms")]
    ResponseTimeout { timeout_ms: u64 },
    #[error("backend reported: {0}")]
    BackendFailed(String),
    #[error(
        "wait-timeout: {role:?} binding `{selector}` waiting for `{for_selector}` condition `{condition:?}` timed out after {timeout_ms}ms"
    )]
    WaitTimeout {
        role: upeg_core::BindingRole,
        selector: String,
        for_selector: String,
        condition: upeg_core::BindingWaitCondition,
        timeout_ms: u64,
    },
}

impl ControlledEmbedError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Discovery(_) | Self::FeatureDisabled | Self::Unavailable { .. } => {
                "controlled_embed_unavailable"
            }
            Self::Cancelled => "cancelled",
            Self::ResponseTimeout { .. } => "controlled_embed_execution_timeout",
            Self::BackendFailed(_) => "controlled_embed_execution_failed",
            Self::WaitTimeout { .. } => CONTROLLED_EMBED_WAIT_TIMEOUT_CODE,
        }
    }
}

/// Try to find a CDP-compatible browser on this host.
///
/// Lookup order:
/// 1. `$UPEG_BROWSER_PATH` — explicit override.
/// 2. `which` on common names (Linux/macOS).
/// 3. macOS application bundle paths.
/// 4. Windows program-files paths.
///
/// Returns the first hit. Falls back to [`BrowserDiscoveryError::NotFound`].
pub fn discover_system_browser() -> Result<BrowserDiscovery, BrowserDiscoveryError> {
    // Explicit overrides come first, in priority order. `UPEG_BROWSER_PATH`
    // is the upeg-specific override; `CHROME_EXECUTABLE` is what the
    // devcontainer already sets for `flutter test --platform=chrome`
    // and what most chromium tooling reads (recognized by the
    // wider ecosystem).
    for env_var in ["UPEG_BROWSER_PATH", "CHROME_EXECUTABLE"] {
        if let Ok(override_path) = std::env::var(env_var) {
            let path = PathBuf::from(override_path);
            return if is_executable_file(&path) {
                Ok(BrowserDiscovery {
                    kind: guess_kind_from_path(&path),
                    executable: path,
                })
            } else if path.exists() {
                Err(BrowserDiscoveryError::NotExecutable { path })
            } else {
                Err(BrowserDiscoveryError::NotFound)
            };
        }
    }
    for candidate in candidate_paths() {
        if is_executable_file(&candidate.executable) {
            return Ok(candidate);
        }
    }
    Err(BrowserDiscoveryError::NotFound)
}

fn candidate_paths() -> Vec<BrowserDiscovery> {
    // Per-platform candidate list. PATH-based names are looked up via
    // `which`; absolute paths are probed directly.
    let mut list: Vec<BrowserDiscovery> = Vec::new();
    if cfg!(target_os = "linux") {
        push_named(&mut list, BrowserKind::Chrome, "google-chrome");
        push_named(&mut list, BrowserKind::Chrome, "google-chrome-stable");
        push_named(&mut list, BrowserKind::Chromium, "chromium");
        push_named(&mut list, BrowserKind::Chromium, "chromium-browser");
        push_named(&mut list, BrowserKind::Edge, "microsoft-edge");
    } else if cfg!(target_os = "macos") {
        for path in [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ] {
            list.push(BrowserDiscovery {
                kind: guess_kind_from_path(Path::new(path)),
                executable: PathBuf::from(path),
            });
        }
    } else if cfg!(target_os = "windows") {
        for path in [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        ] {
            list.push(BrowserDiscovery {
                kind: guess_kind_from_path(Path::new(path)),
                executable: PathBuf::from(path),
            });
        }
    }
    list
}

fn push_named(list: &mut Vec<BrowserDiscovery>, kind: BrowserKind, name: &str) {
    if let Some(path) = which_on_path(name) {
        list.push(BrowserDiscovery {
            kind,
            executable: path,
        });
    }
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    for prefix in std::env::split_paths(&path_env) {
        let candidate = prefix.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    const EXEC_BITS: u32 = 0o111;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & EXEC_BITS != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable_file(path: &Path) -> bool {
    const EXECUTABLE_EXTS: &[&str] = &["exe", "bat", "cmd", "com"];
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .is_some_and(|ext| EXECUTABLE_EXTS.contains(&ext.as_str()))
}

#[cfg(not(any(unix, windows)))]
fn is_executable_file(path: &Path) -> bool {
    // wasi/wasm32/other: file mode bits aren't introspectable through
    // `std::fs`. Controlled Embed is a native-only feature, so this path
    // is only reached in tests / cross-compilation; defer to `is_file()`.
    std::fs::metadata(path)
        .map(|m| m.is_file())
        .unwrap_or(false)
}

fn guess_kind_from_path(path: &Path) -> BrowserKind {
    let s = path.to_string_lossy().to_ascii_lowercase();
    if s.contains("edge") || s.contains("msedge") {
        BrowserKind::Edge
    } else if s.contains("chromium") {
        BrowserKind::Chromium
    } else {
        BrowserKind::Chrome
    }
}

/// The default Controlled Embed backend in upeg builds without the
/// optional chromiumoxide integration. Every call returns
/// [`ControlledEmbedError::FeatureDisabled`].
#[derive(Default)]
pub struct NoopControlledEmbedBackend;

impl ControlledEmbedBackend for NoopControlledEmbedBackend {
    fn run(
        &self,
        _request: ControlledEmbedRequest<'_>,
    ) -> Result<ControlledEmbedResponse, ControlledEmbedError> {
        Err(ControlledEmbedError::FeatureDisabled)
    }
}

// ─── Global backend registry ──────────────────────────────────────
// Stored as `Arc<dyn ControlledEmbedBackend>` behind a `RwLock` so a
// binary can install the headless impl at boot (`set_controlled_embed_backend`)
// and the dispatcher can read it on every call without taking ownership.
// The `OnceLock`-of-`RwLock` pattern lets us hand out `Arc` clones cheaply.

use std::sync::{Arc, OnceLock, RwLock};

type SharedBackend = Arc<dyn ControlledEmbedBackend>;

fn backend_slot() -> &'static RwLock<SharedBackend> {
    static SLOT: OnceLock<RwLock<SharedBackend>> = OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(Arc::new(NoopControlledEmbedBackend)))
}

/// Install a [`ControlledEmbedBackend`] for this binary. CLI/desktop
/// binaries call this from boot when the `controlled-embed` feature is
/// enabled and a backend impl is compiled in; WASM builds and
/// `--no-default-features` callers leave it untouched and inherit
/// [`NoopControlledEmbedBackend`].
#[allow(
    clippy::expect_used,
    reason = "RwLock poisoning means another thread already panicked while \
              holding the slot; the process is in an unrecoverable state."
)]
pub fn set_controlled_embed_backend(backend: SharedBackend) {
    *backend_slot()
        .write()
        .expect("controlled embed backend slot poisoned") = backend;
}

/// Fetch the currently installed backend. Returns an `Arc` clone so
/// the caller can drop the slot lock immediately.
#[allow(
    clippy::expect_used,
    reason = "RwLock poisoning is fatal — see set_controlled_embed_backend."
)]
pub fn controlled_embed_backend() -> SharedBackend {
    backend_slot()
        .read()
        .expect("controlled embed backend slot poisoned")
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_kind_labels_are_unique_per_variant() {
        let labels = [
            BrowserKind::Chrome.label(),
            BrowserKind::Chromium.label(),
            BrowserKind::Edge.label(),
        ];
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), labels.len());
    }

    #[test]
    fn path_based_kind_guess_identifies_edge() {
        assert_eq!(
            guess_kind_from_path(Path::new("/usr/bin/microsoft-edge")),
            BrowserKind::Edge
        );
        assert_eq!(
            guess_kind_from_path(Path::new(r"C:\Program Files\Microsoft\Edge\msedge.exe")),
            BrowserKind::Edge
        );
    }

    #[test]
    fn path_based_kind_guess_identifies_chromium() {
        assert_eq!(
            guess_kind_from_path(Path::new("/usr/bin/chromium-browser")),
            BrowserKind::Chromium
        );
    }

    #[test]
    fn path_based_kind_guess_defaults_to_chrome() {
        assert_eq!(
            guess_kind_from_path(Path::new("/usr/bin/google-chrome")),
            BrowserKind::Chrome
        );
        assert_eq!(
            guess_kind_from_path(Path::new("/opt/whatever/browser")),
            BrowserKind::Chrome
        );
    }

    #[test]
    fn noop_backend_returns_feature_disabled() {
        let backend = NoopControlledEmbedBackend;
        let bindings: &[upeg_core::SelectorBinding] = &[];
        let inputs: &[(&str, &str)] = &[];
        let result = backend.run(ControlledEmbedRequest {
            tool_id: "test.noop",
            url: "https://example.com/",
            bindings,
            inputs,
            settings: upeg_core::ControlledEmbedSettings::default(),
        });
        assert!(matches!(result, Err(ControlledEmbedError::FeatureDisabled)));
    }

    #[test]
    fn controlled_embed_request_keeps_settings_as_owned_value() {
        let settings = upeg_core::ControlledEmbedSettings {
            user_agent: Some(upeg_core::ControlledEmbedUserAgent::Custom(
                "Test UA".into(),
            )),
            viewport: Some(upeg_core::ControlledEmbedViewport::Preset(
                upeg_core::ControlledEmbedViewportPreset::Tablet,
            )),
        };
        let bindings: &[upeg_core::SelectorBinding] = &[];
        let inputs: &[(&str, &str)] = &[];

        let request = ControlledEmbedRequest {
            tool_id: "test.settings",
            url: "https://example.com/",
            bindings,
            inputs,
            settings: settings.clone(),
        };

        assert_eq!(request.settings, settings);
    }

    #[test]
    fn selector_binding_preserves_role_field_and_selector() {
        let binding = upeg_core::SelectorBinding {
            role: upeg_core::BindingRole::Input,
            field: "json".to_string(),
            selector: "#input textarea".to_string(),
            trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
            wait: None,
        };
        assert_eq!(binding.role, upeg_core::BindingRole::Input);
        assert_eq!(binding.field, "json");
        assert_eq!(binding.selector, "#input textarea");
    }

    #[test]
    fn candidate_path_detection_produces_platform_path_list() {
        // Depending on the host platform there must be at least one
        // candidate path. (Whether a browser is actually installed on the
        // system depends on the CI environment, so we only check the
        // candidate *list*.)
        let candidates = candidate_paths();
        if cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows"
        )) {
            // A CI Linux container may have no browser on PATH, so an
            // empty list is allowed — but the call itself must not panic.
            assert!(
                candidates
                    .iter()
                    .all(|c| !c.executable.as_os_str().is_empty())
            );
        } else {
            assert!(candidates.is_empty());
        }
    }

    #[cfg(unix)]
    #[test]
    fn file_without_executable_permission_is_not_accepted_as_browser() {
        // Create an empty file without the executable bit in a temp
        // directory and verify `is_executable_file` clearly rejects it.
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("upeg-exec-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("not-executable");
        {
            let mut f = std::fs::File::create(&path).expect("create file");
            f.write_all(b"#!/bin/sh\nexit 0\n").expect("write");
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("set perms");
        assert!(
            !is_executable_file(&path),
            "executable bit absent: must be rejected"
        );

        // Turning the executable bit on must make it pass.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("set perms");
        assert!(
            is_executable_file(&path),
            "executable bit present: must accept"
        );

        // A directory is rejected either way.
        assert!(!is_executable_file(&dir), "directory must be rejected");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
