# upeg — Universal Pegboard

하나의 Tool 정의가 7개 surface(CLI / TUI / Desktop / PWA / Chrome ext / MCP / HTTP)에서 동일하게 동작하는 Rust 워크스페이스.

`#[upeg::tool]` 어노테이션 한 번으로 모든 surface에 노출되며, TOML / WASM 플러그인 / 외부 MCP 서버에서도 같은 레지스트리로 통합된다.

## 사전 요구사항 (Prerequisites)

로컬 개발 및 빌드 환경에서 필요한 도구들을 설치합니다:

- **Rust 1.92+**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **`wasm32-unknown-unknown` target**: `rustup target add wasm32-unknown-unknown`
- **just**: `cargo install just --locked`
- **python3**: 대부분의 Linux 배포판에 기본 포함 (`apt install python3` 또는 `brew install python3`)
- **Flutter SDK**: [https://docs.flutter.dev/get-started/install](https://docs.flutter.dev/get-started/install)에서 Linux 다운로드, Linux desktop 의존성은 아래 [빌드 & 실행](#빌드--실행) 의 packaging table 참고
- **flutter_rust_bridge_codegen 2.12.0**: `cargo install flutter_rust_bridge_codegen --locked --version 2.12.0`
- **wasm-pack**: `cargo install wasm-pack --locked`
- **actionlint**: `bash <(curl -s https://raw.githubusercontent.com/rhysd/actionlint/main/scripts/download-actionlint.bash)`

## 빌드 & 실행

```bash
# 빌드 (릴리즈)
cargo build --release -p upeg-cli --bin upeg

# 등록된 tool 목록
./target/release/upeg tool list

# tool 실행
./target/release/upeg num hex-to-decimal 0xff      # → 255
./target/release/upeg call num.hex_to_decimal -a input=0xff
./target/release/upeg call num.hex_to_decimal -a input=0xff --json
./target/release/upeg call num.hex_to_decimal -a input=0xff --field result
./target/release/upeg call num.hex_to_decimal -a input=0xff --pretty

# board pin 관리 (pin / unpin / move)
./target/release/upeg board dev pin num.hex_to_decimal

# 셸 자동완성 (bash / zsh / fish / elvish / powershell)
# bash — bash-completion 2.x의 사용자 경로
mkdir -p ~/.local/share/bash-completion/completions
./target/release/upeg completions bash > ~/.local/share/bash-completion/completions/upeg
# zsh — fpath에 든 디렉터리에 두고, ~/.zshrc에서 compinit보다 먼저
#       fpath=(~/.zfunc $fpath) 를 실행한다
mkdir -p ~/.zfunc && ./target/release/upeg completions zsh > ~/.zfunc/_upeg
# fish
mkdir -p ~/.config/fish/completions
./target/release/upeg completions fish > ~/.config/fish/completions/upeg.fish
# 생성 시점의 Tool 목록이 스크립트에 박히므로, Toolkit이나 플러그인을
# 새로 설치한 뒤에는 다시 생성해야 새 Tool이 완성된다.
# 자세한 계약: docs/architecture/call-envelope.md 의 "셸 자동완성" 절

# MCP 서버 (stdio JSON-RPC) — Claude Desktop / Cursor 연동용
./target/release/upeg mcp

# 특정 보드의 안내·도구 확인과 MCP 연결 설정 생성
./target/release/upeg board dev context --json
./target/release/upeg board dev connect

# HTTP 서버
./target/release/upeg http --addr 127.0.0.1:7173

# 브라우저(PWA) 오리진에서 붙는 경우 — loopback·chrome-extension://은 기본 허용,
# 그 외 웹 오리진은 --cors-origin으로 정확히 일치하는 값만 허용 (반복 지정 가능)
./target/release/upeg http --addr 127.0.0.1:7173 --cors-origin https://upeg.example.com

# 백그라운드 host (셸 비점유, 동일 바이너리)
./target/release/upeg host start --daemon

# call은 discovery file(~/.upeg/server.json)을 통해 실행 중인 host에
# 자동으로 attach된다 — 별도 플래그가 필요 없다
./target/release/upeg call num.hex_to_decimal -a input=0xff

# HTTP REST API — bearer token 인증
UPEG_HTTP_TOKEN=dev-token ./target/release/upeg host start --addr 127.0.0.1:7174 --daemon
curl -H 'Authorization: Bearer dev-token' http://127.0.0.1:7174/v1/tools
curl -H 'Authorization: Bearer dev-token' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' \
  http://127.0.0.1:7174/mcp

# 헬스체크 — 인증 불필요, importsPending으로 MCP 임포트 진행 상태를 알려준다
curl http://127.0.0.1:7174/healthz

# 페어링 — 실행 중인 host의 endpoint+token을 텍스트로 출력 (local operator 전용,
# 텍스트로 된 status 출력에만 있고 어떤 HTTP 라우트로도 노출되지 않는다)
./target/release/upeg http status --pairing

# 진단
./target/release/upeg doctor
```

### Tool output contract

Tool 실행 결과의 source of truth는 raw stdout이 아니라 canonical structured
`ToolResult`이다. 성공 envelope는 `ok=true`, `primary_output_id`, `outputs[]`
를 포함하고, 각 output row는 `id`, `label`, `kind`, `value`를 가진다.
`primary_output_id`는 output이 하나 이상이면 필수이며, action-only tool은
비워 둔다.

- CLI 기본 모드는 primary output 값만 stdout으로 출력한다.
- `upeg call ... --json`은 성공/실패 canonical JSON envelope를 stdout으로 출력한다.
- `upeg call ... --field <id>`는 특정 output 값만 출력한다.
- `upeg call ... --pretty`는 label + value 행으로 출력한다.
- HTTP/host/FRB는 canonical JSON envelope를 반환한다.
- MCP `tools/call`의 `structuredContent`가 canonical envelope이며, text
  `content`는 primary output 표시용 fallback이다.
- TUI/Desktop/PWA/Chrome-ext/Controlled Embed는 같은 output row의 label과
  value를 surface별 presentation으로 렌더링한다.
- `Chain` tool 결과는 각 step의 `id`/`status`/`duration_ms`를 담은 `steps` output row를
  포함하며, 승인 step은 서버가 각인한 `_upeg.surface`가 `approval_surfaces`에 인가된
  경우에만 인정된다. CLI(`-a approve=true`)·TUI(확인 대화상자)·Desktop(확인 다이얼로그)
  셋 다 사람의 확인을 받은 뒤에만 `approve`를 싣는다.

실패 envelope는 `ok=false` + `error`다. `External` invoker(외부 바이너리)는 여기에
`error.details`를 추가로 싣는다 — `exit_code`(비정상 종료 시 `null`), 시그널로 죽었으면
`signal`, `timeout_ms`를 넘겼으면 `timed_out`, 그리고 **캡된 stdout/stderr**. `--check`
계열 도구는 진단을 stdout에 쓰므로 이 필드가 없으면 실패 이유가 통째로 사라진다.
CLI(사람/`--json`)·MCP·HTTP·TUI·Flutter가 모두 같은 키를 읽는다. 전체 계약은
[`docs/architecture/manifest.md`](docs/architecture/manifest.md) 참고.

envelope는 실행이 **끝난 뒤** 오는 하나뿐이고, 그건 그대로다. 오래 걸리는 `External`
tool을 위해 그 옆에 실행 **중** 출력을 흘리는 선택적 통로가 하나 더 있다 — 소비하지
못하는 surface는 예전과 똑같이 최종 envelope만 받는다.

- CLI 기본/`--pretty`는 자식의 stdout·stderr를 실행 중 **stderr로** 그대로 비춘다.
  stdout은 최종 결과 전용이다. `--json`/`--field`는 아무것도 흘리지 않는다.
- HTTP는 `POST /v1/tools/{id}/stream`이 `application/x-ndjson`으로 `chunk` 줄들 뒤에
  `result` 줄 하나를 보낸다.
- MCP `tools/call`은 실행 중 `notifications/message`(level `info`, logger `upeg.tool`)를
  보내고, 최종 응답 프레임은 바뀌지 않는다.
- TUI와 Desktop은 실행 중 마지막 몇 줄을 live tail로 보여주고, `Esc`(TUI) / Cancel
  버튼(Desktop)으로 실행을 취소한다. host에 attach한 TUI도 같다 — 위의 NDJSON
  라우트를 타고 들어오므로 tail과 취소가 독립 실행일 때와 똑같이 동작한다.
  스트리밍 라우트를 모르는 옛 host에 붙으면 버퍼 경로로 되돌아가고, tail에
  "실시간 출력 없음"이라고 한 줄 적어 둔다.

## Desktop UI

Controlled Embed는 앱이 소유한 WebView 세션에서 실행한다. 카드의 Run과 디버그의
Run/Re-run은 같은 실행 경로를 사용하며, Debug를 열거나 닫아도 페이지를 새로 만들지 않는다.
Desktop의 **Local HTTP host**를 켜고 앱을 시작하면, 그 내장 호스트에 연결되는 CLI 호출도
같은 세션을 사용할 수 있다. 입력 프리셋과 출력 타입·대표 결과는 Rust dispatcher가 처리한다.
프로젝트 Tool의 로컬 실행이나 별도 CLI 호스트의 headless 실행과는 구분된다.
자세한 범위는 [Controlled Embed 세션 계약](docs/ui-ux-surface-contract.md#desktop-controlled-embed의-공통-실행과-디버그)을 참고한다.

Desktop/PWA surface는 **Flutter + flutter_rust_bridge**로
빌드한다. Rust 워크스페이스는 도구 레지스트리·실행·플랫폼 IPC를 책임지고,
UI/렌더링은 `flutter_app/`이 책임진다 (2026-05-24 cutover 완료). Chrome 확장은
`chrome-ext/`의 JavaScript 구현이며 HTTP와 현재 탭의 content script를 사용한다. 이전
Dioxus 기반 `desktop-ui/` 비교 빌드는 Phase 11 cleanup에서 삭제되었다
(`6597a3c`, 범위 `dd28130..d404cec` 및 후속 closure 커밋).

## Running the app

### Linux

```bash
cd flutter_app
flutter run -d linux
```

CMake가 `cargo build -p upeg-frb`를 자동으로 호출하고 생성된 cdylib
(`libupeg_frb.so`)을 Flutter 번들의 `lib/` 디렉터리로 install한다.
별도의 수동 `cargo build` 단계는 필요 없다 — `flutter_app/rust_builder/`의
cargokit plugin이 Flutter build pipeline에서 처리한다.

### macOS

```bash
just flutter-run-macos
UPEG_FLUTTER='fvm flutter' just flutter-run-macos
```

macOS 런타임 smoke는 실제 Mac에서 실행한다. `flutter-build-macos`는 release
build 확인용이고, `flutter-run-macos`는 window manager, tray, FRB init,
board/tag selection parity 같은 GUI 시작 경로를 직접 확인하기 위한 레시피다.

### Web

```bash
just flutter-run-web         # build-frb-wasm + flutter run -d chrome
just flutter-run-web-server  # build-frb-wasm + flutter run -d web-server
just flutter-web-smoke       # release web build + headless Chromium boot check
```

Web target은 `upeg-frb`의 wasm-pack 산출물을 요구하므로 `just` recipe를
사용한다. `RustLib.init()`가 FRB Web loader를 통해 `web/pkg/upeg_frb.js`와
`upeg_frb_bg.wasm`을 로드한다.

PWA 경로에서도 local-first 기능은 실제 FRB wasm 구현을 사용한다:
tool 목록, built-in tool 실행, pegboard/tweaks/backup 저장소, embed URL
해결, 외부 브라우저 열기. 백그라운드 host 제어처럼 호스트
프로세스가 필요한 기능만 desktop 전용이다.

### Manual build (alternative)

```bash
cargo build --release -p upeg-frb
cd flutter_app && flutter run -d linux
```

CMake 통합이 이미 같은 일을 해주므로 위의 수동 `cargo build` 사전 단계는
redundant — 하지만 무해하다. **cargokit bundling 단계를 대체하지는
않는다**: 그것이 없으면 cdylib가 빌드는 되지만 Flutter 번들에 들어가지 않는다.

### Flutter 빌드

```bash
cd flutter_app
flutter pub get
flutter run -d linux            # 또는 -d chrome, -d web-server
```

Chrome MV3 확장은 `./chrome-ext/build.sh`로 빌드. 확장의 투자 방향은 **확장에서만
가능한 in-page 기능**이다 ([Surface 계약](docs/ui-ux-surface-contract.md#chrome-extension-contract)):

- **In-page 감지기** — `chrome-ext/detectors.js`의 얼린 표. hex → `num.hex_to_decimal`,
  base64 → `convert.base64_decode`, epoch → 오프라인 ISO 미리보기(해당 tool 없음).
  hover하면 host가 실제 결과를 툴팁에 채우고, host가 없으면 `upeg://open?...` deep link로
  되돌아간다.
- **In-page selector adapter** — `ControlledEmbed` 핀의 selector binding을 지금 보고 있는
  탭에 적용한다. Desktop은 자기 webview를, 확장은 사용자의 탭을 움직인다.
- **사이트별 켜기** — 대상 사이트는 manifest 상수가 아니라 저장된 allow-list다. popup의
  "Enable on this site"가 권한을 요청하고 service worker가
  `chrome.scripting.registerContentScripts`로 등록한다. etherscan/polygonscan은 설치 시
  미리 켜져 있다.

Popup에서 dispatch할 수 없는 tool(`static` invoker, 띄울 페이지가 없는 Controlled Embed)은
`upeg://open?...` deep link로 Flutter desktop 인스턴스를 연다.

### Packaging (설치 가능한 산출물 생성)

```bash
# 호스트 OS에 맞는 모든 산출물(현재 OS에서 만들 수 있는 것만)
just package

# 개별 산출물
just package-linux-deb        # → target/packages/linux/upeg_<ver>.deb
just package-linux-appimage   # → target/packages/linux/upeg-<ver>-x86_64.AppImage
just package-macos-dmg        # → target/packages/macos/upeg-<ver>.dmg     (macOS only)
just package-windows-msix     # → target/packages/windows/*.msix          (Windows only)
just package-web              # → target/packages/web/ (정적 호스팅용 PWA 번들, .js/.css 사전 gzip)
```

산출물 위치는 모두 `target/packages/<platform>/`. Linux desktop packages include
the app bundle, `/usr/bin/upeg`, a desktop launcher, a 512px icon, and
AppStream metadata. 로컬 호스트 의존성:

| Recipe | 필요한 도구 |
|---|---|
| `flutter-build-linux`, `flutter run -d linux` | `libayatana-appindicator3-dev`, `libkeybinder-3.0-dev` (hotkey_manager X11 백엔드, 런타임: `libkeybinder-3.0-0`) + Flutter Linux 기본 의존성 (`libgtk-3-dev`, `pkg-config` 등) |
| `package-linux-deb` | `fpm` (`gem install fpm`) |
| `package-linux-appimage` | `appimagetool` ([AppImageKit releases](https://github.com/AppImage/AppImageKit/releases)) + `appstreamcli` (`appstream`); recipe uses `APPIMAGE_EXTRACT_AND_RUN=1` so devcontainers do not need FUSE |
| `package-macos-dmg` | `create-dmg` (`brew install create-dmg`) — macOS host only |
| `package-windows-msix` | `pubspec.yaml`의 `msix` dev dep + `msix_config` — Windows host only |
| `package-web` | `gzip` |

서명/노터라이즈는 의도적으로 범위 밖 — 산출물은 모두 unsigned이다.
host HTTP transport는 loopback TCP이므로 Windows/macOS/Linux에서 같은
모델로 동작한다.

## 테스트

검증은 두 단계(tier)다.

| tier | 명령 | 언제 | 소요 |
| --- | --- | --- | --- |
| pre-commit | `just check` | **모든 커밋 직전** | 웜 캐시 기준 ~2–3분 |
| full closure | `just verify` | push / PR 올리기 전 | CI 한 판과 동일 |

`just check`는 fmt-check · clippy-native · file-size-budget(Rust와 손으로 쓴 Dart 모두
≤1000줄 — FRB 생성 코드와 `*.freezed.dart`/`*.g.dart`는 제외) ·
lexicon-check · `cargo test --workspace --lib`(유닛 테스트만)이다. Flutter도, drift fixture도,
통합 테스트도 없다 — 커밋마다 돌려도 부담 없는 최소 게이트.

## 생성물 재생성 (Regenerating generated artifacts)

tool 출력, interface 메타데이터, 스키마, FRB 바인딩, UI parity baseline 같은 생성물은
`*-check` 레시피로 drift를 검사합니다. 도구·tool·UI를 변경했을 때는 각 재생성 레시피를 실행해
생성물을 갱신하고 커밋합니다.

| Drift 게이트 | 검사 레시피 | 재생성 레시피 |
|---|---|---|
| Tool 실행 baseline | `just test-baseline-check` | `just test-baseline` |
| Interface 메타데이터 | `just interface-inventory-check` | `just interface-inventory` |
| Toolkit schema | `just toolkit-schema-check` | `just toolkit-schema` |
| FRB 바인딩 (Rust/Dart) | `just frb-codegen-check` | `just frb-codegen` |
| UI parity (golden regression) | `just ui-parity-check` | `just ui-parity` |

## 사용자 tool 추가

| 방식 | 위치 | 비고 |
| --- | --- | --- |
| Rust 빌트인 | `upeg-tools/src/lib.rs`에 `#[upeg::tool]` 함수 | 컴파일 타임 등록 |
| TOML | `~/.upeg/toolkits/{toolkit_id}.toml` | [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md), `examples/tools/` 참고 |
| WASM 플러그인 | `~/.upeg/wasm/*.wasm` | [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md), `examples/plugins/greet/` 참고 |
| MCP 임포트 | `~/.upeg/mcp-imports/*.toml` | [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md), `examples/mcp-imports/` 참고 |
| Project Manifest | 프로젝트 루트의 `upeg.toml` | 자동 탐지 (cwd → `$HOME`까지의 상위 → `$HOME`). [`docs/architecture/project-manifest.md`](docs/architecture/project-manifest.md) 참고 |

각 디렉토리는 `$UPEG_TOOLKITS_DIR` / `$UPEG_WASM_DIR` / `$UPEG_MCP_IMPORTS_DIR`로 override 가능.
Project Manifest 자동 탐지는 `$UPEG_PROJECT_MANIFEST_PATH`로 끄거나(`off`) 절대 경로로 고정할 수 있다.
Project Manifest는 최상위 `[[boards]]`로 프로젝트 전용 board도 선언할 수 있다.
외부 매니페스트 작성 문서는 [`docs/TOOL_MANIFEST.md`](docs/TOOL_MANIFEST.md), editor/CI 검증용 schema는 [`fixtures/toolkit.schema.json`](fixtures/toolkit.schema.json)에 있다. 둘 다 `upeg-loader` 메타데이터에서 생성된다. 작성한 파일은 `upeg tool validate <path>`로 검증하고, 생성물 drift는 `just toolkit-schema-check`로 확인한다.
`External` invoker는 `color = "force"`로 non-TTY 파이프에서도 자식 프로세스의 색 출력을 강제할 수 있다.

File 입력 정책은 Rust macro와 TOML에서 같은 코어 계약으로 변환된다.

```rust
required images: File(
    extensions = ["png", "jpg", "jpeg"],
    max_count = 100,
    max_file_bytes = 52_428_800,
    max_total_bytes = 52_428_800,
)
```

```toml
[[tools.inputs]]
name = "images"
type = "file"
required = true
extensions = ["png", "jpg", "jpeg"]
max_count = 100
max_file_bytes = 52428800
max_total_bytes = 52428800
```

`max_count`는 재귀 `FileValue` 안의 실제 파일 수를 세며 기본값은 1이다.
확장자는 점 없이 선언하고 대소문자를 구분하지 않는다. 크기 제한은 각각 개별
파일과 전체 파일 바이트 합계에 적용된다.

<a id="file-input-wire"></a>

### File 입출력 JSON wire 계약

모든 surface의 File 입력과 출력은 같은 canonical `FileValue` JSON을 사용한다.
최상위 필드는 `name`, `is_dir`, 선택적인 `mime`, `content` 순서다. `mime`이
없으면 키를 생략한다.
`is_dir`는 별도 상태가 아니라 `content.kind`에서 파생되며, JSON을 읽을 때는
`bytes`일 때 `false`, `directory`일 때 `true`여야 한다. 서로 다르면 거부한다.

일반 파일의 `content`는 `{"kind":"bytes","bytes":"..."}`다. `bytes` 값은
패딩을 포함한 표준 RFC 4648 Base64 문자열이어야 한다. 예를 들어
`hello.txt`의 내용이 UTF-8 `hello`라면 정확한 JSON은 다음과 같다.

```json
{
  "name": "hello.txt",
  "is_dir": false,
  "mime": "text/plain",
  "content": {
    "kind": "bytes",
    "bytes": "aGVsbG8="
  }
}
```

디렉터리의 `content`는 `{"kind":"directory","entries":[...]}`이며, `entries`
각 항목도 같은 `FileValue` 형식이라 재귀 디렉터리를 표현할 수 있다. 여러 파일은
별도 최상위 배열이 아니라 하나의 `Directory`로 전달한다. 예를 들어 `A`와
바이트 `00 ff`를 담은 두 파일의 정확한 JSON은 다음과 같다.

```json
{
  "name": "files",
  "is_dir": true,
  "content": {
    "kind": "directory",
    "entries": [
      {
        "name": "a.txt",
        "is_dir": false,
        "mime": "text/plain",
        "content": {
          "kind": "bytes",
          "bytes": "QQ=="
        }
      },
      {
        "name": "data.bin",
        "is_dir": false,
        "content": {
          "kind": "bytes",
          "bytes": "AP8="
        }
      }
    ]
  }
}
```

URL-safe Base64의 `-`/`_`, 공백이나 줄바꿈, 빠졌거나 비정규인 패딩, 기존
숫자 배열 형태의 바이트 값은 모두 거부한다. 빈 파일만 빈 문자열 `""`을
사용한다.

> **Breaking migration (beta):** 이전의 `"bytes":[0,255]` 숫자 배열은 입력과
> 출력 모두에서 더 이상 지원하지 않는다. File 값을 생성하거나 소비하는 코드는
> 반드시 `"bytes":"AP8="`처럼 패딩된 표준 RFC 4648 Base64 문자열로 바꿔야 한다.

HTTP에서는 File 값을 Tool 인자 안에 그대로 넣어 `POST /v1/tools/{id}`로
보낸다. 다음 요청은 한 개의 PNG를 담은 canonical Directory를
`media.images_convert`에 전달하고 최종 출력은 8 MiB로 제한한다.

```bash
curl -X POST http://127.0.0.1:7174/v1/tools/media.images_convert \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data-binary @- <<'JSON'
{
  "images": {
    "name": "images",
    "is_dir": true,
    "content": {
      "kind": "directory",
      "entries": [{
        "name": "pixel.png",
        "is_dir": false,
        "mime": "image/png",
        "content": {
          "kind": "bytes",
          "bytes": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        }
      }]
    }
  },
  "output_format": "png",
  "max_output_bytes": 8388608
}
JSON
```

MCP stdio의 `tools/call`도 같은 Directory/Base64 값을 `arguments`에 넣는다.
MCP framing은 요청 하나당 JSON 한 줄이므로 실제 호출은 다음처럼 줄바꿈 없는
한 줄을 전송한다.

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"media.images_convert","arguments":{"images":{"name":"images","is_dir":true,"content":{"kind":"directory","entries":[{"name":"pixel.png","is_dir":false,"mime":"image/png","content":{"kind":"bytes","bytes":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="}}]}},"output_format":"png","max_output_bytes":8388608}}}' \
  | ./target/release/upeg mcp
```

JSON Schema의 File property에는 입력 `inputSchema`와 출력 `outputSchema`
양쪽 모두 `x-upeg-file-wire`가 추가된다. 이 닫힌 확장 객체는 현재 version 1,
`base64-rfc4648-padded`, 숫자 배열 비지원, 재귀 Directory 지원과 이 절의
공개 경로 `README.md#file-input-wire`를 machine-readable하게 선언한다. 외부
레거시 schema에서 확장 자체가 없는 것은 허용하지만, 확장이 있으면 모든 키와
값이 현재 계약과 정확히 같아야 한다.

Tool의 `max_file_bytes`/`max_total_bytes` 정책과 아래 surface 안전 상한은 함께
적용되며 더 엄격한 제한이 우선한다.

- Canonical File 출력은 root 하나마다 decoded raw bytes 합계 64 MiB
  (67,108,864 bytes), root를 포함한 전체 node 128개, 이름과 MIME metadata
  합계 16 KiB(16,384 bytes), root를 1로 센 최대 재귀 깊이 64로 제한한다.
  Core/CLI의 출력 처리와 Flutter의 File 출력 codec이 같은 root 예산을 적용한다.
- Core/CLI/Flutter File 입력은 실제 파일 최대 100개, 전체 `FileValue` node
  128개, 이름과 MIME metadata 합계 16 KiB(16,384 bytes), root를 1로 센
  최대 재귀 깊이 64, decoded raw bytes 합계 50 MiB(52,428,800 bytes)로
  제한한다. 이 입력 예산은 위의 출력 root 예산과 별개다.
- HTTP 요청 본문과 MCP 요청 한 줄은 각각 최대 1,000,000 bytes다. Base64,
  파일명, MIME, JSON/JSON-RPC envelope를 모두 포함한 framing 상한이다.
- Chrome extension(Ext)은 File 입력 decoded raw bytes 합계를 640 KiB
  (655,360 bytes)로 먼저 제한하고 최종 HTTP 요청도 1,000,000 bytes 미만으로
  제한한다. File 출력은 decoded raw bytes 64 MiB(67,108,864 bytes)까지
  읽는다.
- `media.images_convert`는 작업 중 추정 working set을 512 MiB로, 이미지
  하나의 encoded 결과를 64 MiB로, 최종 ZIP 전체를 64 MiB로 제한한다. 이
  batch 도구는 CLI/Desktop/MCP/HTTP/PWA/Ext에만 노출되며 TUI에서는 제외된다.
- `media.image_convert`는 입력 파일을 50 MiB(52,428,800 bytes)까지 허용한다.
- `media.pdf_extract_images`는 고유 이미지 XObject 100개, PDF Form XObject
  재귀 깊이 64, 추출한 encoded 이미지 합계 64 MiB, 헤더와 notes를 포함한
  최종 ZIP 64 MiB로 제한한다.
- `media.pdf_inspect`/`media.pdf_to_markdown`은 입력 32 MiB, 페이지 500개,
  반환 Markdown 8 MiB로 제한한다.

`x-upeg-file-policy`는 계속 입력 전용이며, 위의 고정 File 출력 root 예산은
output policy metadata가 아니다. 대신 `media.image_convert`,
`media.images_convert`, `media.image_to_pdf`, `media.pdf_to_images`는 일반
optional 입력 `max_output_bytes`를 받는다. 범위는 양 끝을 포함한 1..=67,108,864 bytes이고
기본값은 64 MiB다. 이 값은 반환할 File의 최종 `content.bytes` 크기를 제한하며,
초과하면 실패한다. 품질이나 DPI를 자동으로 낮춰 상한에 맞추지 않는다.
`media.pdf_inspect`/`media.pdf_to_markdown`은 이 인자를 받지 않는다.

## Media tools

Media tool은 경로가 아니라 **바이트**를 주고받는다. 파일 입력은 `-a
<key>=@<경로>`로 읽어 넘기고, File 출력은 `--out`으로 받는다. 이 계약 덕분에
같은 tool이 CLI뿐 아니라 브라우저(PWA/Chrome ext)에서도 동작한다.

```bash
# Images → PDF — 이미지 한 장, 플랫 디렉터리, zip 세 가지를 모두 받는다.
# 디렉터리와 zip은 이름순으로 한 장씩 페이지가 된다.
upeg call media.image_to_pdf -a input=@scan.png --out out.pdf
upeg call media.image_to_pdf -a input=@images --out out.pdf
# 출력은 최대 8 MiB
upeg call media.image_to_pdf \
  -a input=@images.zip -a max_output_bytes=8388608 --out out.pdf

# PDF → PNG pages, 출력 ZIP은 최대 8 MiB
upeg call media.pdf_to_images \
  -a input=@out.pdf -a dpi=144 -a max_output_bytes=8388608 --out pages.zip

# 디렉터리의 이미지를 한 번에 JPEG로 변환, 출력 ZIP은 최대 8 MiB
upeg call media.images_convert \
  -a images=@images -a output_format=jpeg -a max_output_bytes=8388608 \
  --out converted.zip

# 문서에 임베드된 원본 이미지 추출 (페이지 래스터화와 별개)
upeg call media.pdf_extract_images -a input=@doc.pdf --out images.zip
upeg call media.pptx_extract_images -a input=@deck.pptx --out images.zip

# PDF 검사와 네이티브 텍스트 → Markdown (OCR 필요 페이지도 함께 반환)
upeg call media.pdf_inspect -a input=@doc.pdf --json
upeg call media.pdf_to_markdown -a input=@doc.pdf --json
```

PDF 텍스트 도구의 추출 상태·제한·배포 방식은
[PDF 도구 안내](docs/pdf-tools.md)를 참고한다.

`media.pdf_to_images`가 반환하는 zip 안의 파일명은 `{pdf_stem}-p1.png`,
`{pdf_stem}-p2.png` 형식이다. `dpi`는 기본 144, 유효 범위 36~600.
`media.images_convert`의 CLI 디렉터리 입력은 바로 아래의 일반 파일만 읽고
이름순으로 처리한다. 하위 디렉터리와 symlink는 거부한다. Flutter에서는 같은
File 정책을 사용해 여러 파일 선택과 드래그 앤 드롭을 지원한다. 지원 이미지
입력과 출력 포맷은 PNG·JPEG·WebP·GIF·BMP·TIFF·ICO·QOI이며, SVG 입력도
지원한다. SVG는 픽셀 이미지로 변환하며 출력 폭을 지정할 수 있다.

```bash
# 이미지 한 장은 ZIP 대신 변환된 파일을 바로 저장
upeg call media.image_convert -a input=@photo.png -a output_format=webp --out photo.webp
# SVG를 종횡비를 유지하면서 PNG로 렌더링
upeg call media.image_convert -a input=@icon.svg -a output_format=png -a svg_width=512 --out icon.png
# 투명 영역을 흰색으로 합성하고 JPEG 품질 지정
upeg call media.image_convert -a input=@logo.png -a output_format=jpeg \
  -a jpeg_quality=85 -a 'background=#FFFFFF' --out logo.jpeg
```

WebP 출력은 무손실이며 GIF·다중 페이지 TIFF는 첫 프레임/페이지만 변환한다.
SVG 파일은 4 MiB, ICO 출력은 256×256 이하로 제한한다. UI는 선택한 형식에
맞는 옵션을 표시하고, 지원 이미지 미리보기와 파일 저장을 제공한다.
개발 계획·형식 계약에 세부 동작을 정리했다.

기존 파일이 있으면 CLI는 **덮어쓰지 않고 거부**한다. 덮어쓰려면 `--force`를
명시한다. `--out`을 생략하면 tool이 정한 이름으로 현재 디렉토리에 쓴다.

PDF 엔진은 전부 순수 Rust(`hayro` 렌더 / `lopdf` 추출 / `krilla` 생성)라
네이티브 dynamic library나 별도 환경변수 설정이 필요 없다. `pdf_extract_images`가
지원하지 않는 필터·색공간(JPX/JBIG2, CMYK/Indexed 등)을 만나면 손상된 파일을
내보내지 않고 건너뛴 뒤 사유를 zip 안 `EXTRACTION-NOTES.txt`에 기록한다.
OCR, page ranges, PDF passwords, compression/quality controls는 범위 밖이다.

## 문서

`docs/`는 OKF v0.2 지식 번들이다. [`docs/index.md`](docs/index.md)에서 시작한다.

- [`docs/LEXICON.md`](docs/LEXICON.md) — 용어 정의 (개명 전에 여기부터)
- [`docs/product/identity-and-boundaries.md`](docs/product/identity-and-boundaries.md) — 제품 정체성과 경계
- [`docs/architecture/`](docs/architecture/index.md) — 크레이트 경계, 도메인·프로토콜·호스트 계약, 기술 스택
- [`docs/guides/tool-author.md`](docs/guides/tool-author.md) — 도구 작성자 가이드

## 공개 미러 검증과 라이선스

공개 미러의 `just verify`는 Rust·WASM·Flutter 검증과 Linux·web 빌드, 라이선스 검사를
실행한다. 도구 버전과 명령은 `scripts/verify_public.sh`와 공개 CI에 명시되어 있다.
공개 기여 절차는 [기여 안내](CONTRIBUTING.md)를 따른다.

UPeg 자체 코드는 [Apache-2.0](LICENSE)이다. 플러그인 작성자가 사용하는
`upeg-plugin-api`와 `upeg-plugin-macros`는 각 디렉터리의 MIT 또는 Apache-2.0
라이선스를 선택할 수 있다. 벤더 코드·글꼴 등 서드파티 구성 요소에는 각자의
라이선스가 적용되며 [NOTICE](NOTICE)에 출처와 고지를 기록한다.
