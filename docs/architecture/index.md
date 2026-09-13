# 아키텍처

## 구조

* [크레이트 경계](crate-boundaries.md) - 도메인 / 런타임 / 어댑터 / surface 네 계층의 소유 범위와 금지 사항.
* [기술 스택](stack.md) - 워크스페이스가 채택한 기술과 거부한 기술, 그리고 각각의 근거.

## 도메인

* [Toolkit과 Tool](toolkit-and-tool.md) - 2단계 호출 계층, Invoker 종류, Tag 상속, 그리고 단일 dispatch 경계.
* [매니페스트 계약](manifest.md) - Toolkit TOML의 구조, invoker별 필수 필드, credential 참조 규칙.
* [I/O 타입 시스템](io-types.md) - 모든 surface가 공유하는 닫힌 입력/출력 타입 집합과 CLI 직렬화 규칙.
* [Chain Tool](chain.md) - Chain의 노드/연결 모델, 표현식 문법, 실행 규칙.
* [프로젝트 매니페스트](project-manifest.md) - `upeg.toml` 자동 탐지, toolbox 병합 우선순위, Board 실행 컨텍스트 주입.

## 프로토콜과 호스트

* [호출 봉투와 예약 컨텍스트](call-envelope.md) - 모든 비-UI 프로토콜이 공유하는 call 봉투, `_upeg` 예약 컨텍스트, CLI positional 바인딩 규칙.
* [HTTP API](http-api.md) - `/v1/*` 리소스 모델, 응답 규칙, CORS 및 bearer 인증 경계.
* [호스트 토폴로지와 Precedence](host-topology.md) - 어떤 surface가 HTTP 호스트가 되는지 결정하는 L1-L4 등급, discovery file, 토큰 인증, 라이프사이클.
* [MCP — Surface와 Import](mcp.md) - upeg이 MCP 서버가 되는 방향(serve)과 MCP 클라이언트가 되는 방향(import), 그리고 각각의 게이트.
