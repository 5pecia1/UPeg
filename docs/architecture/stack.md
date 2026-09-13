---
type: Technology Decision
title: 기술 스택
description: 워크스페이스가 채택한 기술과 거부한 기술, 그리고 각각의 근거.
tags: [architecture, stack, dependencies]
status: stable
---

# 원칙

1. Rust-first 코어/도메인. CLI/TUI/MCP/HTTP surface는 단일 Rust 바이너리다. UI surface
   (Desktop/Web/PWA)는 Dart(Flutter) 런타임에 view + 플랫폼 통합을 위임하되 비즈니스 로직은
   Rust crate에 남긴다. Dart AOT 비용(~10MB)을 수용하는 대신 muda/tao/wry 직접 통합 유지비를 회피한다.
2. Electron/JVM/Node 코어 런타임 없음. Dart는 UI 계층 한정.
3. 하나의 도메인 toolbox가 모든 surface에 공급한다.
4. 도입하는 복잡도보다 더 많은 복잡도를 제거할 때만 의존성을 추가한다.
5. 보안은 검증된 OS 기능을 쓴다. crypto나 vault 의미론을 발명하지 않는다.
6. 폐기된 개념은 호환 유지가 아니라 삭제한다.

# 채택

| 영역 | 선택 | 근거 |
|---|---|---|
| 언어 | Rust 2024 | 강한 타입, proc macro, static inventory, WASM 타깃 |
| Toolbox | `inventory` + 런타임 overlay 맵 | Static/Declarative/Project/WASM Tool이 한 lookup 경로를 공유 |
| 직렬화 | `serde`, `serde_json`, `toml` | 매니페스트/API 표준 스택 |
| CLI | `clap` + `clap_complete` | 직접 호출, 로그, credential, trigger, 자동완성 |
| TUI | `ratatui` + `crossterm` | 가벼운 키보드 우선 surface |
| HTTP | `axum` + `tokio` | REST API와 OpenAPI 호환 메타데이터 |
| HTTP 클라이언트 어댑터 | 작은 동기 std/어댑터 계층 | 로컬 결정론적 dispatch 테스트에 무거운 클라이언트 의존성을 피한다 |
| WASM 호스트 | `extism` (feature flag 뒤) | 기본 빌드 비용 없이 플러그인 모델 |
| Desktop / Web (PWA) | Flutter + `flutter_rust_bridge` | UI cross-platform 일관성 + 네이티브 위젯/플랫폼 통합. Rust 도메인은 cdylib(데스크톱)과 wasm(웹) 두 타깃으로 같은 코드를 노출 |
| Chrome ext | 순수 HTML/JS | 팝업이 직접 dispatch와 `upeg://` deep link만 다루므로 프레임워크 불필요 |
| 명령/CI 미러 | `Justfile` | 검증 명령이 발견 가능하고 조합 가능하면서 가볍다 |
| Credential | 환경변수/OS keychain 참조 어댑터 경계 | TOML은 이름만 저장하고 값은 실행 경계에서 해석되며 로그되지 않는다 |

# 저장

| 필요 | 선택 |
|---|---|
| Pegboard 상태, 메모, 실행 로그, 마지막 결과 | 설정 루트 아래 단일 WAL 모드 SQLite (`upeg.db`) |
| 스키마 버전 | `PRAGMA user_version` + append-only 마이그레이션. **파일명에 버전을 넣지 않는다** |
| Credential | 이름 참조를 어댑터가 해석. 평문 매니페스트/로그 영속 없음 |
| 실행 로그 | 메타데이터 전용 이벤트: 시각, tool id, invoker, surface, board, status, duration, error class |
| 네트워크 인터페이스 | explicit-start, loopback-first, remote-bind 동의, 가시적 상태 |

# 라이선스 게이트

의존성은 다음 라이선스 정책을 따른다: MIT, Apache-2.0, BSD-2/3-Clause, ISC, MPL-2.0, Zlib,
Unicode-3.0, CC0-1.0. 카피레프트(GPL/AGPL/CC-BY-SA/SSPL)는 거부한다.

서드파티 crate를 vendoring할 때는 `vendor/<crate>/UPEG.md`에 출처·아카이브
SHA·로컬 패치·라이선스 고지를 기록하고 워크스페이스 멤버와 소스 예산에서
제외한다(첫 사례: pdf-inspector 1.17.0, MIT + Adobe BSD-3 CMap).
