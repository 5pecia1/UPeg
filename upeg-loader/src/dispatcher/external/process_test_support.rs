use std::io::ErrorKind;
use std::path::Path;
use std::time::{Duration, Instant};

const PROCESS_STATE_PREFIX: &str = "State:";
const PROCESS_STATE_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessLifecycle {
    Running,
    Exited,
}

pub(super) fn lingering_process_state(pid: u32, max_wait: Duration) -> Option<String> {
    let status_path = Path::new("/proc").join(pid.to_string()).join("status");
    let deadline = Instant::now() + max_wait;

    loop {
        match std::fs::read_to_string(&status_path) {
            Ok(status) if lifecycle(&status) == ProcessLifecycle::Exited => {
                return None;
            }
            Ok(status) if Instant::now() >= deadline => return Some(status),
            Ok(_) => {}
            Err(source) if source.kind() == ErrorKind::NotFound => return None,
            Err(source) if Instant::now() >= deadline => {
                return Some(format!("status read failed: {source}"));
            }
            Err(_) => {}
        }
        std::thread::sleep(PROCESS_STATE_POLL_INTERVAL);
    }
}

fn lifecycle(status: &str) -> ProcessLifecycle {
    let state = status
        .lines()
        .find_map(|line| line.strip_prefix(PROCESS_STATE_PREFIX))
        .and_then(|value| value.trim_start().chars().next());

    match state {
        Some('X' | 'Z') => ProcessLifecycle::Exited,
        Some(_) | None => ProcessLifecycle::Running,
    }
}
