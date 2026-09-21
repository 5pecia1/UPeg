# upeg — Universal Pegboard

[English](README.md) | **한국어**

도구를 한 번 핀해두면 어디서든 호출한다. upeg은 매일 찾게 되는 명령·변환·검사를
보드 위에 사는 **Tool**로 만들고, 각 Tool을 그것이 지원하는 surface — 데스크톱
앱, 터미널, MCP로 연결한 AI 에이전트, 작은 로컬 HTTP API — 에 노출한다.

하나의 정의 — Rust `#[upeg::tool]` 함수, TOML 매니페스트, WASM 플러그인,
임포트한 MCP 서버 — 가 Tool을 한 번 등록하면 **CLI, TUI, Desktop, PWA,
Chrome extension, MCP, HTTP**에서 호출할 수 있다. surface별 코드는 없다.
각 Tool은 자신이 나타날 surface를 선언한다 — batch 파일 Tool은 TUI를 생략할
수 있고, 브라우저 surface는 페어링된 로컬 host를 통해 도구를 실행한다.

## 무엇을 할 수 있는가

- **내장 도구상자를 바로 실행**: 숫자·진수 변환, Base64/URL/HTML/JSON 인코딩과
  포매팅, 해시, UUID/NanoID 생성, 텍스트 diff/정규식/단어 수, CSV
  select/diff/to-JSON, QR 인코드/디코드, 이미지 변환, PDF/Office 검사와 추출.
- **Tool을 Board에 핀.** 보드는 한 맥락의 도구와 저장된 입력 프리셋, 사용
  지침을 함께 담는다 — 개인 보드는 개인 프로필에, 프로젝트 보드는 저장소의
  `upeg.toml`에 선언한다.
- **키보드만으로 전부 조작**하는 TUI와 Desktop 앱, 또는 안정적인 JSON 출력을
  제공하는 CLI로 같은 Tool을 스크립트한다.
- **보드를 에이전트에 넘긴다.** `upeg mcp`가 stdio JSON-RPC로 Tool을 Claude
  Desktop, Cursor 등 MCP 클라이언트에 제공한다 — 원하면 하나의 보드와 그
  보드의 지침으로 범위를 한정한다.
- **직접 만든 명령을 TOML로 감싼다**(`invoker = "External"`). Tool을 승인
  step이 있는 Chain으로 묶고, WASM 플러그인을 쓰거나, 상위 MCP 서버를
  임포트한다 — 모두 같은 레지스트리에 들어온다. External·HTTP·임포트한
  MCP 도구는 감싼 명령이나 서버가 하는 일을 그대로 한다 — 필요한 네트워크
  접근을 포함한다.
- **비밀을 설정 밖에 둔다.** 매니페스트는 credential을 이름으로만 참조하고,
  값은 환경변수나 OS keychain에만 존재한다.

로컬 우선: 내장 도구상자는 사용자의 기기에서 실행된다. HTTP host는 기본으로
loopback에 바인드하고 `/healthz`를 제외한 모든 라우트에 bearer token을
요구한다. 클라우드 의존성은 없다 — 클라우드 동기화도 아직 없다
([프로젝트 상태](#프로젝트-상태) 참고).

## 프로젝트 상태

upeg은 초기 단계의 pre-release다.

- **아직 게시된 다운로드가 없다.** 지금 시점에 버전이 붙은 릴리스나 빌드된
  바이너리는 없다 — 소스에서 직접 빌드한다(아래). 패키징 레시피(`just
  package-*`)는 있지만 서명되지 않은 산출물을 만든다.
- **이 저장소는 공개 미러다.** 개발은 비공개 소스 저장소에서 이뤄지고,
  업데이트는 여기에 export된 pull request로 도착해 일반 PR처럼 merge된다.
  개발 히스토리 자체는 미러링되지 않는다 — 각 export가 squash된 검증 커밋
  하나를 담는다.
- **CI:** 모든 export가 4개 공개 lane — Rust build/test/clippy, WASM clippy,
  Flutter analyze/test/Linux+web 빌드, 라이선스 검사 — 를 통과한다
  (`scripts/verify_public.sh`).
- **포맷과 surface 계약이 안정화되는 동안 breaking change를 예상해야 한다.**
  알려진 breaking change는 이 문서에 표시한다(예: 아래 File wire migration
  안내).

지원은 GitHub Issues를 통한 best-effort다 — [SUPPORT.md](SUPPORT.md)
참고. 보안 신고는 [SECURITY.md](SECURITY.md)를 따른다.


## 설치

아직 인스톨러가 없으므로 CLI를 소스에서 빌드한다. 필요한 것:

- **Git**, 링커용 **C 툴체인**(Debian/Ubuntu는 `build-essential`, macOS는
  Xcode Command Line Tools, Windows는 MSVC Build Tools), 그리고
  **Rust 1.92+** —
  `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

```bash
git clone https://github.com/5pecia1/UPeg.git
cd UPeg
cargo install --locked --path upeg-cli   # `upeg`를 ~/.cargo/bin에 설치
upeg doctor                              # 바이너리/기능/소스 디렉터리 진단 출력
```

`cargo install`은 `upeg`를 `PATH`에 올린다(rustup이 이미 추가하는
`~/.cargo/bin`을 통해). 설치 없이 빌드만 하려면 `cargo build --release
-p upeg-cli`를 실행하고 아래에서 `upeg` 대신 `./target/release/upeg`를 쓴다.

Desktop 앱, PWA, WASM 플러그인에는 더 많은 것이 필요하다(Flutter SDK,
`wasm-pack`, 추가 타깃) — 단, CLI에는 필요 없다. 선택적 패키징 도구를 포함한
전체 표는 [설치 가이드](docs/guides/installation.md)에 있다.

## 첫 성공

```bash
upeg call num.hex_to_decimal -a input=0xff
# → 255
```

이 내장 호출에는 네트워크도, 계정도, 토큰도 필요 없다 — `upeg call`은
프로세스 안에서 바로 dispatch한다. 이어서 둘러본다:

```bash
upeg tool list                 # 등록된 모든 Tool
upeg tool list --tag convert   # 태그로 필터
upeg num hex-to-decimal 0xff   # 같은 호출의 짧은 동적 형태
upeg call num.hex_to_decimal -a input=0xff --json    # canonical envelope
upeg call num.hex_to_decimal -a input=0xff --pretty  # 라벨 있는 행
upeg                           # 터미널에서: 대화형 TUI
```

모든 surface는 같은 구조의 `ToolResult` envelope(`ok`, `primary_output_id`,
`outputs[]`)을 반환한다. 기본 CLI는 primary 값만 출력한다. 보드, 프리셋,
파일 입력은 [빠른 시작 가이드](docs/guides/quick-start.md)에서 안내한다.

## Surface 선택

**터미널.** `upeg`은 키보드 우선 TUI를 연다. `upeg call`과 동적 `upeg
{toolkit} {tool}` 라우트로 스크립트한다. bash/zsh/fish/elvish/powershell용
셸 자동완성: `upeg completions bash`(새 Toolkit을 설치하면 다시 생성한다).

**Desktop 앱.** 진짜 페그보드를 가진 Flutter 앱: 핀 드래그, 저장된 프리셋,
라이브 도구, 임베디드 웹뷰.

```bash
cd flutter_app && flutter run -d linux    # macOS: just flutter-run-macos
```

CMake가 Rust bridge 라이브러리를 자동으로 빌드한다 — 별도의 cargo 단계는
없다.

**MCP (에이전트).** `upeg mcp`는 stdio JSON-RPC를 말한다. 클라이언트를
연결하거나, 준비한 보드 하나로 서버 범위를 한정한다:

```bash
upeg board dev connect        # 보드 "dev"용 MCP 클라이언트 설정 출력
upeg mcp --board dev          # 그 보드에 핀된 도구만 제공
```

**HTTP.** 앱과 브라우저 확장을 위한 작은 로컬 REST + MCP-over-HTTP API.
한 터미널에서 명시한 포트로 포그라운드 서버를 시작한다:

```bash
upeg http --addr 127.0.0.1:7173
```

이어서 다른 터미널에서 서버가 발행한 endpoint와 bearer token을 읽고
호출한다 — `/v1/*` 라우트에는 `Authorization: Bearer`가 필요하다:

```bash
upeg http status --pairing                  # endpoint + token 출력
curl -H 'Authorization: Bearer <token>' http://127.0.0.1:7173/v1/tools
curl http://127.0.0.1:7173/healthz          # 유일한 무인증 라우트
```

백그라운드 host를 원하면 `upeg host start --daemon`이 같은 서버를 임시
loopback 포트에서 detached로 실행한다 — `upeg http status --pairing`이 실제
endpoint를 알려준다. Desktop 앱이 이미 실행 중이라면 이미 host를 맡고 있을
수 있다. 멈추는 대신 재사용한다 — `upeg http status`가 살아 있는 endpoint를
보여준다. `upeg call`은 discovery file(`~/.upeg/server.json`)로 실행 중인
host에 자동으로 붙는다(`upeg call --local`이 아니라면). loopback과
`chrome-extension://` 오리진은 항상 CORS 허용이며, 그 외 웹 오리진은 반복
가능한 `--cors-origin`이 필요하다.

**PWA / Chrome extension.** `just package-web`은 정적 호스팅 가능한 PWA
번들을 만들고, `./chrome-ext/build.sh`는 MV3 확장을 `chrome-ext/dist/`에
준비해 `chrome://extensions` → "Load unpacked"로 올린다. 둘 다 샌드박스된
surface다 — 네이티브 host가 필요한 도구는 페어링한 로컬 host를 통해
실행된다. 현재 어느 쪽도 호스팅되어 제공되지 않는다. 직접 빌드하고 로컬에서
실행한다.

오래 걸리는 `External` 도구는 실행 중 출력을 스트리밍한다(CLI는 stderr로
비추고, HTTP는 `POST /v1/tools/{id}/stream`으로 NDJSON을, MCP는
`notifications/message`를 보낸다). 그 뒤 모든 surface는 여전히 같은 최종
envelope를 받는다.

## 직접 도구 추가

| 방식 | 위치 | 비고 |
| --- | --- | --- |
| TOML Toolkit | `~/.upeg/toolkits/{toolkit_id}.toml` | 명령·HTTP 호출·스크립트를 감싼다. [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md)와 `examples/tools/` 참고 |
| Project manifest | 프로젝트 루트의 `upeg.toml` | 자동 탐지(cwd → 상위, `$HOME` 안에서 한정). 프로젝트 보드 선언 가능. [`docs/architecture/project-manifest.md`](docs/architecture/project-manifest.md) 참고 |
| WASM 플러그인 | `~/.upeg/wasm/*.wasm` | `upeg plugin new`가 게스트 크레이트를 scaffold한다. `examples/plugins/greet/` 참고 |
| MCP 임포트 | `~/.upeg/mcp-imports/*.toml` | 상위 MCP 서버의 도구를 재노출한다. `examples/mcp-imports/` 참고 |
| Rust 빌트인 | `upeg-tools/`의 `#[upeg::tool]` | 컴파일 타임 등록. [도구 작성자 가이드](docs/guides/tool-author.md) 참고 |

디렉터리는 `$UPEG_TOOLKITS_DIR`, `$UPEG_WASM_DIR`, `$UPEG_MCP_IMPORTS_DIR`로
바꿀 수 있다. 프로젝트 매니페스트 탐지는 `$UPEG_PROJECT_MANIFEST_PATH=off`로
끈다. 작성한 파일은 `upeg tool validate <path>`로 검증한다.

TOML 형태를 맛보기 — `git log`를 기본값이 있는 Tool로 감싼다:

```toml
[[tools]]
id = "git_log"
invoker = "External"
pegboard_units = "U1"
command = "git"
args_template = ["log", "--oneline", "-n", "{count}"]

[[tools.inputs]]
name = "count"
type = "integer"
default = 10
```

## Media 도구

Media Tool은 경로가 아니라 **바이트**를 주고받는다 — 파일 입력은 `-a
<key>=@<경로>`로 넘기고 File 출력은 `--out`으로 받는다. 그래서 같은 Tool이
CLI와, 페어링된 host를 통해 브라우저 surface에서도 동작한다.

```bash
upeg call media.image_convert -a input=@photo.png -a output_format=webp --out photo.webp
upeg call media.pdf_to_markdown -a input=@doc.pdf --json
```

기존 파일이 있으면 CLI는 `--force`를 주지 않는 한 덮어쓰기를 거부한다.
`--out`이 없으면 Tool이 정한 이름으로 현재 디렉터리에 쓴다. 포맷, 엔진,
도구별 제한은 [PDF 도구 안내](docs/pdf-tools.md)에 있다.

<a id="file-input-wire"></a>

## File 입출력 wire 계약

File 입력과 출력은 모든 surface에서 하나의 canonical `FileValue` JSON을
사용한다 — `name`, `is_dir`, 선택적 `mime`, `content`. `content.bytes`는
패딩된 표준 RFC 4648 Base64이고, 디렉터리는 `content.entries` 아래에 재귀로
중첩된다. schema 확장 `x-upeg-file-wire`가 이 위치를 가리킨다.

> **Breaking migration (beta):** 이전의 `"bytes":[0,255]` 숫자 배열은 더 이상
> 지원하지 않는다 — File 값은 `"bytes":"AP8="`처럼 패딩된 표준 Base64
> 문자열을 담아야 한다.

정확한 JSON 예시, HTTP/MCP 호출 형태, 전체 크기 상한을 담은 전체 계약은
영문 레퍼런스
[`docs/architecture/file-wire.md`](docs/architecture/file-wire.md)에 있다.

## 문서

[`docs/index.md`](docs/index.md)에서 시작한다. 주요 문서:

- [가이드](docs/guides/index.md) — 설치, 빠른 시작, 문제 해결, 개발,
  Tool 작성
- [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md) — Toolkit TOML 전체
  필드 레퍼런스(생성 문서)
- [`docs/architecture/`](docs/architecture/index.md) — 모든 surface가
  공유하는 계약: call envelope, manifest, I/O 타입, File wire, HTTP API,
  host topology, MCP
- [`docs/product/`](docs/product/index.md) — upeg이 무엇이고 무엇이
  아닌지, 그리고 비타협 보안 규칙
- [`docs/LEXICON.md`](docs/LEXICON.md) — 제품·UI·CLI·매니페스트·코드가
  공유하는 단일 어휘

## 기여·보안·라이선스

기여를 환영한다 — 단, 개발은 비공개 소스 저장소에서 이뤄진다. 여기서 열린
pull request는 검토를 거쳐 소스 저장소에 반영되고, 이후 export된 업데이트
PR을 통해 다시 게시된다. 절차는 [CONTRIBUTING.md](CONTRIBUTING.md),
지원 기대치는 [SUPPORT.md](SUPPORT.md), 주요 변경 사항은
[CHANGELOG.md](CHANGELOG.md)에 있다.

보안 취약점으로 의심되는 문제는 비공개로 신고한다 —
[SECURITY.md](SECURITY.md) 참고. 제품이 스스로 지키는 비타협 보안
규칙은
[`docs/product/security-absolutes.md`](docs/product/security-absolutes.md)에
있다.

UPeg 자체 코드는 [Apache-2.0](LICENSE)이다. 플러그인 작성자가 사용하는
`upeg-plugin-api`와 `upeg-plugin-macros`는 각 디렉터리의 MIT 또는
Apache-2.0 라이선스를 선택할 수 있다. 벤더 코드·글꼴 등 서드파티 구성
요소에는 각자의 라이선스가 적용되며 [NOTICE](NOTICE)에 출처와 고지를
기록한다.
