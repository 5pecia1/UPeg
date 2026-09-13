---
type: API Contract
title: HTTP API
description: "`/v1/*` 리소스 모델, 응답 규칙, CORS 및 bearer 인증 경계."
tags: [architecture, http, api, security]
status: stable
sources:
  - id: http-surface
    resource: ../../upeg-cli/src/surfaces/http
    title: HTTP surface 구현
---

# 리소스 모델

```text
GET  /healthz
GET  /v1/toolkits
GET  /v1/toolkits/{toolkit}
GET  /v1/toolkits/{toolkit}/{tool}
GET  /v1/tags
GET  /v1/tags/{tag}
GET  /v1/boards
GET  /v1/boards/{board}
GET  /v1/credentials
GET  /v1/logs
GET  /v1/tools
GET  /v1/triggers
GET  /v1/clients
POST /v1/clients/heartbeat
POST /v1/tools/{toolkit}.{tool}
POST /v1/tools/{toolkit}.{tool}/stream
POST /v1/boards/{board}/tools/{toolkit}.{tool}
POST /v1/boards/{board}/tools/{toolkit}.{tool}/stream
POST /v1/trigger/{toolkit}.{tool}
GET  /v1/openapi.json
POST /mcp
GET  /mcp
```

# 응답 규칙

- 탐색 응답은 JSON이며 유효 tags를 포함한 Tool 메타데이터를 담는다.
- `/healthz`는 인증 없이 응답하며 비밀이 아닌 힌트만 노출한다: name, version,
  그리고 MCP 임포트 로딩 신호(`importsPending` + 개수만 담은 `mcpImports` 블록,
  [MCP](/architecture/mcp.md)의 "importsPending"). 토큰이나 포트/capability 목록,
  upstream 서버 이름은 절대 포함하지 않는다.
- 호스트 프로세스가 떠 있으면 `/v1/*`는 **항상** 서비스된다 — 별도의 desired-state 게이트가
  없다. 호스트를 시작하는 것 자체가 명시적 활성화다 ([호스트 토폴로지](/architecture/host-topology.md)).
- `/v1/tools`는 MCP 호환 클라이언트가 쓰는 `tools/list`와 같은 메타데이터 형태로
  HTTP-가시 Tool 목록을 반환한다.
- `/v1/credentials`, `/v1/logs`, `/v1/triggers`는 각각 참조 전용 credential 메타데이터,
  메타데이터 전용 Execution Log 행, 등록된 Trigger 바인딩을 노출한다. 어느 것도 비밀 값이나
  인자 값을 반환하지 않는다.
- Tool 호출 성공은 `{ "result": "..." }`, 프로토콜/본문/Tool 에러는 `{ "error": "..." }`와
  400/404/422를 반환한다.
- Board Tool 호출은 해당 Board에 핀되지 않은 Tool을 거부하고, `_upeg` 컨텍스트와 프로젝트
  매니페스트 경로를 주입한다.
- OpenAPI는 구체적인 Tool 호출 경로와 리소스 라우트를 모두 포함한다.

# 스트리밍 호출 — `POST …/stream`

`POST /v1/tools/{id}`는 도구가 끝난 뒤 봉투 하나로 답한다. 10분짜리 명령이면 10분 동안
아무것도 오지 않는다. `/stream` 형제 경로는 즉시 `application/x-ndjson` 본문을 열고
**한 줄에 JSON 객체 하나씩** 채워 나간다.

```text
POST /v1/tools/dev.verify/stream
Authorization: Bearer <token>
Content-Type: application/json

{}
```

```json
{"event":"chunk","stream":"stderr","seq":0,"data":"   Compiling upeg-core\n"}
{"event":"chunk","stream":"stdout","seq":1,"data":"ok\n"}
{"event":"result","result":{"ok":true,"primary_output_id":"result","outputs":[...]}}
```

| 줄 | 필드 |
|---|---|
| `chunk` | `stream`(`stdout`\|`stderr`), `seq`(한 호출 안에서 두 스트림과 **모든 chain step**에 걸쳐 0부터 끊김 없이 증가), `data`(도구가 쓴 바이트 그대로) |
| `dropped` | `bytes` — 소비자가 늦게 읽어서 호스트가 버린 도구 출력의 바이트 수 |
| `result` | `result` — [비스트리밍 경로와 **글자 하나 다르지 않은** canonical 봉투](/architecture/call-envelope.md) |

계약:

- **마지막 줄은 언제나 `result`다.** `"event":"result"`를 볼 때까지 읽는 것이 소비자의
  종료 조건이다. `result` 없이 끝난 본문은 연결이 끊긴 것이지 성공이 아니다.
- **`seq`는 호출 단위다.** Chain이 `External` step을 세 개 돌려도 순번은 0, 1, 2 …로
  이어진다. 단계마다 0으로 되돌아가지 않으므로 소비자는 stdout·stderr·step을 하나의
  전체 순서로 복원할 수 있다.
- **읽지 않는 소비자는 청크를 잃지, 호스트의 메모리를 먹지 않는다.** 아직 쓰이지 않은
  NDJSON은 호출당 정해진 **바이트 예산**까지만 쌓인다. 넘치면 그 청크는 버려지고, 자리가
  나는 즉시 버린 양이 `{"event":"dropped","bytes":N}` 한 줄로 합산 보고된다. `seq`는
  계속 이어지므로 **손실은 이 줄로만 드러난다** — 순번의 구멍으로 알 수 있다고 가정하면 안 된다.
- **상태 코드는 결과보다 먼저 정해진다.** 헤더가 첫 바이트와 함께 나가므로 도구 실패가
  `422`가 될 수 없다 — 실패는 마지막 `result` 줄의 봉투로 온다. 미리 알 수 있는 라우팅
  오류(알 수 없는 도구, board에 핀되지 않은 도구, 잘못된 본문)는 스트림을 열지 않고
  평범한 JSON `404`/`400`으로 답한다.
- **인증·Origin 가드·board 핀 게이트·`_upeg` 주입은 전부 동일하다.** 이 경로는 정책이
  아니라 본문 인코딩 하나를 추가한다.
- 청크를 만드는 것은 [`External` invoker](/architecture/manifest.md)뿐이다. 다른 invoker는
  `result` 줄 하나만 나가고, 그것이 정상이다.
- **연결을 끊으면 도구가 멈춘다.** 소비자가 사라지면 응답 본문이 drop되고, 그 drop이
  이 호출의 취소다 — 호출은 애초에 취소 토큰을 설치한 채로 dispatch되기 때문이다
  ([매니페스트 계약](/architecture/manifest.md)의 "취소"). `External` invoker는 대기
  루프의 매 tick마다 토큰을 읽고, 취소가 서면 자식의 **process group을 종료한 뒤**
  `error.code = "cancelled"`(`details.cancelled = true`) 봉투로 끝난다. 그 봉투를
  읽을 사람은 이미 없지만, 자식은 확실히 죽는다.
  - **출력이 없는 도구도 멈춘다.** 신호는 "청크를 큐에 넣지 못했다"가 아니라 본문의
    drop이다. 10분 동안 조용한 빌드가 바로 멈춰야 하는 실행이기 때문이다.
  - 취소를 읽지 못하는 invoker(WASM, 내장 함수)는 여전히 끝까지 달린다. 취소는
    요청이고, 계약은 언제나 마지막 봉투 하나다.
  - `timeout_ms`는 여전히 유용하다 — 취소는 **소비자가 있었다가 사라진** 경우만
    다루고, 애초에 너무 오래 걸리는 실행은 예산이 막는다.

OpenAPI 문서(`/v1/openapi.json`)는 두 스트리밍 경로를 템플릿 경로로 싣는다 — 스트림 본문의
모양은 도구마다 달라지지 않기 때문이다.

# MCP JSON-RPC — `/mcp`

`/mcp`는 `/v1/*`의 리소스 모델이 아니라 JSON-RPC 2.0 계약이다. 메서드와 프레임 모양은
[MCP](/architecture/mcp.md)가 소유하고, 여기서는 **HTTP로서** 어떻게 생겼는지만 적는다.

| 메서드 | `Accept` | 응답 |
|---|---|---|
| `POST` | `text/event-stream` 명시 + 요청에 `id` 있음 | `200 text/event-stream` — 진행 `notifications/message` 이벤트들, 마지막 이벤트가 JSON-RPC 응답, 그 뒤 스트림 종료 |
| `POST` | 그 외 (`*/*` 포함) | `200 application/json` 응답 하나. `id` 없는 알림은 `204` |
| `POST` | — | 본문이 JSON이 아니면 `400`, 헤더가 잘못됐으면 `400` |
| `GET` | `text/event-stream` 명시 | `200 text/event-stream` — 어떤 요청에도 속하지 않는 서버발 프레임(오늘은 `notifications/tools/list_changed`)과 keep-alive 주석 |
| `GET` | 그 외 | `406 Not Acceptable` |

- **`*/*`는 스트림을 열지 않는다.** 와일드카드는 "아무거나"이지 "스트림"이 아니므로,
  `curl`을 포함해 기존 클라이언트는 지금까지 받던 JSON을 그대로 받는다.
- **인증·Origin 가드·pause 게이트는 `/v1/*`와 완전히 동일하다.** 두 방향 모두
  bearer 토큰을 요구한다. SSE는 정책이 아니라 본문 인코딩을 하나 더한 것이다.
- **`Mcp-Session-Id`**: `initialize` 응답에 붙고, 이후 요청이 되돌려 보내면
  `logging/setLevel`이 옮긴 심각도 바닥이 그 세션에 남는다. 요구되지는 않으며 인가와
  아무 상관이 없다 ([MCP](/architecture/mcp.md)).
- **읽지 않는 소비자는 알림을 잃지, 호스트의 메모리를 먹지 않는다.** `/stream`과 같은
  바이트 예산 규칙이며, 잃은 **프레임 수**가 `upeg.transport` logger의 `warning`
  프레임 하나로 합산 보고된다. JSON-RPC 응답 자체는 예산과 무관하게 나간다.
- **연결을 끊으면 도구가 멈춘다.** `POST …/stream`과 같은 계약이며 같은 구현을 쓴다
  ([MCP](/architecture/mcp.md)).
- **조용한 호출도 바이트를 낸다.** 프레임이 없는 동안 SSE 주석(keep-alive)이 흐르므로 중간의
  idle-timeout 프록시가 조용한 호출을 죽은 연결로 오인하지 않는다. 주석은 이벤트가 아니라
  SSE 소비자가 무시하는 줄이다.

**이 lane의 호출자 신원**은 `/v1/*`와 규칙이 다르다.

| 절반 | 값 | 이유 |
|---|---|---|
| surface | 언제나 `mcp` | MCP 클라이언트는 프로그램이다. 어느 프로세스가 중계하든, 어떤 헤더를 붙이든 `mcp`는 `mcp`다 |
| role | bearer 토큰이 증명한 것 (`operator` / `agent`, 인증 못 하면 `agent`) | 이 lane은 리스너를 건너온다. stdio `upeg mcp`의 기본값인 `local`은 여기 호출자를 설명하지 못한다 |

- 헤더 `X-Upeg-Origin-Surface`는 이 경로에 오지 않는다 (아래 "원점 surface").
- **surface를 옮기는 요청 헤더는 없다.**


# 인증

`/healthz`를 제외한 모든 라우트는 상수 시간 비교로 `Authorization: Bearer <token>` 일치를
요구한다. 토큰은 호스트가 발행하며 [호스트 토폴로지](/architecture/host-topology.md)를 따른다.

토큰은 **두 종류**이고, 형태는 같고 권한만 다르다.

| 토큰 | 출처 | 각인되는 주체 |
|---|---|---|
| operator | `~/.upeg/server.json`, `--token` / `--token-file` / `UPEG_HTTP_TOKEN` | `_upeg.principal.role = "operator"` |
| agent | `UPEG_HTTP_AGENT_TOKENS`(콤마 구분) | `_upeg.principal.role = "agent"` |

- **인증과 권한은 다른 질문이다.** 두 토큰 모두 bearer 게이트를 통과해 데이터 평면에 들어온다.
  갈리는 것은 그 다음이다.
- **agent 토큰이 못 하는 것 두 가지.** `X-Upeg-Origin-Surface`가 인정되지 않고(항상 `http`로
  각인된다), Chain의 승인 장벽을 넘지 못한다 — 인가된 표면에서 보내도
  `approval_denied_for_principal`이다 ([Chain Tool](/architecture/chain.md)).
  `approval_surfaces`를 넓혀도 답은 같다.
- **두 갈래 모두 주체를 각인한다.** `/v1/*`는 `{role, 원점 surface}`, `/mcp`는 `{role, mcp}`.
  각인하지 않는 라우트는 없다 — 각인을 빠뜨리면 surface별 기본값이 그 자리를 채우고,
  `mcp`의 기본값은 승인 가능한 `local`이다.
- **인증하지 못한 호출자는 `agent`다.** 토큰을 요구하지 않는 브링업 라우터가 유일한 경우이며,
  아무도 식별하지 못하는 호스트는 아무도 operator로 승격시키지 않는다.
- 주체는 서버가 각인하고 호출자가 보낸 값은 지워진다
  ([호출 봉투](/architecture/call-envelope.md)의 "주체"). 실행 로그에는 **역할 라벨만**
  남는다 — 토큰 값은 저장되지 않는다.

# 원점 surface — `X-Upeg-Origin-Surface`

호스트가 떠 있으면 `upeg call`과 TUI는 in-process 대신 `/v1/tools/{id}`로 붙는다. 그 호출을
전부 `http`로 각인하면 **전송 방식이 호출자 신원을 바꿔 버린다** — 사람이 자기 터미널에서
`upeg call <chain> -a approve=true`를 쳐도 `http`는 승인 surface가 아니라서 거부됐고, TUI에는
`--local` 같은 탈출구조차 없었다 ([Chain Tool](/architecture/chain.md)).

그래서 attach 클라이언트는 **자신이 어느 로컬 surface에서 부르는지**를 헤더로 밝힌다.

```text
POST /v1/tools/dev.precommit
Authorization: Bearer <token>
X-Upeg-Origin-Surface: cli
```

| 규칙 | 내용 |
|---|---|
| 허용 값 | `cli`, `tui` — 이 바이너리에서 실제로 attach하는 두 surface. `desktop`은 FRB로 in-process 실행이라 attach하지 않고, `pwa`/`ext`는 페어링 토큰으로 닿는 원격 클라이언트라 `http` 그대로다 |
| 인정 조건 | 요청이 **이 호스트의 operator 토큰으로 인증**되어야 한다. 토큰은 `~/.upeg/server.json`에 있고 같은 OS 사용자만 읽는다 — "로컬 클라이언트"라는 말의 신뢰 경계가 정확히 그것이다. agent 토큰은 프로그램을 인증할 뿐이고, 프로그램이 어느 사람용 surface에 앉아 있는지를 자칭할 수는 없다 |
| 불인정 시 | 오류가 아니라 `http`다. 토큰이 없거나, agent 토큰이거나, 값이 허용 목록 밖이거나, 헤더가 없으면 헤더가 없던 때와 똑같이 동작한다 |
| 적용 범위 | `_upeg.surface` 각인과 **surface 가시성 게이트 둘 다**, 버퍼 경로와 `/stream` 경로 모두에서. attach된 `upeg call`은 `--local`로 돌렸을 때와 같은 도구 집합을 본다. `_upeg.surface`와 `_upeg.principal.surface`는 언제나 같은 값을 가리킨다 |
| MCP proxy 제외 | `upeg mcp`가 `/mcp`로 중계할 때는 이 헤더를 보내지 않는다. MCP 클라이언트는 프로그램이고, 어느 프로세스가 중계하든 `mcp`는 `mcp`다 |

헤더는 **신원이 아니라 선언**이다. 인증 경계를 넓히지 않는다 — operator 토큰을 가진
클라이언트는 이미 그 호스트에서 도구를 실행할 수 있고, 이 헤더는 그중 어느 로컬 surface인지만
좁혀 말한다. 토큰 단위 호출자 신원은 위의 [인증](#인증)이 답한다.

# CORS

브라우저 origin이 무엇을 할 수 있는지는 *누가 행위할 수 있는지*와 분리해서 결정한다.

- **기본 허용**: 모든 `chrome-extension://<id>` origin, 그리고 loopback origin
  (`http(s)://127.0.0.1:*`, `localhost:*`, `[::1]:*`).
- **임의 웹 origin**은 `--cors-origin <ORIGIN>` 정확 일치를 요구한다. 반복 지정 가능하고
  와일드카드는 없다. `*`, 스킴 누락, path/query 포함은 거부된다.
- **Preflight(`OPTIONS`)는 의도적으로 토큰이 없다** — CORS 계층이 가장 바깥에 있어 bearer
  검사 전에 short-circuit된다. 데이터 평면은 origin과 무관하게 항상 bearer 인증을 거친다.
- **허용 요청 헤더**: `Authorization`, `Content-Type`, `Mcp-Session-Id`, `X-Upeg-Board`.
  앞의 둘은 모든 호출이 싣고, 뒤의 둘은 `/mcp` lane의 요청 계약이다. 브라우저는 preflight가
  허용하지 않은 헤더를 아예 보내지 못하므로, 빠뜨리면 브라우저 MCP 클라이언트는 자기가 받은
  세션 id를 되돌려 보낼 수 없다.
- **노출 응답 헤더**: `Mcp-Session-Id`. 브라우저는 expose되지 않은 응답 헤더를 스크립트에
  숨긴다 — 서버가 발급하고 클라이언트가 읽을 수 없는 세션 id는 세션 id가 아니다.
- **`X-Upeg-Origin-Surface`는 열지 않는다.** `cli`/`tui` + operator 토큰에서만 인정되는
  헤더이고, 브라우저로 배달되는 surface는 둘 중 어느 것도 될 수 없다. 여는 것은 작동하지 않는
  레버를 광고하는 일이다.
- **Host anti-rebinding은 CORS와 독립적으로 유지된다**: 존재하지만 허용되지 않은 `Origin`
  헤더를 거부하고, loopback이 아닌 `Host` 헤더를 거부한다. CORS는 브라우저 JS가 무엇을
  *읽을* 수 있는지만 통제한다.
