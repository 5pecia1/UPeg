// upeg popup — PRD §6.6 / docs/ui-ux-surface-contract.md "Chrome extension
// contract".
//
// Renders compact Board tabs + the pinned tool list for the selected
// board, sourced from the local HTTP surface. Click or `Enter` on a tool
// opens the parameterized Desktop deep link
// (`upeg://open?surface=ext&board=<board>&tool=<tool>`). When the local
// host is unreachable (not installed / not running), falls back to a
// clear message + the generic "Open upeg" link + a one-line run hint —
// never a silent dead end.
//
// Dependency-free vanilla JS (PRD §6.6): no build step, no npm deps. The
// pieces other extension entry points also need live in their own modules
// — host_api.js (host wire + deep links), tool_routing.js (activation
// policy), site_access.js (per-site enablement) — so the popup is not the
// place any of them get re-invented.

(() => {
  'use strict';

  // === Constants (no magic strings/numbers — 매직 넘버·문자열 지양) ===
  //
  // The host wire (address, paths, deep links, dispatch classification)
  // lives in host_api.js because background.js needs the identical thing.
  // The activation policy lives in tool_routing.js because it is a pure
  // decision worth testing on its own. Per-site enablement lives in
  // site_access.js for the same reason. This file keeps the popup's own
  // DOM, keyboard, and rendering concerns and nothing else.

  const {
    BOARDS_LIST_PATH,
    DEEP_LINK_ACTION,
    DISPATCH_RESULT_KIND,
    HTTP_ADDRESS,
    HTTP_BASE_URL,
    RUNTIME_MESSAGE,
    RUN_HINT_COMMAND,
    TOKEN_STORAGE_KEY,
    authHeaders,
    boardShowPath,
    buildDeepLink,
    callTool,
    isAuthStatus,
    primaryOutput,
  } = UpegHostApi;

  const {
    ACTIVATION_ROUTE,
    TOOL_FIELD,
    activationRouteFor,
    toolInputProperties,
    toolLabel,
    toolRequiredFields,
    toolSelectorBindings,
  } = UpegToolRouting;
  const TOOL_ID_FIELD = TOOL_FIELD.ID;
  const TOOL_LABEL_FIELD = TOOL_FIELD.LABEL;
  const TOOL_INPUT_SCHEMA_FIELD = TOOL_FIELD.INPUT_SCHEMA;

  const {
    CONTENT_SCRIPT_FILES,
    disableUrl,
    enablePattern,
    ensureSeededPatterns,
    grantedPatterns,
    isUrlEnabled,
    matchPatternForUrl,
    removeSitePermission,
    requestSitePermission,
    writeEnabledPatterns,
  } = UpegSiteAccess;

  const FOCUS_REGION = { TABS: 'tabs', TOOLS: 'tools' };

  // JSON-Schema `type` values the inline form can render a dedicated
  // control for (see upeg-core/src/input/json_schema.rs
  // `kind_schema_value`). Anything else (arrays, `x-upeg-kind` markdown
  // /json/file/datetime/etc.) falls back to a raw-JSON textarea — the
  // popup is a compact surface, not a full form renderer.
  const JSON_SCHEMA_TYPE = {
    STRING: 'string',
    NUMBER: 'number',
    INTEGER: 'integer',
    BOOLEAN: 'boolean',
  };

  // `File` (byte) inputs are `{ "type": "object", "x-upeg-kind": "file" }`
  // (see upeg-core/src/input/json_schema.rs). The popup renders them with a
  // native `<input type="file">`, reads the bytes, and encodes the canonical
  // `FileValue` the dispatcher's `read_file` expects — instead of the
  // raw-JSON textarea fallback that byte inputs can't sensibly use.
  const UPEG_KIND_FIELD = 'x-upeg-kind';
  const UPEG_KIND_FILE = 'file';
  const FILE_FIELD_TYPE = 'file';
  const {
    FILE_POLICY_CONTROL_PROPERTY,
    FILE_SELECTION_ERROR,
    FileSelectionError,
    configureFileInput,
    readFileSelection,
  } = UpegFileInput;
  const { BASE64_WIRE_ERROR, Base64WireError, decodeBase64 } = UpegWire;

  // === i18n (chrome.i18n + _locales/{en,ko}/messages.json) ===
  //
  // Every user-facing string lives in messages.json (En/Ko parity); this
  // file only references keys. `i18nMessage` is the single lookup point.

  const i18nMessage = (key, substitutions) => chrome.i18n.getMessage(key, substitutions);

  // Static popup.html nodes carry `data-i18n*` attributes naming the
  // messages.json key to inject; `localizeStaticDom` resolves them once
  // at startup so the markup itself stays string-free.
  const I18N_TEXT_ATTR = 'data-i18n';
  const I18N_ATTR_TARGETS = [
    ['data-i18n-title', 'title'],
    ['data-i18n-aria-label', 'aria-label'],
    ['data-i18n-placeholder', 'placeholder'],
  ];

  function localizeStaticDom() {
    document.querySelectorAll(`[${I18N_TEXT_ATTR}]`).forEach((element) => {
      element.textContent = i18nMessage(element.getAttribute(I18N_TEXT_ATTR));
    });
    for (const [sourceAttr, targetAttr] of I18N_ATTR_TARGETS) {
      document.querySelectorAll(`[${sourceAttr}]`).forEach((element) => {
        element.setAttribute(targetAttr, i18nMessage(element.getAttribute(sourceAttr)));
      });
    }
  }

  const daemonUnreachableText = () => i18nMessage('daemonUnreachable', [HTTP_ADDRESS]);

  // === DOM handles ===

  const boardTabsEl = document.getElementById('board-tabs');
  const toolListEl = document.getElementById('tool-list');
  const messageEl = document.getElementById('message');
  const messageTextEl = document.getElementById('message-text');
  const messageHintEl = document.getElementById('message-hint');
  const messageTokenActionEl = document.getElementById('message-token-action');
  const openUpegLink = document.getElementById('open-upeg');

  const settingsToggleEl = document.getElementById('settings-toggle');
  const settingsPanelEl = document.getElementById('settings-panel');
  const tokenInputEl = document.getElementById('token-input');
  const tokenSaveEl = document.getElementById('token-save');
  const tokenClearEl = document.getElementById('token-clear');
  const tokenStatusEl = document.getElementById('token-status');

  const siteRowEl = document.getElementById('site-row');
  const siteOriginEl = document.getElementById('site-origin');
  const siteToggleEl = document.getElementById('site-toggle');
  const siteStatusEl = document.getElementById('site-status');

  const dispatchPanelEl = document.getElementById('dispatch-panel');
  const dispatchToolLabelEl = document.getElementById('dispatch-tool-label');
  const dispatchFormEl = document.getElementById('dispatch-form');
  const dispatchRunEl = document.getElementById('dispatch-run');
  const dispatchCancelEl = document.getElementById('dispatch-cancel');
  const dispatchStatusEl = document.getElementById('dispatch-status');
  const dispatchResultEl = document.getElementById('dispatch-result');
  const dispatchResultLabelEl = document.getElementById('dispatch-result-label');
  const dispatchResultValueEl = document.getElementById('dispatch-result-value');

  // === State ===

  const state = {
    boards: /** @type {string[]} */ ([]),
    activeBoardIndex: 0,
    toolsByBoard: /** @type {Map<string, object[]>} */ (new Map()),
    activeToolIndex: 0,
    focusRegion: FOCUS_REGION.TOOLS,
    token: /** @type {string | null} */ (null),
    // Non-null while the direct-dispatch panel (Task B2) is open for a
    // pinned tool: which tool/board it targets, whether the form is
    // still being filled in or a run is in flight, and the last result.
    dispatch: /** @type {{ board: string, tool: object, running: boolean, route: string } | null} */ (null),
    // The tab the popup was opened over, and whether upeg is allowed to
    // work inside it. `enabled` gates the whole `IN_PAGE` activation
    // route — the popup never offers an in-page run it cannot perform.
    // `allowed` = the site is on the allow-list AND Chrome granted its host
    // permission; that is what the toggle shows. `enabled` additionally
    // requires this tab's content script to answer a ping, because a tab
    // loaded before the site was enabled has none — and the popup must
    // never offer an in-page run it cannot perform.
    site: /** @type {{ tabId: number|null, url: string|null, pattern: string|null, allowed: boolean, enabled: boolean }} */ ({
      tabId: null,
      url: null,
      pattern: null,
      allowed: false,
      enabled: false,
    }),
  };

  function openDeepLink(board, toolId) {
    window.location.href = buildDeepLink({ board, toolId });
  }

  async function fetchJson(path) {
    const response = await fetch(`${HTTP_BASE_URL}${path}`, {
      headers: authHeaders(state.token),
    });
    if (!response.ok) {
      const error = new Error(`upeg http ${response.status}`);
      error.status = response.status;
      throw error;
    }
    return response.json();
  }

  function isAuthError(error) {
    return Boolean(error) && isAuthStatus(error.status);
  }


  // === Fallback / message rendering (REQUIRED — never a silent dead end) ===

  function showMessage(text, { showRunHint = false, showTokenHint = false } = {}) {
    boardTabsEl.hidden = true;
    toolListEl.hidden = true;
    closeDispatchPanel();
    messageEl.hidden = false;
    messageTextEl.textContent = text;
    messageHintEl.hidden = !showRunHint;
    if (showRunHint) {
      messageHintEl.textContent = i18nMessage('runHint', [RUN_HINT_COMMAND]);
    }
    messageTokenActionEl.hidden = !showTokenHint;
    openUpegLink.setAttribute('href', DEEP_LINK_ACTION);
  }

  function hideMessage() {
    messageEl.hidden = true;
    boardTabsEl.hidden = false;
    toolListEl.hidden = false;
  }

  // === Rendering ===

  function renderBoardTabs() {
    boardTabsEl.innerHTML = '';
    state.boards.forEach((board, index) => {
      const tab = document.createElement('button');
      tab.type = 'button';
      tab.className = 'tab';
      tab.setAttribute('role', 'tab');
      tab.setAttribute('id', `board-tab-${index}`);
      tab.setAttribute('aria-selected', String(index === state.activeBoardIndex));
      tab.tabIndex = index === state.activeBoardIndex ? 0 : -1;
      tab.textContent = board;
      if (index === state.activeBoardIndex && state.focusRegion === FOCUS_REGION.TABS) {
        tab.classList.add('focused');
      }
      tab.addEventListener('click', () => selectBoard(index));
      boardTabsEl.appendChild(tab);
    });
  }

  function renderToolList() {
    toolListEl.innerHTML = '';
    const board = state.boards[state.activeBoardIndex];
    const tools = state.toolsByBoard.get(board);

    if (tools === undefined) {
      const loading = document.createElement('p');
      loading.className = 'tool-list-status';
      loading.textContent = i18nMessage('toolListLoading');
      toolListEl.appendChild(loading);
      return;
    }
    if (tools === null) {
      const failed = document.createElement('p');
      failed.className = 'tool-list-status';
      failed.textContent = i18nMessage('boardLoadFailed');
      toolListEl.appendChild(failed);
      return;
    }
    if (tools.length === 0) {
      const empty = document.createElement('p');
      empty.className = 'tool-list-status';
      empty.textContent = i18nMessage('boardEmpty');
      toolListEl.appendChild(empty);
      return;
    }

    tools.forEach((tool, index) => {
      const toolId = tool[TOOL_ID_FIELD];
      const label = tool[TOOL_LABEL_FIELD] || toolId;
      const item = document.createElement('button');
      item.type = 'button';
      item.className = 'tool';
      item.setAttribute('role', 'option');
      item.setAttribute('id', `tool-item-${index}`);
      item.setAttribute('aria-selected', String(index === state.activeToolIndex));
      item.title = toolId;
      item.tabIndex = index === state.activeToolIndex ? 0 : -1;
      if (index === state.activeToolIndex && state.focusRegion === FOCUS_REGION.TOOLS) {
        item.classList.add('focused');
      }

      const primary = document.createElement('span');
      primary.className = 'tool-label';
      primary.textContent = label;
      item.appendChild(primary);

      const secondary = document.createElement('span');
      secondary.className = 'tool-id';
      secondary.textContent = toolId;
      item.appendChild(secondary);

      item.addEventListener('click', () => activateTool(board, tool));
      toolListEl.appendChild(item);
    });
  }

  function render() {
    renderBoardTabs();
    renderToolList();
  }

  // === Data loading (tools per board) ===

  async function loadToolsForBoard(board) {
    if (state.toolsByBoard.has(board)) {
      return;
    }
    // Placeholder so concurrent selects don't double-fetch, and the list
    // shows a loading state immediately.
    state.toolsByBoard.set(board, undefined);
    renderToolList();
    try {
      const data = await fetchJson(boardShowPath(board));
      state.toolsByBoard.set(board, Array.isArray(data.tools) ? data.tools : []);
    } catch {
      state.toolsByBoard.set(board, null);
    }
    if (state.boards[state.activeBoardIndex] === board) {
      renderToolList();
    }
  }

  async function selectBoard(index) {
    if (index < 0 || index >= state.boards.length) {
      return;
    }
    state.activeBoardIndex = index;
    state.activeToolIndex = 0;
    closeDispatchPanel();
    render();
    await loadToolsForBoard(state.boards[index]);
  }

  // === Token storage (settings panel) ===

  async function loadToken() {
    try {
      const stored = await chrome.storage.local.get(TOKEN_STORAGE_KEY);
      const value = stored[TOKEN_STORAGE_KEY];
      state.token = typeof value === 'string' && value.length > 0 ? value : null;
    } catch {
      state.token = null;
    }
    renderTokenStatus();
  }

  function renderTokenStatus() {
    tokenStatusEl.textContent = state.token
      ? i18nMessage('tokenSavedStatus')
      : i18nMessage('tokenMissingStatus');
  }

  async function saveToken() {
    const value = tokenInputEl.value.trim();
    if (!value) {
      tokenStatusEl.textContent = i18nMessage('tokenEmptyPrompt');
      return;
    }
    await chrome.storage.local.set({ [TOKEN_STORAGE_KEY]: value });
    state.token = value;
    tokenInputEl.value = '';
    renderTokenStatus();
    await connect();
  }

  async function clearToken() {
    await chrome.storage.local.remove(TOKEN_STORAGE_KEY);
    state.token = null;
    tokenInputEl.value = '';
    renderTokenStatus();
  }

  function openSettingsPanel() {
    settingsPanelEl.hidden = false;
    settingsToggleEl.setAttribute('aria-expanded', 'true');
  }

  function closeSettingsPanel() {
    settingsPanelEl.hidden = true;
    settingsToggleEl.setAttribute('aria-expanded', 'false');
  }

  function toggleSettingsPanel() {
    if (settingsPanelEl.hidden) {
      openSettingsPanel();
      tokenInputEl.focus();
    } else {
      closeSettingsPanel();
    }
  }

  // === Per-site enablement (site_access.js) ===
  //
  // The manifest grants `<all_urls>` only as an OPTIONAL permission, so a
  // site becomes eligible for the in-page capabilities in two steps the
  // user drives: Chrome grants the host permission, and the origin joins
  // the stored allow-list the service worker compiles registrations from.

  function storedPatterns() {
    return ensureSeededPatterns(chrome.storage.local);
  }

  async function pingContentScript() {
    if (state.site.tabId === null) return false;
    try {
      const reply = await chrome.tabs.sendMessage(state.site.tabId, {
        type: RUNTIME_MESSAGE.PING,
      });
      return Boolean(reply && reply.ok);
    } catch {
      return false;
    }
  }

  async function loadSiteState() {
    let tab = null;
    try {
      [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    } catch {
      tab = null;
    }
    state.site.tabId = tab && typeof tab.id === 'number' ? tab.id : null;
    state.site.url = tab && typeof tab.url === 'string' ? tab.url : null;
    state.site.pattern = state.site.url === null ? null : matchPatternForUrl(state.site.url);
    state.site.allowed = false;
    state.site.enabled = false;
    if (state.site.pattern !== null) {
      const granted = await grantedPatterns(chrome.permissions, [state.site.pattern]);
      state.site.allowed =
        granted.length > 0 && isUrlEnabled(await storedPatterns(), state.site.url);
      state.site.enabled = state.site.allowed && (await pingContentScript());
    }
    renderSiteRow();
  }

  function renderSiteRow() {
    const supported = state.site.pattern !== null;
    siteToggleEl.hidden = !supported;
    siteOriginEl.textContent = supported ? state.site.pattern : i18nMessage('siteUnsupported');
    if (!supported) return;
    siteToggleEl.textContent = state.site.allowed
      ? i18nMessage('siteDisableButton')
      : i18nMessage('siteEnableButton');
    siteToggleEl.setAttribute('aria-pressed', String(state.site.allowed));
  }

  async function syncRegisteredSites() {
    try {
      await chrome.runtime.sendMessage({ type: RUNTIME_MESSAGE.SYNC_SITES });
    } catch {
      // The worker was asleep and the message woke nothing; its own
      // onInstalled/onStartup sync still converges the registration.
    }
  }

  // A dynamic registration only affects FUTURE loads, so the tab the user is
  // looking at is injected directly — otherwise "Enable on this site" would
  // appear to do nothing until a reload.
  //
  // Ping first. A tab that already runs the content script (the user hit the
  // toggle twice, or the site was enabled and the page has since reloaded
  // through the dynamic registration) must not be injected again: a second
  // evaluation of content.js is a second `chrome.runtime.onMessage` listener
  // and a second MutationObserver over the same DOM. content.js also refuses
  // re-entry on its own (its isolated-world sentinel), so this is the outer
  // half of one contract: ask before injecting, refuse when asked twice.
  async function ensureContentScript() {
    if (await pingContentScript()) return true;
    return injectContentScript();
  }

  async function injectContentScript() {
    if (state.site.tabId === null) return false;
    try {
      await chrome.scripting.executeScript({
        target: { tabId: state.site.tabId },
        files: [...CONTENT_SCRIPT_FILES],
      });
      return true;
    } catch {
      return false;
    }
  }

  async function enableSite() {
    const pattern = state.site.pattern;
    if (pattern === null) return;
    if (!(await requestSitePermission(chrome.permissions, pattern))) {
      siteStatusEl.textContent = i18nMessage('siteDenied');
      return;
    }
    await writeEnabledPatterns(
      chrome.storage.local,
      enablePattern(await storedPatterns(), pattern),
    );
    await syncRegisteredSites();
    const running = await ensureContentScript();
    state.site.allowed = true;
    state.site.enabled = running;
    siteStatusEl.textContent = running
      ? i18nMessage('siteEnabled', [pattern])
      : i18nMessage('siteEnabledNeedsReload', [pattern]);
    renderSiteRow();
  }

  async function disableSite() {
    const { pattern, url } = state.site;
    if (pattern === null || url === null) return;
    await writeEnabledPatterns(chrome.storage.local, disableUrl(await storedPatterns(), url));
    await syncRegisteredSites();
    // Best effort: a pattern granted as a REQUIRED host permission (the
    // seeded explorers) cannot be removed, and Chrome answers `false`.
    await removeSitePermission(chrome.permissions, pattern);
    state.site.allowed = false;
    state.site.enabled = false;
    siteStatusEl.textContent = i18nMessage('siteDisabled', [pattern]);
    renderSiteRow();
  }

  function toggleSite() {
    return state.site.allowed ? disableSite() : enableSite();
  }

  // === Data loading (boards) ===

  async function connect() {
    try {
      const data = await fetchJson(BOARDS_LIST_PATH);
      const boards = Array.isArray(data.boards)
        ? data.boards.map((entry) => entry.board).filter((name) => typeof name === 'string')
        : [];
      if (boards.length === 0) {
        showMessage(i18nMessage('noBoardsFound'));
        return;
      }
      state.boards = boards;
      hideMessage();
      render();
      await loadToolsForBoard(state.boards[state.activeBoardIndex]);
    } catch (error) {
      if (isAuthError(error)) {
        handleAuthFailure();
        return;
      }
      showMessage(daemonUnreachableText(), { showRunHint: true });
    }
  }

  function handleAuthFailure() {
    showMessage(i18nMessage(state.token ? 'tokenRejected' : 'tokenRequired'), {
      showTokenHint: true,
    });
  }

  async function init() {
    localizeStaticDom();
    await loadToken();
    await loadSiteState();
    await connect();
  }

  // === Keyboard (docs/ui-ux-surface-contract.md: "Filter bar / compact
  // tabs" for the board strip, "Popup / popover menu" for the tool list) ===

  const BOARD_PREV_KEYS = new Set(['ArrowLeft', 'h']);
  const BOARD_NEXT_KEYS = new Set(['ArrowRight', 'l']);
  const TOOL_PREV_KEYS = new Set(['ArrowUp', 'k']);
  const TOOL_NEXT_KEYS = new Set(['ArrowDown', 'j']);

  function moveBoardSelection(delta) {
    if (state.boards.length === 0) return;
    const next =
      (state.activeBoardIndex + delta + state.boards.length) % state.boards.length;
    state.focusRegion = FOCUS_REGION.TABS;
    selectBoard(next);
  }

  function moveToolSelection(delta) {
    const board = state.boards[state.activeBoardIndex];
    const tools = state.toolsByBoard.get(board);
    if (!Array.isArray(tools) || tools.length === 0) return;
    state.activeToolIndex =
      (state.activeToolIndex + delta + tools.length) % tools.length;
    state.focusRegion = FOCUS_REGION.TOOLS;
    renderBoardTabs();
    renderToolList();
  }

  function activateSelection() {
    const board = state.boards[state.activeBoardIndex];
    const tools = state.toolsByBoard.get(board);
    if (!Array.isArray(tools) || tools.length === 0) return;
    const tool = tools[state.activeToolIndex];
    if (!tool) return;
    activateTool(board, tool);
  }

  // === Activation (three routes — see tool_routing.js) ===
  //
  // A tool click or `Enter` resolves to exactly one of IN_PAGE / DISPATCH /
  // DEEP_LINK. IN_PAGE and DISPATCH share the same inline form and result
  // panel; only the run step differs, so the form code below never branches
  // on the route beyond `runDispatch`.
  function activateTool(board, tool) {
    const route = activationRouteFor(tool, { inPageAvailable: state.site.enabled });
    if (route === ACTIVATION_ROUTE.DEEP_LINK) {
      openDeepLink(board, tool[TOOL_ID_FIELD]);
      return;
    }
    const properties = toolInputProperties(tool);
    if (Object.keys(properties).length === 0) {
      runToolImmediately(board, tool, route);
    } else {
      openDispatchForm(board, tool, route);
    }
  }

  function clearDispatchResult() {
    dispatchResultEl.hidden = true;
    dispatchResultLabelEl.textContent = '';
    dispatchResultValueEl.textContent = '';
    dispatchStatusEl.textContent = '';
  }

  function closeDispatchPanel() {
    state.dispatch = null;
    dispatchPanelEl.hidden = true;
    dispatchFormEl.innerHTML = '';
    clearDispatchResult();
  }

  // JSON-Schema `type` → an `<input>`/`<textarea>` the popup can render
  // without a component library (Task B2: "keep it small"). Anything
  // outside string/number/integer/boolean — arrays, the `x-upeg-kind`
  // markdown/json/datetime/file_path/url/file markers — falls back to
  // a raw-JSON textarea rather than growing a bespoke widget per kind.
  function buildDispatchField(name, propertySchema, required) {
    const wrapper = document.createElement('div');
    wrapper.className = 'dispatch-field';

    const label = document.createElement('label');
    const title = (propertySchema && propertySchema.title) || name;
    label.textContent = required ? `${title} *` : title;
    label.setAttribute('for', `dispatch-field-${name}`);
    wrapper.appendChild(label);

    const ty = propertySchema && propertySchema.type;
    const upegKind = propertySchema && propertySchema[UPEG_KIND_FIELD];
    let control;
    let jsonPassthrough = false;
    let isFile = false;

    if (upegKind === UPEG_KIND_FILE) {
      isFile = true;
      control = document.createElement('input');
      control.type = 'file';
      configureFileInput(control, propertySchema);
    } else if (ty === JSON_SCHEMA_TYPE.BOOLEAN) {
      control = document.createElement('input');
      control.type = 'checkbox';
    } else if (Array.isArray(propertySchema && propertySchema.enum)) {
      control = document.createElement('select');
      if (!required) {
        const blank = document.createElement('option');
        blank.value = '';
        blank.textContent = '—';
        control.appendChild(blank);
      }
      for (const value of propertySchema.enum) {
        const option = document.createElement('option');
        option.value = value;
        option.textContent = value;
        control.appendChild(option);
      }
    } else if (ty === JSON_SCHEMA_TYPE.NUMBER || ty === JSON_SCHEMA_TYPE.INTEGER) {
      control = document.createElement('input');
      control.type = 'number';
      if (ty === JSON_SCHEMA_TYPE.INTEGER) {
        control.step = '1';
      }
    } else if (ty === JSON_SCHEMA_TYPE.STRING) {
      control = document.createElement('input');
      control.type = 'text';
    } else {
      jsonPassthrough = true;
      control = document.createElement('textarea');
      control.placeholder = i18nMessage('jsonValuePlaceholder');
    }

    control.id = `dispatch-field-${name}`;
    control.name = name;
    control.dataset.fieldType = isFile
      ? FILE_FIELD_TYPE
      : jsonPassthrough
        ? 'json'
        : ty || JSON_SCHEMA_TYPE.STRING;
    control.dataset.required = required ? 'true' : 'false';
    wrapper.appendChild(control);

    if (jsonPassthrough) {
      const note = document.createElement('p');
      note.className = 'dispatch-field-note';
      note.textContent = i18nMessage('jsonFieldNote');
      wrapper.appendChild(note);
    }

    return wrapper;
  }

  function openDispatchForm(board, tool, route) {
    state.dispatch = { board, tool, running: false, route };
    dispatchToolLabelEl.textContent = toolLabel(tool);
    dispatchFormEl.innerHTML = '';
    clearDispatchResult();

    const properties = toolInputProperties(tool);
    const required = new Set(toolRequiredFields(tool));
    for (const [name, propertySchema] of Object.entries(properties)) {
      dispatchFormEl.appendChild(buildDispatchField(name, propertySchema, required.has(name)));
    }

    dispatchPanelEl.hidden = false;
    const firstField = dispatchFormEl.querySelector('input, select, textarea');
    if (firstField) firstField.focus();
  }

  function formatFileSelectionError(fieldName, error) {
    if (!(error instanceof FileSelectionError)) {
      return i18nMessage('fileReadFailed', [fieldName]);
    }
    const details = error.details;
    switch (error.code) {
      case FILE_SELECTION_ERROR.EMPTY:
        return i18nMessage('fieldRequired', [fieldName]);
      case FILE_SELECTION_ERROR.TOO_MANY_FILES:
        return i18nMessage('fileTooMany', [
          fieldName,
          String(details.actual),
          String(details.limit),
        ]);
      case FILE_SELECTION_ERROR.TOO_MANY_NODES:
        return i18nMessage('fileTooManyNodes', [
          fieldName,
          String(details.actual),
          String(details.limit),
        ]);
      case FILE_SELECTION_ERROR.EXTENSION_NOT_ALLOWED:
        return i18nMessage('fileExtensionNotAllowed', [
          details.fileName,
          details.allowed.map((extension) => `.${extension}`).join(', '),
        ]);
      case FILE_SELECTION_ERROR.FILE_TOO_LARGE:
        return i18nMessage('fileTooLarge', [
          details.fileName,
          String(details.actual),
          String(details.limit),
        ]);
      case FILE_SELECTION_ERROR.TOTAL_TOO_LARGE:
        return i18nMessage('fileTotalTooLarge', [
          fieldName,
          String(details.actual),
          String(details.limit),
        ]);
      case FILE_SELECTION_ERROR.TRANSPORT_TOO_LARGE:
        return i18nMessage('fileTransportTooLarge', [
          fieldName,
          String(details.actual),
          String(details.limit),
        ]);
      case FILE_SELECTION_ERROR.METADATA_TOO_LARGE:
        return i18nMessage('fileMetadataTooLarge', [
          fieldName,
          String(details.actual),
          String(details.limit),
        ]);
      case FILE_SELECTION_ERROR.READ_TOO_LARGE:
        return i18nMessage('fileReadTooLarge', [details.fileName, String(details.limit)]);
      default:
        return i18nMessage('fileReadSizeMismatch', [details.fileName]);
    }
  }

  // Read + coerce the dispatch form's current values per each field's
  // JSON-Schema type (Task B2: "string/number/boolean coercion ... is
  // enough; JSON passthrough for anything else"). Returns `null` (and
  // renders the offending field's problem in `dispatchStatusEl`) when a
  // required field is empty or a JSON-passthrough field doesn't parse.
  async function readDispatchFormValues() {
    const args = {};
    const controls = dispatchFormEl.querySelectorAll('input, select, textarea');
    for (const control of controls) {
      const name = control.name;
      const fieldType = control.dataset.fieldType;
      const required = control.dataset.required === 'true';

      if (fieldType === JSON_SCHEMA_TYPE.BOOLEAN) {
        args[name] = control.checked;
        continue;
      }

      if (fieldType === FILE_FIELD_TYPE) {
        const files = Array.from(control.files || []);
        if (files.length === 0) {
          if (required) {
            dispatchStatusEl.textContent = i18nMessage('fieldRequired', [name]);
            return null;
          }
          continue;
        }
        const policy = control[FILE_POLICY_CONTROL_PROPERTY];
        try {
          args[name] = await readFileSelection(files, policy, name);
        } catch (error) {
          dispatchStatusEl.textContent = formatFileSelectionError(name, error);
          return null;
        }
        continue;
      }

      const raw = control.value.trim();
      if (raw.length === 0) {
        if (required) {
          dispatchStatusEl.textContent = i18nMessage('fieldRequired', [name]);
          return null;
        }
        continue;
      }

      if (fieldType === JSON_SCHEMA_TYPE.NUMBER || fieldType === JSON_SCHEMA_TYPE.INTEGER) {
        const value = Number(raw);
        if (Number.isNaN(value)) {
          dispatchStatusEl.textContent = i18nMessage('fieldNotNumber', [name]);
          return null;
        }
        args[name] = value;
      } else if (fieldType === 'json') {
        try {
          args[name] = JSON.parse(raw);
        } catch {
          dispatchStatusEl.textContent = i18nMessage('fieldNotJson', [name]);
          return null;
        }
      } else {
        args[name] = raw;
      }
    }
    return args;
  }

  // Object URL backing the download link currently on screen, if any. A Blob
  // URL pins its Blob until revoked, so the popup would otherwise hold every
  // file it has ever rendered — one leaked copy per dispatch.
  let dispatchFileDownloadUrl = null;

  // Drop the on-screen download link's Blob. Safe to call when there is none.
  function releaseFileDownloadUrl() {
    if (dispatchFileDownloadUrl !== null) {
      URL.revokeObjectURL(dispatchFileDownloadUrl);
      dispatchFileDownloadUrl = null;
    }
  }

  // Render a `File` output value (canonical `{name, content:{kind:'bytes',
  // bytes:'AAH/'}, mime}`) as a download link built from an in-memory Blob.
  // Returns `false` when the value isn't a byte file, so the caller falls
  // back to the JSON dump.
  function renderFileDownload(value) {
    if (!value || typeof value !== 'object') return false;
    const content = value.content;
    if (!content || content.kind !== 'bytes') {
      return false;
    }
    const name =
      typeof value.name === 'string' && value.name.length > 0 ? value.name : 'output';
    let bytes;
    try {
      bytes = decodeBase64(content.bytes);
    } catch (error) {
      if (!(error instanceof Base64WireError)) throw error;
      if (error.code === BASE64_WIRE_ERROR.TOO_LARGE) {
        dispatchResultValueEl.textContent = i18nMessage('fileOutputTooLarge', [
          String(error.actualBytes),
          String(error.limitBytes),
        ]);
        return true;
      }
      return false;
    }
    const mime =
      typeof value.mime === 'string' && value.mime.length > 0
        ? value.mime
        : 'application/octet-stream';
    dispatchFileDownloadUrl = URL.createObjectURL(new Blob([bytes], { type: mime }));
    dispatchResultValueEl.textContent = '';
    const link = document.createElement('a');
    link.href = dispatchFileDownloadUrl;
    link.download = name;
    link.className = 'dispatch-file-download';
    link.textContent = `Download ${name} (${bytes.length} bytes)`;
    dispatchResultValueEl.appendChild(link);
    return true;
  }

  function renderDispatchResult(outcome) {
    // Every branch below replaces whatever is on screen, so the previous
    // render's Blob (if it was a download link) is now unreachable.
    releaseFileDownloadUrl();
    if (outcome.kind === DISPATCH_RESULT_KIND.SUCCESS) {
      const primary = primaryOutput(outcome.result);
      dispatchStatusEl.textContent = i18nMessage('dispatchDone');
      if (primary) {
        dispatchResultLabelEl.textContent = primary.label || primary.id;
        if (primary.kind === UPEG_KIND_FILE && renderFileDownload(primary.value)) {
          // A `File` output (zip/pdf/…) renders as a download link, not a
          // multi-megabyte number-array dump.
        } else {
          dispatchResultValueEl.textContent =
            typeof primary.value === 'string'
              ? primary.value
              : JSON.stringify(primary.value, null, 2);
        }
        dispatchResultEl.hidden = false;
      }
      return;
    }
    if (outcome.kind === DISPATCH_RESULT_KIND.FAILURE) {
      // A body-less non-2xx carries only its status (host_api.js does not
      // know this surface's catalog), so the status text is composed here.
      dispatchStatusEl.textContent =
        outcome.error.message ||
        (outcome.status
          ? i18nMessage('httpErrorStatus', [String(outcome.status)])
          : i18nMessage('toolRunFailed'));
      return;
    }
    if (outcome.kind === DISPATCH_RESULT_KIND.HOST_UNAVAILABLE) {
      // 503 is transport-level: the host answered, but is not serving
      // requests right now. It names no service — the `service
      // enable|disable` control plane is gone — so the fallback text is
      // about the host, and the server's own `hint` still wins.
      dispatchStatusEl.textContent = outcome.hint || i18nMessage('hostUnavailable');
      return;
    }
    if (outcome.kind === DISPATCH_RESULT_KIND.REQUEST_TOO_LARGE) {
      dispatchStatusEl.textContent = i18nMessage('requestBodyTooLarge', [
        String(outcome.actualBytes),
        String(outcome.limitBytes),
      ]);
      return;
    }
    if (outcome.kind === DISPATCH_RESULT_KIND.NETWORK_ERROR) {
      // Falls all the way back to the same whole-popup message the
      // initial board load uses — the daemon going away mid-session is
      // the same "not reachable" situation, not a per-tool error.
      closeDispatchPanel();
      showMessage(daemonUnreachableText(), { showRunHint: true });
      return;
    }
    // AUTH_ERROR falls back to the shared token-hint flow (Task B2 §3).
    closeDispatchPanel();
    handleAuthFailure();
  }

  // === In-page run (ACTIVATION_ROUTE.IN_PAGE) ===
  //
  // A Controlled Embed tool's selector bindings, applied to the tab the
  // popup was opened over. content.js owns the planning and the DOM
  // writes (selector_adapter.js); the popup only ships the bindings and
  // the collected args across and renders what comes back.
  async function runInPage(tool, args) {
    if (state.site.tabId === null) return null;
    try {
      return await chrome.tabs.sendMessage(state.site.tabId, {
        type: RUNTIME_MESSAGE.APPLY_SELECTOR_BINDINGS,
        bindings: toolSelectorBindings(tool),
        args,
      });
    } catch {
      // The site is enabled but this tab has no content script yet (it was
      // loaded before the site was enabled, or it is a page Chrome refuses
      // to inject into).
      return null;
    }
  }

  function renderInPageResult(tool, applied) {
    if (applied === null || applied.ok !== true) {
      dispatchStatusEl.textContent = i18nMessage('inPageUnreachable');
      return;
    }
    dispatchStatusEl.textContent = applied.triggered
      ? i18nMessage('inPageTriggered', [String(applied.written.length)])
      : i18nMessage('inPageFilled', [String(applied.written.length)]);
    const outputs = applied.outputs || {};
    if (Object.keys(outputs).length === 0) return;
    dispatchResultLabelEl.textContent = toolLabel(tool);
    dispatchResultValueEl.textContent = JSON.stringify(outputs, null, 2);
    dispatchResultEl.hidden = false;
  }

  async function runDispatch(board, tool, args, route) {
    state.dispatch = { board, tool, running: true, route };
    dispatchStatusEl.textContent = i18nMessage('dispatchRunning');
    dispatchRunEl.disabled = true;
    if (route === ACTIVATION_ROUTE.IN_PAGE) {
      const applied = await runInPage(tool, args);
      dispatchRunEl.disabled = false;
      if (state.dispatch) state.dispatch.running = false;
      renderInPageResult(tool, applied);
      return;
    }
    const outcome = await callTool({
      fetchImpl: fetch,
      token: state.token,
      toolId: tool[TOOL_ID_FIELD],
      args,
    });
    dispatchRunEl.disabled = false;
    if (state.dispatch) {
      state.dispatch.running = false;
    }
    renderDispatchResult(outcome);
  }

  function runToolImmediately(board, tool, route) {
    state.dispatch = { board, tool, running: true, route };
    dispatchToolLabelEl.textContent = toolLabel(tool);
    dispatchFormEl.innerHTML = '';
    clearDispatchResult();
    dispatchPanelEl.hidden = false;
    runDispatch(board, tool, {}, route);
  }

  async function runDispatchForm() {
    if (!state.dispatch || state.dispatch.running) return;
    const { board, tool, route } = state.dispatch;
    const args = await readDispatchFormValues();
    if (args === null) return;
    await runDispatch(board, tool, args, route);
  }

  siteToggleEl.addEventListener('click', toggleSite);
  settingsToggleEl.addEventListener('click', toggleSettingsPanel);
  tokenSaveEl.addEventListener('click', saveToken);
  tokenClearEl.addEventListener('click', clearToken);
  tokenInputEl.addEventListener('keydown', (event) => {
    // Local to the input — let Enter submit instead of falling through to
    // the global tool/board navigation handler below.
    if (event.key === 'Enter') {
      event.preventDefault();
      saveToken();
    }
  });
  messageTokenActionEl.addEventListener('click', () => {
    openSettingsPanel();
    tokenInputEl.focus();
  });

  dispatchRunEl.addEventListener('click', runDispatchForm);
  dispatchCancelEl.addEventListener('click', () => closeDispatchPanel());
  dispatchFormEl.addEventListener('keydown', (event) => {
    // Enter runs the tool — the same "Enter = run/submit" contract the
    // token field above uses — instead of falling through to the
    // global tool/board navigation handler below.
    if (event.key === 'Enter') {
      event.preventDefault();
      runDispatchForm();
    }
  });

  document.addEventListener('keydown', (event) => {
    const focusedTag = event.target && event.target.tagName;
    const focusInsideFormField =
      focusedTag === 'INPUT' || focusedTag === 'TEXTAREA' || focusedTag === 'SELECT';

    if (event.key === 'Escape') {
      if (!settingsPanelEl.hidden) {
        event.preventDefault();
        closeSettingsPanel();
        settingsToggleEl.focus();
        return;
      }
      if (!dispatchPanelEl.hidden) {
        event.preventDefault();
        closeDispatchPanel();
        return;
      }
      window.close();
      return;
    }
    if (focusInsideFormField) {
      // Don't hijack j/k/h/l/Enter while the user is typing in the token
      // field or a dispatch-form field — those are single-letter nav
      // shortcuts everywhere else.
      return;
    }
    if (messageEl && !messageEl.hidden) {
      return;
    }
    if (settingsToggleEl.contains(event.target) || settingsPanelEl.contains(event.target)) {
      // Let native button semantics (Enter/Space activates) work for the
      // gear toggle and the Save/Clear buttons instead of being reinterpreted
      // as tool/board navigation.
      return;
    }
    if (dispatchPanelEl.contains(event.target)) {
      // Same idea for the Run/Cancel buttons in the dispatch panel.
      return;
    }
    if (siteRowEl.contains(event.target)) {
      // ...and for the per-site enable toggle.
      return;
    }
    if (BOARD_PREV_KEYS.has(event.key)) {
      event.preventDefault();
      moveBoardSelection(-1);
    } else if (BOARD_NEXT_KEYS.has(event.key)) {
      event.preventDefault();
      moveBoardSelection(1);
    } else if (event.key === 'Home') {
      event.preventDefault();
      state.focusRegion = FOCUS_REGION.TABS;
      selectBoard(0);
    } else if (event.key === 'End') {
      event.preventDefault();
      state.focusRegion = FOCUS_REGION.TABS;
      selectBoard(state.boards.length - 1);
    } else if (TOOL_PREV_KEYS.has(event.key)) {
      event.preventDefault();
      moveToolSelection(-1);
    } else if (TOOL_NEXT_KEYS.has(event.key)) {
      event.preventDefault();
      moveToolSelection(1);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      activateSelection();
    }
  });

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
