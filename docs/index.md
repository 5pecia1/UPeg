---
okf_version: "0.2"
---

# Universal Pegboard 지식 번들

하나의 Tool 정의가 7개 surface(CLI / TUI / Desktop / PWA / Chrome ext / MCP / HTTP)에서
동일하게 동작하는 Rust 워크스페이스의 계약 문서다.

* [Lexicon](LEXICON.md) - 제품·UI·CLI·매니페스트·코드가 공유하는 단일 어휘. 개명 전에 여기부터 본다.
* [UI/UX Surface Contract](ui-ux-surface-contract.md) - TUI / Desktop·PWA / Chrome extension이 공유하는 Tool 라이프사이클, 키 바인딩, capability 렌더링.
* [External Tool Manifest Guide](TOOL_MANIFEST.md) - Toolkit TOML 전체 필드 레퍼런스. Rust 타입에서 생성되며 손으로 고치지 않는다.

# 제품

* [제품 정체성과 경계](product/identity-and-boundaries.md) - 무엇이고 무엇이 아닌지.
* [보드와 에이전트 작업 흐름](product/board-agent-workflow.md) - 개인·프로젝트 보드의 도구와 안내를 준비해 MCP에서 사용하는 절차.
* [보안 절대 원칙](product/security-absolutes.md) - 비밀·네트워크·임베드 취급의 비타협 규칙.

# 아키텍처

* [크레이트 경계](architecture/crate-boundaries.md) - 도메인 / 런타임 / 어댑터 / surface 네 계층의 소유 범위.
* [Toolkit과 Tool](architecture/toolkit-and-tool.md) - 2단계 호출 계층, Invoker, Tag 상속, 단일 dispatch 경계.
* [매니페스트 계약](architecture/manifest.md) - Toolkit TOML 구조, invoker별 필수 필드, credential 참조 규칙.
* [I/O 타입 시스템](architecture/io-types.md) - 닫힌 입출력 타입 집합과 인라인 제약.
* [Chain Tool](architecture/chain.md) - 노드/연결 모델, 표현식 문법, 실행 규칙.
* [호출 봉투와 예약 컨텍스트](architecture/call-envelope.md) - 공유 call 봉투, `_upeg` 컨텍스트, CLI positional 바인딩과 셸 자동완성.
* [프로젝트 매니페스트](architecture/project-manifest.md) - `upeg.toml` 자동 탐지와 병합 우선순위.
* [HTTP API](architecture/http-api.md) - `/v1/*` 리소스 모델, 응답 규칙, CORS와 bearer 인증.
* [호스트 토폴로지와 Precedence](architecture/host-topology.md) - L1-L4 호스트 등급, discovery file, 토큰, 라이프사이클.
* [MCP — Surface와 Import](architecture/mcp.md) - upeg이 서버가 되는 방향과 클라이언트가 되는 방향, MCP 임포트 eager load 규칙.
* [기술 스택](architecture/stack.md) - 채택한 기술, 거부한 기술, 라이선스 게이트.

# 가이드

* [도구 작성자 가이드](guides/tool-author.md) - `#[tool(...)]` 매크로의 키와 변형, 검증 절차.
* [PDF 도구 안내](pdf-tools.md) - `media.pdf_inspect`/`media.pdf_to_markdown`의 추출 상태·제한·배포 방식.

# 다이어그램

`diagrams/` 의 `*.drawio.svg` 는 편집 가능한 SVG다 — 브라우저·GitHub에서 그대로 보이고,
draw.io 로 열면 원본 다이어그램이 그대로 복원된다. 별도 소스 파일을 두지 않는다.
고친 뒤에는 같은 이름으로 다시 export 한다.
