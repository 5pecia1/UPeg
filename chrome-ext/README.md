# upeg Chrome extension

Dependency-free MV3 extension: a popup that lists Board tabs + pinned tools
from the local HTTP surface (`http://127.0.0.1:7173`), and — the part that
only a browser extension can do — a content script that annotates detected
tokens in the page and applies a Controlled Embed tool's selector bindings to
the tab the user is already on.

Files, and who owns what:

| File | Owns |
|---|---|
| `detectors.js` | the frozen detector table (pattern → label → optional host tool + arg → optional offline preview) |
| `selector_adapter.js` | Controlled Embed planning + DOM application, mirroring `upeg_runtime::selector_pipeline` |
| `site_access.js` | the enabled-site allow-list, the permission gate, the dynamic registration diff |
| `tool_routing.js` | the closed 3-way activation policy (`IN_PAGE` / `DISPATCH` / `DEEP_LINK`) |
| `host_api.js` | host wire (paths, deep links, dispatch classification) + the internal message types |
| `background.js` | the service worker: registration sync + every host fetch (the token never enters a page) |
| `content.js` | the DOM and chrome.* glue for the two in-page capabilities |
| `popup.js` | the popup's own DOM, keyboard, and rendering |

No build step, no npm deps, no external JS test framework — `build.sh` just stages
the static files into `dist/`. Automated coverage for the extension lives
in `upeg-cli/tests/chrome_ext.rs` plus the dependency-free node tests:

```
node --test chrome-ext/tests/*.test.js
```

Anything that needs an actual browser + daemon is manual — this file is that
manual test plan.

## Build

```
bash chrome-ext/build.sh
```

Then `chrome://extensions` → enable "Developer mode" → "Load unpacked" →
select `chrome-ext/dist/`.

## Manual test plan

Prereqs: a debug `upeg` binary and a running host serving `/v1`
(`upeg host start`, or the command in each step below).

### 1. Board list + pinned tools

```
upeg http --addr 127.0.0.1:7173 --token test-tok
```

- Open the popup. Expect Board tabs across the top and the first
  board's pinned tools below, each showing a display label + id.
- `←`/`→` (or `h`/`l`) move between boards, `↑`/`↓` (or `j`/`k`) move
  between tools, `Home`/`End` jump to first/last board.

### 2. Direct dispatch — no-input tool

- Pin/select a tool whose `invoker` is `function`/`external`/`http`/
  `chain`/`llm`/`wasm` and has an empty `inputSchema.properties`
  (e.g. `id.uuid_v7`).
- Click it (or navigate to it and press `Enter`). Expect it to run
  immediately — no form — and show the primary output value inline
  under the tool list within ~1s.

### 3. Direct dispatch — tool with inputs

- Select a tool with required inputs (e.g. `num.hex_to_decimal`,
  one required `input: String`).
- Activating it opens an inline form instead of running immediately.
  Required fields are marked with `*`. Fill it in, press `Enter` (or
  click "Run"). Expect the result to render inline; `Escape` closes
  the form without navigating away or closing the popup.
- Leave a required field blank and try to run — expect an inline
  validation message, no request sent.
- For a field whose schema type isn't string/number/integer/boolean
  (e.g. an array/multi-select or `x-upeg-kind: json` field), expect a
  JSON textarea with a note, and invalid JSON to block the run with an
  inline message instead of sending a bad request.
- **File (byte) input + File output** (e.g. `media.pptx_extract_images`,
  `media.pdf_to_images`, `media.image_to_pdf`, `qr.decode`): a `File`
  input (`x-upeg-kind: file`) renders a native `<input type="file">`.
  Pick a real file (a `.pptx`/`.pdf`, or a zip of images for
  `image_to_pdf`), press Run. The popup reads the bytes and sends the
  canonical `FileValue` with padded RFC 4648 base64 bytes. The extension
  preflights count, raw-byte, and UTF-8 metadata limits before reading.
  When the tool returns a `File` output (zip/pdf),
  expect a **"Download …"** link (not a giant number-array dump); click
  it and confirm the downloaded file opens (zip of images / a PDF).
  Leaving a required File field empty must block the run with an inline
  "required" message.
- For `media.images_convert`, expect the picker to allow multiple
  `.png`/`.jpg`/`.jpeg` files. Selecting one or more valid images must send
  a flat canonical Directory in picker order; a disallowed extension,
  too many files, or a per-file/total size violation must show an inline
  error without dispatching.

### 4. Tools that stay on the deep link

- Select a tool whose `invoker` is `static` (Passive Embed). Activating it
  must open `upeg://open?surface=ext&board=...&tool=...` (Desktop deep link)
  — no inline form, no direct POST — on every page, enabled or not.
- A `ControlledEmbed` pin deep-links the same way **unless** the current tab
  is enabled for in-page work; see step 8.

### 5. Fallback matrix

- **Daemon not running**: stop the daemon (`upeg http stop`), reopen
  the popup. Expect "Could not reach upeg on 127.0.0.1:7173." plus the
  `upeg http --addr 127.0.0.1:7173` run hint. Restart the daemon before
  continuing.
- **401/403 (bad/missing token)**: open the gear icon → Host token →
  save a wrong token. Expect "upeg rejected this token — check it and
  save again." with a link back to the token field. Triggering a
  dispatch (step 2/3) with a bad token must fall back to this same
  message, not a raw HTTP error. Save the correct token
  (`test-tok` in the example above) to continue.
- **503 host not serving requests**: 503 is a transport-level "the host
  is up but refusing to serve right now" answer (e.g. paused, shutting
  down). There is no desired-state switch to reproduce it — the
  `service enable|disable` control plane is gone; a host process either
  serves or is not running. Expect the inline dispatch status to show
  the server's `hint` text verbatim when the body carries one, and
  otherwise the generic "upeg is not serving requests right now."
  message — never a raw `upeg http 503` string.
- **Tool-level failure** (422, e.g. `num.hex_to_decimal` with
  `input: "zz"`): expect the tool's own error message inline in the
  dispatch panel, not a generic "upeg http 422" string.

### 6. Content script — in-page detectors

Pre-enabled sites are `etherscan.io` and `polygonscan.com`; any page with a
`0x…` token, a 10/13-digit epoch, or a padded base64 blob works once enabled
(step 7).

- Expect each detected token wrapped with a dotted underline and a
  `data-upeg-detector` attribute naming the row that matched.
- **With no host running**: hovering a `hex` token shows
  `hex · <decimal> · click: upeg`; a `base64` blob has no offline preview so
  it shows `base64 · click: upeg`. Click or `Enter`/`Space` while focused
  opens the Desktop deep link
  (`upeg://open?surface=ext&board=dev&tool=num.hex_to_decimal&input=…`).
- **With a host running and a token saved**: hovering resolves through the
  host and the tooltip becomes the tool's own result — the decoded text for
  a base64 blob. Resolution is lazy (first hover/focus only) and cached per
  value, so a page with hundreds of matches makes no request until you look
  at one.
- An `epoch` token shows `epoch · <ISO 8601 UTC>` and is deliberately **not**
  clickable: no upeg tool converts an epoch, so there is nothing to open.
- Nothing in the page's network log should show a request to
  `127.0.0.1:7173` from the page itself — the service worker makes it.

### 7. Per-site enablement

- Open the popup on any `http`/`https` page. The site row shows the page's
  match pattern and an "Enable on this site" button. On a page a content
  script cannot run in (`chrome://`, the extension's own pages) it shows
  "upeg cannot run inside this page." and no button.
- Click "Enable on this site". Chrome prompts for the host permission;
  accept. Expect the status line to confirm, the button to flip to "Disable
  on this site", and the detectors to start annotating the **current** page
  without a reload.
- Decline the prompt instead: expect "Chrome denied access to this site." and
  no change in behaviour.
- Click "Enable on this site" a second time on the same tab (or open the popup
  over a tab whose page already loaded through the registration and hit it
  there): expect exactly the same annotations, not doubled ones. The popup
  pings before it injects, and content.js refuses a second evaluation in a
  frame it already lives in — two copies would mean two answers to every PING
  and two MutationObservers scanning the same DOM.
- With the site enabled, reload and confirm the annotations come back — that
  is the persisted `chrome.scripting` registration, not the one-off inject.
- Click "Disable on this site", then reload: expect no annotations. On a
  subdomain of a seeded explorer (`https://api.etherscan.io`), disabling must
  also stop the seeded wildcard from matching it.
- Revoke the site's permission from `chrome://extensions` → Site access.
  Expect the registration to disappear (the worker re-syncs on
  `permissions.onRemoved`) rather than lingering as a registration Chrome
  refuses to inject.

### 8. In-page Controlled Embed

Prereq: a pinned tool with `pin = ControlledEmbed` and selector bindings that
match the page you are on, and that page enabled per step 7.

- Activate the tool in the popup. Instead of the Desktop deep link, expect an
  inline form (its input schema) and, on Run, the page's fields to fill, its
  trigger to fire, and any `output` bindings to render inline as JSON.
- Activate the same tool on a page that is **not** enabled: expect the
  Desktop deep link, exactly as before.
- Activate it on an enabled page that was loaded **before** you enabled the
  site: expect "This tab has no upeg content script — reload it and try
  again.", never a silent no-op.
