use std::io::ErrorKind;
use std::path::Path;
use std::time::{Duration, Instant};

const 프로세스_상태_접두사: &str = "State:";
const 프로세스_상태_확인_간격: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum 프로세스_생명주기 {
    실행_중,
    종료됨,
}

pub(super) fn 종료되지_않은_프로세스_상태(
    pid: u32,
    최대_대기: Duration,
) -> Option<String> {
    let status_path = Path::new("/proc").join(pid.to_string()).join("status");
    let deadline = Instant::now() + 최대_대기;

    loop {
        match std::fs::read_to_string(&status_path) {
            Ok(status) if 생명주기(&status) == 프로세스_생명주기::종료됨 => {
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
        std::thread::sleep(프로세스_상태_확인_간격);
    }
}

fn 생명주기(status: &str) -> 프로세스_생명주기 {
    let state = status
        .lines()
        .find_map(|line| line.strip_prefix(프로세스_상태_접두사))
        .and_then(|value| value.trim_start().chars().next());

    match state {
        Some('X' | 'Z') => 프로세스_생명주기::종료됨,
        Some(_) | None => 프로세스_생명주기::실행_중,
    }
}
