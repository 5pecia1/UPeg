---
type: Lexicon
title: Lexicon
description: 제품·UI·CLI·매니페스트·코드가 공유하는 단일 어휘 — 개념 하나에 단어 하나.
tags: [lexicon, vocabulary, naming]
status: stable
---

# 원칙

**개념 하나에 단어 하나, 모든 surface에서 동일하게.** UI 라벨, CLI 출력, TOML 키, Rust 심볼,
JSON 필드, 한국어 카피가 같은 용어를 쓴다. `code term` / `user term` 분리는 없다.

호출 계층은 **Toolkit → Tool**, 탐색은 **Tag**, `category`는 폐기되었다.

이 문서의 경로는 `upeg-core`와 `upeg-runtime`의 interface inventory가 참조하고 존재를
검증한다. 옮기려면 코드도 함께 고쳐야 한다.

# 제품 정체성

| Term | 한국어 | 의미 |
|---|---|---|
| Universal Pegboard | 유니버설 페그보드 | 정식 제품명 |
| `upeg` | 유페그 | CLI 명령, crate/패키지 prefix, 짧은 제품명 |
| Pegboard | 페그보드 | 핀된 Tool을 사용하는 메인 캔버스 |

# 어휘

각 행이 그 개념의 **유일한 정규 용어**다. 열에 다른 표기가 명시되지 않는 한 UI/TOML/Rust/JSON이
같은 단어를 쓴다. 한국어는 산문의 정본 번역이고, 식별자는 영어로 남는다.

| Term | 한국어 | TOML 키 | Rust 심볼 | JSON 필드 | 정의 |
|---|---|---|---|---|---|
| Toolkit | 툴킷 | `[toolkit]` | `Toolkit`, `ToolkitMeta` | `toolkit` | 호출 불가능한 그룹 / 배포 네임스페이스. 예: `convert`. |
| Tool | 도구 | `[tool]` | `Tool`, `ToolMeta` | `tool` | 정확히 하나의 Toolkit이 소유하는 호출 단위. Full id: `{toolkit}.{tool}`. |
| Tag | 태그 | `tags = [...]` | `tags` | `tags` | Toolkit에서 상속되고 Tool 수준에서 추가되는 탐색/필터 라벨. |
| Pin | 핀 | `boards = [...]` (멤버십) | `pin_tool_to_board()` | (동작) | Tool을 Board에 붙이는 행위, 그리고 그 결과로 보이는 부착물. |
| PinKind | 핀 종류 | `pin = "Inline"` | `PinKind` | `pin` | Tool이 보드에 제시되는 방식. 값: `Inline`, `Launcher`, `Live`, `Action`, `Embed`, `ControlledEmbed`, `Chain`, `Llm`. |
| Board | 보드 | `boards = [...]` | `Board`, `BoardData` | `boards` | 사용자가 소유하는 Pegboard의 최상위 탭 하나. 한 맥락의 핀 집합을 담는다. |
| Toolbox | 툴박스 | — | `Toolbox` (내부) | — | `upeg`이 현재 아는 모든 Tool의 집합. Static + Declarative + Wasm + MCP Import를 프로젝트 매니페스트와 병합해 만든다. 내부 자료구조이며 surface는 `Registry`라는 단어를 절대 노출하지 않는다. |
| Project Manifest | 프로젝트 매니페스트 | `upeg.toml` (루트) | `ProjectManifest` | — | 자동 탐지되어 현재 프로젝트의 Toolbox에 병합되는 `upeg.toml`. |
| Board Context | 보드 컨텍스트 | 예약 `_upeg` 인자 | `BoardExecutionContext` | `_upeg.boardEnv`, `_upeg.projectManifest` | 실행 메타데이터: board 키, board env, 프로젝트 매니페스트 경로. |
| Chain Tool | 체인 도구 | `invoker = "Chain"` | `Invoker::Chain` | `invoker: "chain"` | 다른 Tool들을 조합하는 invoker를 가진 Tool. |
| Trigger | 트리거 | `[[triggers]]` | `Trigger` | `triggers` | Tool을 자동 실행하는 이벤트 소스. |
| Credential | 자격 증명 | `credential = "..."` | `Credential`, `CredentialSpec` | `credential` | 매니페스트 밖에 저장되는 이름 붙은 비밀 참조. 값은 OS keychain 또는 환경변수에 산다. |
| Execution Log | 실행 로그 | — | `ExecutionLog` | — | Tool 호출의 로컬 메타데이터 전용 기록. 인자/비밀 값은 제외된다. |
| Invoker | 인보커 | `invoker = "..."` | `Invoker` | `invoker` | 호출 메커니즘: `Function`, `External`, `Http`, `Static`, `Embed`, `Chain`, `Llm`, `Wasm`. |
| Source | 소스 | `source = ...` | `Source` | `source` | 도구가 시작되는 출처. 값: `UserInput`(기본), `Timer`, `Shortcut`, `Manual`, `Static`. GUI 제시 힌트이며 비-GUI surface는 변형을 무시하고 함수를 직접 호출한다. |
| Surface | 서피스 | `surfaces = [...]` | `Surface` | `surfaces` | 접근 인터페이스: `cli`, `tui`, `desktop`, `pwa`, `ext`, `mcp`, `http`. |
| Principal | 주체 | — | `Principal`, `PrincipalRole` | `_upeg.principal` | 런타임이 각인하는 호출자 신원 `{ role, surface }`. Surface가 "어느 문"이라면 주체는 "무슨 권한"이다. `role` 값은 `operator`(운영자), `agent`(에이전트), `local`(로컬). 호출자가 보낸 값은 지워진다 ([호출 봉투](/architecture/call-envelope.md)). |
| Agent Token | 에이전트 토큰 | — | `HostTokens` | — | `UPEG_HTTP_AGENT_TOKENS`로 발급하는 두 번째 bearer 토큰. 데이터 평면에 들어오지만 `agent` 주체로 각인되어 Chain 승인 장벽을 넘지 못한다. operator 토큰(`~/.upeg/server.json`)과 형태는 같고 권한만 다르다. |
| pty | 의사 터미널 | `pty = true` | — | `pty` | `External` invoker의 자식에게 파이프 두 개 대신 진짜 터미널을 주는 선언. `isatty(3)`로만 판단하는 프로그램을 위한 것이며 Unix 전용이다. 한국어 산문은 **의사 터미널**, 식별자는 `pty`로 남는다 ([매니페스트 계약](/architecture/manifest.md)). |
| IoType | I/O 타입 | `input_spec`/`output_spec` 내부 | `IoType` | `inputSchema`/`outputSchema` 내부 | 모든 surface가 공유하는 닫힌 입출력 타입 어휘. |
| View Embed | 뷰 임베드 | `outputs = [v: EmbeddedView(url)]` | `IoType::EmbeddedView` | `embedded_view` | 외부 웹사이트를 Pin 출력으로 표시하는 임베드 모드. I/O 브리지가 없고 사용자가 iframe을 직접 조작한다. |
| Controlled Embed | 조종 임베드 | `invoker = "Embed"` + `[[tools.controlled_embed.bindings]]` | `Invoker::Embed` + `controlled_embed.bindings` | `controlled_embed.bindings` | upeg이 CSS selector로 외부 페이지를 조종하는 임베드 모드. Pin은 평범한 폼/결과를 보여주고 iframe이 엔진 역할을 한다. |
| PegboardUnits | 페그보드 단위 | `pegboard_units = "U1"` | `PegboardUnits` | `pegboardUnits` | Compact 셀 footprint: `U1`(1×1), `U2`(2×1), `U2T`(1×2). |
| Upstream MCP Server | 상류 MCP 서버 | — | `UpstreamMcpServer` | — | MCP Import가 spawn해서 tool 목록을 읽어오는 외부 MCP 서버 프로세스. |
| Reexport | 재노출 | `reexport` | `McpReexport` | — | MCP Import의 tool을 upeg 자신의 `mcp` Surface에 다시 노출할지 여부. 기본 차단(루프 방지), 서버 설정별 `reexport = true`로 opt-in. |
| Skipped Tool | 스킵된 도구 | — | `SkippedTool` / `SkipReason` | — | 스키마가 upeg typed I/O로 변환되지 않아(또는 `name`이 비어) MCP Import 중 탈락한 upstream tool. 구조화된 사유와 함께 기록되고, 서버의 나머지 tool은 정상 등록된다. |

## Pegboard 은유 지도

출하된 은유는 **Pegboard + Board + Pin + Tool + Toolbox** 다섯 단어다. 이 집합 밖의 것
(Peg, Hook, Slot, Card, Tile, Workshop, …)은 어휘가 **아니다**.

![Pegboard 은유 — Toolbox의 Tool을 Board에 핀한다](diagrams/pegboard-metaphor.drawio.svg)

# 내장 Toolkit id

CLI에서 첫 명령 토큰으로, HTTP/MCP에서 Tool id prefix로 나타난다.

| Toolkit id | 설명 |
|---|---|
| `convert` | Hex / Base64 / Base32 / HTML / URL / JSON 인코딩 유틸리티 |
| `text` | diff / regex / case / slug / trim / split / join / replace / repeat / contains |
| `hash` | MD5 / SHA-1 / SHA-256 / SHA-512 / CRC-32 |
| `id` | UUID v4 / UUID v7 / NanoID 생성기 |
| `time` | 현재 Unix epoch와 ISO 8601 타임스탬프 헬퍼 |
| `color` | Hex ↔ RGB 변환 |
| `security` | 비밀번호 생성기, 강도 추정, 랜덤 바이트 생성 |
| `num` | 진법 변환: `num.hex_to_decimal`, `num.decimal_to_hex`, `num.decimal_to_binary`, `num.binary_to_decimal` |
| `qr` | QR 생성 (`qr.encode`, 유니코드 블록 렌더)과 디코드 (`qr.decode`, 이미지 → 텍스트) |
| `csv` | 행 단위 diff (`csv.diff`), CSV→JSON (`csv.to_json`), 열 선택 (`csv.select`) |
| `media` | 이미지 변환(단일·일괄)과 Image ↔ PDF (`media.image_convert`, `media.images_convert`, `media.image_to_pdf`, `media.pdf_to_images`, `media.pdf_extract_images`, `media.pptx_extract_images`, `media.pdf_inspect`, `media.pdf_to_markdown`) |
| `eth` | 최소 Ethereum JSON-RPC 읽기: `eth.gas`, `eth.address_lookup`. 네이티브 전용, keyless 공개 RPC 기본값에 `endpoint` override |

`timestamp` Toolkit은 없다 — `time`을 쓴다.

# 소스

| Source | 정의 |
|---|---|
| Static | `#[upeg::toolkit]` / `#[upeg::tool]`로 upeg에 컴파일된 Rust 코드 |
| Declarative | `~/.upeg/toolkits/*.toml` 또는 프로젝트 `upeg.toml`의 Toolkit TOML |
| Wasm | upeg 매니페스트와 호출 가능 함수를 export하는 `.wasm` Toolkit 바이너리 |
| MCP Import | `~/.upeg/mcp-imports/*.toml`에 선언된 **Upstream MCP Server**에서 임포트한 Tool. 여기서 upeg은 MCP *클라이언트*다. 한국어: **MCP 임포트**. 장수명 서버 프로세스가 시작 시 eager load한다. 아래 `mcp` Surface와 구별된다 |

# 헷갈리는 쌍

## `Toolkit` vs `Tool`

Toolkit은 Tool을 묶고 배포한다. Tool은 호출된다. `convert`는 Toolkit이고
`num.hex_to_decimal`은 Tool이다.

## `Invoker::Embed` vs `PinKind::Embed`

둘은 서로 짝이 **아니다** — 각각 반대편의 상대와 짝을 이룬다.

- `PinKind::Embed`(Passive Embed, 웹뷰가 곧 도구)는 `Invoker::Static`(호출 없음, dispatch는
  no-op)과만 짝한다.
- `Invoker::Embed`(WebView selector 어댑터 — CSS selector로 DOM에 폼 값을 기록)는
  `PinKind::ControlledEmbed`(조종석 폼/결과, 웹뷰는 숨은 엔진)와만 짝한다.

둘 다 "Embed"를 담고 있다는 이유로 혼동하는 것이 이 항목이 막으려는 표류다.

## `Tag` vs `Board`

Tag는 전역 탐색 라벨이고 Board는 사용자 소유의 탭/핀 맥락이다. 한 Tool은 여러 tag를 갖고
여러 board에 핀될 수 있다.

## `Pin`(동사) vs `pin`(TOML 키)

"Tool을 Board에 핀한다"는 사용자 행위다. Tool 매니페스트의 `pin = "..."` 키는 그 결과 핀의
표시 종류인 `PinKind`를 저장한다. 두 의미가 한국어 **핀**을 공유한다.

## `Toolbox` vs `Board`

Toolbox는 `upeg`이 아는 모든 Tool의 *카탈로그*이고, Board는 탭으로 보이는 *사용자 큐레이션
부분집합*이다. Tool은 어떤 Board에도 핀되지 않은 채 Toolbox에 존재할 수 있다.

## `mcp` Surface vs `MCP Import`

정반대 두 방향이며, 두 번째를 맨 `mcp`로 부르지 않는다.

- **`mcp` Surface** (`Surface::Mcp`): upeg이 MCP 서버*다*. stdio(`upeg mcp`)나 HTTP `/mcp`로
  자기 Tool을 노출한다. 맨 `mcp`는 항상 이것이다.
- **`MCP Import`** (`~/.upeg/mcp-imports/*.toml`): upeg이 Upstream MCP Server에서 Tool을
  *임포트*한다. 코드·CLI·JSON·UI 카피에서 항상 `import`/`upstream`으로 한정한다.

# 폐기된 용어

| 폐기 | 대체 | 규칙 |
|---|---|---|
| `category` / Category | `tags` | 매니페스트·API·UI에 `category` 필드를 추가하지 않는다. |
| `WidgetKind` (enum) | `PinKind` | 코드·TOML·JSON·문서 전부에서 개명. alias 없음. |
| `widget_kind` (TOML/Rust 필드) | `pin` | 개명. `widget_kind`를 쓰는 매니페스트는 로더가 unknown field로 거부한다. |
| `widget` (JSON 필드) | `pin` | JSON 응답은 `pin`만 낸다. |
| `Pegboard widget` (산문, 주석) | `Pin` | 모든 산문·docstring·주석·i18n 키가 `Pin`/`pin`을 쓴다. |
| `widget.*` i18n 키 prefix | `pin.*` | 전체 로케일에서 개명. alias 없음. |
| `Registry` (Tool 레지스트리 개념) | `Toolbox` | 문서·다이어그램·공개 Rust 심볼에서 개명. 내부 private 이름은 남을 수 있으나 새 코드는 `toolbox`를 쓴다. |
| Plugin | Tool 또는 Wasm source | "WASM plugin"은 바이너리 패키징 소스에만 쓴다. |
| Item | Tool | 호출 가능 개념에 일반적인 item 명명을 피한다. |
| Workshop | Pegboard + Board + Toolbox + 매니페스트 | 제품 어휘로 쓰지 않는다. |
| Peg / Hook / Slot / Card / Tile 어휘층 | Pegboard + Board + Pin + Tool + Toolbox | 다섯 단어 은유는 확정이다. 하위 은유를 도입하지 않는다. |
| `upeg daemon` (Unix socket) | `upeg host` (HTTP loopback) | transport가 HTTP loopback으로 통일되었다. |

# 한국어 카피 대응표

| English | 한국어 |
|---|---|
| Principal | 주체 |
| operator (role) | 운영자 |
| agent (role) | 에이전트 |
| local (role) | 로컬 |
| Agent Token | 에이전트 토큰 |
| Operator Token | 운영자 토큰 |
| Approval | 승인 |
| pty | 의사 터미널 |
| Add Tool | 도구 추가 |
| Pin Tool | 도구 핀하기 / 보드에 고정 |
| Toolkit list | 툴킷 목록 |
| Tag filter | 태그 필터 |
| Board tab | 보드 탭 |
| Toolbox | 툴박스 |
| Pin kind | 핀 종류 |
| Shell completion | 셸 자동완성 |
| Project Manifest | 프로젝트 매니페스트 |
| Execution Log | 실행 로그 |
| Credential | 자격 증명 |
| Source | 소스 |
| View Embed | 뷰 임베드 |
| Controlled Embed | 조종 임베드 |
| Embedded view | 뷰 임베드 |
| File | 파일 |
