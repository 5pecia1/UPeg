---
type: Manifest Contract
title: 매니페스트 계약
description: Toolkit TOML의 구조, invoker별 필수 필드, credential 참조 규칙.
tags: [architecture, manifest, toml]
status: stable
sources:
  - id: loader-model
    resource: ../../upeg-loader/src/model.rs
    title: Toolkit TOML 타입 정의
---

# 형태

매니페스트 하나는 호출 불가능한 Toolkit 하나와 호출 가능한 Tool 하나 이상을 정의한다.

```toml
id = "num"
tags = ["pure", "numeric"]
description = "Numeric base conversions"

[[tools]]
id = "hex_to_decimal"      # local id; full id는 num.hex_to_decimal
tags = ["hex", "number"]
invoker = "External"
pin = "Inline"
pegboard_units = "U1"
surfaces = ["cli", "tui", "desktop", "pwa", "ext", "mcp", "http"]
boards = ["dev"]

[[tools.inputs]]
name = "input"
type = "string"
required = true

command = "hex-to-decimal"
```

# 규칙

- `[[tools]].id`는 Toolkit 내 지역 id다. `{toolkit}.` prefix 중복은 거부된다.
- `pegboard_units`는 필수이며 모든 pegboard surface가 공유한다: `U1`, `U2`, `U2T`.
- 입력/출력은 닫힌 타입 집합만 쓴다 — [I/O 타입 시스템](/architecture/io-types.md) 참조.
- `category`는 거부된다. Tag로 대체한다.
- Static Rust `#[tool]`은 `Function`을 지원한다. 런타임 TOML/WASM/MCP 매니페스트는
  `External`, `Http`, `Embed`, `Chain`, `Llm`, `Wasm` 런타임 어댑터를 쓴다.

# Invoker별 필수 필드

| Invoker | 필수 필드 | 비밀 규칙 |
|---|---|---|
| `External` | `command`, 선택 `args_template`/`cwd`/`env`/`timeout_ms`/`color` | 프로세스 spawn 시점에만 환경으로 credential 값이 전달된다. |
| `Http` | `url`, 선택 `method`/`headers`/`body` | 헤더·본문 템플릿은 credential *이름*만 참조한다. 내장 경량 어댑터는 `http://`와 테스트용 `mock://echo`를 지원하며, TLS가 필요한 경우 `External` 래퍼를 쓴다. |
| `Embed` | `embed_url`, `controlled_embed.bindings` | selector 매핑은 사용자가 확인한 값이다. |
| `Chain` | `steps` | step args는 `{{steps.<id>.output}}` 표현식을 쓸 수 있다. |
| `Llm` | `prompt`, 선택 `provider`/`model`/`credential` | API 키는 credential 이름으로만 해석된다. `provider = "echo"`가 오프라인 기본값이고, `provider = "tool:<id>"`는 설정된 provider Tool에 위임한다. 알 수 없는 provider는 조용히 mock하지 않고 명시적으로 실패한다. |
| `Wasm` | `wasm_path` 또는 로드된 WASM Toolkit 선언 | 호스트가 등록 전에 export된 매니페스트를 검증한다. |

# External 실행 계약

`External`은 `command`만이 아니라 **자식 프로세스가 어떻게 도는지**까지 선언한다.

```toml
[[tools]]
id = "cargo_check"
pegboard_units = "U2"
invoker = "External"
command = "cargo"
args_template = ["check", "--manifest-path={manifest_path}", "--quiet"]
cwd = "."             # 매니페스트 디렉터리 기준 상대 경로
timeout_ms = 600000   # 없으면 무제한

[[tools.env]]
name = "RUST_LOG"
value = "warn"

[[tools.inputs]]
name = "manifest_path"
type = "file_path"
default = "Cargo.toml"
```

| 필드 | 의미 |
|---|---|
| `cwd` | 자식이 실행될 디렉터리. 상대 경로는 **이 매니페스트 파일이 있는 디렉터리** 기준으로 해석된다. |
| `env` | 평문(비밀 아님) 환경변수. `credentials`가 나중에 적용되므로 이름이 겹치면 credential이 이긴다. |
| `timeout_ms` | wall-clock 예산. 초과하면 자식의 **process group 전체**를 종료하고 `details.timed_out = true`로 실패한다. **기본값 없음** — 20분짜리 `just verify`도 합법이어야 한다. |
| `color` | `"inherit"`(기본) 또는 `"force"`. 자식에게 색 지원 여부를 알린다 — 아래 참조. |

작업 디렉터리는 매니페스트 출처에 따라 다르게 정해진다.

선언된 `cwd`가 있으면 언제나 그것이 이긴다. 없을 때:

| 출처 | 규칙 |
|---|---|
| Toolkit 디렉터리(`~/.upeg/toolkits/*.toml`) | 호출자가 보낸 `_upeg.cwd` → 없으면 upeg 프로세스 자신의 cwd. 소속된 프로젝트가 없으므로 호출자가 어디든 지정할 수 있다. |
| Project Manifest(`upeg.toml`) | **그 매니페스트의 디렉터리**가 기본값이자 경계다. 호출자의 `_upeg.cwd`가 매니페스트 디렉터리 **안**(자기 자신 포함)이면 그 값을 쓰고, 밖이면 **무시하고** 매니페스트 디렉터리를 쓴다. |

`_upeg.cwd`는 **절대 경로**여야 하고 실제 디렉터리여야 한다. 아니면 `invalid_args`로
거부된다 — 상대 경로는 호출자의 "현재 위치"를 upeg이 복원할 수 없다.

프로젝트 밖을 가리키는 값은 거부가 아니라 **무시**다. 서피스는 사용자의 셸 디렉터리를
주변 컨텍스트로 함께 보내므로, 거부로 처리하면 "마침 `/tmp`에 있었다"가 하드 실패가
된다. 포함 관계는 양쪽을 canonicalize한 뒤에 판정하므로 `..`나 심링크로 경계를
넘을 수 없다.

stdin은 항상 `/dev/null`이다. 상속된 stdin은 `cat`, `git` credential 프롬프트,
`bash -l`을 영원히 멈추게 하고, dispatch는 셸만이 아니라 daemon·MCP·GUI 핀에서도
들어오므로 건네줄 터미널 자체가 없다.

## `color` — 자식에게 색을 허용한다

upeg은 두 스트림을 파이프로 캡처하므로 자식은 **non-TTY**를 본다. 예의 바른 CLI는
거기서 색을 끄고, `gh` 같은 도구는 사람용 서식 자체를 축약형으로 바꾼다. 그게 기본값이고
의도된 것이다 — 캡처된 출력이 결정론적이고 escape sequence가 섞이지 않는다.

`color = "force"`는 tool 단위 opt-out이다.

```toml
[[tools]]
id = "gh_pr_list"
invoker = "External"
command = "gh"
args_template = ["pr", "list"]
color = "force"
```

순수하게 **환경변수**로만 동작한다. pty를 열지 않으므로 Unix와 Windows에서 똑같이
동작하고, 캡처·타임아웃·process group 종료 machinery는 한 줄도 달라지지 않는다.

| 변수 | 값 | 조건 |
|---|---|---|
| `CLICOLOR_FORCE` | `1` | 항상 |
| `FORCE_COLOR` | `1` | 항상 |
| `NO_COLOR` | **제거** | 항상. [no-color.org](https://no-color.org) 규약은 값이 무엇이든(빈 문자열 포함) 색을 끄라는 뜻이며 위 둘보다 우선한다 — 상속된 채로 두면 `force` 자체가 무의미해진다 |
| `TERM` | `xterm-256color` | **upeg 자신의 환경에 `TERM`이 없을 때만.** 진짜 터미널의 `TERM`이 어떤 추측보다 정확하다 |

넷은 *기본값*으로 적용된다 — 선언된 `env` 항목이 나중에 적용되므로
`env = [{ name = "FORCE_COLOR", value = "0" }]`가 정책을 이기고,
`env = [{ name = "NO_COLOR", value = "1" }]`은 제거를 되돌린다.

`force`는 **규약이지 터미널이 아니다.** `CLICOLOR_FORCE`/`FORCE_COLOR`를 읽는 CLI
(`cargo`, `gh`, chalk 기반 도구 전부)에만 통한다. `isatty(3)`만 보고 판단하는
`git`·`ls`·`grep` 같은 프로그램은 이 변수들을 아예 읽지 않으므로 자기 플래그가 필요하다 —
`git -c color.ui=always`, `ls --color=always`. 두 가지를 함께 쓰는 예가
`examples/tools/dev-external-demo.toml`의 `dev.git_log`다.

알 수 없는 값(`color = "always"`)은 dispatch 때 조용히 무시되지 않고 **로드 시점에**
허용값을 알려주며 거부된다.

## `pty` — 자식에게 진짜 터미널을 준다

`color = "force"`는 규약을 읽는 프로그램에만 통한다. `isatty(3)`만 보고 판단하는
쪽에는 **터미널 자체**가 필요하다. 그것이 `pty = true`다.

```toml
[[tools]]
id = "git_log"
invoker = "External"
command = "git"
args_template = ["log", "--oneline", "-n", "10"]
pty = true
```

pseudoterminal을 열어 자식의 **stdout과 stderr**를 그 터미널에 연결한다. 자식에게
`isatty(1)`과 `isatty(2)`는 참이 되고, `git`·`ls`·`grep`처럼 플래그 없이는 색을
내지 않던 도구가 자기 판단으로 터미널 출력을 낸다.

계약:

| 항목 | 내용 |
|---|---|
| **두 스트림이 합쳐진다** | 터미널의 버퍼는 하나다. 자식이 쓴 순서 그대로 섞여서 `stdout` 하나에 담기고, `stderr`는 **빈 문자열**이 된다. 진행 출력([HTTP 스트리밍](/architecture/http-api.md)·MCP 알림)도 전부 `stdout`으로 나간다 |
| **`color = "force"`를 포함한다** | 진짜 터미널을 요구한 매니페스트는 환경변수를 읽는 프로그램에서도 색을 원한다. `CLICOLOR_FORCE`/`FORCE_COLOR`가 함께 세워진다. **함의이지 덮어쓰기가 아니다** — `color`를 직접 적으면 그 선언이 이기고(`color = "inherit"`이면 터미널만 주고 환경변수는 건드리지 않는다), 선언된 `env`는 여전히 그 위에 얹힌다 |
| **stdin은 여전히 `/dev/null`이다** | 터미널의 반대편에 사람이 없다. tty stdin을 주면 credential 프롬프트 하나가 dispatch를 영원히 멈춘다. `isatty(0)`은 거짓이다 |
| **줄바꿈은 그대로다** | 터미널의 출력 후처리(`OPOST`)를 끄므로 `\n`이 `\r\n`으로 바뀌지 않는다. 캡처된 바이트는 프로그램이 쓴 바이트다 |
| **타임아웃·containment는 그대로다** | `timeout_ms`는 똑같이 process group을 종료하고, process group을 벗어난 후손도 똑같이 회수된다 — Linux의 탈출 프로세스 정리는 pipe inode 대신 **`/dev/pts/N` 장치**로 보유자를 찾는다 |

한계:

- **Unix 전용이다.** pty가 없는 호스트(Windows·wasm)에서는 그 도구 **하나만 로드
  시점에 건너뛴다** — 사유(`pty = true` needs a host that can open a pseudoterminal…)를
  함께 기록하며, 조용히 파이프로 내려앉지 않는다. 같은 매니페스트가 기계마다 다른 뜻이
  되면 안 되고, 그 차이(`isatty`가 거짓)가 바로 이 필드를 선언한 이유이기 때문이다.
  건너뛰는 것은 그 도구뿐이다: 같은 파일의 나머지 도구는 pty와 아무 상관이 없으므로
  그대로 로드된다. Windows는 ConPTY가 필요하며 아직 구현하지 않았다.
- **제어 터미널(controlling terminal)은 아니다.** 자식은 자기 process group에 있고
  이 pts를 controlling terminal로 획득하지 않는다. `/dev/tty`를 직접 여는 프로그램
  (`ssh`의 비밀번호 프롬프트, `sudo`)은 여전히 실패한다.
- **창 크기가 없다.** `TIOCGWINSZ`는 커널 기본값(0×0 또는 24×80)을 돌려준다. upeg에는
  전달할 진짜 창이 없다.
- **stderr를 따로 봐야 하는 도구에는 쓰지 않는다.** 합쳐진 스트림은 되돌릴 수 없다.
  색만 필요하면 `color = "force"`가, 도구 자신의 플래그가 있으면 그 플래그가 더 싸다.

## 취소

실행 중인 자식은 **취소될 수 있다.** 호출자가 취소를 설치한 채로 dispatch하면
(`upeg_runtime::with_cancellation`) External 인보커는 대기 루프의 매 tick마다 그것을
읽고, 취소가 서면 process group을 종료한 뒤 취소 봉투로 답한다.

| 필드 | 값 |
|---|---|
| `error.code` | `cancelled` — 도구가 실패한 것이 아니므로 `tool_error`와 구분된다 |
| `error.details.cancelled` | `true` |
| `error.details.exit_code` | `null` — 자식은 종료 코드를 보고할 기회가 없었다 |
| `error.details.stdout` / `stderr` | 죽기 전까지 쓴 것 |

오늘 취소를 설치하는 서피스는 셋이다.

| 서피스 | 취소를 거는 동작 |
|---|---|
| [HTTP 스트리밍 경로](/architecture/http-api.md) | 소비자가 연결을 끊으면 응답 본문이 drop되고, 그 drop이 취소다 |
| TUI | 실행 중 `Esc` |
| FRB(Flutter) | Dart 쪽에서 `cancel_dispatch(run_id)` |

서피스별 실행 중 표시와 취소 조작은 [UI/UX 서피스 계약](/ui-ux-surface-contract.md)의
"살아 있는 출력" 취소 표가 정본이다. **취소는 요청이지 보장이 아니다**: 토큰을 끝내
보지 않는 도구는 끝까지 달리고, 그래도 최종 봉투 하나로 끝난다. Chain은 dispatch
스레드에서 step을 돌리므로 설치된 취소를 그대로 물려받는다.

## `args_template` 토큰

토큰 하나는 인자 하나가 된다. 토큰은 작은 템플릿이다.

- `{key}`는 토큰 **어디에서나** 치환된다 — `--manifest-path={path}`, `-p{crate}`.
- `{{`와 `}}`는 리터럴 중괄호다.
- `{key}`의 `key`는 **반드시 선언된 `inputs` 필드 이름**이어야 한다. 아니면 로드
  시점에 거부된다 — 오타 하나가 조용히 빈 인자로 렌더되면 안 된다. 예외는 `input`
  하나다: `Chain` 인보커가 모든 step에 `{"input": <상류 출력>}`을 건네므로,
  chain 노드로 쓰이는 도구는 선언 없이도 `{input}`을 읽을 수 있다.
- 셸 한 줄짜리 명령의 `${VAR:-기본값}`도 중괄호를 쓴다. 셸에 그대로 넘기려면
  `${{VAR:-기본값}}`으로 이스케이프한다 — 이스케이프하지 않으면 자리표시자로
  해석되어 로드가 거부된다.
- 값이 없으면 입력의 `default`를 쓴다.
- 값도 `default`도 없을 때 토큰이 사라지는 경우는 **딱 하나**다: 토큰이 리터럴
  없이 `{key}` 하나뿐이고, 그 입력이 `required = false`일 때. 그때만 인자 목록에서
  통째로 빠진다.
- 그 밖에는 빈 문자열로 치환되고 토큰은 **자리를 지킨다**. `["rm", "-rf", "{dir}/build"]`가
  `["rm", "-rf"]`로 줄어들어 뒤따르는 인자가 밀리는 일은 없다.

## 실패 봉투

실패는 canonical `ToolResult` 실패 봉투로 나온다. `External`은 `error.details`를 채운다.

```json
{
  "ok": false,
  "error": {
    "code": "tool_error",
    "message": "`cargo` exited with code 1",
    "details": { "exit_code": 1, "stdout": "…", "stderr": "…" }
  }
}
```

- 시그널로 죽으면 `exit_code`가 `null`이고 `signal`이 붙는다.
- `timeout_ms`가 만료되면 `exit_code`가 `null`이고 `timed_out: true`가 붙는다.
- `cargo fmt --check`, clippy, `cargo test`, `flutter analyze`, `gh pr checks`는
  진단을 **stdout**에 쓴다. 두 스트림 모두 보존하는 이유가 이것이다.
- 두 스트림은 진단 예산으로 잘린다. 봉투가 커져도 CLI `--json`, HTTP 본문,
  MCP `structuredContent`, FRB 브리지가 감당할 수 있는 크기를 넘지 않는다.

성공했을 때 stdout이 primary 출력이다. 도구가 `outputs`를 선언하지 않았고 명령이
stderr에도 썼다면, 그 텍스트는 보조 출력 `stderr`로 남는다 — 성공한 실행의 진행
로그를 조용히 버리지 않는다.

## 실행 중 출력(스트리밍)

10분짜리 명령이 끝날 때까지 화면이 비어 있지 않도록, `External`은 **실행 중에도**
읽어들인 출력을 내보낸다. 이것은 최종 봉투에 **추가**되는 선택적 통로다 — 소비할 수
없는 surface는 예전과 완전히 똑같이 최종 봉투 하나만 받고, 캡처 한도·타임아웃·
process group 종료는 전혀 달라지지 않는다.

- 소비자가 없으면 invoker는 전달 작업 자체를 하지 않는다 (비용 0).
- 청크 경계는 **줄**이다. `\n`으로 끊고, `\n`이 하나도 없으면 진행바가 쓰는 `\r`로
  끊는다. 종결자 없이 64 KiB가 쌓이면 그대로 내보낸다.
- 마지막에 남은 미완성 꼬리는 스트림이 끝날 때 반드시 flush된다 — 시간 초과로
  죽은 자식이 그때까지 쓴 것도 포함해서.
- `seq`는 stdout/stderr **양쪽에 걸쳐** 0부터 끊김 없이 증가하므로 소비자가 전체 순서를
  복원할 수 있다. 순번은 **소비자가 sink를 설치한 범위**, 곧 한 번의 호출에 속한다.
- `Chain` step은 같은 스레드에서 돌기 때문에 각 step의 진행 출력이 그대로 흘러나오고,
  step이 여러 개여도 순번은 하나로 이어진다 — step마다 0으로 되돌아가지 않는다.
- `pty = true`인 도구는 청크도 전부 `stdout`으로 나간다. 터미널의 버퍼가 하나이므로
  나눌 stderr가 애초에 없다.

surface별 소비 방식:

| Surface | 실행 중 출력 |
|---|---|
| CLI (`upeg call`, 동적 라우트, `board <b> call`, `trigger fire`) | 자식의 stdout·stderr를 **터미널의 stderr**로 그대로 비춘다. stdout은 최종 결과 전용으로 남는다. `--json`/`--field`에서는 아무것도 내보내지 않는다 |
| HTTP | [`POST /v1/tools/{id}/stream`](/architecture/http-api.md) — `application/x-ndjson` |
| MCP | `tools/call` 도중 `notifications/message` 로그 프레임 ([MCP 계약](/architecture/mcp.md)) |
| TUI · Flutter · Chrome 확장 | 아직 없다. 최종 봉투만 받는다 |
| attach(호스트에 붙은 CLI) | 아직 없다. 호스트의 최종 봉투를 그대로 받는다 |

# Credential 참조

```toml
credentials = [
  { name = "openai", type = "api_key", store = "keychain", keychain_service = "upeg", keychain_account = "openai" },
  { name = "etherscan", type = "api_key", store = "env", env = "ETHERSCAN_API_KEY" },
]
```

항목은 실행 시점에 비밀을 **어디서 해석할 수 있는지**만 지정한다. `credentials[].value`,
`credentials[].secret_value`, 인라인 리터럴 비밀은 모두 무효다. 비밀 바이트는 Toolkit
매니페스트, 프로젝트 매니페스트, 실행 로그, HTTP/MCP 목록 어디에도 포함되지 않는다.

# 필드 레퍼런스와 검증

전체 필드 표는 손으로 쓰지 않는다 — [생성된 매니페스트 가이드](/TOOL_MANIFEST.md)가 Rust
타입에서 파생된 정본이다. 편집기/CI용 JSON Schema는 `fixtures/toolkit.schema.json`이며
같은 명령으로 함께 생성된다.

```bash
upeg tool validate ~/.upeg/toolkits/demo.toml   # 한 파일 의미 검증
upeg toolkit validate                           # 디렉터리 전체
just toolkit-schema                             # 스키마 + 가이드 재생성
just toolkit-schema-check                       # 생성물 drift 확인
```

JSON Schema는 TOML→JSON 형태만 증명한다. chain 비순환성, credential 존재, 실행 파일
가용성, URL 도달성은 `upeg tool validate`가 정본이다.
