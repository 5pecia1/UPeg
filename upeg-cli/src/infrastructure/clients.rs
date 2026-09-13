//! Live client registry (PRD §5.9 "Connected clients" panel).
//!
//! In-memory map of `client_id → ClientEntry` populated by
//! `POST /v1/clients/heartbeat` and filtered by 30 s TTL on read.
//! No persistence: every host restart starts empty, which matches
//! the PRD's "live" semantics.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const TTL_MS: u64 = 30_000;

#[derive(Debug, Clone)]
pub struct ClientEntry {
    pub label: String,
    pub last_seen_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LiveClient {
    pub client_id: String,
    pub label: String,
    pub last_seen_ms: u64,
    pub age_seconds: u64,
}

fn registry() -> &'static Mutex<HashMap<String, ClientEntry>> {
    static R: OnceLock<Mutex<HashMap<String, ClientEntry>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn record_heartbeat(client_id: String, label: String) {
    let now = now_ms();
    let mut guard = match registry().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    guard.insert(
        client_id,
        ClientEntry {
            label,
            last_seen_ms: now,
        },
    );
    prune_locked(&mut guard, now);
}

pub fn live_clients() -> Vec<LiveClient> {
    let now = now_ms();
    let mut guard = match registry().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    prune_locked(&mut guard, now);
    guard
        .iter()
        .map(|(id, entry)| LiveClient {
            client_id: id.clone(),
            label: entry.label.clone(),
            last_seen_ms: entry.last_seen_ms,
            age_seconds: (now.saturating_sub(entry.last_seen_ms)) / 1000,
        })
        .collect()
}

fn prune_locked(map: &mut HashMap<String, ClientEntry>, now: u64) {
    map.retain(|_, entry| now.saturating_sub(entry.last_seen_ms) < TTL_MS);
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
    use std::time::Duration;

    #[test]
    fn 기록_그뒤_목록은_해당_항목을_반환한다() {
        record_heartbeat("test-1".into(), "mcp".into());
        let list = live_clients();
        assert!(list.iter().any(|c| c.client_id == "test-1"));
        // Cleanup for cross-test isolation.
        registry().lock().unwrap().remove("test-1");
    }

    #[test]
    fn 만료된_항목은_정리된다() {
        let mut guard = registry().lock().unwrap();
        guard.insert(
            "test-expired".into(),
            ClientEntry {
                label: "old".into(),
                last_seen_ms: now_ms().saturating_sub(TTL_MS + 1000),
            },
        );
        drop(guard);
        std::thread::sleep(Duration::from_millis(5));
        let list = live_clients();
        assert!(list.iter().all(|c| c.client_id != "test-expired"));
    }
}
