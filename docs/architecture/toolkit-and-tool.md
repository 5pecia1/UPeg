---
type: Domain Contract
title: Toolkit과 Tool
description: 2단계 호출 계층, Invoker 종류, Tag 상속, 그리고 단일 dispatch 경계.
tags: [architecture, domain, dispatch]
status: stable
---

![네 소스가 하나의 Toolbox와 하나의 dispatch 경로로 합류한다](../diagrams/tool-sources.drawio.svg)

# 계층

```
Toolkit (그룹·배포 단위, 호출 불가)
└── Tool (호출 단위, 항상 정확히 하나의 Toolkit에 속함)
    Full id: {toolkit}.{tool}
```

- Toolkit id는 배포/그룹 네임스페이스이며 호출되지 않는다.
- Tool full id는 항상 `{toolkit}.{tool}`이다. 사용자 개인 도구도 `my.script_name`처럼
  개인 네임스페이스 Toolkit 아래에 둔다.
- id는 정규형이며 padding이 없다. 매니페스트는 앞뒤 공백을 정규화하지 않고 거부한다.

# 소스

| Source | 방법 | 단위 |
|---|---|---|
| Static | Rust `#[upeg::toolkit]` / `#[upeg::tool]` | 코어 내장 Toolkit |
| Declarative | TOML (`~/.upeg/toolkits/{toolkit_id}.toml`, 프로젝트 `upeg.toml`) | 파일 1개 = Toolkit 1개 |
| Wasm | `.wasm` 바이너리 (`~/.upeg/wasm/{toolkit_id}.wasm`) | 바이너리 1개 = Toolkit 1개 |
| MCP Import | `~/.upeg/mcp-imports/*.toml`의 upstream MCP 서버 | 파일 1개 = 네임스페이스 1개 |

# Invoker

Tool의 호출 메커니즘. 닫힌 enum이다.

| Invoker | 설명 | 제약 |
|---|---|---|
| `Function` | Rust 함수 직접 호출 | Static source 전용. 프로세스 불필요. |
| `External` | 외부 바이너리 subprocess | 서브프로세스 spawn 가능한 호스트에서만. |
| `Http` | HTTP 요청 | credential 참조 + URL 선언. |
| `Static` | 호출 없음 — Tool이 셀프 프레젠팅, dispatch는 no-op | `PinKind::Embed`(Passive Embed) 전용. |
| `Embed` | WebView selector 어댑터 — CSS selector로 DOM에 값 기록/회수 | `PinKind::ControlledEmbed` 전용. bindings 필수. |
| `Chain` | 다른 Tool들의 선언적 조합 | 그 자체가 하나의 Tool. |
| `Llm` | LLM API + system prompt | provider는 어댑터 설정이지 도메인이 아니다. |
| `Wasm` | upeg 인터페이스 구현 WASM 바이너리 | extism 호스트 필요(feature gate). |

`Invoker::Embed`와 `PinKind::Embed`는 **짝이 아니다** — 각각 반대편의 상대와 짝을 이룬다.
[Lexicon의 헷갈리는 쌍](/LEXICON.md) 참조.

# Tags

- Toolkit 수준 tags → 모든 하위 Tool에 상속.
- Tool 수준 tags → 추가.
- 유효 tags = `toolkit.tags ∪ tool.tags ∪ toolkit id ∪ capability tags`.
- `category`는 폐기되었다. 매니페스트/API/UI 어디에도 추가하지 않는다.

# Board 선언

Tool의 `boards = [...]` 는 그 Tool을 어느 board 탭에 두는지 고르는 것뿐이고, board 자체를
만들지 않는다. board 목록은 두 곳에서 온다:

1. 내장 board 표 — `upeg_core::BUILTIN_BOARDS` (`dev` / `trading` / `personal`). 데이터이지
   코드가 아니다.
2. **프로젝트 매니페스트의 최상위 `[[boards]]`** — 그 `upeg.toml` 이 탐지되는 동안에만
   존재하는 프로젝트 전용 board다. 선언·네임스페이스·지속성 규칙은
   [프로젝트 매니페스트](/architecture/project-manifest.md)의 "프로젝트 board" 참조.

`~/.upeg/toolkits/*.toml` 은 board를 선언할 수 없다 — 전역 Toolkit에는 board를 매어 둘
프로젝트가 없다. 로더가 그 파일의 등록을 통째로 거부한다.

# Dispatch 경계

모든 surface는 하나의 toolbox + dispatcher 경계를 통해 Tool을 호출한다. surface별 임시
코드를 두지 않는다.

- `ToolMeta`는 순수 메타데이터다: id, toolkit, tags, description, schema, pin kind,
  pegboard units, invoker, surfaces, boards.
- `ToolkitMeta`는 그룹/배포 메타데이터다.
- 런타임 dispatcher는 id로 등록되고 JSON args를 받는다.
- surface는 실행 전에 `Surface`로 게이트한다.
- 결과는 정본 구조화 `ToolResult`다. CLI stdout은 표현(primary/`--json`/`--field`/`--pretty`)을
  고르고, JSON 전송과 UI surface는 같은 정본 출력 행을 소비한다.

## Dispatch 순서

1. 내장 dispatcher 등록을 보장한다.
2. toolbox에 메타데이터가 있는지 확인한다.
3. 런타임 dispatcher가 있으면 실행한다.
4. 메타데이터만 있고 dispatcher가 없으면 명확한 not-implemented Tool 에러를 반환한다.

메타데이터가 보이기 전에 모든 수용된 invoker는 실행 가능한 dispatcher를 등록하거나
capability-explicit 에러를 등록해야 한다.

# CLI 단일 경로

동적 라우트 `upeg {toolkit} {tool} <pos...>`가 Tool 호출의 유일한 경로다. Toolkit별
하드코딩 clap 서브커맨드는 재도입하지 않으며, Toolkit 추가가 CLI 수정을 요구해서는 안 된다.
바인딩 규칙은 [호출 봉투](/architecture/call-envelope.md) 참조.

# 내장 Toolkit id

`convert`, `text`, `hash`, `id`, `time`, `color`, `security`, `num`, `qr`, `csv`, `media`, `eth`.
각 id가 무엇을 담는지는 [Lexicon §3](/LEXICON.md) 참조. `timestamp` Toolkit은 없다 — `time`을 쓴다.
