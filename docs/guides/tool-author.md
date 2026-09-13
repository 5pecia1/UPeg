---
type: Guide
title: 도구 작성자 가이드
description: "`#[tool(...)]` 매크로로 새 Tool을 정의할 때 쓸 수 있는 키와 변형, 그리고 검증 절차."
tags: [guide, tools, macros, authoring]
status: stable
sources:
  - id: tool-macro
    resource: ../../upeg-macros/src/lib.rs
    title: "`#[upeg::tool]` 매크로 구현"
  - id: greet-plugin
    resource: ../../examples/plugins/greet
    title: WASM 게스트 플러그인 예제
---

# 빠른 시작

`inputs`와 `outputs`만 선언하면 모든 surface에서 자동으로 동작한다. 별도 UI 코드 없이
페그보드에 자동 렌더링된다.

```rust
use upeg_core::tool;

#[tool(
    id = "demo.upper",
    toolkit = "demo",
    description = "ASCII 문자열을 대문자로 변환",
    inputs = [
        required input: String = "원본 텍스트",
    ],
    outputs = [
        result: String = "대문자 결과",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn upper(input: &str) -> String {
    input.to_uppercase()
}
```

# Source — 도구가 시작되는 방식

`source` 키는 GUI에서 도구가 **어떻게 시작되는지**를 선언한다. 비-GUI surface는 이를 무시하고
함수를 직접 호출한다.

| 변형 | 의미 | 예시 |
|---|---|---|
| `UserInput` (기본) | 사용자가 폼에 입력 후 실행 | 생략 가능 |
| `Manual` | 버튼 클릭 트리거 | 입력 없이 한 번에 동작하는 UUID 생성 |
| `Timer("30s")` | 주기적 자동 실행 | 라이브 시계, 네트워크 상태 |
| `Shortcut("⌘⇧N")` | 키보드 단축키 | 메모 생성 같은 액션 도구 |
| `Static` | 시작 없음, 정적 출력 | View Embed |

지속시간 접미사: `ms`, `s`, `m`, `h`.

# 입력과 출력

타입 집합과 인라인 제약은 [I/O 타입 시스템](/architecture/io-types.md)이 정본이다.

```rust
inputs = [
    required hex:     String                                     = "Hex 값",
    required base:    Options(["hex", "dec", "bin"])             = "출력 진수",
    required port:    Number(min=1, max=65535, default=8080)     = "포트 번호",
    optional pattern: String(regex="^[a-z]+$", placeholder="abc") = "패턴",
    optional flags:   MultiOptions(["i","m","s","x"])            = "정규식 플래그",
],
outputs = [
    result: Number = "10진수",
    log:    String = "디버그 로그",
],
```

`outputs` 문법은 `inputs`와 대칭이되 `required`/`optional` 키워드가 없다. 출력 전용 변형으로
`EmbeddedView("https://...")`가 있다.

# 임베드 두 모드

## View Embed (Passive)

페그보드에 외부 웹사이트를 그대로 표시한다. upeg은 입출력에 관여하지 않는다.

```rust
#[tool(
    id = "embed.mdn",
    toolkit = "embed",
    source = Static,
    outputs = [
        view: EmbeddedView("https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference"),
    ],
    pin = Embed,
    pegboard_units = U2,
    invoker = Static,
    surfaces = [desktop, pwa, ext],
)]
pub fn mdn() {}
```

## Controlled Embed (조종)

외부 웹사이트를 도구 엔진으로 쓴다. upeg이 CSS selector로 DOM을 조작하고, 사용자에게는
평범한 폼과 결과를 보여준다.

```rust
#[tool(
    id = "embed.json_to_ts",
    toolkit = "embed",
    description = "transform.tools 백엔드 JSON→TS 변환기",
    inputs  = [ required json: String = "JSON 입력" ],
    outputs = [ ts: String = "TypeScript 출력" ],
    pin = ControlledEmbed,
    pegboard_units = U2,
    invoker = Embed,
    surfaces = [desktop, pwa, ext],
)]
pub fn json_to_ts() {}
```

GUI에서는 `webview_flutter`(desktop) 또는 브라우저 자체(PWA/ext)가 실행한다. 비-GUI
surface는 시스템에 설치된 Chrome/Chromium/Edge를 headless로 쓴다.

`Invoker::Embed`는 `PinKind::ControlledEmbed`와만, `Invoker::Static`은 `PinKind::Embed`와만
짝을 이룬다 — [Lexicon의 헷갈리는 쌍](/LEXICON.md) 참조.

## 셀렉터 강건성

- **id 우선**: `#input`, `#output` 같은 id 셀렉터가 가장 안정적이다.
- **데이터 속성 차순**: `[data-testid="input"]`. SPA에서도 잘 유지된다.
- **클래스/구조 최후**: `.col-md-6 > textarea`는 사이트 리뉴얼에 쉽게 깨진다.
- desktop webview와 headless Chrome 양쪽에서 테스트한다.

# 선언적 TOML 도구

TOML 매니페스트 로더도 `inputs`와 `outputs`를 받는다. 매크로의 `source`와 인라인 제약 문법은
아직 별도이며, TOML에서는 `invoker` / `embed_url` / `controlled_embed.bindings`로 런타임
동작을 선언한다. 전체 필드는 [매니페스트 계약](/architecture/manifest.md)과
[생성된 가이드](/TOOL_MANIFEST.md) 참조.

## 로컬 명령을 도구로 감쌀 때

`invoker = "External"`은 자식 프로세스가 **어디서, 무슨 환경으로, 얼마나 오래** 도는지까지
선언한다. 복사해서 쓸 수 있는 전체 예제는 `examples/tools/dev-external-demo.toml`에 있다.

```toml
[[tools]]
id = "git_log"
pegboard_units = "U1"
invoker = "External"
command = "git"
args_template = ["log", "--oneline", "-n", "{count}"]
cwd = "."             # 매니페스트 디렉터리 기준. 생략하면 Project Manifest
                      # 도구는 그 매니페스트 디렉터리에서 돈다
timeout_ms = 10000    # 없으면 무제한
env = [{ name = "GIT_PAGER", value = "cat" }]

[[tools.inputs]]
name = "count"
type = "integer"
default = 10          # 호출자가 생략하면 이 값이 치환된다
```

작성할 때 걸리기 쉬운 지점:

- **선택 입력에는 `default`를 준다.** 값도 `default`도 없는 **선택** 입력의 토큰이
  리터럴 없이 `{count}` 하나뿐이면 그 토큰은 인자 목록에서 통째로 빠지므로,
  `-n {count}`는 `-n`만 남아 깨진다. `["-n{count}"]`처럼 한 토큰으로 붙여 쓰면 함께
  사라져 안전하다. 리터럴이 섞인 토큰(`{dir}/build`)과 필수 입력의 토큰은 빈
  문자열로 치환되고 **자리를 지킨다**.
- **`{key}`는 토큰 어디에나 넣을 수 있다** — `--manifest-path={path}`, `-p{crate}`.
  리터럴 중괄호는 `{{`, `}}`.
- **`{key}`는 선언된 `inputs` 이름이어야 한다.** 아니면 로드가 거부된다(chain step이
  받는 `{input}`만 예외). 셸 한 줄 명령의 `${VAR:-기본값}`도 `${{VAR:-기본값}}`으로
  이스케이프해야 셸까지 그대로 간다.
- **stdin은 항상 `/dev/null`이다.** 입력을 기다리는 명령(`cat`, 인증 프롬프트가 뜨는
  `git`)은 멈추지 않고 즉시 끝난다. 입력이 필요하면 `args_template`으로 넘긴다.
- **긴 작업에 `timeout_ms`를 걸지 않아도 된다.** 기본값이 없으므로 빌드·테스트처럼
  오래 도는 명령이 그대로 완주한다. 거는 순간 초과 시 process group 전체가 종료된다.
- **비밀은 `env`가 아니라 `credentials`로.** `env`는 평문 전용이고, credential이 나중에
  적용되므로 이름이 겹치면 credential이 이긴다.

명령이 0이 아닌 코드로 끝나면 실패 봉투의 `error.details`에 `exit_code`·`stdout`·`stderr`가
그대로 담긴다 — `cargo`/clippy/`flutter analyze`는 진단을 stdout에 쓰기 때문에 이게
결정적이다. 봉투 모양은 [매니페스트 계약](/architecture/manifest.md) 참조.

```bash
upeg call dev.cargo_check              # 사람용: 메시지 + stderr + stdout
upeg call dev.cargo_check --json       # 기계용: details까지 담긴 canonical 봉투
```

# Surface 노출 제어

```rust
surfaces = [desktop, pwa, ext],   // GUI 전용
surfaces = [cli, mcp, http],      // 비-GUI 전용
// 생략하면 전체 (cli, tui, desktop, pwa, ext, mcp, http)
```

`EmbeddedView`처럼 GUI에서만 의미 있는 출력을 가진 도구는 명시적으로 GUI surface로 제한한다.
선언한 surface에서 실제로 실행 가능한지는 capability 계약이 감사한다
([Surface 계약](/ui-ux-surface-contract.md)).

# Bespoke 렌더러 — 최후의 수단

자동 렌더링으로 표현할 수 없는 시각화(오디오 파형, 3D 뷰어 등)는
`flutter_app/lib/src/widgets/pin_renderers/registry.dart`의 bespoke renderer 맵에 등록한다.
bespoke 렌더러는 surface 다양성을 해치므로 자동 렌더링으로 충분한지 먼저 확인한다.

Expanded modal의 bespoke 폼 자격 규칙은 별도다: **입력하는 대로 출력이 갱신되는 라이브
프리뷰가 필요한 도구만** 자격이 있다. 실행 단계가 따로 있으면 라이브 프리뷰가 아니다.
지금 자격이 있는 도구는 `num.hex_to_decimal` 하나뿐이고, 나머지는 전부 generic 폼을 쓴다.

bespoke 폼의 사유가 **되지 않는** 것들 — 전부 호스트가 generic 폼 주위에 이미 제공한다:

- 원클릭 재생성: `F1` 실행과 `F2` 복사는 모든 도구에 바인딩되어 있다.
- 결과 복사 버튼: 모든 결과 블록이 이미 달고 있다.
- 입력이 없는 도구의 실행 버튼: modal의 primary 버튼과 인라인 핀 본문의 Run 버튼이
  "입력 없음" 안내판 대신 실행 어포던스를 준다.

자세한 규칙은 [Surface 계약](/ui-ux-surface-contract.md)에 있다.

# 검증 체크리스트

- [ ] `cargo test --workspace` 통과
- [ ] `just verify` 통과 (fmt, file-size, clippy, lexicon, baseline)
- [ ] 새 도구가 `upeg tool list`에 표시된다
- [ ] `upeg call <id> -a key=value`가 동작한다
- [ ] desktop 페그보드에서 자동 렌더링을 확인했다
- [ ] (Controlled Embed인 경우) headless 모드에서도 동작한다
