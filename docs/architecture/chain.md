---
type: Domain Contract
title: Chain Tool
description: Chain의 노드/연결 모델, 표현식 문법, 실행 규칙.
tags: [architecture, domain, chain]
status: stable
sources:
  - id: chain-dispatcher
    resource: ../../upeg-loader/src/dispatcher.rs
    title: Chain dispatcher 구현
  - id: chain-approval
    resource: ../../upeg-loader/src/dispatcher/chain/approval.rs
    title: 승인 인가 정책
  - id: chain-principal
    resource: ../../upeg-core/src/principal.rs
    title: 호출자 주체(principal) 어휘
  - id: chain-approval-policy
    resource: ../../upeg-runtime/src/approval.rs
    title: UI가 dispatch 전에 읽는 승인 정책
  - id: chain-step-summary
    resource: ../../upeg-loader/src/dispatcher/chain/summary.rs
    title: step 요약 메타데이터
---

# 계약

Chain은 `invoker = "Chain"`인 Tool이다. 다른 모든 Tool과 동일하게 핀·목록·필터·로그·트리거·
dispatch된다. 복잡성은 Chain 엔진 내부에 봉인되고 사용자는 핀 하나만 본다.

# 런타임 모델

`steps`가 노드 인스턴스를, `connections = [{ from, to }]`가 방향 간선을 선언한다. 선형 체인,
fan-out, join 모두 같은 연결 목록을 쓴다. 들어오는 연결이 없는 노드는 체인 입력을 받는다.

각 step은 Tool id를 참조하며 다음을 선언할 수 있다.

- `args`: `{{ }}` 표현식을 담은 JSON 객체.
- `when`: 불리언 표현식. false면 step을 건너뛴다.
- `requires_approval`: 실행 전 승인 장벽. 누가 그 장벽을 넘을 수 있는지는
  체인 수준의 `approval_surfaces`가 정한다 (아래 [승인 인가](#승인-인가) 참고).

위치 이름 대신 의미 있는 노드 id를 쓴다.

```toml
connections = [{ from = "hash", to = "uppercase" }]

[[steps]]
id = "hash"
tool = "hash.md5"

[[steps]]
id = "uppercase"
tool = "text.uppercase"
```

# 표현식 문법

의도적으로 작다. `{{step.field}}` 치환 이상은 지원하지 않는다.

- `{{input.key}}` — 최상위 호출 인자.
- `{{steps.step_id.output}}` — 이전 step의 텍스트 출력.
- `{{steps.step_id.ok}}` — 이전 step의 성공 여부.
- `{{context.board}}` — 예약 `_upeg` 컨텍스트.

잘못된 참조는 결정론적 Tool 에러이며, 값 없이 로그된다.

# 실행 규칙

1. 중복 노드 id, 알 수 없는 연결 끝점, 순환은 로드 시점에 거부한다.
2. 상류 연결이 충족된 준비된 step을 실행한다.
3. 실패한 필수 step에서 멈춘다.
4. 각 step의 결말은 결과 봉투에 실려 나간다 (아래 [step 요약](#step-요약)).
5. 승인 step은 명시적 승인을 요구하고, 그 승인은 **호출자(principal)**가 배제되지 않았고
   **호출 surface**가 인가된 경우에만 인정된다 (아래 [승인 인가](#승인-인가)).
6. 최종 출력은 `output` 표현식이 설정되지 않았다면 마지막으로 완료된 step의 출력이다.

# 승인 인가

`approve = true`와 `_upeg.approvedSteps`는 호출 봉투 안의 평범한 값이다. 호출자가 자유롭게
채워 넣을 수 있으므로 그 자체로는 **의도**만 말한다. 반면 `_upeg.surface`와
`_upeg.principal`은 런타임이 각인하고 호출자가 보낸 값은 dispatch 전에 지워진다
([호출 봉투](/architecture/call-envelope.md)) — 그래서 dispatcher가 믿을 수 있는 호출자
신원이다. 승인 인가는 의도와 신원을 분리하고, 신원을 다시 **두 개의 독립된 게이트**로 나눈다.

| 게이트 | 묻는 것 | 근거 | 거부 코드 |
|---|---|---|---|
| 주체 | 이 호출자가 승인할 자격이 있는가 | `_upeg.principal.role` | `approval_denied_for_principal` |
| 표면 | 이 체인이 그 문에서 온 승인을 인정하는가 | `_upeg.surface` + `approval_surfaces` | `approval_denied_for_surface` |

두 게이트는 서로를 대신하지 않는다. 주체 게이트를 통과해도 표면 게이트가 남고, 표면을
`approval_surfaces`로 넓혀도 주체 게이트는 열리지 않는다.

## 주체 게이트

`_upeg.principal = { role, surface }`의 `role`은 셋 중 하나다
([호출 봉투](/architecture/call-envelope.md)의 "주체").

| role | 누구 | 승인 |
|---|---|---|
| `operator` | 이 호스트를 띄운 사람 — in-process `cli`/`tui`/`desktop`, 또는 operator bearer 토큰을 실은 HTTP 요청 | 가능 |
| `local` | OS 사용자가 띄운 in-process 프로그램 — MCP stdio lane | 가능하지만 표면 게이트가 따로 막는다 |
| `agent` | 프로그램으로 인증된 호출자 — agent 토큰을 실은 HTTP 요청, 그리고 이 호스트가 아예 식별하지 못한 호출자 | **불가능** |

배제되는 것은 `agent` 하나뿐이고, 그 이유는 운영자 자신의 설정이다. agent 토큰을 발급한다는
것은 "너는 내가 아니다"라고 말한 것이므로, 매니페스트가 `approval_surfaces`를 넓혀서 그 판단을
뒤집을 수는 없다. `local`을 함께 막으면 더 엄격해 **보이지만** 틀리다 — `cli`와 같은 OS 사용자
경계에 서 있고, 막는 순간 체인 저자가 일부러 적은 `approval_surfaces = ["mcp"]`가 조용히
무효가 된다.

## 표면 게이트

체인은 `approval_surfaces`로 승인을 인정할 surface를 선언한다.

```toml
[[tools]]
id = "precommit"
invoker = "Chain"
approval_surfaces = ["cli", "tui", "desktop"]   # 생략 시 기본값과 같다
```

- **기본값은 사람이 앉아 있는 세 표면(`cli`/`tui`/`desktop`)이다.** 셋은 한 가지 성질을
  공유한다: 호출자가 OS 사용자 계정 자신이라, 토큰이 증명해 주지 않아도 `operator` 주체다.
- **세 표면 모두 승인 제스처가 있다.** `cli`는 `upeg call <chain> -a approve=true`,
  `tui`는 실행 앞에 서는 확인 대화상자(`Enter`/`F1`/`y` 승인, `Esc`/`n`/`q` 취소),
  `desktop`은 실행 전 확인 다이얼로그다. 셋 다 **사람의 확인을 받은 다음에만** `approve`를
  싣는다 — 확인을 건너뛰고 dispatch하는 경로는 어느 UI에도 없다. 표면별 제스처는
  [UI/UX 표면 계약](/ui-ux-surface-contract.md)의 "Approval and live output"에 한 표로 있다.
- **GUI surface에서 `approve`는 데이터가 아니라 typed 파라미터다.** Dart는 `approve: bool`을
  FRB 경계로 넘기고 예약 키를 args에 넣는 것은 Rust뿐이다. `shape_approval_arg`
  (`upeg-frb/src/api/tools.rs`)는 호출자가 실어 보낸 **두 레버를 모두** 먼저 지운다 —
  `approve` 키와 `_upeg.approvedSteps`. 후자는 예약 블록 지우기에서 살아남는 유일한 키라,
  `approve`만 지우는 동안에는 `upeg://open` 딥 링크가 실어 보낸
  `{"_upeg":{"approvedSteps":["gate"]}}`가 그대로 장벽을 열었다. 딥 링크의 입력은
  그 위에서 한 번 더, Tool이 선언한 입력 필드로 걸러진다. 위젯 트리 어디에서 조립한 args도,
  핀에 저장된 args preset도, URL 하나도 스스로를 승인할 수 없다.
- **GUI surface는 이 빌드의 런타임이 정한다.** native 빌드는 `desktop`, wasm32(PWA) 빌드는
  `pwa`다(`upeg-frb/src/api/capability.rs`의 `surface_for`). 둘을 하나로 굳혀 놓으면 브라우저
  탭이 `desktop` — 기본 승인 표면 — 으로 dispatch하게 된다.
- **인정되지 않는 표면은 묻지 않는다.** `approval_surfaces`에 자기 표면이 없으면 TUI는 result
  pane에, desktop은 다이얼로그에 **누가 승인할 수 있는지**를 적고 dispatch하지 않는다.
  인정되지 않을 "예"를 받아내는 것은 사람에게 없는 권한을 있는 척하는 일이다.
- **기계가 건 실행은 승인하지 않는다.** desktop의 timer 폴링·핀 활성화 같은 machine-timed
  경로는 언제나 `approve: false`로 dispatch하고, 승인 장벽이 있는 Tool은 inline 핀에서
  자동 실행되지 않는다(명시적 Run 버튼만 남는다). 승인은 사람에게 묻는 질문이지 주기가
  대신 답할 수 있는 것이 아니다.
- **UI는 dispatch 전에 물어볼 수 있다.** `ToolMeta`가 `requires_approval`(확인을 띄워야 하는가)와
  `approval_surfaces`(내 표면의 확인이 인정되는가)를 노출하고, FRB `ToolDto`도 같은 두 값을
  `requiresApproval` / `approvalSurfaces`로 싣는다. 로더가 체인 등록 시점에 채운다
  (`upeg-runtime/src/approval.rs`). 승인 장벽이 없는 Tool은 `requires_approval = false`이고
  `approval_surfaces`는 빈 목록이다 — 승인할 것이 없으면 승인자도 지목하지 않는다.
- `mcp`/`http`는 스스로 봉투를 채우는 프로그램 호출자이고, `pwa`/`ext`는 페어링 토큰 HTTP
  surface를 거쳐 원격에서 닿는다. 넷 다 이름을 명시해야 승인할 수 있다.
- **호스트에 attach해도 터미널은 터미널이다.** 호스트가 떠 있을 때 `upeg call`은 HTTP로
  붙지만, 클라이언트가 보내는 원점 surface 헤더를 호스트가 검증해 `_upeg.surface`로 각인하므로
  각인되는 값은 여전히 `cli`다 ([HTTP API](/architecture/http-api.md)의 "원점 surface").
  `--local`은 그래서 승인의 필수 조건이 아니라 in-process 실행을 고르는 선택지로 남는다.
- **반대로 `/mcp`는 어떤 헤더로도 옮겨지지 않는다.** MCP 클라이언트는 프로그램이고, 그
  lane의 surface는 언제나 `mcp`다. 주체는 bearer 토큰이 정한다 — `approval_surfaces`가
  `mcp`를 지목한 체인이라도 agent 토큰으로 온 요청은 `approval_denied_for_principal`이다
  ([HTTP API](/architecture/http-api.md)의 "MCP JSON-RPC").
- 로드 시점에 검증한다. 알 수 없는 surface, 빈 항목, 아무도 인가하지 않는 빈 목록,
  체인이 아닌 Tool에 붙인 선언, 그리고 **Tool의 `surfaces`와 하나도 겹치지 않는 목록**
  (`approval_surfaces = ["cli"]` + `surfaces = ["mcp"]` — 승인할 수 있는 호출자가 이 Tool에
  닿을 수 없다)은 모두 로드 에러다.
- 인자 모양은 그대로다. `approve` / `_upeg.approvedSteps`는 예전과 같이 동작하며,
  달라지는 것은 그 값이 **인정되는 조건**뿐이다.

## 거부 코드

거부는 정직한 타입 코드로 나간다. AI 에이전트가 "여기서는 승인할 수 없다"를 재시도와 구분할 수
있어야 하기 때문이다.

| 상황 | `error.code` |
|---|---|
| 호출자 주체가 `agent` | `approval_denied_for_principal` |
| 인가되지 않은 surface (또는 `_upeg.surface`가 없는 호출) | `approval_denied_for_surface` |
| 인가된 surface인데 승인을 보내지 않음 | `approval_required` |

판정 순서는 주체 → 표면이다. agent가 인가된 표면에 도달했을 때 "표면이 틀렸다"고 답하면 열리지
않았을 문을 찾아다니게 만들기 때문이다.

`approval_denied_for_surface` 메시지는 어느 surface가 승인할 수 있는지 나열하고, 그 목록에
`cli`가 있으면 **실행할 명령을 그대로 적어 준다** — `upeg call <chain-id> -a approve=true`.
`approval_denied_for_principal` 메시지는 표면을 바꾸거나 `approval_surfaces`를 넓혀도 답이
같다는 것을 명시한다. 에이전트가 받는 답은 어느 쪽이든 "사람에게 이 명령을 부탁하라"이지
"다시 보내 보라"가 아니다.

**중첩 체인도 같은 호출자로 판정된다.** step의 args는 매니페스트 템플릿이 만든 새
객체이고 `{{input.*}}`로 호출자 텍스트가 섞여 들어오므로, 거기 적힌 `_upeg`는 신원이 아니다.
엔진은 step args의 `_upeg` 블록을 버리고 **호출의 블록을 그대로 물려준다**
(`upeg_runtime::inherit_call_context`) — 그래서 안쪽 체인의 승인 장벽은 바깥 호출의 진짜
surface와 주체로 판정되고, step args로 `_upeg.surface`나 `_upeg.principal`을 지어내 장벽을
여는 길은 없다.

**경계의 한계.** 주체는 여전히 사용자 신원이 아니다. 두 operator 세션을 구분하지 못하고,
승인을 그것을 준 사람의 이름에 묶지도 못한다 — 그건 진짜 사용자 계정을 요구하는데 upeg에는
없다. 지금 증명할 수 있는 것은 "이 호스트가 이 호출자를 operator로 인증했는가"까지다 →
결정 기록.

# step 요약

모든 Chain 결과는 step별 메타데이터를 싣는다. 실행 규칙 4가 약속하는 "건너뛴 step 기록"이
실제로 호출자에게 도달하는 지점이며, 정본 봉투를 타므로 surface별 배선 없이 CLI `--json`,
HTTP, MCP `structuredContent`가 같은 값을 본다.

- 성공: `steps`라는 output row (kind `json`).
- 실패: `error.details.steps`. 같은 배열 모양이다.

각 행은 `{ id, tool, status, duration_ms }`이다.

| `status` | 뜻 |
|---|---|
| `ran` | dispatch되어 성공했다 |
| `skipped` | `when`이 false여서 dispatch되지 않았다 |
| `failed` | dispatch되어 실패했다 (또는 Tool을 찾지 못했다) |
| `denied` | 승인 장벽에서 막혔다. 미승인인지 주체/표면 인가 실패인지는 `error.code`가 구분한다 |

결말에 도달한 step만 실린다 — 체인이 중간에 멈추면 아직 닿지 못한 링크는 행 자체가 없다.
`duration_ms`는 그 step의 dispatch에 걸린 벽시계 시간이고, dispatch되지 않은 step은 0이다.

`steps`는 Chain에서 예약된 출력 id다. Chain Tool이 같은 이름의 `outputs`를 선언하면 로드
에러이고, 마지막 step이 런타임에 같은 id의 출력을 내면 `chain_step_summary_conflict`로
실패한다 — 엔진 행이 작성자의 행을 조용히 덮지 않는다. 자기 출력이 하나도 없던 체인은
`steps`가 primary 출력이 된다.

**마지막 step이 또 다른 체인이면** 그 안쪽 실행의 엔진 행이 봉투에 실려 돌아온다. 모든 체인은
자기 step에 대해 답하므로, 바깥 엔진은 그 행을 **접고** 자기 행을 놓는다 — 중첩 체인은 바깥
요약에서 매니페스트가 선언한 대로 step 하나로 기록된다. 접히는 것은 엔진이 만든 모양
(`steps` + `json` + `{ id, tool, status, duration_ms }` 배열)뿐이라, 체인이 아닌 도구가 낸
`steps` 출력은 예전처럼 `chain_step_summary_conflict`로 보고된다.

step이 실패하면 **그 step의 `error.code`가 그대로 체인의 코드가 된다.** 중첩 체인이 승인
장벽에서 멈췄다면 바깥에서도 `approval_denied_for_surface`(또는
`approval_denied_for_principal`)로 읽혀야, 에이전트가 재시도할 일이
아님을 알 수 있다.

관련: [매니페스트 계약](/architecture/manifest.md), [호출 봉투](/architecture/call-envelope.md)
