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

    fn fake_server_info() -> ServerInfo {
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
    fn pairing_block_contains_endpoint_and_token_labels() {
        let info = fake_server_info();
        let block = format_pairing_block(&info);
        assert!(block.starts_with("Pairing:\n"));
        assert!(block.contains("http://127.0.0.1:49317"));
        assert!(block.contains("demo-tok"));
    }

    #[test]
    fn pairing_block_has_only_header_url_and_token_lines() {
        // QR rendering was removed — pairing is endpoint + token text.
        let block = format_pairing_block(&fake_server_info());
        assert_eq!(block.lines().count(), 3, "header + url + token: {block}");
    }

    #[test]
    fn uses_unavailable_constant_when_no_host_is_running() {
        assert_eq!(
            PAIRING_UNAVAILABLE,
            "Pairing: unavailable (no running host)\n"
        );
    }
}
