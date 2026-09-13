//! Network status — TCP-probe-based reachability check.
//!
//! Backs the `net.status` GUI Live+Timer pin (see `gui_meta.rs`). Pure
//! stdlib: no extra deps. The probe target and timeout are picked for
//! a wide audience — Cloudflare 1.1.1.1:53 is one of the most reliable
//! anycast endpoints, accepts TCP DNS, and a 1-second timeout is short
//! enough that a 5-second poll never overlaps with itself.
//!
//! WASM target (`wasm32-unknown-unknown`) has no `std::net`; the
//! dispatcher is gated to native targets only in `dispatch.rs`. WASM
//! surfaces fall back to the "no dispatcher" path until a web-friendly
//! implementation lands.

#![cfg(not(target_arch = "wasm32"))]

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// IPv4 reachability probe target. Cloudflare anycast — small, fast,
/// and TCP DNS is open by design. Hardcoded here (not a runtime knob)
/// because there's no surface today that needs per-deployment override.
const PROBE_IPV4: &str = "1.1.1.1:53";

/// IPv6 reachability probe target. Same Cloudflare anycast over v6.
const PROBE_IPV6: &str = "[2606:4700:4700::1111]:53";

/// Probe budget per call. Five-second polling interval (gui_meta.rs)
/// gives plenty of headroom; one second is short enough that a
/// reachable host always answers in time and an unreachable one trips
/// the timeout cleanly.
const PROBE_TIMEOUT: Duration = Duration::from_millis(1000);

/// Typed reachability snapshot.
///
/// `status` mirrors the gui_meta.rs `net.status` output schema: `"OK"`
/// when the IPv4 probe succeeded, `"FAIL"` otherwise. `ping_ms` is the
/// IPv4 connect round-trip; `ipv6` is `true` iff the IPv6 probe also
/// connected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetStatus {
    pub status: NetReachability,
    pub ping_ms: u64,
    pub ipv6: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetReachability {
    Ok,
    Fail,
}

impl NetReachability {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Fail => "FAIL",
        }
    }
}

/// Sample current network reachability. Blocking — typical RTT is
/// tens of milliseconds on a healthy connection, capped by
/// [`PROBE_TIMEOUT`]. Safe to call from a dispatcher closure; the
/// runtime invokes dispatchers off the UI thread.
pub fn net_status() -> NetStatus {
    let (status, ping_ms) = probe(PROBE_IPV4);
    let ipv6 = match probe(PROBE_IPV6) {
        (NetReachability::Ok, _) => true,
        (NetReachability::Fail, _) => false,
    };
    NetStatus {
        status,
        ping_ms,
        ipv6,
    }
}

fn probe(target: &str) -> (NetReachability, u64) {
    let Some(addr) = first_socket_addr(target) else {
        return (NetReachability::Fail, 0);
    };
    let started = Instant::now();
    match TcpStream::connect_timeout(&addr, PROBE_TIMEOUT) {
        Ok(_) => {
            let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            (NetReachability::Ok, elapsed_ms)
        }
        Err(_) => (NetReachability::Fail, 0),
    }
}

fn first_socket_addr(target: &str) -> Option<SocketAddr> {
    target
        .to_socket_addrs()
        .ok()
        .and_then(|mut iter| iter.next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_reachability_라벨은_ok_fail_매핑이다() {
        assert_eq!(NetReachability::Ok.label(), "OK");
        assert_eq!(NetReachability::Fail.label(), "FAIL");
    }

    #[test]
    fn first_socket_addr는_유효한_target에서_addr를_반환한다() {
        assert!(first_socket_addr("127.0.0.1:80").is_some());
        assert!(first_socket_addr("[::1]:80").is_some());
    }

    #[test]
    fn first_socket_addr는_빈_문자열에_none을_반환한다() {
        assert!(first_socket_addr("").is_none());
    }

    /// Probe behavior is exercised against a closed loopback port —
    /// the OS rejects with ConnectionRefused (or RST), which the
    /// probe normalizes to `Fail`. This avoids any external network
    /// dependency in tests. The probe target is intentionally weird
    /// (port 1, which never has a service) so the connect always fails.
    #[test]
    fn probe는_도달불가_host에_대해_fail을_반환한다() {
        let (status, ping) = probe("127.0.0.1:1");
        assert_eq!(status, NetReachability::Fail);
        assert_eq!(ping, 0);
    }
}
