---
type: Protocol Contract
title: 호출 봉투와 예약 컨텍스트
description: 모든 비-UI 프로토콜이 공유하는 call 봉투, `_upeg` 예약 컨텍스트, CLI positional 바인딩과 셸 자동완성 규칙.
tags: [architecture, protocol, cli, ipc]
status: stable
sources:
  - id: cli-args
    resource: ../../upeg-cli/src/app/args.rs
    title: positional 바인딩 구현
  - id: execution-context
    resource: ../../upeg-runtime/src/execution.rs
    title: 예약 블록 병합과 호출자 보존 키
  - id: fired-trigger
    resource: ../../upeg-runtime/src/triggers.rs
    title: FiredTrigger 라벨 구성
  - id: principal
    resource: ../../upeg-core/src/principal.rs
    title: Principal / PrincipalRole 어휘
  - id: cli-completion
    resource: ../../upeg-cli/src/surfaces/cli/completion.rs
    title: 자동완성 스크립트 생성과 동적 Tool 후보 주입
---

# 공유 봉투

모든 비-UI 프로토콜은 다음으로 환원된다.

```json
{
  "tool": "toolkit.tool",
  "args": { "input": "...", "_upeg": { "board": "dev" } }
}
```

| Surface | 형태 |
|---|---|
| CLI 일반 | `upeg call <id> <json>` 또는 `upeg call <id> -a key=value` |
| CLI 동적 | `upeg <toolkit> <tool> [pos1] [pos2]...` |
| MCP | `tools/call`이 같은 `name` + `arguments` 형태를 쓴다 |
| HTTP | `POST /v1/tools/{id}`, `POST /v1/boards/{board}/tools/{id}` |

# 예약 컨텍스트 `_upeg`

surface가 제공하는 컨텍스트 전용으로 예약된 인자 키다. 사용자 입력과 충돌하지 않으므로
dispatcher API가 안정적으로 유지된다.

키는 두 갈래다. **surface가 각인하는 키**는 호출자가 보낸 값이 항상 지워진다(스푸핑 방지).
**호출자가 실어 보내는 키**는 surface 쪽에 각인할 원본이 없어서, 그 지우기에서 예외로 살아남는
좁은 allow-list다 (`upeg-runtime/src/execution.rs`의 `CALLER_PRESERVED_CONTEXT_KEYS`).

| 키 | 출처 | 의미 |
|---|---|---|
| `board` | surface | 활성 Board 키 |
| `boardEnv` | surface | Board 범위 환경변수 맵 |
| `projectManifest` | surface | 탐지된 `upeg.toml` 경로 |
| `surface` | surface | 호출 surface 라벨 (`cli`, `mcp`, `http`, …). 로그와 parity 진단, 그리고 Chain 승인 인가의 기준이다. 호스트에 attach한 로컬 클라이언트는 `X-Upeg-Origin-Surface`로 자기 surface를 밝히고 호스트가 그것을 검증해 각인한다 ([HTTP API](/architecture/http-api.md)) — 전송이 HTTP라는 사실이 호출자 신원을 바꾸지 않는다 |
| `principal` | surface | 호출자 주체 `{ role, surface }`. `surface`가 "어느 문"이라면 이쪽은 "무슨 권한". 아래 [주체](#주체) |
| `trigger` | surface | 이 호출을 시작한 Trigger의 라벨 — `<source>` 또는 `<source>:<condition>` |
| `cwd` | 호출자 | 호출자가 준 절대 작업 디렉터리. `External` invoker의 child process 위치 결정에 쓰인다 |
| `approvedSteps` | 호출자 | Chain step 승인 allow-list. 호출자가 자유롭게 채울 수 있으므로 **의도**만 말한다 — 그 의도를 인정할지는 `principal.role`과 `surface`가 각각 판정한다 ([Chain Tool](/architecture/chain.md)). GUI surface(`desktop`/`pwa`)는 예외로, 이 키가 args로 **도착하는 것 자체를** 막는다 (아래 [GUI surface의 승인 레버](#gui-surface의-승인-레버)) |

`External` invoker는 추가로 `UPEG_BOARD`, `UPEG_PROJECT_MANIFEST`, 그리고 각 `boardEnv`
항목을 프로세스 환경변수로 받는다.

## GUI surface의 승인 레버

`approvedSteps`가 호출자 보존 키인 것은 CLI·MCP·HTTP처럼 **호출자가 봉투를 직접 쓰는**
surface를 위해서다. GUI surface(`desktop`, `pwa`)는 그 반대다 — 사람이 버튼을 누르고, 승인은
typed 파라미터 하나로 FRB 경계를 건넌다.

그래서 GUI dispatch는 args가 실은 승인 레버를 **둘 다** 지우고 나서 typed 플래그를 쓴다
(`upeg-frb/src/api/tools.rs`의 `shape_approval_arg`): `approve` 키와 `_upeg.approvedSteps`.
`approve`만 지우던 동안에는 남은 하나가 데이터 경로였다 — `upeg://open?...&input=` 딥 링크가
`{"_upeg":{"approvedSteps":["gate"]}}`를 싣고 오면, `desktop`은 기본 승인 표면이므로 사람이
답한 적 없는 장벽이 열렸다. 딥 링크 쪽에서도 같은 규칙을 한 번 더 적용한다: URL이 실은 입력은
Tool이 **선언한 입력 필드**로 걸러진 뒤에만 dispatch에 도달한다
(`flutter_app/lib/src/widgets/launch_intent_applier.dart`).

## 주체

`_upeg.principal = { "role": "operator|agent|local", "surface": "cli" }`.

`surface`만으로는 같은 문으로 들어온 두 호출자를 구분할 수 없다. HTTP 리스너 하나에 사람과
에이전트가 같이 붙어 있고, 둘을 가르는 것은 어느 bearer 토큰을 실었느냐다. `principal`이 그
구분을 봉투에 싣는다.

| role | 증명 |
|---|---|
| `operator` | in-process `cli`/`tui`/`desktop`(OS 사용자 계정이 곧 호출자), 또는 operator bearer 토큰을 실은 HTTP 요청 |
| `local` | OS 사용자가 띄운 in-process 프로그램 — MCP **stdio** lane |
| `agent` | agent 토큰을 실은 HTTP 요청, 그리고 이 호스트가 식별하지 못한 호출자(`http`/`pwa`/`ext`의 바닥값) |

`mcp` surface는 두 lane을 갖고, 주체는 lane마다 다르다. stdio(`upeg mcp`)는 OS 사용자가
띄운 프로세스이므로 `local`이고, 호스트의 `POST /mcp`는 리스너를 건너온 요청이므로 **토큰이
증명한 역할**(`operator` 또는 `agent`)이다. surface는 양쪽 다 `mcp`다.

- **호출자가 각인할 수 없다.** `surface`와 같은 갈래에 있어서, 호출자가 보낸 `principal`은
  예약 블록 지우기에서 버려지고 런타임이 자기 값을 쓴다
  (`upeg_runtime::apply_execution_context`).
- **모든 호출이 주체를 받는다.** 표면별 기본값이 전역(total)이므로 dispatch 경로가 주체를
  빠뜨릴 수 없고, 인증하는 표면(HTTP)만 토큰으로 그 기본값을 좁힌다 — `/v1/*`와 `/mcp`
  **양쪽 다**.

- **쓰임은 두 곳이다.** Chain 승인의 주체 게이트([Chain Tool](/architecture/chain.md))와
  실행 로그의 `principal` 열(`upeg log`가 `principal=<role>`로 보여주고 `--json`에도 실린다).
  로그에는 **역할 라벨만** 들어간다 — 토큰 값은 어디에도 저장되지 않는다.

## 하위 호출은 `_upeg`를 물려받는다

Chain step의 args는 매니페스트 템플릿이 만든 **새 객체**이고, `{{input.*}}` 표현식이 호출자
텍스트를 그 안으로 흘려 넣는다. 그래서 step args에 적힌 `_upeg`는 surface의 각인이 아니라
호출자가 쓴 값일 수 있다. 엔진은 그것을 버리고 **호출의 `_upeg` 블록을 그대로 물려준다**
(`upeg_runtime::inherit_call_context` — 위의 예약 블록 지우기와 같은 정의를 쓴다).

- step은 호출의 `surface`/`principal`/`board`/`boardEnv`/`cwd`를 그대로 본다. 중첩 체인의
  승인 장벽도 바깥 호출의 진짜 surface와 주체로 판정된다
  ([Chain Tool](/architecture/chain.md)).
- step args로 `_upeg.surface`나 `_upeg.principal`을 지어내 인가를 통과하는 길은 없다.
- 호출 자체에 `_upeg`가 없으면 step args에도 그 키가 붙지 않는다 — 아무도 쓰지 않은 키를
  dispatcher가 보게 되는 일이 없다.

## `_upeg.trigger` 라벨

Tool id가 아니라 **발화한 Trigger**를 각인한다. Tool은 자기 id를 이미 알고 있고, Trigger를
여러 개 선언한 Tool은 id만으로는 어느 것이 발화했는지 구분할 수 없기 때문이다.

- 형태: `<source>`, 조건이 있으면 `<source>:<condition>`
  (`clipboard`, `webhook`, `file:/tmp/drop.txt`, `schedule:every:30s`, `hotkey:ctrl+shift+u`)
- `condition` 자체가 `:`를 품을 수 있으므로, 되돌려 나눌 때는 **첫** 구분자에서만 나눈다.
- JSON object가 아니라 문자열인 이유: 실행 로그의 `trigger` 열과 `--trigger` 필터가
  문자열로 읽는다 (`upeg-cli/src/adapters/execution_log.rs`). object로 바꾸면 모든 로그
  레코드에서 trigger가 조용히 사라진다.
- 각인 지점: CLI trigger 폴 루프, `hotkey` 어댑터, HTTP `POST /v1/trigger/{tool_id}` 라우트,
  `upeg trigger fire`.
- `upeg trigger fire <tool_id>`는 binding이 아니라 **Tool**을 지목하므로, 여러 binding 중
  어느 것을 흉내 내려는지 알 수 없다. 규칙은 정직하게 유지되는 가장 단순한 것이다:
  **먼저 선언된 binding이 이긴다**. binding이 하나도 없는 Tool은 아무것도 각인하지 않는다 —
  `_upeg.trigger`는 "무엇이 이 호출을 시작했는가"에 답하는 필드이고, 그런 Tool의 답은
  "Trigger가 아니라 사람"이기 때문이다. 여기서 합성 source를 지어내면 어떤 Tool도 선언할 수
  없는 값이 실행 로그의 `trigger` 열에 들어간다.

# CLI 동적 라우트 — positional 바인딩

`upeg {toolkit} {tool} <pos1> <pos2> ...`가 유일한 동적 라우트다. Toolkit별 하드코딩
서브커맨드 enum은 없다. positional 인자는 `ToolMeta.input_spec`의 필드에 **선언 순서대로**
1:1 바인딩된다 (`InputSpec`이 작성자 선언 순서를 보존한다).

1. `input_spec.fields`를 선언 순서로 나열한다.
2. positional 인자를 그 순서에 1:1로 묶는다.
3. 각 값은 필드의 `InputKind`로 강제 변환된다.
   - `string` / `markdown` / `file_path` / `url` / `datetime`: 그대로.
   - `number` / `integer`: 파싱. 실패하면 명확한 Tool 에러.
   - `boolean`: `true` / `false` (대소문자 무시).
   - `json`: JSON으로 파싱.
   - `options`: 선택지 검증.
   - `multi_options`: 콤마 구분 파싱 후 각 선택지 검증.
4. input spec이 비어 있으면 `{ "input": "<joined>" }` 형태로 폴백한다 — 단일 입력 도구의
   `upeg num hex-to-decimal 0xff`가 계속 동작한다. 이 폴백은 **positional 인자에만**
   적용된다: 필드를 하나도 선언하지 않은 Tool에는 stdin이 될 자리가 없으므로 아래 6번의
   자동 stdin 규칙이 적용되지 않는다. (`upeg time iso-now`, `upeg id uuid-v7` 같은
   무입력 Tool이 EOF가 오지 않는 상속 파이프에서 영원히 멈추던 원인이었다.)
5. 입력 필드 수를 넘는 positional 인자는 명확한 Tool 에러다. `upeg text uppercase hello world`가
   `world`를 조용히 버리지 않는다.
6. stdin이 파이프(non-TTY)이고 positional 인자가 없으면, stdin을 첫 필수 입력 필드의 값으로
   읽는다. 필수 입력 필드가 없는 Tool(무입력 Tool 포함)에는 적용되지 않는다.

`-`는 "이 자리는 stdin에서 읽는다"는 뜻의 positional 자리표시자다.

positional 바인딩은 이 경로에만 적용된다. `upeg call <id> <json>`과
`upeg call <id> -a key=value`는 스키마에 무관한 명시적 형태로 남는다.

# 셸 자동완성 — 생성 시점 스냅샷

`upeg completions <shell>`이 bash · zsh · fish · elvish · powershell용 스크립트를 stdout으로
낸다(`completion`이 별칭이다). 설치는 셸의 자동완성 디렉터리로 리다이렉트하는 방식이다.

```bash
# bash — bash-completion 2.x의 사용자 경로
mkdir -p ~/.local/share/bash-completion/completions
upeg completions bash > ~/.local/share/bash-completion/completions/upeg
```

```zsh
# zsh — fpath에 든 디렉터리에 `_upeg`로 두고, compinit 전에 fpath를 넓힌다
mkdir -p ~/.zfunc
upeg completions zsh > ~/.zfunc/_upeg
# ~/.zshrc: fpath=(~/.zfunc $fpath) 를 compinit 호출보다 앞에 둔다
```

```fish
mkdir -p ~/.config/fish/completions
upeg completions fish > ~/.config/fish/completions/upeg.fish
```

동적 라우트에는 clap이 아는 서브커맨드가 없으므로, 생성기가 **생성하는 시점의 toolbox를
읽어** 스크립트에 후보를 심는다. 두 가지를 심는다.

1. `upeg call <TAB>`의 자리에는 CLI surface에 노출된 Tool의 **정본 id**를 넣는다.
2. Toolkit마다 가짜 서브커맨드를 하나씩 합성하고 그 아래에 kebab-case Tool 이름을 넣는다.
   그래서 `upeg <TAB>`이 Toolkit을 제시하고 `upeg num <TAB>`이 Tool을 제시한다. 이미 있는
   내장 서브커맨드와 이름이 겹치면 합성하지 않는다 — 내장 서브커맨드를 가리지 않는다.

이 합성은 **생성 전용**이다. 실제 파싱은 그대로 `Cli::parse`를 거치므로 `external_subcommand`
의미론은 바뀌지 않는다.

## 스냅샷이라는 것의 의미

생성된 스크립트는 그 시점의 Tool 목록을 문자열로 박아 둔 산출물이다. 따라서 Toolkit TOML을
새로 설치하거나 WASM 플러그인을 추가하거나 다른 프로젝트 디렉터리로 이동하면, **스크립트를
다시 생성하기 전까지 새 Tool은 완성되지 않는다.** 이것은 결함이 아니라 이 방식이 지불하는
비용이며, 설치 안내에 재생성이 따라붙어야 하는 이유다.

## 후보가 없는 자리 — 두 부류를 섞지 않는다

후보가 비어 있는 자리에는 성질이 다른 둘이 섞여 있다. **생성 시점에 목록을 열거해 심을 수
있는 자리**와, **현재의 고정 `PossibleValuesParser` 주입만으로는 표현되지 않는 자리**다.
정적 스크립트 전체의 한계가 아니라 현재 생성기 구현의 구분이다.

### 정적으로 심을 수 있으나 아직 심지 않았다

후보 집합이 다른 인자에 의존하지 않으므로, Toolkit 후보를 심는 것과 같은 생성 시점 열거로
해결된다.

| 자리 | 후보의 출처 |
|---|---|
| `--surface` | `Surface`는 7개 variant의 닫힌 enum이다 (`upeg-core/src/types.rs`). Tool·사용자 상태와 무관하다 |
| `--tag` | `upeg_runtime::tags_for_surface()` (`toolbox.rs:462`). 생성기가 이미 쓰는 `toolkits_for_surface()`와 같은 계열이다 |
| `--board` (매니페스트 선언분) | `upeg_runtime::boards_for_surface()` (`toolbox.rs:479`). 매니페스트가 선언한 board는 디스크 없이 열거된다 |
| `--board` (사용자 pin 기반) | 생성 시점에 pegboard 상태를 읽어 스냅샷으로 심을 수 있다. 이후 board 변경은 재생성 전까지 반영되지 않는다 |
| `tool show <id>` · `trigger fire <id>` | Tool 정본 id. `call`의 positional에 이미 심고 있는 것과 같은 목록이다 |

### 현재 고정 후보 주입만으로 표현되지 않는다

후보 집합이 **앞선 인자에 의존**하거나 scoped 경로의 clap 메타데이터가 없다.
현재 생성기는 인자마다 고정된 `PossibleValuesParser` 후보를 넣을 뿐, 이런 문맥별 분기를
생성하지 않는다. 생성 시점 스키마로 셸별 분기를 만들거나 메타데이터를 합성하는 방식도
가능하므로, 정적 스크립트로 원리적으로 불가능하다는 뜻은 아니다.

| 자리 | 무엇에 의존하는가 |
|---|---|
| `-a <name>=` 의 이름 | 앞선 positional(`tool_id`)이 정해지기 전에는 필드 이름을 알 수 없다 |
| `-a name=<값>` 의 `options` 선택지 | 위와 같고, 값 집합은 필드마다 다르다 |
| `--field <ID>` | 출력 필드 id 역시 대상 Tool에 따라 달라진다 |
| `upeg board <board> ...` 이후 | `BoardAction::Scoped`가 두 번째 `external_subcommand`이므로 clap 메타데이터에 보이지 않는다 |

`-a`를 받는 자리는 셋이다: `call`, `board <b> call`, `trigger fire`. 동적 라우트는 `-a`를
받지 않으므로 이 항목과 무관하다.

## 콜백 방식으로 가려면

`clap_complete`에는 셸이 매 입력마다 바이너리에 후보를 되묻는 기계장치가 이미 들어 있다
(`CompleteEnv`, `ArgValueCandidates`). 이 콜백은 앞선 인자에 따라 후보를 고르거나 최신
사용자 상태를 반영하는 한 가지 방법이다. 첫째 표는 생성 시점 스냅샷만으로도 채울 수 있다.

대가는 자동완성 한 번이 프로세스 기동 한 번이 되는 것이고, 그래서 기동 비용이 곧 응답
지연이 된다. `main`은 `run()`에 들어가기 전에 `load_local_runtime_sources`를 무조건
지불하며 여기에는 프로젝트 매니페스트 탐색과 `~/.upeg/wasm/*.wasm` 로드가 포함된다.
`-a`의 필드 이름과 `options` 선택지, `--field`의 출력 필드 id는 그 로드가 끝난 뒤라면
메모리에서 바로 열거되므로 추가 비용이 없지만, 사용자 pin 기반 board는 pegboard 상태를
읽으므로 SQLite 열기가 한 번 더 필요하다.

# 출력 표현

`call`과 동적 라우트가 같은 플래그를 공유한다.

| 플래그 | 출력 |
|---|---|
| (없음) | primary 출력 값 |
| `--json` | 정본 성공/에러 JSON 봉투 |
| `--field <ID>` | 출력 필드 하나의 값 |
| `--pretty` | 라벨이 붙은 출력 행 전체 |
| `--out <PATH>` | `File` 출력의 기록 위치. `--force` 없이는 기존 파일을 덮어쓰지 않는다 |
| `--local` | 호스트가 떠 있어도 in-process로 dispatch (discovery 자동 attach 생략) |

## 실행 중 출력은 stdout에 섞이지 않는다

봉투는 그대로다 — 실행 중 출력은 봉투에 필드를 추가하지 않는다. 대신 **어디로 나가느냐**가
출력 모드에 따라 갈린다.

| 모드 | 실행 중 |
|---|---|
| (없음) · `--pretty` | 자식의 stdout·stderr를 접두사 없이 **터미널의 stderr**로 그대로 비춘다 |
| `--json` · `--field` | 아무것도 내보내지 않는다 |

기준은 "장황함"이 아니라 **기계 대 사람**이다. `--json`과 `--field`는 무언가에 파싱되려고
있는 모드이고, 그 무언가는 진행 텍스트를 둘 데가 없다.

그래서 사람용 모드에서는 성공한 명령의 stdout이 **두 번** 보인다 — 도는 동안 stderr로
한 번, 끝난 뒤 결과로 stdout에 한 번. stdout을 결과의 단일 표현으로 남겨 두는 값이 그
중복보다 크다고 판단했고, `2>/dev/null`이면 사라진다.

TTY 여부로 켜고 끄지 않는다. 진행 출력을 파일이나 로그 수집기로 파이프하는 것은 정당한
요구이고, isatty 검사는 이 동작을 `tee` 아래에서 예측 불가능하게 만든다.

호스트에 attach된 호출은 예외다: 호스트가 최종 봉투 하나를 HTTP로 돌려주므로 비출 것이
없다. 그 경우 라이브 출력이 필요하면 호스트의
[HTTP 스트리밍 경로](/architecture/http-api.md)를 직접 쓴다.
