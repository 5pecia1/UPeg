---
type: Architecture Contract
title: 크레이트 경계
description: 도메인 / 런타임 / 어댑터 / surface 네 계층의 소유 범위와 금지 사항.
tags: [architecture, soc, crates]
status: stable
sources:
  - id: cargo-workspace
    resource: ../../Cargo.toml
    title: 워크스페이스 멤버 목록
  - id: boundary-gate
    resource: ../../upeg-core/tests/crate_boundaries.rs
    title: 크레이트 의존 그래프 게이트
---

# 계층

의존은 항상 안쪽을 향한다: `surface → adapter → runtime → domain`.

![크레이트 계층 — Surface / Adapter / Runtime / Domain](../diagrams/crate-layers.drawio.svg)

이 그림은 계층 소속을 보여줄 뿐, 실제 엣지 목록은 아래
[강제](#강제)의 게이트가 소유한다.

| Crate | 역할 | 소유 | 소유하지 않음 |
|---|---|---|---|
| `upeg-core` | 도메인 | Toolkit/Tool/Chain/Board 값 타입, 스키마 계약, 순수 검증, 순수 positional 바인딩, capability 판정 | 런타임 toolbox, dispatch, 호스트 I/O, UX 라벨 |
| `upeg-runtime` | 애플리케이션/런타임 경계 | toolbox overlay, dispatch, Trigger 바인딩·실행, embed 바인딩, manifest lowering, 충돌 정책 | 소스 포맷 파싱 세부, surface UI 흐름 |
| `upeg-loader` | 어댑터 | `upeg.toml` 등 소스 포맷 파싱 후 runtime lowering 호출 | 독자적인 toolbox/dispatch 의미론 |
| `upeg-wasm` | 어댑터 | WASM 소스 파싱·호스팅 후 runtime lowering 호출 | 독자적인 toolbox/충돌 정책 |
| `upeg-sources` | 애플리케이션 소스 경계 | 런타임 소스 발견·등록: 사용자 Toolkit, 프로젝트 매니페스트, WASM 플러그인, upstream MCP 서버 | surface UI 흐름, 파서 내부 |
| `upeg-cli` | Surface + 애플리케이션/호스트 | CLI/TUI/HTTP/MCP 진입점과 사용자 I/O, 그리고 **호스트 런타임**: 발견(`server.json`), bearer 인증, 데몬 감독, 임베디드 HTTP 서버, pause 상태, MCP import 로딩, 프로세스 생존 확인(`pid_alive`) | core/runtime에서 복제한 도메인 정책, 순수 경로 해석(→ `upeg-core::paths`) |
| `upeg-pegboard-ui` | UI 상태 | UI 프레임워크 비의존 pegboard 상태(보드/레이아웃/tweak/메모/백업), deep-link 계약, pin chrome, i18n 카탈로그 | 렌더링, 프레임워크별 위젯 코드, surface 크레이트 의존, 그리드 **기하**(셀 크기·포인터 앵커 — Flutter 셸의 Dart가 소유), 그리드 배치 알고리즘(→ `upeg-runtime`) |
| `upeg-frb` | Surface 경계 | Rust↔Dart `flutter_rust_bridge` 표면, 호스트 부트스트랩, 인스턴스 락 | 도메인/런타임 정책 |
| `flutter_app/` | Surface | Flutter desktop/PWA UI 흐름 | toolbox 의미론, 도메인 검증 |

`flutter_app/`은 `upeg-frb`를 통해 `upeg-pegboard-ui` 위에 얹힌다. Flutter가 유일한 GUI
surface이며, 이전 Dioxus `desktop-ui/` 크레이트는 제거되었다.

# surface 사이의 유일한 엣지

`upeg-cli`는 두 얼굴을 가진다. 사용자 진입점(CLI/TUI/HTTP/MCP)이면서 동시에
**호스트 애플리케이션 계층**이다 — 데스크톱 셸이 별도 프로세스를 띄우지 않고
호스트를 *임베드*하기 때문이다(PRD §5.9). 그래서 딱 하나의 surface→surface
엣지가 존재한다:

- **`upeg-frb → upeg-cli` (허용, 문서화된 예외).** `embedded_http_with_ready`,
  `current_host`/`ServerInfo`/`HostOrigin`, `is_paused`/`toggle_paused`,
  `load_mcp_imports_for_host`, `pid_alive` — 모두 호스트 런타임 그 자체다.
  이 엣지는 좁게 유지한다: 순수 경로 해석(`config_root`, `desktop.lock`)은
  `upeg-core::paths`에서 온다.
- **`upeg-frb → upeg-loader`는 테스트에만 있다.** `[target.'cfg(not(target_arch =
  "wasm32"))'.dev-dependencies]`에만 적혀 있어 출하 그래프에는 들어가지 않는다.
  승인 관문은 로더가 등록 시점에 설치하므로 "deep link는 스스로 승인할 수 없다"를
  끝까지 증명하려면 진짜로 등록된 Chain이 필요하고, 부팅 요약의 건너뛴-도구 줄도
  이 호스트가 실제로 건너뛰지 않는 이상 `SkippedTool`을 손으로 만들어야 한다.
  surface→어댑터 방향이라 계층 규칙 자체는 어기지 않는다.
- **`upeg-pegboard-ui → upeg-cli`는 더 이상 존재하지 않는다.** 이 크레이트가
  `upeg-cli`를 필요로 한 이유는 `config_root()` 하나뿐이었고, 그 해석은 원래부터
  `upeg-core::paths`가 소유하고 있었다. UI 상태 크레이트는 이제 공유 저장소
  어댑터(`upeg-sources`)와 도메인/런타임까지만 내려간다.

프로세스 생존 확인(`pid_alive`)의 소유자도 하나다: `upeg-cli`의
`infrastructure::process`. 발견 파일 회수(`server.json` staleness)와 데스크톱
단일 인스턴스 락(`upeg-frb`)이 같은 구현을 호출한다 — 예전처럼 두 벌을 두고
Windows `tasklist` 파싱이 서로 다르게 흘러가는 일은 없다.

# 강제

계층은 리뷰 관습이 아니라 테스트다. `upeg-core/tests/crate_boundaries.rs`가
모든 워크스페이스 멤버의 `Cargo.toml`(`[dependencies]`, `[dev-dependencies]`,
`[build-dependencies]`, `[target.*]` 포함)을 읽어 `upeg-*` 엣지 집합을 만들고
세 가지를 확인한다.

1. 엣지 집합이 `ALLOWED_EDGES` 표와 **정확히 일치**한다. 새 엣지도, 표에만 남은
   낡은 엣지도 실패한다.
2. 모든 엣지가 같거나 더 안쪽 계층을 향한다(`LAYERS`의 rank).
3. 진입점을 소유한 크레이트(`upeg-cli`, `upeg-frb`)를 향하는 엣지는
   `DOCUMENTED_EXCEPTIONS`에 등록된 것뿐이다 — 현재 `upeg-frb → upeg-cli` 하나.

의존을 새로 추가하려면 코드와 함께 그 표를 갱신하고, 이 문서도 같이 고친다.

# Runtime lowering

로더는 자기만의 런타임 진실을 만들지 않는다. 소스별 문법을 타입화된 입력으로 파싱한 뒤
`upeg-runtime` lowering을 호출한다. overlay 우선순위, static id 보호, 런타임 중복 교체,
Trigger 등록, embed 바인딩 정규화, manifest→toolbox 변환은 **오직 lowering에서만** 적용된다.

# 아키텍처 스타일

- 백엔드는 실제 I/O 이음매에서만 Hexagonal을 적용한다: CLI 인자, 파일 시스템, 환경변수,
  HTTP, WASM 호스트, credential 해석, 프로세스 실행. 순수 도메인 함수에는 trait 래퍼를 두지 않는다.
- TUI는 `ratatui` + `crossterm` 위의 직접 TEA(update/view)를 쓴다. `tui-realm`은 프레임워크
  상태 기계를 더할 뿐 이득이 부족해 거부되었다.
- `Justfile`이 명령·CI 미러다.

관련: 모듈 레이아웃 규칙, [Toolkit과 Tool](/architecture/toolkit-and-tool.md)
