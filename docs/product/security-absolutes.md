---
type: Policy
title: 보안 절대 원칙
description: 어떤 기능도 위반할 수 없는 비밀·프로젝트 매니페스트·네트워크·임베드 취급 규칙.
tags: [product, security, credentials, network, manifest]
status: stable
---

이 절의 항목은 트레이드오프 대상이 아니다. 충돌하는 기능은 기능 쪽을 접는다.

# 비밀

1. Credential은 TOML에 **이름(참조)만** 저장한다. 평문 값은 매니페스트, 프로젝트 상태,
   실행 로그, HTTP/MCP 응답 어디에도 기록하지 않는다.
2. 비밀 값은 OS keychain 또는 환경변수에만 존재하며 실행 경계에서만 해석된다.
3. 비밀 값은 클라우드 동기화 대상이 아니다.
4. 실행 로그는 메타데이터 전용이다: 시각, tool id, invoker, surface, board, status,
   duration, error class. 인자 값과 비밀은 기록하지 않는다.
5. 검증된 암호 라이브러리만 쓴다. 자체 crypto/vault를 발명하지 않는다.

# 동기화

6. Zero-knowledge — 서버는 평문을 볼 수 없다.
7. 마스터 비밀번호를 서버에 저장하지 않는다.
8. 비밀번호 분실은 데이터 분실이다 (1Password 모델).

# 프로젝트 매니페스트

9. 프로젝트 `upeg.toml` 탐지는 상위 디렉터리를 **`$HOME` 밖으로 걷지 않는다** — cwd와,
   `$HOME` 안에 머무는 조상 디렉터리, 그리고 `$HOME` 자신까지만 확인한다. World-writable한
   조상 디렉터리(예: `/tmp/x/upeg.toml`)가 `invoker = "External"` Tool로 임의 명령을 동의
   없이 끌어들이는 경로를 차단한다 ([프로젝트 매니페스트](/architecture/project-manifest.md)).

# 네트워크

10. 네트워크 인터페이스는 explicit-start이며 loopback-first다.
11. 비-loopback bind는 명시적 동의(`UPEG_HTTP_ALLOW_NON_LOOPBACK=1`)를 요구하고, 이 경우
    토큰 자동 생성이 금지되며 반드시 주입해야 한다.
12. `/healthz`를 제외한 모든 HTTP 라우트는 bearer 인증을 거친다. Host anti-rebinding 가드는
    CORS와 독립적으로 유지된다.
13. 실행 중인 네트워크 인터페이스는 상태 표시로 상시 가시화한다: HTTP 활성 여부, MCP 활성
    여부, trigger listener 수, remote-bind 동의 상태.

# 임베드

14. Embed WebView는 별도 샌드박스에서 돈다.
15. Embed selector 매핑은 사용자가 명시적으로 확인한다.
16. 숨은 Controlled Embed 엔진 webview는 포인터·시맨틱·키보드 포커스를 절대 받지 않는다.

구현 계약: [HTTP API](/architecture/http-api.md), [호스트 토폴로지](/architecture/host-topology.md),
[프로젝트 매니페스트](/architecture/project-manifest.md), [Surface 계약](/ui-ux-surface-contract.md)
