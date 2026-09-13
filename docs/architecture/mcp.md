---
type: Surface Contract
title: MCP — Surface와 Import
description: upeg이 MCP 서버가 되는 방향(serve)과 MCP 클라이언트가 되는 방향(import), 그리고 각각의 게이트.
tags: [architecture, mcp, surfaces, ai]
status: stable
sources:
  - id: mcp-surface
    resource: ../../upeg-cli/src/surfaces/mcp
    title: MCP surface 구현
  - id: mcp-import-real-smoke
    resource: ../../upeg-cli/tests/mcp_import_real_server.rs
    title: 실서버(공식 SDK) 임포트 스모크
---

upeg과 MCP의 관계는 양방향이며, 두 방향은 절대 같은 이름을 쓰지 않는다. 맨 `mcp`는 항상
Surface를 뜻한다.

![MCP 두 방향 — serve(mcp Surface)와 import(MCP Import)](../diagrams/mcp-directions.drawio.svg)

# Serve — upeg이 MCP 서버 (`mcp` Surface)

`upeg mcp`는 stdio JSON-RPC로, 호스트의 `/mcp`는 HTTP로 자기 Tool을 노출한다. Tool id는
언제나 `{toolkit}.{tool}`이며, MCP `tools/call`은 [공유 호출 봉투](/architecture/call-envelope.md)와
같은 `name` + `arguments` 형태를 쓴다.

## Board 게이트 — "Board = 서버"

`upeg mcp --board <b>`로 시작하면 해당 Board에 핀되고 MCP surface에서 사용할 수 있는
Tool이 `tools/list`에 노출된다. 핀되지 않은 실행 Tool 호출은 거부한다. 조회 전용
`upeg.board_context`는 예외로 항상 제공하며, 이 이름은 Board 연결용으로 예약된다.
이 이름을 일반 Tool로 핀한 Board는 연결을 거부하고 이름 변경을 안내한다.
`--board` 없이 시작하면 surface 필터된 Toolbox 전체를 제공한다.

각 핀의 args preset은 호출 인자의 기본값이며 명시 인자가 우선한다. MCP 입력 스키마에도
적용 기본값을 표시하고, 유효한 핀 프리셋으로 충족한 필드는 `required`에서 제외한다.
Tool 선언의 기본값 표시만으로 필수 입력을 생략할 수는 없다. 실제 도구의 입력 검증과
승인 규칙은 계속 적용된다.

## 보드 안내와 연결 준비

`upeg board <b> context [--json]`은 설명, Markdown 지침, 실행 위치, 프로젝트 파일,
실제 MCP 도구 목록과 기본값, 실행 전 확인 결과를 보여준다. `ready`는 알려진 전제 조건을
확인했다는 뜻이며 실행 성공의 보장이 아니다. 외부 실행 파일이나 작업 디렉터리가 없으면
`unavailable`, 네트워크·인증 등 실행 시 확인할 항목은 `unchecked`로 표시한다.
저장된 핀 중 현재 프로세스에 등록되지 않은 도구는 `unresolved_pins`로 따로 표시하고
연결 미리보기가 불완전함을 안내한다. MCP Import 로딩 후 사용 가능해질 수 있으며,
현재 호출 가능한 도구 목록에 섞지 않는다.

`upeg board <b> connect`는 현재 환경을 재현하는 `mcpServers` JSON을 출력한다.
생성된 `command`/`args`는 CLI의 `--working-directory`로 실행 디렉터리를 고정하고,
`env.UPEG_PROJECT_MANIFEST_PATH`는 프로젝트 파일의 절대 경로나 `off`로 고정한다.
개인 저장소와 명시적인 도구 소스 경로도 유지한다. 비밀값을 복사하지 않는다.
클라이언트별 설정 컨테이너가 다르면 해당 서버의 command/args/env 항목을 옮긴다.

초기화의 표준 `instructions`에는 짧은 설명과 `upeg.board_context` 조회 안내를 넣는다.
조회 결과는 전체 지침, 실제 실행 구성, 설정 revision을 text와 structuredContent에 함께
제공한다. 임의 메타데이터를 클라이언트가 Skill로 자동 해석한다고 가정하지 않는다.
지침을 모델에게 전달하고 활용하는지는 실제 에이전트 클라이언트에서 확인해야 한다.

## 변경 반영과 실행 위치

Board를 지정한 stdio 연결은 해당 프로세스에서 도구를 실행한다. 다른 프로젝트에서 실행된
호스트가 있더라도 그 호스트의 도구 목록이나 실행 환경으로 바뀌지 않는다. MCP 호출에는
연결의 작업 디렉터리를 적용하며 Tool의 명시적 `cwd`와 프로젝트 경계 규칙이 우선한다.

연결 중 보드 지침·핀·프리셋 또는 프로젝트/Toolkit/MCP import TOML이 변경되면
다음 초기화·목록·호출 요청에 오류 `-32001`과 `data.reconnectRequired = true`를 반환한다.
MCP 서버를 재연결해야 새 설정을 로딩한다. 도구 실행 도중의 작업을 소급 취소하지 않는다.
배치 위치만 바꾸는 것은 재연결 사유가 아니다. 백그라운드 import 완료는 기존
`notifications/tools/list_changed`로 처리한다.

프로젝트 지침은 로딩 당시 원본과 비교한다. 파일이 변경되거나 사라지면 오래된 지침을
새 지침처럼 미리보기하지 않고 UPeg 재시작 또는 MCP 재연결을 안내한다.
HTTP의 요청별 Board 호출은 stdio 연결의 세션 revision을 공유하지 않는다.

개인/프로젝트 작성 절차와 전체 예시는 [보드와 에이전트 작업 흐름](/product/board-agent-workflow.md)을 참고한다.

## 실행 중 출력 — `notifications/message`

`tools/call`은 도구가 도는 **동안** 그때까지의 출력을 서버→클라이언트 로그 알림으로
흘려보낸다. 최종 응답 프레임은 조금도 달라지지 않는다 — 알림은 그 앞에 추가되는
정보이지, 결과의 일부가 아니다.

```json
{"jsonrpc":"2.0","method":"notifications/message","params":{
  "level":"info","logger":"upeg.tool",
  "data":{"tool":"dev.verify","stream":"stderr","seq":0,"text":"   Compiling upeg-core\n"}}}
```

| 항목 | 값 | 이유 |
|---|---|---|
| `level` | 항상 `info` | 진행 출력은 경고가 아니라 평범한 정보다. `info`로 거른 클라이언트가 바로 이걸 보고 싶어 한 클라이언트다 |
| `logger` | 항상 `upeg.tool` | 클라이언트가 도구 출력만 따로 라우팅하거나 음소거할 수 있다 |
| `data.stream` | `stdout` \| `stderr` | 자식이 실제로 쓴 스트림 |
| `data.seq` | 0부터 끊김 없이 증가 | 두 스트림과 chain의 모든 step에 걸쳐 이어지므로 전체 순서를 복원할 수 있다 |
| `data.text` | 도구가 쓴 바이트 그대로 | 줄 단위로 끊어 보낸다 ([매니페스트 계약](/architecture/manifest.md)) |

`tools/call`이 아닌 요청은 알릴 것이 없으므로 알림을 만들지 않는다.

## `logging` 능력은 lane마다 다르다

`capabilities.logging`은 **지킬 수 있는 lane만 선언한다.** 능력을 선언하고 알림을 보내지
않는 서버는 클라이언트를 오지 않을 프레임 앞에서 기다리게 하고, 선언 없이 알림을 보내는
서버는 프로토콜 위반으로 취급될 수 있다. 둘 다 거짓말이므로 lane이 프레임을 **쓸 곳이
있는지**로 갈린다.

| lane | 서버발 프레임 | `capabilities.logging` | `logging/setLevel` |
|---|---|---|---|
| stdio in-process (`upeg mcp`, 호스트 없음) | 응답 사이에 stdout으로 쓴다 | 선언한다 | 있다, 세션 단위 |
| HTTP `/mcp` | 클라이언트가 연 SSE 스트림에 쓴다 (아래 "서버→클라이언트 push") | 선언한다 | 있다, `Mcp-Session-Id` 단위 |
| stdio proxy (호스트에 붙음) | 호스트가 실행하므로 이 세션에는 없다 | 선언하지 않는다 | `-32601 Method not found` |

**HTTP lane에서 능력은 요청이 아니라 lane의 것이다.** `Accept`에
`text/event-stream`을 넣지 않은 요청 하나는 프레임을 실을 몸통을 열지 않았을
뿐이고, 그건 클라이언트의 선택이지 서버가 어긴 약속이 아니다. 그 요청의 진행
프레임은 버려진다 — 같은 세션의 다음 SSE 요청은 그대로 받는다.

Proxy 모드에서 실행 중 출력이 필요하면 호스트의
[HTTP 스트리밍 경로](/architecture/http-api.md)가 같은 정보를 갖고 있다.

## 서버→클라이언트 push — `/mcp`의 SSE (Streamable HTTP)

`/mcp`는 오랫동안 한 방향뿐이었다: 요청 하나에 JSON 응답 하나. 그래서 stdio lane이
당연히 하던 두 가지를 HTTP에서는 할 수 없었다 — 도는 도구의 출력을 흘려보내는 것과,
임포트 로드가 끝났다고 **알리는** 것. 지금은 MCP Streamable HTTP transport(2025-03-26)의
모양 그대로 두 방향이 있다.

| 요청 | 조건 | 응답 |
|---|---|---|
| `POST /mcp` | `Accept`에 `text/event-stream`이 **명시적으로** 있고, 요청에 `id`가 있다 | `text/event-stream`. 진행 `notifications/message` 프레임들이 먼저 흐르고, **마지막 이벤트가 JSON-RPC 응답**이며 그 뒤 스트림이 닫힌다 |
| `POST /mcp` | 그 외 전부 | 예전과 **글자 하나 다르지 않은** JSON 응답 하나 (알림은 `204 No Content`) |
| `GET /mcp` | `Accept`에 `text/event-stream` | 어떤 요청에도 속하지 않는 프레임을 위한 장수명 스트림 |
| `GET /mcp` | 그 외 | `406 Not Acceptable` — 이 리소스에는 다른 표현이 없다 |

```text
POST /mcp
Authorization: Bearer <token>
Accept: text/event-stream
Content-Type: application/json

{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"dev.verify","arguments":{}}}
```

```text
event: message
data: {"jsonrpc":"2.0","method":"notifications/message","params":{"level":"info","logger":"upeg.tool","data":{"tool":"dev.verify","stream":"stderr","seq":0,"text":"   Compiling upeg-core\n"}}}

event: message
data: {"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"ok"}],"structuredContent":{...}}}
```

계약:

- **`Accept`가 결정하고, `*/*`는 세지 않는다.** 와일드카드는 "아무거나"이지
  "스트림"이 아니다. `curl`을 포함해 기존 클라이언트 전부가 지금까지 받던 응답을
  그대로 받는다.
- **마지막 이벤트가 응답이다.** 소비자의 종료 조건은 자기 요청 `id`를 단 프레임을
  보는 것이다. 그 없이 끝난 스트림은 연결이 끊긴 것이다.
- **응답이 없는 요청은 스트림을 열지 않는다.** `id` 없는 알림에는 실어 보낼 응답이
  자체가 없으므로 `Accept`와 무관하게 `204`다.
- **읽지 않는 소비자는 알림을 잃지, 호스트의 메모리를 먹지 않는다.** 아직 쓰이지
  않은 SSE 본문은 호출당 정해진 **바이트 예산**까지만 쌓인다. 넘치면 그 알림
  프레임은 버려지고, 자리가 나는 즉시 잃은 **프레임 수**가 `upeg.transport` logger의
  `warning` 프레임 하나로 합산 보고된다. JSON-RPC 응답은 예산과 무관하게 나간다 —
  계약은 응답이고 알림이 아니다.
- **연결을 끊으면 도구가 멈춘다.** 소비자가 사라지면 SSE 본문이 drop되고, 그 drop이
  이 호출의 취소다 — `POST …/stream`과 **같은 계약이고 같은 구현**이다
  (`upeg-cli/src/surfaces/http/cancel_on_drop.rs`). `External` invoker는 대기 루프의
  매 tick마다 취소 토큰을 읽고, 취소가 서면 자식의 process group을 종료한다. 신호는
  "프레임을 큐에 넣지 못했다"가 아니라 본문의 drop이다 — 10분 동안 조용한 빌드가
  바로 멈춰야 하는 실행이기 때문이다. 취소를 읽지 못하는 invoker(WASM, 내장 함수)는
  여전히 끝까지 달린다.
- **조용한 호출도 바이트를 낸다.** 프레임이 없는 동안 `GET`과 같은 간격으로 keep-alive
  주석이 흐른다. SSE 주석은 이벤트가 아니라 소비자가 무시하는 줄이므로 프레임 순서는
  달라지지 않고, 중간의 idle-timeout 프록시가 조용한 `tools/call`을 죽은 연결로
  오인하지 않는다.
- **인증은 기존 `/mcp` 그대로다.** bearer 토큰이 `GET`에도 똑같이 걸린다.
- **호출자 주체는 토큰이 정한다.** surface는 두 lane 모두 `mcp`지만, stdio는 OS 사용자가
  띄운 프로세스라 `local`이고 HTTP는 리스너를 건너오므로 bearer가 증명한 역할
  (`operator`/`agent`)이다. `approval_surfaces = ["mcp"]`인 체인이라도 agent 토큰으로 온
  `tools/call`은 승인하지 못한다 ([Chain Tool](/architecture/chain.md)).

### `Mcp-Session-Id` — 발급하지만 강요하지 않는다

`initialize` 응답에 `Mcp-Session-Id` 헤더가 붙는다(bearer 토큰과 같은 난수원). 이후
요청이 그 값을 되돌려 보내면 **`logging/setLevel`이 옮긴 심각도 바닥이 그 세션에
남는다.** 그게 이 lane에서 세션 id가 존재하는 이유의 전부다.

- transport는 서버가 이 헤더를 **요구**하는 것을 허용하지만, upeg은 요구하지
  않는다. 헤더 없는 요청도 똑같이 처리되며 기본 바닥(`info`)을 쓴다.
- 이 헤더는 **인가가 아니다.** 인가는 bearer 토큰 하나뿐이다.
- 호스트는 마지막 64개 세션의 바닥만 기억한다. HTTP에는 "세션 종료" 신호가 없으므로
  잊는 쪽이 무한히 쌓는 쪽보다 낫고, 잊힌 세션은 `logging/setLevel`을 한 번도 부른
  적 없는 세션과 같은 상태가 된다.

## `logging/setLevel`

능력을 선언한 lane은 세션의 **심각도 바닥**을 옮기는 이 메서드를 답한다.

```json
{"jsonrpc":"2.0","id":2,"method":"logging/setLevel","params":{"level":"warning"}}
```

- 값은 MCP가 고정한 여덟 가지(syslog, RFC 5424)뿐이다 — `debug`, `info`, `notice`,
  `warning`, `error`, `critical`, `alert`, `emergency`. 그 밖의 값은 조용히 뭉개지 않고
  `-32602 Invalid params`로 거절하며, 허용 목록을 메시지에 담아 돌려준다.
- 기본값은 `info`다. 한 번도 부르지 않은 클라이언트는 이 메서드가 없던 때와 똑같은
  것을 본다.
- 진행 프레임은 `info`로 나가므로 `warning` 이상으로 올리면 **진행 알림이 멈춘다.**
  최종 응답 프레임은 그대로다 — 심각도는 알림만 거른다.
- 설정은 세션 단위이며 그 세션이 끝날 때까지 유지된다.

## 출처(`source`) 필드

`tools/list`의 각 항목은 1급 `source` 필드로 출처를 명시한다. 값은 셋 중 하나다
(`upeg-runtime/src/provenance.rs`):

| 값 | 뜻 |
|---|---|
| `local` | 내장 inventory, `~/.upeg/toolkits`의 TOML Toolkit, WASM 플러그인 — in-process |
| `mcp-import:<server>` | upstream MCP 서버에서 프록시된 임포트 |
| `project-manifest:<path>` | 탐지된 프로젝트 `upeg.toml`이 등록한 Tool ([프로젝트 매니페스트](/architecture/project-manifest.md)) |

MCP 클라이언트와 관리 surface가 id를 파싱하지 않고도 upeg 자신의 Tool, 프록시된 임포트,
호출자 디렉터리에만 존재하는 프로젝트 Tool을 구분한다.

## Proxy 모드

Board를 지정하지 않은 `upeg mcp`는 호스트가 reachable하면 stdio ↔ HTTP `/mcp` proxy로 동작한다. 없으면 in-process
handler로 단독 동작한다 — 호스트를 auto-spawn하지 않는다
([호스트 토폴로지](/architecture/host-topology.md)).

proxy는 단순 파이프가 아니다. CLI/TUI와 **같은 프로젝트 매니페스트 규칙**을 적용한다
([프로젝트 매니페스트](/architecture/project-manifest.md)의 D-1/D-2):

- `tools/call`의 `params.name`이 `project-manifest:*` provenance를 가지면 host로 보내지 않고
  **in-process로 dispatch**한다. host는 자기 나름의 `upeg.toml`을(또는 아무것도) 해석했으므로
  그 Tool을 모른다.
- 그 외 `tools/call`은 `params.arguments`에 호출자의 절대 cwd를 `_upeg.cwd`로 실어 전달한다.
- `tools/list` 응답에는 이 프로세스의 project-manifest Tool을 이름 기준 dedupe로 병합한다
  (이름이 겹치면 host 항목이 남는다). Board를 지정한 stdio 연결은 위의 독립 실행 규칙을 따른다.

# Import — upeg이 MCP 클라이언트 (`MCP Import`)

MCP 설정이 클라이언트마다 흩어지는 문제에 대해 upeg이 설정 허브가 된다. upstream 서버를
`~/.upeg/mcp-imports/<server>.toml` 한 곳에 선언하면 upeg이 서브프로세스로 spawn해 tool
목록을 읽고, 각 tool을 **typed I/O로 정규화**해 `<server>.<id>`로 Toolbox에 등록한다.
파일 stem이 네임스페이스가 된다. 임포트된 tool은 다른 Tool과 똑같이 Board에 핀하고
CLI/TUI/Desktop/HTTP에서 호출할 수 있다.

## 부분 성공 (tool 단위)

- 스키마가 upeg typed I/O로 변환되지 않는 tool(`$ref`/`oneOf` 등)이나 `name`이 빈 tool은
  **tool 단위로 스킵**되고 구조화된 사유(`SkipReason`)가 기록된다. 서버의 나머지 tool은
  정상 등록된다. `ImportOutcome { registration, skipped }`가 스킵 목록을 호출자에게 노출한다.
- 서버 임포트가 실패하는 경우는 셋뿐이다: MCP 프로토콜 위반, 네임스페이스 충돌, 모든 tool이
  스킵된 경우.
- **네임스페이스 충돌은 서버 단위로 원자적이다.** 중복 id나 내장 Tool shadow는 신뢰/설정
  문제이며 해결책(서버 설정 이름 변경)이 네임스페이스 전체에 적용되기 때문이다. 충돌하는
  tool을 조용히 스킵하면 의도적 shadowing을 가릴 수 있다.

## 재노출(reexport)

임포트된 tool은 기본적으로 upeg 자신의 `mcp` Surface에 **다시 노출되지 않는다**. 다른 서버의
tool을 조용히 되돌려 내보내는 프록시 체인과 self-import 루프를 막기 위한 default-off다.
서버별 TOML에 `reexport = true`로 opt-in하며, opt-in은 그 서버를 spawn하겠다는 신뢰 결정을
선언하는 바로 그 파일에 있다. 스킵된 tool은 어떤 surface에도 등록되지 않으므로 reexport가
적용될 여지가 없다.

## 임포트 로딩: 장수명 서버 프로세스만, lane마다 다른 시점, reload는 재시작

`~/.upeg/mcp-imports/*.toml`은 **장수명 서버 프로세스가 시작될 때** 로드된다. 그 프로세스는
정확히 셋뿐이고, **셋이 같은 방식으로 로드하지는 않는다**. 공통 진입점은
`upeg_sources::load_mcp_imports_for_host(&RuntimeSourceConfig)`이며, 세 lane 모두
`upeg-cli/src/infrastructure/mcp_imports.rs`를 거친다.

| Lane | 시점 | 로딩 중 노출 |
|---|---|---|
| `upeg host start` (foreground / `--daemon`) | 리스너를 열기 전, 동기 eager load | 로드가 끝난 뒤에야 요청을 받는다 |
| desktop 내장 host (`upeg-frb` host bootstrap) | embed 스레드를 띄우기 **전에** pending 마크, 로드 자체는 ready 이후 백그라운드 스레드 | 리스너가 열리는 순간부터 `/healthz`의 `importsPending`이 true다 — 아래 "비동기 창" 참조 |
| in-process `upeg mcp` (proxy가 아닐 때) | 백그라운드 스레드 + 조건부 | 로드 완료 시 `notifications/tools/list_changed` 발행 |

One-shot CLI 명령과 TUI는 임포트를 로드하지 **않는다** — 이들은 subprocess를 spawn하지 않고,
붙어 있는 host를 통해 임포트된 tool을 dispatch한다. proxy 모드의 `upeg mcp`도 로드하지
않는다 — host 쪽이 이미 로드해 둔 목록을 그대로 쓴다.

**Reload는 곧 host 재시작이다.** 실행 중에 다시 로드하는 별도 액션은 없으며, 이는 의도적으로
받아들인 손실이다. `reexport = true` opt-in 규칙은 위와 동일하게 유지된다.

### in-process `upeg mcp`: 조건부 + 지연 로드

stdio MCP 서버는 자기 클라이언트의 `initialize`에 즉시 답해야 한다. upstream 하나가 죽어
있으면 spawn + handshake가 재시도 포함 수십 초까지 갈 수 있으므로, 이 lane은 로딩을 요청
경로에서 떼어낸다.

1. **사전 스캔**: `upeg_sources::mcp_import_reexport_policy`가 선언 파일만 읽어
   `reexport = true`가 하나라도 있는지 본다 (spawn 없음, `read_dir` 한 번). 전부 기본값
   `Blocked`면 임포트된 tool은 `ALL_SURFACES_EXCEPT_MCP`에 등록되어 이 surface의
   `tools/list`에 애초에 나오지 않으므로, **로딩 자체를 건너뛴다**.
2. **백그라운드 로드**: opt-in이 하나라도 있으면 워커 스레드에서 로드한다.
3. **완료 알림**: 로드가 끝나고 `initialize` 응답이 이미 나간 뒤에 stdout으로
   `notifications/tools/list_changed`(JSON-RPC notification, `id` 없음)를 한 줄 쓴다.
   클라이언트는 이걸 보고 `tools/list`를 다시 읽는다. 쓰기는 best-effort다 — 이미 나간
   클라이언트 때문에 서버가 죽지 않는다.

### desktop 내장 host: 비동기 창(window)

desktop lane은 임포트를 백그라운드로 로드하는데, `server.json` 출판과 요청 수신은 그보다
먼저 시작된다. 따라서 **호스트가 ready된 직후 수 초 동안 `/v1/tools`와 `/mcp`의
`tools/list`에 임포트된 tool이 아직 없을 수 있다.** 창 자체는 그대로 있다 — 부팅을
임포트 뒤로 미뤄 스플래시가 upstream 타임아웃만큼 늘어나는 쪽을 택하지 않았기
때문이다. 달라진 것은 **그 창이 닫힐 때 클라이언트가 알게 되는 방법**이다.

| 신호 | 방향 | 누가 쓰나 |
|---|---|---|
| `/healthz`의 `importsPending` | 묻는 쪽 (polling) | 인증 없이 아무나. `upeg host status --json`, desktop 상태바 |
| `GET /mcp`의 `notifications/tools/list_changed` | 알리는 쪽 (push) | `/mcp`에 붙어 SSE 스트림을 연 MCP 클라이언트 |

- push는 **로드가 끝났을 때**(`McpImportPhase::Done`) 나간다. `Loading`은 창이 열린
  것이고 `Skipped`/`NotStarted`는 애초에 아무것도 가져오지 않은 것이라, 둘 다 다시
  읽으라고 할 이유가 없다. tool이 0개로 끝난 `Done`도 나간다 — "창이 닫혔고 아무것도
  오지 않았다"가 바로 기다리던 클라이언트가 들어야 할 말이다.
- 단계 전이는 `set_import_phase` 한 곳에서만 쓰이고, 그 한 곳이 곧 push 지점이다
  (`mcp_imports::subscribe_phase_changes`). 묻는 신호와 알리는 신호가 서로 다른 사실을
  말할 수 없는 이유다.
- 스트림을 열지 않은 클라이언트의 복구는 여전히 단순하다: 잠시 뒤 `tools/list`를 다시
  읽으면 된다.

#### `importsPending` — 창이 열려 있다는 신호

푸시를 받지 않는 클라이언트도 **묻는 건 된다.** host는 자기 임포트 로드 단계를 타입으로
들고 있고
(`upeg-cli/src/infrastructure/mcp_imports.rs`의 `McpImportPhase`:
`NotStarted | Loading | Done{serversLoaded, serversFailed, tools} | Skipped`),
그것을 인증 없는 `/healthz`에 싣는다:

```json
{
  "name": "upeg",
  "version": "…",
  "importsPending": true,
  "mcpImports": { "state": "loading" }
}
```

- `importsPending`이 true인 단계는 `Loading` 하나뿐이다. false를 본 클라이언트는
  방금 읽은 `tools/list`를 완결된 목록으로 믿어도 된다.
- `Loading`은 로더 스레드가 **생기기 전에** 동기적으로 찍힌다. spawn과 스레드 진입
  사이에 `/healthz`를 읽은 클라이언트가 `not-started`를 보고 "더 올 것이 없다"고
  결론내면 안 되기 때문이다.
- desktop lane은 한 단계 더 앞이다. embed 워커 스레드는 ready 신호를 보내기 **전에**
  리스너를 열고 `/healthz`에 답하기 시작하므로, 마크는 그 스레드를 spawn하기 전에
  찍는다(`upeg_cli::mark_mcp_imports_pending`). host가 끝내 뜨지 않은 경로
  (bind 실패, embed slot 경쟁에서 패배)는 마크를 그대로 반납한다
  (`clear_mcp_imports_pending`) — 예약된 적 없는 로드를 기다리게 두지 않기 위해서다.
  ready 타임아웃은 반납하지 **않는다**: 스레드는 아직 살아 있고 곧 바인드할 수 있으며,
  그 host는 우리 것이라 임포트도 우리가 실어야 한다.
- `Done`에는 `serversLoaded` / `serversFailed` / `tools` 카운트가 붙는다. **개수만**이다 —
  이 라우트는 인증이 없으므로 어떤 upstream을 선언했는지는 나가지 않는다.
- `Skipped`는 이 프로세스가 자기 자신이 누군가의 MCP-import 자식이라 로드를 건너뛴
  경우다(위 자기임포트 방지).
- `upeg host status --json`은 이제 이 문서를 되읽어 `mcpImports` 블록으로 보고한다.
  이 one-shot 명령이 *선언*이 아니라 host의 **라이브** 상태를 말하는 유일한 지점이다.
  host가 답하지 않거나 필드가 없으면 `"state": "unknown"`이다 — 없는 값을 false로
  채우면 "다 로드됐다"로 읽히기 때문이다. 블록은 **항상** 나온다: host가 아예 없거나
  `server.json`이 stale일 때도 키가 사라지는 대신 `unknown`이다. "키 없음"과
  "unknown"은 같은 사실이고, 두 모양을 다 내보내면 소비자가 둘 다 처리해야 한다.
- 이 `/healthz` 읽기는 discovery의 liveness probe와 **같은 예산**으로 돈다
  (`RequestBudget::HEALTH_PROBE`, 연결/응답 각 300ms). 도구를 실제로 돌리는
  라우트용 30s 예산을 여기 쓰면, 바로 옆 probe가 이미 "죽었다"고 판정한 host를
  `upeg host status`가 백 배 더 기다리게 된다.
- desktop 상태바는 같은 신호를 FRB 스냅샷의 `McpImportPhaseDto`로 받아, 로딩 중에는
  임포트 칩을 개수 대신 "imports loading…"으로 렌더한다.

### 부분 성공과 안전 타임아웃

서버별 부분 성공 규칙(위 "부분 성공" 절)은 로딩 시점에도 그대로 적용된다. 로딩 경로가
subprocess/프로토콜 안전을 소유한다.

- `initialize`: 5s bounded timeout
- `tools/list`: 5s bounded timeout
- `tools/call`: 30s bounded timeout
- shutdown: 500ms bounded timeout
- timeout 경로는 자식 프로세스를 kill하고 wait한다
- 최초 spawn + handshake(`initialize` + `tools/list`)는 일시적 실패(타임아웃,
  응답 전 pipe 종료)에 한해 고정 backoff로 최대 시도 횟수만큼만 재시도된다
  (`upeg-sources/src/mcp_import/spawn_retry.rs`). 잘못된 명령이나 명시적 RPC
  오류처럼 재시도해도 결과가 바뀌지 않는 실패는 즉시 반환된다 — 영구적으로
  실패하는 임포트가 무한정 재시도되는 일은 없다.

### 핸드셰이크 스펙 준수

실제 `@modelcontextprotocol` TypeScript SDK 서버는 JSON-RPC 프레임을 엄격히
검증한다. 두 가지를 지키지 않으면 `tools/list`가 조용히 무시되어 타임아웃으로
보인다:

- `params`가 없는 요청(`tools/list` 등)은 `params` 멤버 자체를 생략한다.
  `"params": null`을 보내면 공식 SDK 서버가 그 요청을 프레임 검증 단계에서
  버린다 — `null`과 멤버 부재는 JSON-RPC 2.0에서 다른 의미다.
- `initialize` 응답을 받은 직후, 다른 어떤 요청보다 먼저
  `notifications/initialized` 알림(`id` 없음, 응답 없음)을 보낸다. 쓰기
  실패는 handshake를 실패시키지 않는다 — best-effort이며, 실제 연결 문제는
  바로 다음 요청에서 드러난다.

#### 실서버 검증

이 두 규칙은 **가짜 stdio 서버로는 증명되지 않는다.** 스위트의 fake 서버도,
`upeg mcp`를 자기 자신에게 물리는 dogfood 테스트도 양쪽 끝이 모두 upeg이라,
양쪽이 똑같이 틀린 프레임은 그대로 통과한다. E-5가 정확히 그 사각지대였다.

그래서 `upeg-cli/tests/mcp_import_real_server.rs`가 **우리가 짜지 않은 서버**
하나를 상대로 임포트 전 구간을 돌린다 —
`npx -y @modelcontextprotocol/server-filesystem <tmpdir>` (공식 TypeScript
SDK). **세 테스트**가 같은 임포트를 세 층위에서 본다:

1. 핸드셰이크 + `tools/list` 응답 (raw `UpstreamMcpServer`),
2. 네임스페이스 등록과 등록된 dispatcher를 통한 실제 `tools/call` 왕복,
3. **host lane** — scratch `UPEG_HOME`에 선언을 써 두고 `upeg host start
   --daemon`을 띄운 뒤, 별개의 `upeg call` 프로세스가 그 host를 통해 임포트된
   tool을 호출한다. 사용자가 실제로 밟는 경로이고, `upeg host status --json`의
   `mcpImports` 신호도 여기서 끝까지 확인된다.

- 세 테스트 모두 `#[ignore]`다. 평범한 `cargo test`가 Node 툴체인과 npm
  레지스트리를 전제하면 안 된다.
- `just mcp-import-real-smoke`가 돌리고, `just ci-smoke`가 그걸 부른다.
- 개발 머신에서 `npx`가 없거나 패키지를 받지 못하면 **실패가 아니라 skip**이며
  사유를 출력한다(툴체인 없음 / 패키지 fetch 실패 / pre-warm 비정상 종료를
  구분해서 적는다). Node가 없는 것은 upeg의 회귀가 아니다.
- **CI에서는 skip이 곧 실패다.** `just mcp-import-real-smoke`는 `CI`가 설정돼
  있으면 `UPEG_REQUIRE_REAL_MCP=1`을 켜고, 그러면 위의 not-ready 사유가 그대로
  panic 메시지가 된다. Node를 잃어버린 러너가 "초록 세 개"로 보이면 이 lane의
  존재 이유가 사라지기 때문이다.
- 타이밍: 테스트는 먼저 서버를 stdin 없이 한 번 띄워 `npx` 캐시를 데운다.
  임포트 계층의 5s `initialize` 예산이 패키지 다운로드에 쓰이면 네트워크
  문제가 upeg 타임아웃 버그로 보이기 때문이다. 이 pre-warm은 프로세스당 한 번만
  돈다(`OnceLock`) — 세 테스트가 각자 받으면 cold-cache 비용이 세 배가 된다.
  캐시가 비어 있으면 이 워밍업만 수십 초(네트워크 바운드), 이후 세 테스트는
  합쳐서 수 초다. CI 러너의 per-job 캐시 볼륨은 cargo만 덮으므로 job마다 한 번
  다시 받는다.

### 자기임포트(self-import) 재귀 방지

`examples/mcp-imports/local.toml`처럼 upeg 자신의 `mcp` Surface를 upstream으로
선언하면(`command = "upeg", args = ["mcp"]`), spawn된 자식도 그대로 `upeg mcp`
프로세스다. 이 재귀를 막지 않으면 자식이 자기 시작 경로에서 다시
`~/.upeg/mcp-imports`를 eager load하면서 손자 `upeg mcp`를 spawn하고, 그
손자가 또 증손자를 spawn하는 식으로 OS의 process/fd 한도에 부딪힐 때까지
포크폭탄이 이어진다.

upeg은 자신이 spawn하는 모든 MCP-import upstream 서브프로세스에 마커
환경변수를 심는다(`upeg_sources::mcp_import::MCP_IMPORT_CHILD_ENV =
"UPEG_MCP_IMPORT_CHILD"`, `spawn_child`에서 설정). 장수명 서버 진입점(`upeg
host start`, desktop host, in-process `upeg mcp`)은 eager import 로딩 전에
`upeg_sources::mcp_import::is_mcp_import_child()`로 이 마커를 확인한다 —
`true`면 로딩을 건너뛴다. 세 진입점 모두
`upeg-cli/src/infrastructure/mcp_imports.rs::load_and_report_for_host`를
거치므로, 가드는 그 함수 한 곳에만 있으면 충분하다. 이 마커는 값이 아니라
존재 여부만 의미가 있다 — 값 비교가 필요한 CLI 쪽 코드가 없도록
`is_mcp_import_child()`가 타입이 있는 predicate로 노출된다.
