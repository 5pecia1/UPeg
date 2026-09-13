//! Status snapshot FRB surface.
//!
//! Single read-only call (`status_snapshot`) feeds the Flutter status
//! bar (`flutter_app/lib/src/widgets/status_bar.dart`) and any future
//! consumers that need a coarse "what's the host doing right now"
//! summary. Every field is a typed DTO — no stringly-typed payloads
//! cross the bridge.

/// Coarse classification of the host's network reachability surface.
/// Drives the status-bar dot color via an exhaustive Dart switch.
///
/// An active surface may be process-local or a separately discovered
/// reachable host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum NetworkReachabilityDto {
    /// Only loopback (127.0.0.1) is bound — the default safe posture.
    LoopbackOnly,
    /// The process-local HTTP surface permits a non-loopback bind, or
    /// a separately discovered host exposes a non-loopback endpoint.
    RemoteBindAllowed,
    /// No MCP/HTTP interface is currently active.
    Offline,
}

/// Typed network-status snapshot for the Flutter status bar.
///
/// `label` is the short human-readable string the bar renders next to
/// the dot. `reachability` drives the dot color via a sealed switch in
/// the widget — never reach into `label` to infer reachability.
#[derive(Clone, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct NetworkStatusDto {
    pub reachability: NetworkReachabilityDto,
    pub label: String,
}

/// Coarse MCP-import load phase of the process this snapshot came
/// from. Drives the status bar's imports chip via an exhaustive Dart
/// switch — the bar must never guess "still loading" from a count.
///
/// The desktop-embedded host registers imports on a background thread
/// AFTER it starts serving, so `Loading` is a real, seconds-long state
/// a user can watch go by (docs/architecture/mcp.md, "desktop 내장
/// host: 비동기 창"). A desktop that only attaches to a separate host
/// imports nothing itself and honestly reports `NotStarted`.
///
/// `Done` deliberately carries no counts: the chip renders
/// [`StatusSnapshotDto::mcp_import_count`], which is the live
/// provenance-registry count — what is registered NOW, not what one
/// load reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum McpImportPhaseDto {
    /// This process never scheduled an import load.
    NotStarted,
    /// A load is scheduled or running — imported tools are still on
    /// their way.
    Loading,
    /// The load finished.
    Done,
    /// Deliberately loaded nothing (this process is itself somebody's
    /// MCP-import upstream child).
    Skipped,
}

#[cfg(not(target_arch = "wasm32"))]
impl McpImportPhaseDto {
    /// Single mapping point from the CLI's typed phase. `#[frb(ignore)]`
    /// keeps it out of the generated Dart bindings.
    #[flutter_rust_bridge::frb(ignore)]
    fn from_cli(phase: upeg_cli::McpImportPhase) -> Self {
        match phase {
            upeg_cli::McpImportPhase::NotStarted => Self::NotStarted,
            upeg_cli::McpImportPhase::Loading => Self::Loading,
            upeg_cli::McpImportPhase::Done(_) => Self::Done,
            upeg_cli::McpImportPhase::Skipped => Self::Skipped,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
const HTTP_ENDPOINT_SCHEME_PREFIX: &str = "http://";

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiscoveredHostReachability {
    Absent,
    LoopbackOnly,
    RemoteBindAllowed,
}

#[cfg(not(target_arch = "wasm32"))]
impl DiscoveredHostReachability {
    fn from_host(host: &upeg_cli::ServerInfo) -> Self {
        let socket = host
            .endpoint
            .strip_prefix(HTTP_ENDPOINT_SCHEME_PREFIX)
            .and_then(|rest| rest.split('/').next())
            .and_then(|authority| authority.parse::<std::net::SocketAddr>().ok());
        match socket {
            Some(address) if address.ip().is_loopback() => Self::LoopbackOnly,
            Some(_) | None => Self::RemoteBindAllowed,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl NetworkStatusDto {
    /// Map process-local network state and discovered-host presence
    /// to the typed Dart-facing DTO. Single source of truth for the
    /// reachability classification.
    ///
    /// `#[frb(ignore)]` keeps this helper out of the generated Dart
    /// bindings — the Dart side only ever sees [`status_snapshot`].
    #[flutter_rust_bridge::frb(ignore)]
    fn from_runtime(
        status: &upeg_runtime::NetworkStatus,
        discovered_host: DiscoveredHostReachability,
    ) -> Self {
        let local_interface_active = status.mcp_active || status.http_active;
        let local_reachability = match local_interface_active {
            false => NetworkReachabilityDto::Offline,
            true => match status.remote_bind_allowed {
                false => NetworkReachabilityDto::LoopbackOnly,
                true => NetworkReachabilityDto::RemoteBindAllowed,
            },
        };
        let reachability = match discovered_host {
            DiscoveredHostReachability::Absent => local_reachability,
            DiscoveredHostReachability::LoopbackOnly => match local_reachability {
                NetworkReachabilityDto::LoopbackOnly | NetworkReachabilityDto::Offline => {
                    NetworkReachabilityDto::LoopbackOnly
                }
                NetworkReachabilityDto::RemoteBindAllowed => {
                    NetworkReachabilityDto::RemoteBindAllowed
                }
            },
            DiscoveredHostReachability::RemoteBindAllowed => {
                NetworkReachabilityDto::RemoteBindAllowed
            }
        };
        let label = match reachability {
            NetworkReachabilityDto::LoopbackOnly => "loopback-only".to_string(),
            NetworkReachabilityDto::RemoteBindAllowed => "remote-bind allowed".to_string(),
            NetworkReachabilityDto::Offline => "offline".to_string(),
        };
        Self {
            reachability,
            label,
        }
    }
}

/// Typed envelope read by the Flutter status bar on every tick.
///
/// Every field has a sealed Dart mirror — no `String` discriminator
/// crosses the bridge. The build version comes from
/// `CARGO_PKG_VERSION` at compile time so the binary and the label
/// can never drift.
#[derive(Clone, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct StatusSnapshotDto {
    pub network: NetworkStatusDto,
    pub paused: super::pause::PausedStateDto,
    /// Tools this process imported from upstream MCP servers. Only a
    /// long-lived server process imports (see `docs/architecture/mcp.md`),
    /// so an attach-only desktop reports 0.
    pub mcp_import_count: u32,
    /// Whether that count is final. A count of 0 means "none imported"
    /// only when the phase is not
    /// [`McpImportPhaseDto::Loading`] — during the embedded host's
    /// async import window the count is simply not in yet.
    pub mcp_import_phase: McpImportPhaseDto,
    pub build_version: String,
}

/// Read a fresh status snapshot. It performs atomic reads, filesystem
/// discovery, and the discovered host's bounded health probe. Safe to
/// call on every 2s status-bar tick.
///
/// Native reads process-local runtime state plus any separately
/// discovered host; Web has no host process, so the wasm implementation
/// returns a local static snapshot below.
#[cfg(not(target_arch = "wasm32"))]
#[flutter_rust_bridge::frb(sync)]
pub fn status_snapshot() -> Result<StatusSnapshotDto, super::boot::FrbError> {
    let trigger_count = upeg_runtime::registered_trigger_bindings().len();
    let runtime_status = upeg_runtime::NetworkStatus::current(trigger_count);
    let discovered_host = match upeg_cli::current_host() {
        Some(host) => DiscoveredHostReachability::from_host(&host),
        None => DiscoveredHostReachability::Absent,
    };
    let network = NetworkStatusDto::from_runtime(&runtime_status, discovered_host);

    let paused = super::pause::PausedStateDto::from_paused(upeg_cli::is_paused());
    // Process-local count: a desktop that only attaches to a separate
    // host imported nothing itself and honestly reports 0.
    let mcp_import_count = upeg_runtime::mcp_import_tool_count() as u32;
    let mcp_import_phase = McpImportPhaseDto::from_cli(upeg_cli::mcp_import_phase());

    Ok(StatusSnapshotDto {
        network,
        paused,
        mcp_import_count,
        mcp_import_phase,
        build_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(target_arch = "wasm32")]
#[flutter_rust_bridge::frb(sync)]
pub fn status_snapshot() -> Result<StatusSnapshotDto, super::boot::FrbError> {
    Ok(StatusSnapshotDto {
        network: NetworkStatusDto {
            reachability: NetworkReachabilityDto::Offline,
            label: "offline".to_string(),
        },
        paused: super::pause::PausedStateDto::Running,
        mcp_import_count: 0,
        // The PWA has no host process at all, so no load was ever
        // scheduled — the same answer an attach-only desktop gives.
        mcp_import_phase: McpImportPhaseDto::NotStarted,
        build_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::pause::PausedStateDto;

    #[test]
    fn status_snapshot은_현재_cargo_pkg_version을_반환한다() {
        let snap = status_snapshot().expect("status_snapshot ok");
        assert_eq!(snap.build_version, env!("CARGO_PKG_VERSION"));
        // Paused enum is compiler-enforced over the two variants —
        // assert exhaustiveness rather than the current on-disk value.
        match snap.paused {
            PausedStateDto::Paused | PausedStateDto::Running => {}
        }
    }

    #[test]
    fn network_status_dto는_loopback_only_runtime을_정확히_매핑한다() {
        let runtime = upeg_runtime::NetworkStatus {
            mcp_active: true,
            http_active: false,
            trigger_count: 0,
            remote_bind_allowed: false,
        };
        let dto = NetworkStatusDto::from_runtime(&runtime, DiscoveredHostReachability::Absent);
        assert_eq!(dto.reachability, NetworkReachabilityDto::LoopbackOnly);
        assert_eq!(dto.label, "loopback-only");
    }

    #[test]
    fn network_status_dto는_remote_bind_허용을_remote_bind_allowed로_매핑한다() {
        let runtime = upeg_runtime::NetworkStatus {
            mcp_active: false,
            http_active: true,
            trigger_count: 0,
            remote_bind_allowed: true,
        };
        let dto = NetworkStatusDto::from_runtime(&runtime, DiscoveredHostReachability::Absent);
        assert_eq!(dto.reachability, NetworkReachabilityDto::RemoteBindAllowed);
        assert_eq!(dto.label, "remote-bind allowed");
    }

    #[test]
    fn network_status_dto는_인터페이스_없으면_offline으로_매핑한다() {
        let runtime = upeg_runtime::NetworkStatus {
            mcp_active: false,
            http_active: false,
            trigger_count: 3,
            remote_bind_allowed: false,
        };
        let dto = NetworkStatusDto::from_runtime(&runtime, DiscoveredHostReachability::Absent);
        assert_eq!(dto.reachability, NetworkReachabilityDto::Offline);
        assert_eq!(dto.label, "offline");
    }

    #[test]
    fn network_status_dto는_로컬_인터페이스가_꺼져도_발견된_host가_있으면_loopback_only다() {
        // Given: 이 프로세스의 MCP/HTTP는 꺼졌지만 별도 host는 도달 가능하다.
        let runtime = upeg_runtime::NetworkStatus {
            mcp_active: false,
            http_active: false,
            trigger_count: 0,
            remote_bind_allowed: false,
        };

        // When: runtime 상태와 발견된 host 상태를 함께 매핑한다.
        let dto =
            NetworkStatusDto::from_runtime(&runtime, DiscoveredHostReachability::LoopbackOnly);

        // Then: status bar는 외부 loopback host를 online으로 표시한다.
        assert_eq!(dto.reachability, NetworkReachabilityDto::LoopbackOnly);
        assert_eq!(dto.label, "loopback-only");
    }

    #[test]
    fn 발견된_host_도달_범위는_endpoint_주소에_따라_분류된다() {
        use DiscoveredHostReachability::{LoopbackOnly, RemoteBindAllowed};

        let cases = [
            ("IPv4 loopback", "http://127.0.0.1:7173", LoopbackOnly),
            ("IPv6 loopback", "http://[::1]:7173", LoopbackOnly),
            (
                "IPv4 non-loopback",
                "http://192.0.2.10:7173",
                RemoteBindAllowed,
            ),
            (
                "IPv6 non-loopback",
                "http://[2001:db8::10]:7173",
                RemoteBindAllowed,
            ),
            ("malformed endpoint", "not-an-endpoint", RemoteBindAllowed),
        ];

        for (case, endpoint, expected) in cases {
            let host = upeg_cli::ServerInfo::new(endpoint, "test-token");
            let actual = DiscoveredHostReachability::from_host(&host);
            assert_eq!(actual, expected, "{case}: {endpoint}");
        }
    }

    #[test]
    fn status_snapshot은_현재_runtime과_discovery_network_status를_반영한다() {
        let trigger_count = upeg_runtime::registered_trigger_bindings().len();
        let runtime = upeg_runtime::NetworkStatus::current(trigger_count);
        let discovered_host = match upeg_cli::current_host() {
            Some(host) => DiscoveredHostReachability::from_host(&host),
            None => DiscoveredHostReachability::Absent,
        };
        let expected = NetworkStatusDto::from_runtime(&runtime, discovered_host);

        let snap = status_snapshot().expect("status_snapshot ok");
        assert_eq!(snap.network, expected);
    }

    #[test]
    fn mcp_import_count는_프로세스가_임포트한_도구_수를_따라간다() {
        // 임포트는 장수명 서버 프로세스만 한다. 스냅샷은 이 프로세스의
        // provenance 레지스트리를 그대로 읽으므로, attach 전용 desktop은
        // 정직하게 0을 보고한다.
        let id = "test.status.mcp_import_chip";
        let before = status_snapshot()
            .expect("status_snapshot ok")
            .mcp_import_count;

        upeg_runtime::register_tool_provenance(
            id,
            upeg_runtime::ToolProvenance::McpImport {
                server: "upstream".to_string(),
            },
        );
        let after = status_snapshot()
            .expect("status_snapshot ok")
            .mcp_import_count;
        assert_eq!(after, before + 1);

        upeg_runtime::clear_tool_provenance(id);
        assert_eq!(
            status_snapshot()
                .expect("status_snapshot ok")
                .mcp_import_count,
            before
        );
    }

    #[test]
    fn mcp_import_phase_dto는_cli_페이즈를_빠짐없이_매핑한다() {
        use upeg_cli::{McpImportPhase, McpImportTally};

        let cases = [
            (McpImportPhase::NotStarted, McpImportPhaseDto::NotStarted),
            (McpImportPhase::Loading, McpImportPhaseDto::Loading),
            (
                McpImportPhase::Done(McpImportTally::default()),
                McpImportPhaseDto::Done,
            ),
            (McpImportPhase::Skipped, McpImportPhaseDto::Skipped),
        ];
        for (phase, expected) in cases {
            assert_eq!(McpImportPhaseDto::from_cli(phase), expected, "{phase:?}");
        }
    }

    #[test]
    fn status_snapshot은_프로세스의_import_phase를_반영한다() {
        // 스냅샷은 프로세스 전역 페이즈를 그대로 읽는다. 값을 고정하지
        // 않고 소스와 일치하는지만 본다 — 다른 테스트가 병렬로 같은
        // 전역을 만질 수 있다.
        let snap = status_snapshot().expect("status_snapshot ok");
        assert_eq!(
            snap.mcp_import_phase,
            McpImportPhaseDto::from_cli(upeg_cli::mcp_import_phase())
        );
    }

    #[test]
    fn status_snapshot은_paused_플래그를_반영한다() {
        let _guard = crate::api::test_support::host_lock().lock().unwrap();
        // Paused state is a process-local AtomicBool. status_snapshot
        // must report the post-flip value via the typed enum. Force
        // a clean starting value so this test is independent of any
        // other test touching the same global.
        upeg_cli::set_paused(false);
        let before = status_snapshot().expect("status_snapshot ok").paused;
        let _flipped = upeg_cli::toggle_paused();
        let after = status_snapshot().expect("status_snapshot ok").paused;
        upeg_cli::set_paused(false);

        assert_ne!(before, after, "status_snapshot must follow paused state");
    }
}
