---
type: Surface Contract
title: UI/UX Surface Contract
description: TUI, Flutter Desktop/PWA, Chrome extension이 공유하는 Tool 라이프사이클 — 키 바인딩, 활성화 정책, capability 렌더링, 페어링.
tags: [surfaces, ui, keyboard, capability, tui, flutter, chrome-ext]
status: stable
sources:
  - id: pin-activation
    resource: ../upeg-frb/src/api/pin_activation.rs
    title: 활성화 정책 구현
  - id: capability
    resource: ../upeg-core/src/capability.rs
    title: dispatch capability 판정
  - id: pwa-service-worker
    resource: ../flutter_app/web/upeg_service_worker.js
    title: PWA app shell 캐시 계약
  - id: approval-policy
    resource: ../upeg-runtime/src/approval.rs
    title: UI가 dispatch 전에 읽는 승인 정책
  - id: progress-sink
    resource: ../upeg-runtime/src/progress.rs
    title: 실행 중 출력의 선택적 통로
---

터미널 TUI, `flutter_app/` Desktop/PWA 빌드, Chrome extension popup/content script는 같은
Tool 라이프사이클의 surface다. 시각적 레이아웃은 매체마다 달라도 라벨, 라이프사이클 동사,
폼 의미론, 결과 상태 표현은 정렬되어 있어야 한다.

## Desktop Controlled Embed의 공통 실행과 디버그

Desktop은 앱 수준 WebView 세션 서비스가 Tool id마다 페이지를 소유한다. 카드나 디버그
모달은 브라우저 수명을 소유하지 않는다. 일반 실행과 디버그의 Run/Re-run은 비동기 Rust
dispatcher를 호출하고, dispatcher가 해석한 입력·bindings·설정을 FRB 요청 스트림으로
WebView 서비스에 전달한다. 이 앱의 내장 HTTP 호스트에 연결된 CLI도 같은 경로를 사용한다.

- 일반 실행과 디버그는 같은 runner의 binding별 대기·입력·trigger·결과 읽기를 사용한다.
  디버그는 같은 실행의 단계 이벤트와 selector 검사 결과를 관찰한다.
- DOM에서 읽은 원시 문자열은 Rust에서 선언한 output 타입·label·primary output으로
  정규화한다. 디버그의 마지막 결과에도 같은 정규화 결과와 오류를 적용한다.
- Debug를 여는 것만으로 Tool을 실행하거나 페이지를 다시 로드하지 않는다. 디버그 화면은
  기존 컨트롤러를 표시하고, 닫을 때 세션을 종료하지 않는다. 한 컨트롤러는 한 번에 한
  WebViewWidget에만 연결한다.
- 평소에는 앱의 숨긴 호스트가 설정한 viewport를 유지한다. 디버그 창 크기 때문에 실행
  viewport가 바뀌지 않도록 큰 페이지는 스크롤해서 표시한다.
- 같은 Tool의 요청은 순서대로 실행한다. 취소는 아직 시작하지 않은 후속 조작을 막지만,
  이미 실행한 클릭을 되돌리지는 않는다. 공급자 종료 시 다른 브라우저로 재실행하지 않는다.
- 세션은 앱 프로세스 수명 동안 유지되며 URL·브라우저 설정 변경 시 재생성한다. 실행 또는
  디버그 중에는 설정 변경으로 페이지를 교체하지 않는다. Tool별 페이지가 별도 계정의
  쿠키 저장소까지 격리한다는 계약은 아니다.

CLI에서 Desktop WebView를 공유하려면 Desktop의 `Local HTTP host`를 켜고 앱을 시작해야
한다. 이미 별도 CLI 호스트가 실행 중이면 기존 discovery 규칙상 그 호스트가 선택되며,
그 프로세스가 Desktop WebView를 중계하지는 않는다. 프로젝트 Tool의 로컬 실행 규칙도
유지된다. 이 범위의 공유 검증은 동일 사용자 toolkit의 Tool과 Desktop 내장 호스트를 대상으로 한다.

WebView 서비스는 Flutter 엔진과 네이티브 플랫폼 호스트를 필요로 한다. PWA 내부에서
네이티브 WebView를 생성하거나 디스플레이 없는 서버를 지원하는 기능은 아니다. 호스트가
없는 네이티브 CLI의 기존 headless 경로는 별도로 유지된다.

Linux 실환경 회귀 검증은 `just flutter-controlled-embed-linux-test`로 실행한다.
WebKitGTK와 Xvfb가 필요하며, 전용 임시 설정 루트에서 실제 WebView의 숨김/디버그 전환과
별도 CLI 프로세스의 HTTP 호출을 확인한다. 일반 통합 테스트에서는 CLI 검증용 환경변수가
없으면 그 사례를 건너뛴다.

이 문서의 경로와 아래 네 앵커(`#canonical-lifecycle-verbs`, `#chrome-extension-contract`,
`#display-anatomy`, `#approval-and-live-output`)는 interface inventory가 참조하고 존재를
검증한다.

## Canonical lifecycle verbs

| Verb | Meaning | Shared key binding |
| --- | --- | --- |
| Open / Inspect | Show detail, manifest, or modal without dispatching the tool. | `o` (board scope) on keyboard; right-click / long-press → the pin's context-menu "open" entry for mouse/touch (`GestureDetector.onSecondaryTapDown` / `onLongPressStart` in `pin.dart`). |
| Run | Dispatch the selected tool with current arguments. | `Enter` / `Space` from a board item; `F1` in every context that confirms/commits — detail/form/modal, BoardEditor, PinColorEditor, ToolPicker, ConfirmDelete, and the coordinate-move overlay. |
| Copy | Copy current output. | `F2` where an output-bearing modal exposes copy — TUI Result/Detail and the GUI's modal-local F2. |
| Pin | Add a tool to the current board/popup context. | `p` with a focused board item (modeless); `F3` in expanded modal contexts. |
| Back / Close | Leave the current view/modal one level. | `Esc`. `Esc` at the board root is a no-op. TUI `q` closes Detail/Result back to the list, and at the board root opens the quit-confirm dialog. |
| Search | Narrow discovery. | `/` or Cmd/Ctrl+`K`; text input keeps `/` as text and reserves Cmd/Ctrl+`K` for global search. |
| Clear input | Clear the focused text buffer/draft in one step. | `Ctrl`+`U` (TUI Form / BoardEditor / ToolPicker / PinColorEditor). |

`Enter` always means Run and never Open/Inspect, so the two intents cannot collide.

**헤드리스 대응.** GUI가 없는 호스트에서도 같은 동사를 쓸 수 있어야 한다. `Pin` /
`Coordinate move`에 대응하는 CLI 명령은 `upeg board <board> pin <tool> [--units U1|U2|U2T]
[--at <row>,<col>]`, `upeg board <board> unpin <tool>`, `upeg board <board> move <tool>
--at <row>,<col>` 이고, GUI 제스처와 **같은 store·같은 reconcile·같은 밀어내기 규칙**을
쓴다(`upeg_sources::pegboard`). 그래서 CLI로 만든 핀과 드래그로 만든 핀은 이후 구분되지
않는다. `--at`은 `<row>,<col>` 순서이며, 저장 좌표 `(x, y)`는 `(col, row)`다.

## Canonical Board Keys

| Key | Command |
| --- | --- |
| `←` `↓` `↑` `→` / `h` `j` `k` `l` | Move board focus |
| `b` / `t` | Cycle board / tag filters |
| `1`-`9` / `0` | Switch to board slot / clear board filter where the surface has an all-board state |
| `n` / `R` / `D` | New / rename / delete board — always live (modeless). `D` opens a delete-confirm; a single remaining board can't be deleted. |
| `a` | Open add-tool picker — always live; requires a concrete board (not the all-board state). |
| `p` / `c` | Toggle pin / edit pin color of the focused tool. Gated only on a focused pin. |
| `[` / `]` | Reorder the focused tool. Gated only on a focused pin. |
| `m` then arrows or `h` `j` `k` `l` | Coordinate move of the focused tool; `Enter` commits and `Esc` cancels |
| `e` then arrows or `h` `j` `k` `l` | Resize the focused tool. Gated only on a focused pin, symmetric to `m`. |
| `q` | Quit — opens the quit-confirm dialog |
| `?` | Keyboard cheatsheet — Desktop opens an overlay rendering the shared binding catalog (`upeg_core::binding_catalog`, the data the resolver is tested against, so the sheet can't drift); `Esc` closes it via the shared confirm scope. The TUI keeps its always-visible hint bar instead. |

## Canonical Scoped Keys

| Scope | Key | Command |
| --- | --- | --- |
| Detail / modal | `F1` / `Enter` / `r` | Run |
| Detail / modal | `F2` | Copy latest output when the modal has output (TUI Detail copies the tool id — there is no output yet at this point) |
| Detail / modal | `F3` | Pin current tool |
| Detail / modal | `Esc` / `q` | Close |
| TUI Result | `Enter` / `F1` / `Space` | Re-run the same tool; the previous result stays visible until replaced |
| TUI Result | `F2` | Copy: error message, else primary output text, else canonical JSON of all outputs |
| TUI Result | `Esc` / `q` | Close back to the tool list |
| TUI Result | anything else | Ignored — the result stays as-is |
| BoardEditor / PinColorEditor / ToolPicker / Moving / Resize | `F1` | Commit (alongside `Enter`) |
| Resize | `→` / `l`, `←` / `h` | Grow / shrink the pin one column (clamped to 1..=6 board columns) |
| Resize | `↓` / `j`, `↑` / `k` | Grow / shrink the pin one row (min 1; rows are unbounded) |
| Resize | `0` | Reset to the manifest footprint (drops the span override; follows the board scope's `0` = clear idiom) |
| Resize | `Enter` / `F1` | Commit the new span (growth pushes colliding pins row-major, same reflow as drag-drop) |
| Resize | `Esc` / `q` | Cancel and restore the original span |
| ConfirmDelete / ConfirmQuit | `y` / `Enter` / `F1` | Confirm; `n` / `q` / `Esc` cancel. One generic yes/no scope backs both dialogs. |
| Form | `↓` / `↑` / `Tab` / `Shift+Tab` | Move between fields |
| Settings | `j` / `k` / `Tab` / `Shift+Tab` | Move focus |
| Settings | `h` / `l` / arrows | Change the focused setting row |
| Filter bar / compact tabs | `←` / `→`, `h` / `l`, `Home` / `End` | Move board or tag selection within the focused strip |
| Tool picker / palette | `↑` / `↓`, `Enter`, `Esc` | Move selection, open selected result, cancel |
| Popup / popover menu | arrows or `h` `j` `k` `l`, `Enter`, `Esc` | Move selection, activate, close |

## Modeless invariants

There is no "edit mode" to enter before manipulating boards or pins.

- **Board-management keys are always live.** `n` / `R` / `D` / `a` resolve in board scope
  without any mode; their natural preconditions still apply.
- **Per-pin keys gate only on focus.** `p` / `c` / `m` / `e` / `[` / `]` require a focused
  pin — the single remaining contextual gate.
- **Drag is always available, always from the move handle.** The move handle is visible on
  every pin. Embed / ControlledEmbed pins are draggable **only** by their handle so the live
  body keeps receiving pointer/tap events.
- **Tap = Run.** A plain tap/click runs the pin; inspecting a manifest is the explicit
  `o` / right-click / modal path, never a side effect of tapping.
- **Embed bodies are always live.** There is no static-chip resting state.
- **Destructive and terminal actions sit behind confirm dialogs, not a mode.** Deleting a
  board opens a delete-confirm; quitting the TUI opens a quit-confirm (`q` at the board root).
  `Esc` at the board root does nothing.

## Activation policy (inline-first)

`upeg-frb/src/api/pin_activation.rs` centralises "what happens when a pin is activated"
(tap, `Enter`, or `Run`) so Rust decides once and both surfaces follow.

![핀 활성화 결정 — 모달은 세 경우에만 열린다](diagrams/pin-activation.drawio.svg)

- **Runnable pins with no required input dispatch immediately.** A runnable pin — `Inline`,
  `Action`, `Live`, or `Launcher` — whose input spec has zero required fields fires right
  away and renders the canonical result **inline in the pin body**. No modal, no navigation.
- **The expanded modal is reserved for:** the same pin kinds when they *do* have required
  inputs, `Chain`/`Llm` pins, and the explicit `o` / Open gesture on any pin — that gesture
  always inspects without dispatching.
- **Snackbars are for errors and unpinned results only.** A successful run of a pin already
  placed on the visible board writes its result inline and shows no toast. A snackbar appears
  when the run failed, or when the activated tool has no on-board placement to render into
  (palette hits, deep-linked or off-board tools).
- **Passive Embed** (`PinKind::Embed`) with a resolvable URL opens the dedicated full-screen
  `EmbedPage` when the tool has no on-board placement; if it is already pinned on the visible
  board, activation focuses the inline embed body instead of navigating away.

## Action pins and honest provider state

- **`memo.create`** is a metadata-declared action (`StaticSource::Shortcut`, chord
  `Cmd+Shift+N`) with no headless dispatcher: activating it creates a new memo and focuses the
  on-board notepad pin so the fresh memo is ready to type into.
- **`memo.scratch`** is a `Live` pin whose inline body is an always-live notepad backed by the
  same memo store.
- **Honest provider state.** A `Live` pin with an `Http` invoker and `Static` source that has
  no configured provider shows a "설정 필요" badge in place of a runnable affordance.
  Activating it yields a clear provider-not-configured message rather than a generic failure.
  The rule is keyed off invoker/source metadata, so any future "live http, no provider" tool
  is honest by construction.

## Expanded modal forms — bespoke qualification rule

`ExpandedModalPage` mounts a bespoke per-tool form when one is registered for the tool id,
else the generic input form. **A tool qualifies for a bespoke form iff it needs a live preview
— output that updates as the user types, with no run step.** One tool qualifies:
`num.hex_to_decimal` (decode-as-you-type). Everything else stays on the generic form.

The generic form carries the whole declared input contract, so nothing else needs a fork:
field `description` renders as helper text, `String(placeholder=…)` as the hint,
`Number`/`Integer`/`String` defaults seed the field, `Integer(min=…, max=…)` bounds and
`String(regex=…)` patterns validate client-side (Rust stays the enforcer), and choice options
render their `label` (falling back to the value) plus `description`. `Integer` is a distinct
field type from `Number`: integer keyboard, integer parsing, `1.5` rejected.

Non-reasons for a bespoke form, all supplied by the host around the generic form:
`F1` run and `F2` copy are bound for every tool; every result block carries a copy button; a
zero-input tool gets a Run button (the modal's primary button, the inline pin body's Run
button) instead of a dead-end "no inputs" panel.

### `File` 입력 — 고르기와 끌어 놓기는 같은 곳으로 수렴한다

`File` 필드는 위 목록에서 빠진 유일한 종류다. 다른 종류와 달리 값을 얻는 경로가 둘이고,
둘의 결과가 같아야 하기 때문에 계약을 따로 적는다.

| 경로 | 어떻게 |
|---|---|
| 고르기 | `Choose file` 버튼이 플랫폼 파일 대화상자를 연다 |
| 끌어 놓기 | 필드 **전체**가 드롭 영역이다. 파일이 들어오면 테두리와 배경이 강조되고, 나가면 되돌아간다 |

**두 경로는 같은 조립 단계로 수렴한다.** 정책 검증(허용 확장자, 개수, 파일별·전체 크기)이
그 단계에 있으므로, 드롭이 픽커보다 느슨할 수 없다. 어느 경로든 읽기는 상한이 정해진
창으로만 스트리밍하고, 열기 전에 크기를 미리 확인한다.

빈 상태에서 필드는 두 경로를 함께 안내한다("파일을 선택하거나 여기로 끌어 놓으세요").
`File` 값은 파일 시스템 경로가 아니라 바이트로 실려 가므로, 이 경로 전체가 브라우저
빌드에서도 그대로 돈다.

**다중 선택은 합성 디렉터리가 된다.** `max_count > 1`인 필드는 고른 파일들을 이름 하나
아래의 `directory` 항목 목록으로 묶어 보낸다. 모양은 고른 개수가 아니라 **정책**이
정한다 — `max_count == 1`이면 파일 하나가 그대로 `bytes`가 되고, `max_count > 1`이면
한 개를 골라도 디렉터리로 감싸인다.

정직하게 적어 두는 어긋남과 미구현:

- **실제 디렉터리 선택은 지원하지 않는다.** 위의 `directory`는 여러 파일을 담는 합성
  컨테이너이고, 폴더를 끌어다 놓으면 거부된다.
- **`FilePath`는 `File`이 아니다.** 경로 문자열을 받는 별개 종류이고, 텍스트 필드와 폴더
  버튼만 있으며 드롭 영역이 아니다.
- **Chrome 확장에는 끌어 놓기가 없다.** 확장은 native `<input type="file">`만 쓴다.
  정책 검사는 확장에도 있지만 드롭 경로 자체가 없다.
- **필드 UI가 정책을 선택 전에 별도로 안내하지 않는다.** 허용 확장자·개수·크기 상한을
  설명하는 문구는 없고 `description`만 보여 준다. 선택기는 이미 Flutter의
  `allowedExtensions`와 Chrome 확장의 `accept`로 확장자 필터를 받으며, 정책 위반은
  선택 후 오류 문구로도 드러난다.

## Implementation Boundary

- GUI keyboard policy resolves through the shared Rust scope contract wherever a command exists.
- Flutter-local shortcut handling is reserved for platform/widget activation surfaces with no
  shared command yet: context-menu key opening, output copy, modal pin, embed reload.
- Widget tests use one shared fake keyboard resolver so fixtures do not grow independent
  shortcut tables.

## Embed focus contract

What the Flutter surface enforces once primary focus is inside an editable text field, a View
Embed webview/platform view body, or a ControlledEmbed cockpit form:

- **All keys route to the focused field/body while it holds focus** — not just characters and
  digits, but arrow keys, `Home`/`End`, `PageUp`/`PageDown`, `Backspace`, and `Delete` too, so
  the caret/scroll moves instead of the board paging. The only exceptions are `Cmd`/`Ctrl`
  chords, the scoped `F1`-`F4` commands, and `Esc`.
- **`Tab` performs ordinary focus traversal within the focused subtree**
  (`FocusScope.nextFocus`/`previousFocus`) rather than resolving to a board command.
- **`Esc` is two-stage** inside an embed body or ControlledEmbed form: the first `Esc` is
  focus-return only and dispatches nothing. A second `Esc`, now with board focus, runs the
  normal Back/Close behavior.
- **Passive-embed gate.** Platform webviews frequently don't propagate DOM focus into the
  Flutter focus tree. Whenever the focused pin's tool metadata reports an
  `Embed`/`ControlledEmbed` pin kind, the key-yield rule above applies regardless of what the
  focus tree reports — the gate is keyed off pin metadata, not DOM focus.
- The hidden ControlledEmbed engine — the off-canvas webview that executes the selector
  bindings — must never receive pointer, semantics, or keyboard focus.

## Chrome extension contract

The extension's reason to exist is the page the user is already on. Anything the popup could
only mirror from another pegboard renderer is not where this surface invests
(백로그).

### Activation routes (`chrome-ext/tool_routing.js`)

`activationRouteFor(tool, { inPageAvailable })` is a pure function of the tool's own metadata
plus one bit of context, and it is closed over exactly three destinations:

| Route | When | What happens |
|---|---|---|
| `IN_PAGE` | pin kind is `ControlledEmbed`, it declares selector bindings, and the active tab is enabled with a live content script | the tool's `SelectorBinding` rows are applied to that tab |
| `DISPATCH` | any other invoker | `POST /v1/tools/{id}` with the pasted bearer token, result rendered inline (immediately when the input schema has no properties, else through an inline form first) |
| `DEEP_LINK` | invoker `static`, or a Controlled Embed with no page to drive | `upeg://open?surface=ext&board=<board>&tool=<tool>` |

`IN_PAGE` is the extension's own answer to Controlled Embed: Desktop drives a webview it owns,
the extension drives the tab. The DOM semantics are the same on both sides — write `.value`
then fire `input`/`change`, click-or-Enter the first declared trigger, read `.value` else
`.textContent` — because `chrome-ext/selector_adapter.js` mirrors
`upeg_runtime::selector_pipeline`.

- When the host is unreachable or the pasted token is rejected, the popup shows a single
  explanatory message with a generic "Open upeg" deep-link fallback. It does **not** silently
  fall back to the per-tool deep link for `DISPATCH` tools; that path is reserved for
  `DEEP_LINK` tools regardless of host reachability.
- An `IN_PAGE` run whose tab turns out to have no content script (the tab predates the
  enablement) reports that inline; it never silently does nothing.

### Per-site enablement (`chrome-ext/site_access.js`)

Which pages the in-page capabilities run on is **stored state, not a manifest constant**. The
manifest asks for `<all_urls>` only as an `optional_host_permissions` entry; the popup's
"Enable on this site" toggle requests the current origin's host permission on demand, adds its
match pattern to `chrome.storage.local`, and the service worker compiles the allow-list into
one `chrome.scripting.registerContentScripts` registration.

- **Only granted patterns are ever registered.** The permission can disappear behind the
  extension's back (the user revokes it in `chrome://extensions`), so every sync filters the
  allow-list through `chrome.permissions.contains` first.
- **etherscan/polygonscan stay pre-enabled.** Their four patterns remain required
  `host_permissions` and are seeded into the allow-list on install, so the migration off the
  hard-coded `content_scripts` block costs a user nothing.
- **Disabling a URL drops every pattern that covers it**, not just the exact one the toggle
  would add — otherwise a seeded `https://*.etherscan.io/*` would keep a subdomain enabled and
  the toggle would look inert.
- Enabling injects into the current tab immediately (`chrome.scripting.executeScript`), because
  a dynamic registration only affects future loads.

### In-page detectors (`chrome-ext/detectors.js`)

Detection is a frozen **table**, not code: a row is a pattern, a label key, an optional host
tool plus the argument name that receives the match, and an optional offline preview. Nothing
else in the extension branches on which detector fired, and adding a detector means adding a
row.

| Row | Host tool | Offline preview |
|---|---|---|
| `hex` (`0x…`) | `num.hex_to_decimal` (`input`) | decimal via `BigInt` |
| `base64` (padded RFC 4648, ≥16 chars) | `convert.base64_decode` (`input`) | none — decoding means validating UTF-8, which is what the tool is for |
| `epoch` (10- or 13-digit) | none | ISO 8601 UTC |

- Matches are wrapped in a `.upeg-mark` span carrying `data-upeg-detector` and a
  `data-upeg-tip` tooltip. A row with a host tool is actionable (`role=button`, `tabindex=0`)
  and resolves **lazily on first hover/focus**; the tooltip then shows the tool's own result.
  With no host reachable it falls back to the offline preview plus the Desktop deep link
  (`upeg://open?surface=ext&board=dev&tool=<tool>&input=<value>`).
- **A row with no host tool is an annotation, not a link.** `epoch` has no upeg tool that
  converts an epoch (`time.epoch_now`/`time.iso_now` take no input), so it renders its preview
  and carries no click affordance — a dead deep link would be worse than plain text.
- Overlaps resolve by row order, so `0xDEADBEEF…` is never read as a base64 blob.

### Trust boundary

A content script's `fetch` is subject to the **page's** CORS, not the extension's host
permissions, so the content script never calls the host and never reads the token. Every host
request is a `upeg:dispatch-tool` message the service worker performs
(`chrome-ext/background.js`), which keeps the bearer token out of every web page's world.

Extension JavaScript stays dependency-free; rich execution remains in Desktop/PWA/host
surfaces.

## PWA offline contract

`flutter_app/`의 웹 빌드는 오프라인에서도 떠야 하는 PWA다. 계약을 지키는 파일은 둘이다.

- `flutter_app/web/upeg_service_worker.js` — app shell 캐시(`upeg-app-shell-<릴리스 버전>`)를
  소유한다. 설치 때 문서 · `flutter_bootstrap.js` · `manifest.json`을 프리캐시하고, navigation
  요청은 network-first(실패하면 캐시된 문서), 나머지 same-origin GET은 stale-while-revalidate로
  응답한다. 버전은 등록 URL의 `?v=`로 들어오고 `activate`가 이름이 다른 옛 캐시를 지우므로,
  셸 캐시는 언제나 하나이며 릴리스가 바뀌면 통째로 새로 난다.
- `flutter_app/web/flutter_bootstrap.js` — 그 SW를 등록하고 **SW가 이 페이지를 제어할 때까지
  기다린 뒤** 앱을 띄운다. 그래야 첫 로드가 받아오는 셸(`main.dart.js` · CanvasKit ·
  `pkg/upeg_frb*`)이 전부 캐시에 들어간다. CanvasKit도 gstatic CDN이 아니라 우리 origin에서
  받는다(`canvasKitBaseUrl`) — 남의 origin에 있는 렌더러는 캐시할 수 없고, 캐시할 수 없는
  렌더러는 오프라인 부팅을 불가능하게 만든다. 등록은 릴리스 빌드에서만 한다: Flutter가
  `flutter run -d chrome`의 dev 서버에서 `{{flutter_service_worker_version}}`을 `null`로 채우고,
  그 신호를 그대로 써서 dev 루프가 캐시된 옛 코드를 되돌려받지 않게 한다.

Flutter가 생성하는 `flutter_service_worker.js`는 **등록하지 않는다.** 3.29부터 그 파일은 캐싱을
버리고 자기 자신을 unregister 하는 청소용 SW가 되었고, `flutter build web`에는 `--pwa-strategy`
플래그조차 없다. 오프라인 계약을 유지하려면 SW를 우리가 소유하는 수밖에 없다.

검증은 `just flutter-web-smoke`(`scripts/flutter_web_smoke_check.mjs`)다. 헤드리스 Chromium을
CDP로 몰면서 (1) 앱 셸이 뜨는지, (2) `navigator.serviceWorker.ready`가 `upeg_service_worker.js`를
activated로 돌려주고 그 SW가 페이지를 제어하는지, (3) app shell 캐시에 부팅 셸이 다 들어 있는지,
(4) **정적 서버를 내린 뒤** reload 해도 캐시에서 뜨는지를 단언한다. Chromium이 없으면 건너뛰지
않고 실패한다 — 없는 검증을 초록으로 보이게 하지 않기 위해서다.

## Capability contract (define once, render honestly)

`upeg-core/src/capability.rs` is the single place that decides whether a Tool can dispatch
**in-process** on the current surface. `dispatch_capability(surface, invoker, host)` is a
closed function of `Invoker` × `RuntimeHost`: `Native` links `upeg-loader` so every invoker
runs; `Wasm` (the Flutter-web/PWA `wasm32` build) links none of it.

`dispatch_capability_for_tool` layers on a per-tool dispatcher probe for the one case the
invoker table cannot see: a `Function` tool whose dispatcher is itself compiled out on this
host (`net.status`, `eth.gas`, `eth.address_lookup`).

- **The probe is keyed on `Invoker::Function`, not trigger `Source`.** Any Function tool —
  `Timer`-sourced or not — produces its value by running a dispatcher, so `eth.gas`
  (a `Launcher`/`UserInput` Function tool) is correctly `NativeOnlyTool` on wasm32. Pure
  Function tools whose dispatcher compiles everywhere (`num.*`, `csv.*`, `qr.encode`) stay
  `Supported`.
- The verdict is a closed enum, never a string: `DispatchCapability::Supported` or
  `Unsupported(UnsupportedReason)`, where `UnsupportedReason` is exactly `NoProcessSpawn`
  (`External` needs a subprocess), `NoLoaderRuntime` (`Http`/`Chain`/`Llm`/`Embed` need
  `upeg-loader`), `NoWasmHost` (`Wasm` needs the extism host), or `NativeOnlyTool` (a
  `Function` tool whose dispatcher needs native-only services, not a loader gap).
- **Surfaces render `Unsupported` as an honest notice, never a runnable affordance.**
  `SurfaceUnsupportedBody` replaces only the pin's rendered body with a static "이 표면에서는
  미지원" body — the pin's declaration stays visible and tappable-to-inspect, never hidden and
  never wired to dispatch. The same pattern backs `ProviderNotConfiguredBody` and the
  controlled-embed inline notice.
- `gui_meta`/built-in `surfaces` declarations are audited against this table so a tool never
  claims a surface it cannot run on without explanation.

## Host-attach contract (browser surfaces pairing with a local host)

PWA and chrome-ext are `wasm32`/sandboxed hosts, so **all four** `Unsupported` reasons are
solvable by pairing with a native host: that host links the full loader
(subprocess/http/chain/llm/wasm-plugin) *and* every native-gated `Function` dispatcher
(net/media/eth). `hostAttachCanSolve()` returns `true` for all four.

- **Pairing** is a host address + bearer token entered once in Settings
  (`flutter_app/lib/src/features/host_attach/`); `HostAttachConfig.isConfigured` is simply "a
  non-empty base URL is set". There is no discovery handshake — the token is generated by the
  host and pasted manually, same as the chrome-ext popup's token field. The CLI pairing aid
  (`upeg http status --pairing`) exists so this does not require hand-copying an ephemeral port
  and token; see [호스트 토폴로지](/architecture/host-topology.md).
- When a pin's capability is `Unsupported` and a host is paired, the board routes the tap
  through `POST /v1/tools/{id}` on the paired host (`HttpAttachClient.dispatch`, 8s timeout)
  and renders the remote result exactly like an in-process result. A failed remote attempt
  keeps the pin tappable-to-retry rather than erroring silently.
- When unsupported and no host is paired, the board falls back to `SurfaceUnsupportedBody`
  with a "pair a host" hint.
- If the paired host reports the tool isn't runnable there, the pin shows
  `HostAttachNoticeBody` carrying the host's own error message. The board never hides a
  paired-but-failing attempt behind a generic "unsupported" badge.

CORS, bearer auth, and `/healthz` exposure rules live in [HTTP API](/architecture/http-api.md).

## Approval and live output

실행은 두 지점에서 사람과 만난다. **실행 전**에는 승인 장벽이 있고, **실행 중**에는 살아 있는
출력이 있다. 표면마다 매체는 달라도 두 계약은 같다.

### 승인 제스처

`requires_approval` step을 가진 Chain은 dispatch 전에 사람의 확인을 요구한다. UI는 dispatch
**전에** `ToolMeta::requires_approval` / `approval_surfaces`(FRB `ToolDto`의 `requiresApproval` /
`approvalSurfaces`)로 "확인을 띄워야 하는가"와 "내 표면의 확인이 인정되는가"를 묻고, 확인을 받은
뒤에만 `approve`를 싣는다 — 근거는 [Chain Tool](/architecture/chain.md)의 표면 게이트다.

| Surface | 제스처 | 이 표면의 승인이 인정되지 않을 때 |
| --- | --- | --- |
| CLI | `upeg call <chain> -a approve=true` | 거부 메시지가 인정되는 surface를 나열한다 |
| TUI | 실행 시 확인 대화상자 — `Enter`/`F1`/`y` 승인, `Esc`/`n`/`q` 취소 | 프롬프트 대신 result pane이 이유와 인정되는 surface 목록을 적는다 |
| Desktop | 실행 전 확인 다이얼로그 — 승인 후 실행 / 취소 | 다이얼로그가 승인 대신 이유와 인정되는 surface를 설명하고 dispatch하지 않는다 |
| MCP · HTTP · PWA · Ext | 없음 | 매니페스트가 `approval_surfaces`로 그 표면의 이름을 적어야 한다 |

**예약 키는 확인이 만들지, 폼이 만들지 않는다.** TUI는 확인 대화상자의 "예"에서만
`approve`를 args에 얹고, desktop은 `approve`를 typed FRB 파라미터로 받아 Rust가 얹는다 —
Dart가 조립한 args도, 핀에 저장된 args preset도 예약 키를 스스로 채워 승인을 자칭할 수 없다
(Rust가 호출자의 `approve` 키를 먼저 지운다). 확인을 우회하는 경로가 UI 안에 남지 않게 하는
것이 이 계약의 요점이다.

### 살아 있는 출력

`External` invoker처럼 점진적으로 출력을 내는 Tool은 실행 중 chunk를 흘린다
(`upeg_runtime::with_progress_sink`). 최종 봉투 하나가 여전히 계약이고 살아 있는 출력은 그 위에
얹은 **선택적인 절반**이다 — 아무도 sink를 설치하지 않으면 invoker는 전달 작업을 아예 건너뛴다.

| Surface | 실행 중 표시 | 취소 |
| --- | --- | --- |
| CLI | 사람이 읽는 모드에서 stderr로 그대로 미러링(`--json`/`--field`는 침묵) | `Ctrl+C` |
| TUI | result pane에 마지막 8줄 tail. dispatch는 worker thread에서 돌기 때문에 UI가 멈추지 않는다. host에 attach한 세션도 같다 — `POST /v1/tools/{id}/stream`을 타고 chunk가 그대로 들어온다 | 실행 중 `Esc`. attach 상태에서는 응답 본문을 끊는 것이 곧 취소 신호다 |
| Desktop | 확장 모달은 마지막 8줄, inline 핀은 마지막 3줄 tail(monospace) | Cancel 버튼 |
| HTTP | NDJSON 스트림 | 응답 본문 drop |
| MCP | 로그 notification | — |

**취소는 요청이지 보장이 아니다**(`upeg_runtime::CancellationToken`). 존중하지 않는 Tool은
끝까지 달리고, 그래도 모든 표면이 렌더하는 최종 봉투 하나로 끝난다. 그래서 TUI는 실행 중
`Esc`를 두 단계로 받는다 — 한 번은 취소 요청("취소 중…"), 두 번째는 [Modeless
invariants](#modeless-invariants)의 quit-confirm overlay를 연다. 토큰을 끝내 보지 않는
invoker 앞에서도 사람에게 나갈 문이 남아야 하지만, 반사적인 `Esc` 두 번이 세션을 떨어뜨려서도
안 되기 때문이다.

**실행 중 화면을 떠나도 실행은 계속된다 — 다만 두 표면이 잃는 것이 다르다.**

TUI에서 quit-confirm을 열었다가 물러나면 tail은 사라지지만 **실행은 모델이 계속 붙들고 있다**
(`State::active_run`). 그래서 그 사이에 다른 Tool을 Run하면 새 실행이 시작되는 대신 "무엇이
실행 중인지"를 말하고 그 실행의 pane으로 돌려보낸다 — 거기서 `Esc`는 여전히 취소다. 돌아간
pane의 tail은 비어 있다: 떠날 때 버린 줄을 지어낼 수는 없다.

desktop에서 확장 모달을 닫으면 그렇지 않다. `run_id`는 모달 위젯의 state에 살기 때문에 모달과
함께 사라지고, 그 순간부터 **그 실행은 취소할 수 없고 최종 봉투도 아무 데도 렌더되지 않는다**.
실행 자체는 끝까지 달리고 실행 로그에는 남는다. 이것을 고치려면 실행 상태를 모달 밖(provider)
으로 끌어올려야 하고, 그건 남은 일 대장의 항목이다.

## Display anatomy

1. Primary label: `upeg_core::ux::display_label_for(tool.id)`.
2. Secondary metadata: raw technical id (`upeg_core::ux::technical_id`) where space permits.
3. Detail/form metadata: toolkit, effective tags, pin kind, invoker, surfaces, inputs, and
   embed metadata remain visible through surface-appropriate layouts.

## Result semantics

Both surfaces use `upeg_core::ux::result_status_label`: `OK` for successful output, `ERROR`
for tool/dispatch errors.

TUI Result view lifecycle: `Enter`/`F1`/`Space` re-run the same tool rather than dismissing —
the prior result stays on screen until the new outcome replaces it. `Esc`/`q` closes back to
the tool list. Every other key is ignored. `F2` copies with the same fallback precedence the
GUI's copy affordance uses (`CanonicalToolResultView`): the error message if the run failed,
else the primary output's display text, else a canonical JSON dump of all outputs.

Desktop board/pin/theme features are wrappers around this shared lifecycle, not a separate
product model.
