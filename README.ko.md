# upeg — Universal Pegboard

[English](README.md) | **한국어**

도구를 한 번 핀해두면 어디서든 호출한다. upeg은 매일 찾게 되는 명령·변환·검사를
보드 위에 사는 **Tool**로 만들고, 각 Tool을 그것이 지원하는 surface — 데스크톱
앱, 터미널, MCP로 연결한 AI 에이전트, 작은 로컬 HTTP API — 에 노출한다.

하나의 정의 — Rust `#[upeg::tool]` 함수, TOML 매니페스트, WASM 플러그인,
임포트한 MCP 서버 — 가 Tool을 한 번 등록하면 **CLI, TUI, Desktop, PWA,
Chrome extension, MCP, HTTP**에서 호출할 수 있다. surface별 코드는 없다.

## 무엇을 할 수 있는가

- **내장 도구상자를 바로 실행** — 변환, 인코더, 해시, ID, 텍스트 도구, CSV,
  QR, 이미지 변환, PDF/Office 검사.
- **Tool을 Board에 핀** — 맥락별 도구·저장된 입력 프리셋·사용 지침을 함께
  담고, 프로젝트 보드는 저장소의 `.upeg/project.toml`에 선언한다.
- **키보드만으로 전부 조작**하는 TUI와 Desktop 앱, 또는 안정적인 JSON 출력을
  제공하는 CLI로 같은 Tool을 스크립트한다.
- **보드를 에이전트에 넘긴다** — `upeg mcp`가 stdio JSON-RPC로 Tool을
  제공하며, 원하면 하나의 보드의 핀과 지침으로 범위를 한정한다.
- **직접 만든 명령을 감싼다** — TOML `External` invoker, Chain, WASM
  플러그인, 임포트한 MCP 서버. credential 값은 환경변수나 OS keychain에만
  둔다.

로컬 우선: 내장 도구상자는 사용자의 기기에서 실행되고, HTTP host는 기본으로
loopback에 바인드하며, `/healthz`를 제외한 모든 라우트에 bearer token을
요구한다. 클라우드 동기화는 없다. 선택한 배포 Toolkit은 첫 사용 전에 버전이
고정된 pack을 한 번 다운로드할 수 있으며, 내장 Tool과 이미 검증해 캐시한 pack은
오프라인에서도 동작한다.

## 프로젝트 상태

upeg은 아직 초기 단계다.

- **빌드된 파일을 내려받아 설치할 수 있다.** 아래 CLI 설치 명령을 사용하거나
  [최신 릴리스](https://github.com/5pecia1/UPeg/releases/latest)에서 데스크톱 패키지를
  선택한다. OS와 CPU별 파일 및 Linux `.deb`, `.rpm`, AppImage 선택은
  [설치 가이드](docs/guides/installation.md#choose-a-download)에서 안내한다.
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

**macOS Apple Silicon**, **GNU/Linux x86_64**, **GNU/Linux ARM64**에서는 다음 명령으로 CLI를 설치한다.

```sh
curl -fsSL https://raw.githubusercontent.com/5pecia1/UPeg/main/install.sh | sh
```

최신 공개 CLI를 `~/.local/bin/upeg`에 설치하며 `sudo`나 Rust는 필요하지 않다.
Linux 아카이브는 glibc 환경을 대상으로 한다. 릴리스별 요구사항과 소스 빌드는
[Linux 호환성 안내](docs/guides/installation.md#linux-cli-compatibility)를 따른다.
같은 명령을 다시 실행하면 해당 CLI를 갱신한다. `upeg`를 찾지 못하면 셸 설정에
`export PATH="$HOME/.local/bin:$PATH"`를 추가하고 새 터미널을 연다.

**데스크톱 창**을 사용하려면 [최신 릴리스](https://github.com/5pecia1/UPeg/releases/latest)에서
universal macOS DMG, Linux x86_64 `.deb` / `.rpm` / AppImage 또는 Windows x86_64
MSIX를 선택한다. Intel Mac에는 universal GUI 패키지가 있으며 이 설치 명령은
Intel Mac용 CLI를 제공하지 않는다. Windows CLI는 ZIP 파일로 수동 설치한다. 현재 공개된 Windows MSIX는
서명되지 않아 일반 Windows 환경에서 설치가 실패할 수 있다.
[수동 설치 안내](docs/guides/installation.md#choose-a-download)를 확인한다.
CLI 설치 명령은 GUI 앱을 설치하거나 갱신하지 않는다.

체크섬 확인, 수동 설치, 웹 배포 및 소스 빌드는
[설치 가이드](docs/guides/installation.md)를 따른다.

## 첫 성공

```bash
upeg call --local num.hex_to_decimal -a input=0xff
# → 255
```

계정이나 토큰은 필요하지 않다. 첫 호출은 해당 버전의 Toolkit pack을 내려받을 수 있으며,
검증된 캐시가 있으면 이후에는 오프라인에서도 실행할 수 있다. `--local`은 실행 중인 로컬
host에 연결하는 대신 현재 프로세스에서 실행하도록 지정하며, 오프라인 모드는 아니다.
릴리스의 `toolkits-*` 파일을 직접 내려받을 필요는 없다. 이어서 둘러본다:

```bash
upeg tool list                 # 등록된 모든 Tool
upeg num hex-to-decimal 0xff   # 같은 호출의 짧은 동적 형태
upeg                           # 터미널에서: 대화형 TUI
```

모든 surface는 같은 구조의 `ToolResult` envelope(`ok`, `primary_output_id`,
`outputs[]`)을 반환한다. 기본 CLI는 primary 값만 출력한다. 보드, 프리셋,
MCP 클라이언트 설정, 파일 입력은
[빠른 시작 가이드](docs/guides/quick-start.md)에서 안내한다.

## Surface 선택

| Surface | 시작 |
|---|---|
| 터미널 (TUI / CLI) | `upeg` · `upeg call <toolkit>.<tool>` |
| Desktop 앱 | 설치한 앱을 연다. [GUI 첫 사용 안내](docs/guides/quick-start.md#desktop-first-use)를 따른다 |
| MCP (에이전트) | `upeg board dev connect` · `upeg mcp --board dev` |
| HTTP host | `upeg http --addr 127.0.0.1:7173` |
| PWA | 운영자가 사용하는 배포 아카이브다. [웹 배포 안내](docs/guides/installation.md#web-deployment)를 따른다 |
| Chrome extension | `./chrome-ext/build.sh` 후 `chrome-ext/dist/`를 unpacked로 로드 |

샌드박스된 surface(PWA, 확장)는 네이티브 런타임이 필요한 도구를 페어링된
로컬 host를 통해 실행한다. PWA 배포에는 cross-origin isolation header가,
확장에는 host endpoint/token 페어링이 필요하며, 둘 다
[설치 가이드](docs/guides/installation.md)에서 안내한다. 직접 만든 명령을
Tool로 감싸는 방법은 [도구 작성자 가이드](docs/guides/tool-author.md)를 참고한다.

<a id="file-input-wire"></a>

File 입력과 출력은 모든 surface에서 하나의 canonical `FileValue` JSON을
사용한다 — 계약은
[`docs/architecture.md#file-wire`](docs/architecture.md#file-wire)에 있다.

> **Breaking migration (beta):** 이전의 `"bytes":[0,255]` 숫자 배열은 더 이상
> 지원하지 않는다 — File 값은 `"bytes":"AP8="`처럼 패딩된 표준 Base64
> 문자열을 담아야 한다.

## 문서

- [`docs/index.md`](docs/index.md) — 문서 홈
- [가이드](docs/guides/quick-start.md) — 설치, 빠른 시작, 도구 작성, 개발,
  문제 해결
- [`docs/architecture.md`](docs/architecture.md) — 모든 surface가 공유하는
  계약
- [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md) — Toolkit TOML 전체 필드
  레퍼런스(생성 문서)
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
[`docs/architecture.md#security-absolutes`](docs/architecture.md#security-absolutes)에
있다.

UPeg 자체 코드는 [Apache-2.0](LICENSE)이다. 플러그인 작성자가 사용하는
`upeg-plugin-api`와 `upeg-plugin-macros`는 각 디렉터리의 MIT 또는
Apache-2.0 라이선스를 선택할 수 있다. 벤더 코드·글꼴 등 서드파티 구성
요소에는 각자의 라이선스가 적용되며 [NOTICE](NOTICE)에 출처와 고지를
기록한다.
