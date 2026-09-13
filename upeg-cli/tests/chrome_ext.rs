#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::manual_let_else,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Integration tests for the root Chrome extension fixtures.
//!
//! The extension is a root product surface; it was never part of the
//! retired Dioxus `desktop-ui` crate. The tests live in `upeg-cli` so the
//! Phase 11 Dioxus removal cost no manifest/content-script coverage, and
//! they guard against the old bundle wiring creeping back in.

use serde_json::Value;
use std::fs;
// Linking the CLI lib pulls `upeg-tools` in with it, which is what puts the
// `#[upeg::tool]` inventory submissions into this test binary. Without the
// edge the toolbox looks empty and the detector-table pin below would pass
// vacuously (it asserts the extraction found rows, so it would fail loudly —
// but the honest fix is to link, not to weaken the pin).
use upeg_cli as _;

mod chrome_ext_support;
use chrome_ext_support::{
    BACKGROUND_JS, BUILD_SH, CONTENT_JS, DETECTORS_JS, HOST_API_JS, MANIFEST_JSON, POPUP_HTML,
    POPUP_JS, SELECTOR_ADAPTER_JS, SITE_ACCESS_JS, TOOL_ROUTING_JS, ext_path, manifest, read,
};

const OPEN_UPEG_LINK: &str = "upeg://open?surface=ext";
const DIOXUS_BUNDLE_ENTRY: &str = "upeg-desktop-ui.js";
const DIOXUS_BUILD_COMMAND: &str = "dx build";
const DIOXUS_PACKAGE: &str = "upeg-desktop-ui";
const DIOXUS_WEB_PLATFORM_FLAG: &str = "--platform web";

// Permission shape. `storage` holds the token + the enabled-site list,
// `scripting` registers the content script dynamically, `activeTab` lets the
// popup read the URL of the tab it was opened over. Host permissions cover
// exactly the loopback daemon plus the two explorers the extension used to
// hard-code in `content_scripts`; everything else is OPTIONAL and granted
// per site by the popup's toggle.
const EXPECTED_PERMISSIONS: [&str; 3] = ["storage", "scripting", "activeTab"];
const DAEMON_HOST_PERMISSION: &str = "http://127.0.0.1:7173/*";
const SEED_HOST_PERMISSIONS: [&str; 4] = [
    "https://etherscan.io/*",
    "https://*.etherscan.io/*",
    "https://polygonscan.com/*",
    "https://*.polygonscan.com/*",
];
const OPTIONAL_HOST_PERMISSION: &str = "<all_urls>";
const HTTPS_SCHEME_PREFIX: &str = "https://";
// `chrome-ext/site_access.js` literals the pins below read back out.
const SEED_PATTERNS_DECL: &str = "const SEED_PATTERNS = Object.freeze([";
const CONTENT_SCRIPT_FILES_DECL: &str = "const CONTENT_SCRIPT_FILES = Object.freeze([";
// `chrome-ext/content.js` invariants the pins below read back out: the
// once-per-frame sentinel and the single skipped-context predicate.
const CONTENT_SCRIPT_SENTINEL_DECL: &str =
    "const CONTENT_SCRIPT_SENTINEL = 'upegContentScriptActive';";
const CONTENT_HELPER_DESTRUCTURE: &str = "} = UpegHostApi;";
const SKIPPED_CONTEXT_DECL: &str = "function isSkippedContext(node)";
const SKIPPED_CONTEXT_GUARD: &str = "if (isSkippedContext(";
const SKIP_TAGS_LOOKUP: &str = "SKIP_TAGS.has(";
// `chrome-ext/detectors.js` row literals: `id: '<tool>', arg: '<field>'`.
const DETECTOR_TOOL_ID_PREFIX: &str = "id: '";
const DETECTOR_TOOL_ARG_PREFIX: &str = "arg: '";

#[test]
fn manifest는_유효한_json으로_파싱된다() {
    let _: Value = manifest();
}

#[test]
fn manifest는_v3이다() {
    let v = manifest();
    assert_eq!(
        v["manifest_version"], 3,
        "Chrome MV2 was deprecated; require V3"
    );
}

#[test]
fn manifest는_필수_필드를_가진다() {
    let v = manifest();
    for field in ["name", "version", "description"] {
        assert!(v[field].is_string(), "missing required field `{field}`");
        assert!(
            !v[field].as_str().unwrap().is_empty(),
            "field `{field}` empty"
        );
    }
}

#[test]
fn manifest의_액션은_popup_html을_가리킨다() {
    let v = manifest();
    assert_eq!(
        v["action"]["default_popup"], POPUP_HTML,
        "popup file must match the actual popup.html we ship",
    );
}

fn string_array(value: &Value, field: &str) -> Vec<String> {
    value[field]
        .as_array()
        .unwrap_or_else(|| panic!("`{field}` must be an array"))
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("`{field}` entries must be strings"))
                .to_string()
        })
        .collect()
}

#[test]
fn manifest_권한은_선언된_최소_집합을_유지한다() {
    let v = manifest();
    assert_eq!(
        string_array(&v, "permissions"),
        EXPECTED_PERMISSIONS.map(str::to_string).to_vec(),
        "permissions must stay pinned to storage + dynamic registration + active tab"
    );

    let mut expected_hosts = vec![DAEMON_HOST_PERMISSION.to_string()];
    expected_hosts.extend(SEED_HOST_PERMISSIONS.map(str::to_string));
    assert_eq!(
        string_array(&v, "host_permissions"),
        expected_hosts,
        "host_permissions must stay pinned to the loopback daemon plus the seeded explorers"
    );
}

#[test]
fn manifest는_다른_사이트_접근을_선택_권한으로만_요구한다() {
    // The whole point of per-site enablement: broad host access is granted
    // by the user per origin at runtime, never demanded at install time.
    let v = manifest();
    assert_eq!(
        string_array(&v, "optional_host_permissions"),
        vec![OPTIONAL_HOST_PERMISSION.to_string()],
        "any site beyond the seeds must arrive through an optional permission"
    );
    assert!(
        !string_array(&v, "host_permissions")
            .iter()
            .any(|host| host == OPTIONAL_HOST_PERMISSION),
        "`{OPTIONAL_HOST_PERMISSION}` must never be a REQUIRED host permission"
    );
}

#[test]
fn manifest는_서비스_워커를_선언한다() {
    let v = manifest();
    assert_eq!(
        v["background"]["service_worker"], BACKGROUND_JS,
        "the worker owns dynamic registration and every host fetch"
    );
}

#[test]
fn popup_html은_desktop_딥_링크_진입점을_제공한다() {
    let html = read(POPUP_HTML);
    assert!(
        html.contains(OPEN_UPEG_LINK),
        "popup.html must expose `{OPEN_UPEG_LINK}` so the static MV3 popup opens the desktop app",
    );
    assert!(
        !html.contains(DIOXUS_BUNDLE_ENTRY),
        "popup.html must not load the old Dioxus bundle entry"
    );
}

#[test]
fn 빌드_스크립트는_executable이다() {
    let path = ext_path(BUILD_SH);
    let meta = fs::metadata(&path).unwrap_or_else(|e| panic!("{} stat: {e}", path.display()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode() & 0o111;
        assert!(mode != 0, "build.sh must have +x (chmod a+x)");
    }
    #[cfg(not(unix))]
    {
        let len = meta.len();
        assert!(len > 0, "build.sh must not be empty");
    }
}

#[test]
fn 빌드_스크립트는_dioxus를_호출하지_않는다() {
    let raw = read(BUILD_SH);
    for marker in [
        DIOXUS_BUILD_COMMAND,
        DIOXUS_PACKAGE,
        DIOXUS_WEB_PLATFORM_FLAG,
    ] {
        assert!(
            !raw.contains(marker),
            "build.sh must stage static extension files without `{marker}`"
        );
    }
}

/// Single-quoted string literals inside the array literal that follows
/// `decl` — the shape both `SEED_PATTERNS` and `CONTENT_SCRIPT_FILES` use
/// in site_access.js.
fn js_string_array_after(source: &str, decl: &str) -> Vec<String> {
    let tail = source
        .split_once(decl)
        .unwrap_or_else(|| panic!("site_access.js must declare `{decl}`"))
        .1;
    let body = tail
        .split_once("]);")
        .unwrap_or_else(|| panic!("`{decl}` must close with `]);`"))
        .0;
    body.split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

#[test]
fn manifest는_정적_content_scripts_블록을_더는_갖지_않는다() {
    // Per-site enablement replaced it: which sites get the content script
    // is stored state compiled into a `chrome.scripting` registration, not
    // a hard-coded match list that needs a release to change.
    let v = manifest();
    assert!(
        v.get("content_scripts").is_none(),
        "a static content_scripts block would silently outrank the dynamic registration"
    );
}

#[test]
fn 동적_등록은_content_js와_그_의존_모듈을_모두_싣는다() {
    let js = read(SITE_ACCESS_JS);
    let files = js_string_array_after(&js, CONTENT_SCRIPT_FILES_DECL);
    for required in [
        "wire.js",
        HOST_API_JS,
        DETECTORS_JS,
        SELECTOR_ADAPTER_JS,
        CONTENT_JS,
    ] {
        assert!(
            files.iter().any(|file| file == required),
            "the content-script registration must load `{required}`, got {files:?}"
        );
    }
    assert_eq!(
        files.last().map(String::as_str),
        Some(CONTENT_JS),
        "content.js reads the helpers above it, so it must load last"
    );
}

#[test]
fn 시드된_사이트는_https에서만_일치한다() {
    let js = read(SITE_ACCESS_JS);
    let seeds = js_string_array_after(&js, SEED_PATTERNS_DECL);
    assert!(!seeds.is_empty(), "at least one seed pattern required");
    for seed in &seeds {
        assert!(
            seed.starts_with(HTTPS_SCHEME_PREFIX),
            "seed pattern `{seed}` must be HTTPS-only"
        );
    }
    // The seeds must be exactly what install-time `host_permissions` grants,
    // or the migration would ship a pre-enabled site Chrome refuses to inject.
    assert_eq!(
        seeds,
        SEED_HOST_PERMISSIONS.map(str::to_string).to_vec(),
        "the seeded allow-list and the granted host permissions must be the same set"
    );
}

#[test]
fn 빌드_스크립트는_content_script가_등록하는_모든_파일을_스테이징한다() {
    let build = read(BUILD_SH);
    let mut files = js_string_array_after(&read(SITE_ACCESS_JS), CONTENT_SCRIPT_FILES_DECL);
    files.push(BACKGROUND_JS.to_string());
    files.push(SITE_ACCESS_JS.to_string());
    files.push(TOOL_ROUTING_JS.to_string());
    for file in files {
        assert!(
            build.contains(&file),
            "build.sh must stage `{file}` into dist/ or the loaded extension breaks at runtime"
        );
    }
}

#[test]
fn 콘텐츠_js는_감지_규칙을_직접_들고_있지_않는다() {
    // Detection is DATA (detectors.js). A pattern literal creeping back into
    // content.js would mean the table stopped being the single source.
    let js = read(CONTENT_JS);
    for smell in ["0[xX]", "HEX_RE", "new RegExp("] {
        assert!(
            !js.contains(smell),
            "content.js must not carry detection rule `{smell}` — detectors.js owns them"
        );
    }
    for marker in [
        "UpegDetectors",
        "const detections = detect(text);",
        "MutationObserver",
        "upeg-mark",
    ] {
        assert!(
            js.contains(marker),
            "content.js must keep in-page wiring marker `{marker}`"
        );
    }
}

#[test]
fn 콘텐츠_js는_한_프레임에_한_번만_사는_표식을_남긴다() {
    // The script arrives by two paths — the dynamic
    // `registerContentScripts` registration (future loads) and the popup's
    // `executeScript` (the tab in front of the user). A second evaluation
    // in the same frame is therefore normal, and without a sentinel it adds
    // a second `chrome.runtime.onMessage` listener and a second
    // MutationObserver over the same DOM.
    //
    // Behaviour is covered by chrome-ext/tests/content.test.js (it evaluates
    // the file twice in a fake frame); this pin keeps the guard at the TOP
    // of the IIFE, before the file touches any helper global.
    let js = read(CONTENT_JS);
    for marker in [
        CONTENT_SCRIPT_SENTINEL_DECL,
        "if (window[CONTENT_SCRIPT_SENTINEL]) return;",
        "window[CONTENT_SCRIPT_SENTINEL] = true;",
    ] {
        assert!(
            js.contains(marker),
            "content.js must keep the single-injection sentinel `{marker}`"
        );
    }
    let sentinel_at = js
        .find(CONTENT_SCRIPT_SENTINEL_DECL)
        .expect("sentinel declaration found above");
    let helpers_at = js
        .find(CONTENT_HELPER_DESTRUCTURE)
        .expect("content.js destructures UpegHostApi");
    assert!(
        sentinel_at < helpers_at,
        "the sentinel must be checked before content.js reads any helper global"
    );
}

#[test]
fn 콘텐츠_js의_건너뛸_문맥_판정은_한_곳에만_있다() {
    // The tree walk and the MutationObserver both decide "may this text node
    // be rewritten?". When each carried its own answer the observer's
    // text-node path had none at all and rewrote inside <textarea>/<code>.
    // One predicate, both callers.
    let js = read(CONTENT_JS);
    assert!(
        js.contains(SKIPPED_CONTEXT_DECL),
        "content.js must declare the shared `{SKIPPED_CONTEXT_DECL}`"
    );
    assert_eq!(
        js.matches(SKIPPED_CONTEXT_GUARD).count(),
        2,
        "`{SKIPPED_CONTEXT_GUARD}` must guard BOTH the tree walk and the mutation \
         path — no path may be left without one"
    );
    assert_eq!(
        js.matches(SKIP_TAGS_LOOKUP).count(),
        1,
        "`{SKIP_TAGS_LOOKUP}` outside `{SKIPPED_CONTEXT_DECL}` means a second copy of the rule"
    );
}

#[test]
fn popup_js는_이미_돌고_있는_content_script를_다시_주입하지_않는다() {
    // `executeScript` on a tab that already runs the content script is what
    // duplicates its listener/observer. The popup asks first.
    let js = read(POPUP_JS);
    for marker in [
        "async function ensureContentScript()",
        "if (await pingContentScript()) return true;",
        "const running = await ensureContentScript();",
    ] {
        assert!(
            js.contains(marker),
            "popup.js must keep the idempotent-injection marker `{marker}`"
        );
    }
    let ensure_body = js
        .split_once("async function ensureContentScript()")
        .expect("ensureContentScript found above")
        .1
        .split_once("\n  }")
        .expect("ensureContentScript closes")
        .0;
    assert!(
        ensure_body.find("pingContentScript()") < ensure_body.find("injectContentScript()"),
        "the ping must come BEFORE the injection, or the guard is decorative"
    );
}

#[test]
fn 감지기_표는_행마다_패턴과_라벨_키와_도구를_데이터로_선언한다() {
    let js = read(DETECTORS_JS);
    for marker in [
        "const DETECTORS = Object.freeze([",
        "labelKey:",
        "normalize:",
        "preview:",
        "tool: null,",
        "function detect(text)",
        "function dispatchArgsFor(detection)",
    ] {
        assert!(
            js.contains(marker),
            "detectors.js must keep detector-table marker `{marker}`"
        );
    }
    assert!(
        js.contains("BigInt"),
        "the hex row must use BigInt so 40-char addresses don't truncate"
    );
}

#[test]
fn 감지기_표가_이름한_도구_id와_인자는_toolbox에_실재한다() {
    // The table's whole promise is "this match dispatches to that tool".
    // A renamed tool or a renamed input field must fail here, not silently
    // 404 in a tooltip on somebody's page.
    let js = read(DETECTORS_JS);
    let block = js
        .split_once("const DETECTOR_TOOL = Object.freeze({")
        .expect("detectors.js must declare DETECTOR_TOOL")
        .1
        .split_once("});")
        .expect("DETECTOR_TOOL must close with `});`")
        .0;

    let mut checked = 0_usize;
    for row in block.lines() {
        let Some(tail) = row.split_once(DETECTOR_TOOL_ID_PREFIX) else {
            continue;
        };
        let tool_id = tail.1.split('\'').next().unwrap();
        let arg = row
            .split_once(DETECTOR_TOOL_ARG_PREFIX)
            .unwrap_or_else(|| panic!("detector row for `{tool_id}` must name its argument"))
            .1
            .split('\'')
            .next()
            .unwrap();

        let meta = upeg_runtime::toolbox_tool(tool_id)
            .unwrap_or_else(|| panic!("detectors.js names `{tool_id}`, which no toolbox tool has"));
        let schema = meta.input_spec.to_json_schema_value();
        assert!(
            schema["properties"].get(arg).is_some(),
            "`{tool_id}` has no input field `{arg}`; the detector would dispatch an unknown argument"
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "the tool-id extraction found nothing — it is stale"
    );
}

#[test]
fn 콘텐츠_js의_감지_span은_키보드로_조작_가능하다() {
    let js = read(CONTENT_JS);
    for marker in [
        "setAttribute('role', 'button')",
        "setAttribute('tabindex', '0')",
        "addEventListener('click'",
        "addEventListener('keydown'",
        "ACTIVATION_KEYS.has(evt.key)",
    ] {
        assert!(
            js.contains(marker),
            "a detected token must remain actionable via `{marker}`"
        );
    }
}

#[test]
fn 콘텐츠_js는_호스트_결과를_툴팁에_넣고_없으면_딥_링크로_되돌아간다() {
    let js = read(CONTENT_JS);
    for marker in [
        // Resolution goes through the worker, never a content-script fetch.
        "RUNTIME_MESSAGE.DISPATCH_TOOL",
        "DISPATCH_RESULT_KIND.SUCCESS",
        "outputText(primary.value)",
        // No host (or a refusing host) falls back to the offline preview
        // plus the Desktop deep link.
        "hosted === null ? previewFor(detection) : hosted",
        "buildDeepLink({ board: CONTENT_SCRIPT_BOARD, toolId, input: detection.value }),",
    ] {
        assert!(
            js.contains(marker),
            "content.js must keep tooltip-resolution marker `{marker}`"
        );
    }
}

#[test]
fn 콘텐츠_js는_직접_호스트를_fetch하지_않는다() {
    // A content script's fetch is subject to the PAGE's CORS, and doing it
    // there would put the bearer token in a web page's world.
    let js = read(CONTENT_JS);
    assert!(
        !js.contains("fetch("),
        "content.js must ask background.js to dispatch instead of fetching the host itself"
    );
    assert!(
        !js.contains("TOKEN_STORAGE_KEY"),
        "the host token must never be read from a content script"
    );
}

#[test]
fn 콘텐츠_js는_도구가_없는_감지_행에_클릭_어포던스를_주지_않는다() {
    let js = read(CONTENT_JS);
    for marker in [
        "if (toolId === null) {",
        "tipFor(detection, { value: preview, linked: false })",
    ] {
        assert!(
            js.contains(marker),
            "a row with no host tool must render as an annotation, not a dead link (`{marker}`)"
        );
    }
}

#[test]
fn 콘텐츠_js는_in_page_selector_어댑터를_노출한다() {
    let js = read(CONTENT_JS);
    for marker in [
        "RUNTIME_MESSAGE.APPLY_SELECTOR_BINDINGS",
        "planSelectorApplication(message.bindings, message.args)",
        "applySelectorPlan(plan, document)",
        "RUNTIME_MESSAGE.PING",
    ] {
        assert!(
            js.contains(marker),
            "content.js must keep the in-page selector-adapter marker `{marker}`"
        );
    }
}

#[test]
fn selector_어댑터는_desktop_러너와_같은_dom_의미를_유지한다() {
    // Mirrors `upeg_runtime::selector_pipeline`: write + input/change,
    // click-or-Enter trigger, `.value` else `.textContent` read.
    let js = read(SELECTOR_ADAPTER_JS);
    for marker in [
        "const INPUT_WRITE_EVENTS = Object.freeze(['input', 'change']);",
        "const ENTER_KEY_EVENTS = Object.freeze(['keydown', 'keypress', 'keyup']);",
        "function planSelectorApplication(bindings, args)",
        "function applySelectorPlan(plan, root, eventFactory = DOM_EVENT_FACTORY)",
        "// First trigger wins",
    ] {
        assert!(
            js.contains(marker),
            "selector_adapter.js must keep parity marker `{marker}`"
        );
    }
}

#[test]
fn 백그라운드_워커는_등록과_디스패치를_모두_담당한다() {
    let js = read(BACKGROUND_JS);
    for marker in [
        "importScripts('wire.js', 'host_api.js', 'site_access.js');",
        "chrome.runtime.onInstalled.addListener",
        "chrome.runtime.onStartup.addListener",
        "chrome.permissions.onRemoved.addListener",
        "syncContentScripts(chrome.scripting, chrome.permissions, patterns)",
        "RUNTIME_MESSAGE.DISPATCH_TOOL",
    ] {
        assert!(
            js.contains(marker),
            "background.js must keep worker marker `{marker}`"
        );
    }
}

#[test]
fn 사이트_접근_모듈은_동적_등록_api를_사용한다() {
    let js = read(SITE_ACCESS_JS);
    for marker in [
        "scriptingApi.registerContentScripts(toRegister)",
        "scriptingApi.updateContentScripts(toUpdate)",
        "scriptingApi.unregisterContentScripts({ ids: toUnregister })",
        "permissionsApi.request({ origins: [pattern] })",
        "function grantedPatterns(permissionsApi, patterns)",
    ] {
        assert!(
            js.contains(marker),
            "site_access.js must keep per-site enablement marker `{marker}`"
        );
    }
}

// === Task B2: direct dispatch (hybrid policy) ===
//
// The popup POSTs non-embed pinned tools straight to the HTTP surface
// instead of always deep-linking to Desktop. These tests pin the
// policy's textual markers in popup.{html,js} so a future edit can't
// silently drop the dispatch path, the embed/static carve-out, or the
// fallback matrix (401/403 token hint, 503 rest-api hint, network
// unreachable) without a test failure — mirrors how the content.js
// deep-link markers above are pinned.

#[test]
fn 활성화_경로는_세_갈래로_닫혀있다() {
    // The popup's whole dispatch policy, as a pure decision: in-page,
    // direct dispatch, or Desktop deep link — nothing else.
    let js = read(TOOL_ROUTING_JS);
    for marker in [
        "const ACTIVATION_ROUTE = Object.freeze({",
        "IN_PAGE: 'in_page',",
        "DISPATCH: 'dispatch',",
        "DEEP_LINK: 'deep_link',",
        "const NON_DISPATCHABLE_INVOKERS = new Set(['static', 'embed']);",
        "CONTROLLED_EMBED: 'ControlledEmbed',",
        "function activationRouteFor(tool, { inPageAvailable = false } = {})",
    ] {
        assert!(
            js.contains(marker),
            "tool_routing.js must keep activation-policy marker `{marker}`"
        );
    }
}

#[test]
fn controlled_embed_핀은_사이트가_켜졌을_때만_in_page로_간다() {
    let js = read(TOOL_ROUTING_JS);
    assert!(
        js.contains(
            "return inPageAvailable && toolSelectorBindings(tool).length > 0\n        ? ACTIVATION_ROUTE.IN_PAGE\n        : ACTIVATION_ROUTE.DEEP_LINK;"
        ),
        "a ControlledEmbed pin must fall back to the deep link when there is no page to drive"
    );
}

#[test]
fn popup_js는_경로별_실행을_분기한다() {
    let js = read(POPUP_JS);
    for marker in [
        "const route = activationRouteFor(tool, { inPageAvailable: state.site.enabled });",
        "if (route === ACTIVATION_ROUTE.DEEP_LINK) {",
        "if (route === ACTIVATION_ROUTE.IN_PAGE) {",
        "type: RUNTIME_MESSAGE.APPLY_SELECTOR_BINDINGS,",
        "bindings: toolSelectorBindings(tool),",
        "function runToolImmediately(board, tool, route)",
        "function openDispatchForm(board, tool, route)",
    ] {
        assert!(
            js.contains(marker),
            "popup.js must keep the activation-route marker `{marker}`"
        );
    }
}

#[test]
fn popup_js는_임베드_계열_도구를_desktop_딥_링크로_유지한다() {
    let js = read(POPUP_JS);
    assert!(
        js.contains("openDeepLink(board, tool[TOOL_ID_FIELD]);"),
        "popup.js must still route embed-family/static tools to the Desktop deep link"
    );
    assert!(
        js.contains(OPEN_UPEG_LINK),
        "popup.js must keep the desktop deep-link action `{OPEN_UPEG_LINK}`"
    );
}

#[test]
fn popup_js는_사이트별_활성화_토글을_구현한다() {
    let js = read(POPUP_JS);
    for marker in [
        "requestSitePermission(chrome.permissions, pattern)",
        "matchPatternForUrl(state.site.url)",
        "disableUrl(await storedPatterns(), url)",
        "type: RUNTIME_MESSAGE.SYNC_SITES",
        // Registration only covers future loads, so the current tab is
        // injected directly — otherwise the toggle looks inert.
        "chrome.scripting.executeScript({",
        "files: [...CONTENT_SCRIPT_FILES],",
        // In-page availability additionally requires a live content script.
        "state.site.enabled = state.site.allowed && (await pingContentScript());",
    ] {
        assert!(
            js.contains(marker),
            "popup.js must keep the per-site enablement marker `{marker}`"
        );
    }
}

#[test]
fn host_api는_디스패치_폴백_매트릭스를_한_곳에서_분류한다() {
    let js = read(HOST_API_JS);
    for marker in [
        "const toolCallPath = (toolId) => `/v1/tools/${encodeURIComponent(toolId)}`;",
        "function classifyDispatchResponse(status, body)",
        "isAuthStatus(status)",
        "HTTP_STATUS.SERVICE_UNAVAILABLE",
        "DISPATCH_RESULT_KIND.NETWORK_ERROR",
        "DISPATCH_RESULT_KIND.REQUEST_TOO_LARGE",
    ] {
        assert!(
            js.contains(marker),
            "host_api.js must keep dispatch classification marker `{marker}`"
        );
    }
    assert!(
        js.contains(OPEN_UPEG_LINK.trim_end_matches("?surface=ext")),
        "host_api.js must keep the desktop deep-link action"
    );
}

#[test]
fn popup_js의_디스패치_결과_처리는_폴백_매트릭스를_렌더한다() {
    let js = read(POPUP_JS);
    for marker in [
        // 401/403 -> shared token-hint flow.
        "function handleAuthFailure()",
        // 503 host not serving -> server's hint text surfaced inline,
        // falling back to the localized catalog message.
        "outcome.hint || i18nMessage('hostUnavailable')",
        // daemon unreachable -> the whole-popup fallback the initial board
        // load uses.
        "showMessage(daemonUnreachableText(), { showRunHint: true });",
        "DISPATCH_RESULT_KIND.NETWORK_ERROR",
        // in-page runs have their own unreachable case: an enabled site
        // whose tab predates the enablement has no content script.
        "i18nMessage('inPageUnreachable')",
    ] {
        assert!(
            js.contains(marker),
            "popup.js must keep dispatch fallback marker `{marker}`"
        );
    }
}

#[test]
fn popup_html은_디스패치_패널과_사이트_토글_마크업을_가진다() {
    let html = read(POPUP_HTML);
    for marker in [
        "id=\"dispatch-panel\"",
        "id=\"dispatch-form\"",
        "id=\"dispatch-run\"",
        "id=\"dispatch-cancel\"",
        "id=\"dispatch-result\"",
        "id=\"site-row\"",
        "id=\"site-origin\"",
        "id=\"site-toggle\"",
        "id=\"site-status\"",
    ] {
        assert!(
            html.contains(marker),
            "popup.html must keep the popup panel marker `{marker}`"
        );
    }
}

#[test]
fn popup_html은_공유_모듈을_popup_js_보다_먼저_읽는다() {
    let html = read(POPUP_HTML);
    let order: Vec<usize> = [
        "wire.js",
        HOST_API_JS,
        SITE_ACCESS_JS,
        TOOL_ROUTING_JS,
        POPUP_JS,
    ]
    .iter()
    .map(|file| {
        html.find(&format!("src=\"{file}\""))
            .unwrap_or_else(|| panic!("popup.html must load `{file}`"))
    })
    .collect();
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "popup.html script order must satisfy the modules' evaluation-time reads"
    );
}

#[test]
fn manifest의_icon_참조는_실제_파일로_해석된다() {
    let v = manifest();
    let mut refs: Vec<String> = Vec::new();
    if let Some(icons) = v.get("icons").and_then(|x| x.as_object()) {
        refs.extend(
            icons
                .values()
                .filter_map(|p| p.as_str().map(str::to_string)),
        );
    }
    if let Some(default_icon) = v.get("action").and_then(|a| a.get("default_icon")) {
        match default_icon {
            Value::String(s) => refs.push(s.clone()),
            Value::Object(m) => {
                refs.extend(m.values().filter_map(|p| p.as_str().map(str::to_string)));
            }
            _ => {}
        }
    }
    let ext_dir = ext_path(MANIFEST_JSON)
        .parent()
        .expect("manifest has an extension directory")
        .to_path_buf();
    for r in &refs {
        let path = ext_dir.join(r);
        assert!(
            path.exists(),
            "manifest icon reference `{r}` is missing on disk"
        );
    }
}
