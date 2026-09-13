---
type: Product Definition
title: 제품 정체성과 경계
description: upeg이 무엇이고 무엇이 아닌지 — 한 줄 정의, 하는 일 / 하지 않는 일 / 한계.
tags: [product, scope, boundaries]
status: stable
---

# 한 줄

> 자주 쓰는 도구(단일 또는 체인)를 한 보드에 핀해두고, 어느 환경에서든 누구나(사람·AI·앱)
> 검색·탭 전환·열기 없이 즉시 호출한다.

utility 모음도 아니고 워크플로우 빌더도 아니다. **임의의 도구를 임의의 호출자에 연결하고,
그 결과를 페그보드에 핀해 즉시 재호출하는 layer**다. 단일 도구가 Tool이듯 **체인 자체도 하나의
Tool이다** — 복잡한 체인도 결국 핀 하나로 소비된다.

# 하는 일

- 자주 쓰는 Tool을 Board에 핀한다.
- 사람·AI·앱이 7개 surface(CLI/TUI/Desktop/PWA/Ext/MCP/HTTP)에서 같은 Tool을 호출한다.
- Toolkit + Tool 매니페스트 하나로 모든 surface에 자동 노출한다.
- 외부 도구를 감싼다 (TOML / WASM / upstream MCP 서버). 코드 0줄 마이그레이션.
- 외부 SaaS를 두 방식으로 임베드한다: Passive Embed(웹뷰 자체가 경험)와 Controlled
  Embed(selector로 조종해 네이티브 폼/결과로 표시).
- Chain Tool — 여러 Tool을 선언적으로 연결하고, 조건 분기와 승인 step을 지원한다.
- Trigger — 클립보드·핫키·스케줄·파일·디렉터리·webhook 조건에 자동 실행한다.
- Credential — 외부 API 인증 정보를 typed schema + OS keychain 참조로 분리 관리한다.
- Execution Log — 모든 Tool 호출 메타데이터를 로컬에 기록한다 (값 제외).
- 키보드만으로 페그보드를 완전히 제어한다.
- 가볍다: 단일 바이너리, lazy load, 필요할 때만 프로세스를 시작한다.
- 로컬 우선. 클라우드는 선택이며 E2E다.
- 모바일 friendly — first는 아니다.

# 하지 않는 일

- 자체 password vault, 자체 crypto, 자체 결제.
- 1등 외부 도구 대체 (Notion / Obsidian / 1Password / Dropbox / Figma / VSCode).
- 사용자 데이터 평문 서버 저장.
- 자체 IDE / 코드 에디터, 실시간 동시 편집.
- 모바일 first 디자인.
- 직군별 특수 핀을 upeg이 직접 제작.
- SaaS UI 자동 추적 (selector 자동 갱신).
- 사용자 동의 없이 네트워크 인터페이스 활성화.
- 무거운 의존성 (Electron, JVM).
- 시각적 워크플로우 에디터를 코어에 내장.
- OS 수준 마우스·키보드 제어. upeg 핫키는 Tool 호출 트리거에 한정한다.
- 언어·런타임 버전 관리 (mise/asdf 영역).

# 한계

- PWA와 Chrome ext는 `wasm32`/샌드박스 호스트다. 로더 런타임과 네이티브 전용 dispatcher가
  없으므로 해당 도구는 정직한 미지원 상태로 렌더되거나, 로컬 호스트와 페어링해 원격 실행된다
  ([Surface 계약](/ui-ux-surface-contract.md)).
- SaaS UI가 바뀌면 Controlled Embed selector가 깨진다. 사전 검사는 없고 호출 시 에러가 나며
  사용자가 재매핑한다.
- SaaS 약관 회색지대 — 사용자 본인 사용에 한정한다.
- Credential 값은 OS keychain에만 있고 클라우드 동기화 대상이 아니다.
- `hotkey` 트리거는 플랫폼 global-hotkey 어댑터와 그래픽 세션에 의존한다. 없는 호스트에서는
  트리거 목록이 미지원 진단을 그대로 보여준다.
- `Invoker::Llm`은 LLM API 네트워크가 필요하다. 로컬 LLM은 credential로 연결한다.
- 닫힌 I/O 타입 집합 밖의 타입은 지원하지 않는다.

