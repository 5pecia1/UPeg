---
type: Contract
title: 프로젝트 매니페스트
description: "`upeg.toml` 자동 탐지, `$HOME` 경계, `UPEG_PROJECT_MANIFEST_PATH` override, toolbox 병합 우선순위, 프로젝트 board 선언과 지속성, Board 실행 컨텍스트 주입."
tags: [architecture, manifest, project, security]
status: stable
---

# 계약

`upeg.toml`은 현재 디렉터리(cwd)에서 위로, `$HOME`을 넘지 않는 범위까지 탐지되는 Toolkit
매니페스트다. 가장 가까운 매니페스트가 CLI dispatch 전에 toolbox에 로드되고, board-aware한
CLI/HTTP 호출에는 Board 실행 컨텍스트로 주입된다.

# 탐지 범위 — `$HOME` 경계 (보안)

프로젝트 매니페스트는 `invoker = "External"` Tool로 임의 명령을 실행할 수 있고, 동의 절차
없이 자동 로드된다. 따라서 상위 디렉터리 탐색은 **`$HOME` 밖으로 절대 나가지 않는다**
(docs/product/security-absolutes.md 참조):

1. cwd 자신은 `$HOME` 안팎에 상관없이 항상 확인한다.
2. cwd가 `$HOME` 안에 있으면, `$HOME`을 포함해 그 안에 머무는 조상 디렉터리까지만 위로 걷는다.
3. cwd가 `$HOME` 밖에 있으면(또는 `$HOME`이 아예 없으면), 조상 디렉터리는 걷지 않는다 — cwd
   자신을 확인한 다음, `$HOME`이 있으면 그 디렉터리 하나만 폴백으로 확인한다.
4. `$HOME`이 없으면(`HOME` 환경변수 미설정) cwd만 확인한다.

예: `$HOME`이 `/home/user`일 때 `/tmp/x`에 심어진 `upeg.toml`은 `cd /tmp/x/anything`으로도
자동 로드되지 않는다 — world-writable한 상위 디렉터리의 매니페스트로 임의 명령을 끌어들이는
경로를 차단한다.

# `UPEG_PROJECT_MANIFEST_PATH` — 탐지 override

입력 환경변수 하나로 탐지 동작을 바꿀 수 있다.

| 값 | 동작 |
|---|---|
| 미설정 또는 빈 문자열 | `Detect` — 위 탐지 규칙 그대로 |
| `off` (대소문자 무관) | `Disabled` — 매니페스트를 절대 로드하지 않는다 |
| 절대 경로 | `Explicit` — 탐지를 건너뛰고 그 경로만 쓴다. 파일이 없으면 "매니페스트 없음"이며 탐지로 폴백하지 않는다 |
| 상대 경로 | 잘못된 값으로 취급해 stderr에 한 줄 경고를 남기고 `Detect`로 폴백한다 |

> **`UPEG_PROJECT_MANIFEST`(`_PATH` 없음)는 이 변수가 아니다.** 그건 upeg이 `External` Tool
> 자식 프로세스에 주입하는 **출력** 변수([호출 봉투](/architecture/call-envelope.md) 참조)이고,
> 여기 설명하는 `UPEG_PROJECT_MANIFEST_PATH`는 upeg 프로세스 자신이 읽는 **입력** 변수다.
> 서로 다른 변수이며 절대 혼용하지 않는다.

# 동의 고지

"upeg이 지금 어느 파일을 신뢰하는가"(동의 고지)와 "그 파일 로드는 어떻게 됐는가"(집계)는
**같은 파일**에 대한 이야기다. 그래서 두 줄이 아니라 **한 줄**로 합쳐, 프로세스당 한 번
stderr에 출력한다 (`upeg-cli/src/main.rs`의 `project_manifest_summary_line`).

| origin | 선언된 Tool | 출력 |
|---|---|---|
| `Detected` (탐지) | 있음 | `upeg: loaded project manifest <path> (N tool(s), M failed)` |
| `Detected` (탐지) | 없음 | `upeg: loaded project manifest <path>` |
| `EnvOverride` (`UPEG_PROJECT_MANIFEST_PATH`) | 있음 | `upeg: loaded N project tool(s) from <path> (M failed)` |
| `EnvOverride` (`UPEG_PROJECT_MANIFEST_PATH`) | 없음 | *(아무것도 출력하지 않는다)* |

동의 고지 부분이 `Detected`에만 붙는 것은 의도다: `UPEG_PROJECT_MANIFEST_PATH`로 사용자가
직접 지목한 경로는 이미 알고 지정한 것이라 고지할 대상이 아니고, 거기에 Tool이 하나도 없으면
집계로 전할 내용도 없어 아예 침묵한다.

`--quiet`/`-q`는 시작 보고 전체를 억제하며, 이 줄도 포함된다.

# 우선순위

1. 내장 static Tool은 불변이다.
2. `~/.upeg/toolkits`의 런타임 Toolkit이 먼저 로드된다.
3. 탐지된 프로젝트 `upeg.toml`이 다음에 로드되며, 같은 id의 이전 런타임 메타데이터/
   dispatcher를 **교체할 수 있다**.
4. 내장 id shadowing은 로더가 거부한다.

# 프로젝트 board — `[[boards]]`

프로젝트 매니페스트는 최상위 `[[boards]]` 배열로 **자기 board를 선언**할 수 있다. Tool의
`boards = ["..."]` 는 그 board를 가리키고, board는 매니페스트가 탐지되는 동안에만 존재한다.

```toml
[[boards]]
id = "upeg-dev"        # 표면에서 그대로 쓰는 이름 (`upeg board upeg-dev list`)
label = "upeg dev"     # GUI 탭 제목. 생략하면 id를 쓴다

[[tools]]
id = "git_status"
boards = ["upeg-dev"]
# ...
```

**프로젝트 매니페스트 전용이다.** `~/.upeg/toolkits/*.toml` 은 전역이라 board를 매어 둘
프로젝트가 없다 — 거기 `[[boards]]` 가 있으면 로더가 `BoardsOutsideProjectManifest` 로
그 파일 전체의 등록을 거부한다.

## 선언 규칙

| 규칙 | 이유 |
|---|---|
| `id` 는 canonical(앞뒤 공백 없음)·비어 있지 않아야 한다 | 표면이 그대로 출력·입력하는 키다 |
| `id` 에 `:` 를 쓸 수 없다 | store key의 네임스페이스 구분자로 예약되어 있다 (아래) |
| `id` 는 내장 board(`dev`/`trading`/`personal`)를 가릴 수 없다 | "어느 board를 말하는가"가 표면마다 달라진다 |
| 같은 매니페스트 안에서 `id` 중복 금지 | 두 선언 중 어느 쪽이 이겼는지 알 수 없다 |

내장 board 목록 자체는 이제 코드가 아니라 데이터다(`upeg_core::BUILTIN_BOARDS`). 프로젝트
board는 런타임에 그 표를 **확장**한다.

## 가시성 — 탐지되는 동안에만

프로젝트 board는 **어떤 표면에서도** 매니페스트가 탐지될 때만 열거된다: `upeg board list`,
`upeg board <b> list`, HTTP `/v1/boards`·`/v1/boards/{b}`, board scope MCP `tools/list`,
Desktop 탭이 모두 같은 `upeg_sources::pegboard` 로드 경로를 지나기 때문이다. 저장소 밖에서
같은 board를 부르면 "unknown board" 이고, HTTP는 404다.

매니페스트가 선언을 지우면 그 board는 다음 로드부터 사라진다. 행은 store에 남아 있지만
보이지 않으며, 선언이 돌아오면 핀도 함께 돌아온다.

## 지속성 규칙 — 매니페스트 경로로 네임스페이스한다

프로젝트 board의 행은 store에 다음 키로 저장된다:

```
project:<manifest-path-digest>:<board-id>
```

`<manifest-path-digest>` 는 매니페스트 **절대 경로**의 안정적인 64비트 digest다
(`upeg_core::ProjectBoardNamespace`). 따라서:

- 서로 다른 두 프로젝트가 같은 board id(`dev-board` 등)를 선언해도 핀을 공유하지 않는다.
- 같은 저장소의 두 checkout도 서로 다른 프로젝트다 — 경로가 다르기 때문이다.
- **프로젝트 디렉터리를 옮기면 프로젝트 board의 핀은 따라오지 않는다.** 매니페스트를 파싱하기
  전에 upeg가 읽을 수 있는 유일한 신원이 경로라서 받아들인 대가다. 매니페스트가 선언한 id를
  신원으로 쓰면 무관한 저장소끼리 충돌한다.

읽기와 쓰기는 같은 `BoardVisibility` 를 통과한다. 한 pass는 **자기가 쓸 행만** 쓸고
(tombstone), 다른 프로젝트의 네임스페이스는 읽지도 지우지도 않는다. 그래서 프로젝트 밖에서
board를 편집해도 프로젝트 board의 핀은 그대로 남는다.

사용자가 만든 전역 board와 프로젝트 board의 id가 겹치면 **프로젝트 안에서는 프로젝트 board가
이긴다.** 가려진 전역 board의 행은 손대지 않으므로, 프로젝트를 벗어나면 원래 핀 그대로
돌아온다. 매니페스트가 선언한 board는 삭제할 수 없다(`n`/`R`/`D` 중 `D`가 거부된다) — 다음
로드에서 다시 병합될 삭제는 정직하지 않기 때문이다.

# 주입되는 컨텍스트

```json
{
  "_upeg": {
    "board": "dev",
    "boardEnv": {},
    "projectManifest": "/path/to/upeg.toml"
  }
}
```

키의 의미는 [호출 봉투](/architecture/call-envelope.md) 참조.

# Provenance와 dispatch 위치

프로젝트 매니페스트가 등록한 Tool에는 `source = "project-manifest:<path>"` provenance가
찍힌다(`upeg tool list --json`, HTTP `/v1/tools`, MCP `tools/list`의 `source` 필드).

이 provenance는 CLI/TUI/MCP-proxy의 auto-attach 판단에 그대로 쓰인다. Project Manifest는
**호출자의 작업 디렉터리**로 정해지는데, 다른 곳에서 띄운 host는 자기 나름의 `upeg.toml`을
(또는 아무것도) 해석했으므로 그 Tool을 모른다. 따라서:

- provenance가 `project-manifest:*`인 Tool은 host가 떠 있어도 **항상 in-process로 실행**한다.
- 같은 이유가 **board 한 단계 위에도** 적용된다. `upeg board <b> call` 은 `<b>` 가 프로젝트
  선언 board면 auto-attach하지 않고 로컬로 dispatch한다. 프로젝트 board는 이 프로세스가
  매니페스트를 탐지하는 동안에만 존재하고, host는 자기 나름의 `upeg.toml`을(또는 아무것도)
  해석했으므로 **그 board 자체를 가지고 있지 않다** — 붙으면 `/v1/boards/<b>` 가 404로
  돌아온다. 바로 여기서 멀쩡히 존재하는 board에 대해 원격 404를 받는 대신, 성공할 수 있는
  유일한 경로인 로컬로 간다. 전역 board는 예전처럼 auto-attach한다.
- 그 외 Tool을 host로 넘길 때는 호출자의 절대 cwd를 `_upeg.cwd`로 실어 보낸다. `External`
  invoker가 이 값을 자식 프로세스의 작업 디렉터리로 적용하므로, host의 cwd가 아니라 호출한
  자리에서 실행된다([호출 봉투](/architecture/call-envelope.md)).

proxy 모드의 `upeg mcp`도 같은 규칙을 따른다. stdio에서 들어온 `tools/call`은
`params.name`의 provenance를 보고 in-process 또는 host로 갈리고, host로 나가는 프레임에는
`params.arguments._upeg.cwd`가 각인된다. host의 `tools/list` 응답에는 이 프로세스의
project-manifest Tool이 이름 기준 dedupe로 병합된다 — 그러지 않으면 agent가 목록에 없는
Tool을 호출할 수 있어야 하는 모순이 생긴다([MCP](/architecture/mcp.md)의 Proxy 모드).

# 진단

`upeg doctor`와 `upeg host status`는 각각 탐지된 매니페스트 경로(없으면 `none`)와 override
상태(`detect` / `off` / `explicit`)를 텍스트·JSON 양쪽으로 보고한다. 둘 다 이 문서의
`UPEG_PROJECT_MANIFEST_PATH` 판정 로직을 그대로 재사용하므로 보고 내용이 실제 탐지 동작과
어긋날 수 없다.
