---
type: Runtime Contract
title: 호스트 토폴로지와 Precedence
description: 어떤 surface가 HTTP 호스트가 되는지 결정하는 L1-L4 등급, discovery file, 토큰 인증, 라이프사이클.
tags: [architecture, host, surfaces, security]
status: stable
sources:
  - id: attach
    resource: ../../upeg-cli/src/infrastructure/attach.rs
    title: 호스트 attach 클라이언트
  - id: host-tokens
    resource: ../../upeg-cli/src/infrastructure/auth.rs
    title: operator / agent 토큰 해석
---

# 원칙

1. **UPeg surface가 호스트를 공유할 때는 HTTP loopback을 사용한다.** 에이전트와의 stdio MCP 연결은 별도다. Unix socket과 `upeg daemon`은 폐기되었다.
2. **`server.json`이 단일 출처다** — discovery + auth + lifecycle 전부.
3. **Host Precedence(L1-L4)가 충돌을 결정론적으로 해소한다.** 사용자가 고르지 않아도 된다.
4. **explicit-start를 보존한다.** 어떤 surface도 다른 surface를 암묵적으로 spawn하지 않는다.
5. **Tray는 surface가 아니라 GUI의 진입 방식이다** — `flutter_app/`에 통합되어 있다.
6. **바이너리 2개로 충분하다**: `upeg`(헤드리스 가능) + desktop GUI. feature flag가 필요 없다.

# Surface 카탈로그와 호스트 등급

같은 설정 루트에서 자동 발견하는 HTTP 호스트는 **최대 하나**다.

| 등급 | Surface | 호스트 자격 | 라이프사이클 |
|---|---|---|---|
| **L1** | `upeg host start --daemon` | 있음 (영속, 강한 의도) | 명시적 stop까지 |
| **L2** | Desktop GUI + Tray | **`Tweaks.local_http_host`가 켜져 있을 때만** (기본 OFF) | 사용자 세션 |
| **L3** | `upeg host start` (foreground) | 있음 (명시 의도, 짧을 수 있음) | 셸 점유 |
| **L4** | `upeg call`, `upeg mcp`, `upeg tui` | 없음 (client 또는 in-process) | 호출 단발 / 셸 점유 / 부모 종속 |

**L2는 조건부다.** Desktop GUI는 호스트가 없다고 해서 자동으로 호스트가 되지 않는다.
사용자가 Settings에서 "Local HTTP host"(`Tweaks.local_http_host`, 기본 OFF)를 켰을 때만
in-process 호스트를 띄운다 — 명시적 결정 없이 네트워크 리스너를 여는 일은 없다(FR-16).
꺼져 있으면 Desktop은 붙을 호스트가 없는 상태로 뜬다.

# 시작 알고리즘

모든 surface가 동일하게 거친다.

![호스트 시작 알고리즘 — 등급에 따라 attach / 호스트 / in-process로 갈린다](../diagrams/host-precedence.drawio.svg)

| 기존 상태 | 새로 시작 | 결과 |
|---|---|---|
| 없음 | L1/L3 | 그 surface가 호스트 |
| 없음 | Desktop GUI, `local_http_host` ON | in-process 호스트를 띄운다 (L2) |
| 없음 | Desktop GUI, `local_http_host` OFF (기본) | 호스트 없이 뜬다 — 리스너를 열지 않는다 |
| 호스트 있음 | Desktop GUI | 기존 호스트를 발견한다. Desktop의 Tool 실행은 자체 런타임을 사용한다 |
| Desktop 호스트 중 | `upeg host start [--daemon]` | 에러 — "already hosted, stop it first" |
| 호스트 있음 | `upeg tui` / `upeg call` | 공유 가능한 호출은 attach. 프로젝트 Tool은 자체 실행 |
| 호스트 있음 | `upeg mcp` | 보드 미지정은 stdio ↔ HTTP `/mcp` proxy. `--board`는 자체 실행 |
| 없음 | L4 전부 | in-process (auto-spawn 없음) |

**호스트 종료 시 client는 자동 takeover하지 않는다.** 사용자에게 명시적으로 알리고 degraded
모드에 들어가거나 in-process로 폴백한다. takeover race는 단순성 대비 가치가 낮다.

Desktop은 Flutter WebView 공급자를 자신의 Rust 런타임에 등록한다. 이 Desktop이 내장
HTTP 호스트를 제공할 때 외부 CLI의 Controlled Embed 호출도 같은 앱 소유 페이지를 사용한다.
일반 실행·디버그·HTTP 요청은 같은 대기 및 결과 변환 규칙을 거친다. 공급자가 사라진 호출은
`controlled_embed_unavailable`을 반환하며 headless로 재실행하지 않는다. 별도 `upeg host`
프로세스는 자기 headless backend를 사용하므로 Desktop 공급자와 교체 가능한 실행 대상이 아니다.

세션 수명과 GUI 디버그의 상세 계약은
[Desktop Controlled Embed의 공통 실행과 디버그](/ui-ux-surface-contract.md#desktop-controlled-embed의-공통-실행과-디버그)를 따른다.

# Discovery file

- 위치: Unix `~/.upeg/server.json`, Windows `%APPDATA%\upeg\server.json`
- 권한: Unix `0600`, Windows 현재 사용자만 허용 ACL

```json
{
  "endpoint": "http://127.0.0.1:49317",
  "mcp_endpoint": "http://127.0.0.1:49317/mcp",
  "token": "<32-byte URL-safe>",
  "pid": 12345,
  "started_at_ms": 1700000000000,
  "origin": "explicit"
}
```

`origin`은 `explicit`(사용자가 실행한 `upeg host start`) 또는 `embedded`(Desktop이 자기
프로세스 안에 띄운 호스트)다. Desktop은 이 값과 pid로 "reachable한 호스트가 내 embed인지
남의 프로세스인지"를 구분한다. 키가 없는 파일은 보수적으로 `explicit`으로 읽는다.

## Stale 감지 (2단 검증)

reachable 판정은 endpoint health check(`GET /healthz`) 하나로 한다. 응답하면 reachable이다.

응답하지 않을 때만 두 번째 신호를 본다: PID alive 확인(Unix `kill(pid, 0)`, Windows
`OpenProcess` + `GetExitCodeProcess`).

- **응답 없음 + PID 죽음** → stale. 파일을 정리한다.
- **응답 없음 + PID 살아 있음** → stale이 **아니다**. 아직 부팅 중이거나 바쁜 호스트일 뿐이며,
  여기서 파일을 지우면 살아 있는 daemon을 고아로 만든다. 파일을 보존하고 "지금은 reachable
  하지 않음"만 호출자에게 알린다.

`started_at_ms`는 파일에 기록되지만 **stale 판정에는 쓰이지 않는다.** OS 보고 프로세스 시작
시각과 대조해 PID 재활용을 막는 3번째 신호는 아직 구현되어 있지 않다(미래 하드닝).

호스트는 시작 시 출판하고 정상 종료 시 RAII로 정리하며(파일이 여전히 자기 pid를 가리킬 때만
지운다), 비정상 종료분은 다음 surface가 stale 감지로 치운다.

# 인증

호스트는 **operator 토큰 하나**와 **agent 토큰 여러 개**를 받아들인다. 형태는 같고 권한만
다르다 — 어느 쪽을 실었는지가 그 호출의 `_upeg.principal.role`이 된다
([호출 봉투](/architecture/call-envelope.md)).

## operator 토큰

- 호스트 시작 시 32-byte URL-safe 랜덤 토큰을 생성해 `server.json`에 출판한다.
- **Same-OS-user 검증은 파일시스템 권한이 담당한다** (`0600` / ACL). 사용자가 토큰을 보거나
  복사할 필요가 없다.
- 명시 주입: `UPEG_HTTP_TOKEN` 환경변수 또는 `--token-file <PATH>` (CI/스크립트용).
- 토큰 회전은 재시작 시에만 일어난다.
- 이 토큰을 실은 요청은 이 호스트를 띄운 사람으로 취급된다: `X-Upeg-Origin-Surface`가
  인정되고, Chain의 승인 장벽을 넘을 수 있다.

## agent 토큰 (선택)

프로그램에게 데이터 평면을 열어 주되 **사람의 권한은 주지 않는** 경로다.

```bash
UPEG_HTTP_AGENT_TOKENS="tok-a,tok-b" upeg host start --daemon
```

| 항목 | 내용 |
|---|---|
| 설정 | `UPEG_HTTP_AGENT_TOKENS` — 콤마 구분. 빈 항목은 무시하고 중복은 합친다. 미설정이면 agent 접근은 없다 |
| 값 | operator 토큰과 같은 형태를 권장한다 (`upeg`가 만들어 주지는 않는다 — 운영자가 고른다) |
| 할 수 있는 것 | `/healthz`를 제외한 모든 라우트. 도구 실행, 목록, 로그 조회 |
| 할 수 없는 것 | `X-Upeg-Origin-Surface` 선언(항상 `http`로 각인된다), Chain 승인(`approval_denied_for_principal`) |
| 기록 | 실행 로그의 `principal` 열에 `agent`로 남는다. 어느 토큰이었는지는 남기지 않는다 |

**왜 플래그가 아니라 환경변수인가.** 반복 가능한 `--agent-token` 플래그가 더 눈에 띄겠지만,
명령줄은 upeg가 지원하는 모든 플랫폼에서 `ps` 출력으로 세어 읽힌다 — 그래서 operator 토큰도
`--token` 옆에 `--token-file`을 두고 있다. 게다가 Desktop 내장 호스트(L2)에는 플래그를 붙일
argv 자체가 없고 환경만 물려받는다. 두 lane을 하나로 답하는 것은 환경변수뿐이다.

## Non-loopback bind

기본은 `127.0.0.1:0`(ephemeral loopback)이다. `UPEG_HTTP_ALLOW_NON_LOOPBACK=1` +
`--addr 0.0.0.0:7173` 명시 opt-in 시:

- 토큰은 **반드시** `UPEG_HTTP_TOKEN` / `--token-file`로 주입해야 한다 (자동 생성 금지).
- Origin 화이트리스트는 별도 정책을 따른다.
- `server.json` 출판은 opt-in이다 — 서버 환경에서 의도치 않은 local discovery를 막는다.

# 라이프사이클 명령

```bash
upeg host start                          # foreground (L3)
upeg host start --daemon                 # detach 백그라운드 (L1)
upeg host status [--json]                # endpoint + pid + uptime
upeg host stop [--force]                 # SIGTERM 5s grace, --force는 SIGKILL
upeg host logs [--lines N]               # 로그 tail
upeg http status --pairing               # 페어링 블록 (아래)
```

- Detach: 워크스페이스가 `unsafe`를 금지하므로 `fork()`/`setsid()`를 쓰지 않는다(`setsid`는
  `pre_exec`가 필요하다). Unix에서는 **현재 실행 파일을 같은 인자로 재spawn**하고
  stdin은 `null`, stdout/stderr는 로그 파일로 붙인 뒤 부모가 종료한다 — 자식은 부모가
  사라질 때 커널 reparenting으로 세션에서 떨어진다. 자식은 마커 환경변수
  (`UPEG_HTTP_DETACHED`)를 보고 다시 detach하지 않는다. Windows는
  `CreationFlags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)`로 같은 일을 네이티브로 한다.
- 로그: `~/.upeg/upeg-http.log` (또는 `--log-file`). `RUST_LOG` 준수. 10MB 도달 시 `.1`로
  rename하는 단일 파일 rotation.
- Shutdown: SIGTERM/SIGINT/Ctrl-Break에 in-flight 요청 5s grace, 그다음 `server.json` 정리.

# 페어링 표시

`upeg http status --pairing`은 실행 중인 호스트의 endpoint + token을 텍스트로 출력한다.
브라우저/모바일 클라이언트가 ephemeral 포트와 토큰을 손으로 옮겨 적지 않아도 된다.

**CLI 전용 표시다.** 값은 `0600` 권한의 로컬 `server.json`에서 읽으며, 어떤 HTTP 라우트도
이 블록이나 토큰을 노출하지 않는다. `--json` 대응물이 없고 `GET /healthz`에도 보이지 않는다.

# 멀티유저

OS 사용자마다 자기 surface 인스턴스와 자기 `~/.upeg/`(또는 `%APPDATA%\upeg\`), 자기 임시
포트를 갖는다. 같은 머신 내 사용자 간 공유 인스턴스는 범위 밖이다.

# 플랫폼별 UX

| 플랫폼 | 기본 | 비고 |
|---|---|---|
| macOS | Menubar 전용 (`LSUIElement=true`) | 설정 토글로 Dock 표시 전환 |
| Windows | 알림 영역 (system tray) | — |
| Linux (SNI 가능) | 시스템 트레이 | — |
| Linux (SNI 미지원) | tray 설치가 silent fail하고 desktop window로만 사용 | AppIndicator 확장 안내 |

관련: [Surface 계약의 host-attach 절](/ui-ux-surface-contract.md)
