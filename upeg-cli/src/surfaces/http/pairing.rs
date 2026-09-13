//! Local-operator pairing display for `upeg http status --pairing`.
//!
//! Prints a running host's endpoint + bearer token as plain text so a
//! browser/mobile client can be pointed at the ephemeral port without
//! digging through `server.json`.
//!
//! Gate: this is CLI-side display only, never an HTTP endpoint. The
//! token and endpoint are read from the local, `0600`-permissioned
//! `server.json` via [`crate::infrastructure::discovery`] — only the
//! operator running this CLI process can read that file. No route in
//! the parent [`crate::surfaces::http`] module ever serves the token
//! or this block over HTTP.

use crate::infrastructure::discovery::ServerInfo;

/// Printed instead of the pairing block when no host is reachable —
/// there is no endpoint/token to pair with.
const PAIRING_UNAVAILABLE: &str = "Pairing: unavailable (no running host)\n";

/// Pure formatting helper: given a resolved [`ServerInfo`], build the
/// pairing block. Extracted for unit tests — no side effects at all.
pub(crate) fn format_pairing_block(info: &ServerInfo) -> String {
    format!(
        "Pairing:\n  url:   {}\n  token: {}\n",
        info.endpoint, info.token
    )
}

/// `upeg http status --pairing` entry point: resolve the current
/// reachable host (local `server.json` — the gate that keeps this
/// operator-only) and format its pairing block, or a clear
/// "unavailable" message when no host is running.
pub(crate) fn pairing_status_block() -> String {
    match crate::infrastructure::discovery::read_reachable() {
        Some(info) => format_pairing_block(&info),
        None => PAIRING_UNAVAILABLE.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn 가짜_서버_정보() -> ServerInfo {
        ServerInfo {
            endpoint: "http://127.0.0.1:49317".into(),
            mcp_endpoint: "http://127.0.0.1:49317/mcp".into(),
            token: "demo-tok".into(),
            pid: 1234,
            started_at_ms: 1_700_000_000_000,
            origin: crate::infrastructure::discovery::HostOrigin::default(),
        }
    }

    #[test]
    fn 페어링_블록은_엔드포인트와_토큰_라벨을_포함한다() {
        let info = 가짜_서버_정보();
        let block = format_pairing_block(&info);
        assert!(block.starts_with("Pairing:\n"));
        assert!(block.contains("http://127.0.0.1:49317"));
        assert!(block.contains("demo-tok"));
    }

    #[test]
    fn 페어링_블록은_텍스트_두_줄뿐이다() {
        // QR 렌더링은 삭제되었다 — 페어링은 endpoint + token 텍스트다.
        let block = format_pairing_block(&가짜_서버_정보());
        assert_eq!(block.lines().count(), 3, "헤더 + url + token: {block}");
    }

    #[test]
    fn 호스트가_없으면_이용불가_상수_메시지를_사용한다() {
        assert_eq!(
            PAIRING_UNAVAILABLE,
            "Pairing: unavailable (no running host)\n"
        );
    }
}
