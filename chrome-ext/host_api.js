'use strict';

// upeg wire contracts — the one place that knows how the extension talks
// to a local upeg host, how it builds Desktop deep links, and what its own
// popup/worker/content-script messages look like.
//
// Three call sites need this and must not drift apart:
//   * popup.js          — board list + direct dispatch of a pinned tool
//   * background.js     — dispatch on behalf of a content script (a
//                         content script's own `fetch` is subject to the
//                         *page's* CORS, so the service worker owns every
//                         host request and the bearer token never enters
//                         a web page's world)
//   * content.js        — deep-link fallback text only (no fetch)
//
// Deep-link construction mirrors `upeg_pegboard_ui::deep_link::
// desktop_deep_link` byte for byte: `surface`, then `board`, `tool`,
// `input` in that order, each value percent-encoded with the RFC 3986
// unreserved set (exactly `encodeURIComponent`).

const UpegHostWire =
  typeof module === 'object' && module.exports ? require('./wire.js') : UpegWire;

const UpegHostApi = (() => {
  const { RequestBodyTooLargeError, serializeJsonRequestBody } = UpegHostWire;

  // Pairing endpoint. `upeg http` binds an ephemeral loopback port by
  // default (`DEFAULT_BIND = "127.0.0.1:0"`), so the endpoint the popup
  // and the service worker must talk to is user-configurable, persisted
  // under ENDPOINT_STORAGE_KEY. `upeg http status --pairing` prints a
  // running host's `url:` + `token:` lines — that is the value to paste.
  // The default below keeps an install paired before ephemeral ports
  // working untouched.
  const DEFAULT_HTTP_HOST = '127.0.0.1';
  const DEFAULT_HTTP_PORT = 7173;
  const DEFAULT_HTTP_BASE_URL = `http://${DEFAULT_HTTP_HOST}:${DEFAULT_HTTP_PORT}`;
  const DEFAULT_HTTP_ADDRESS = `${DEFAULT_HTTP_HOST}:${DEFAULT_HTTP_PORT}`;
  const RUN_HINT_COMMAND = 'upeg http';
  const PAIRING_COMMAND = 'upeg http status --pairing';

  // The extension deliberately uses its own adapter routes. The generic
  // HTTP routes remain available to scripts and detector lookups, but do
  // not carry a Board pin's saved defaults or the extension execution mode.
  const BOARDS_LIST_PATH = '/v1/ext/boards';
  const boardShowPath = (board) => `/v1/ext/boards/${encodeURIComponent(board)}`;
  const toolCallPath = (board, toolId) =>
    `/v1/ext/boards/${encodeURIComponent(board)}/tools/${encodeURIComponent(toolId)}`;
  const globalToolCallPath = (toolId) => `/v1/tools/${encodeURIComponent(toolId)}`;

  // Non-/healthz routes require `Authorization: Bearer <token>`. The token is
  // user-supplied (pasted from the host's config / start-up log) and lives
  // only in this extension's local storage — never logged, never sent
  // anywhere but the configured loopback endpoint above.
  const TOKEN_STORAGE_KEY = 'upegDaemonToken';
  // The paired endpoint, persisted as a normalized `http://host:port`
  // base URL string.
  const ENDPOINT_STORAGE_KEY = 'upegHostEndpoint';

  const HTTP_STATUS = Object.freeze({
    UNAUTHORIZED: 401,
    FORBIDDEN: 403,
    SERVICE_UNAVAILABLE: 503,
  });

  // Internal runtime message types (popup <-> service worker <-> content
  // script). A content script cannot fetch the host itself — its requests
  // are subject to the *page's* CORS — so every host call is a
  // `DISPATCH_TOOL` message the worker performs.
  const RUNTIME_MESSAGE = Object.freeze({
    DISPATCH_TOOL: 'upeg:dispatch-tool',
    SYNC_SITES: 'upeg:sync-sites',
    APPLY_SELECTOR_BINDINGS: 'upeg:apply-selector-bindings',
    PING: 'upeg:ping',
  });

  const DEEP_LINK_ACTION = 'upeg://open';
  const SURFACE_PARAM_VALUE = 'ext';
  // Content-script detections have no board of their own; `dev` matches
  // `upeg_pegboard_ui::deep_link::DEFAULT_DESKTOP_BOARD`.
  const CONTENT_SCRIPT_BOARD = 'dev';

  // Shape mirrors `upeg_core::output::{ToolSuccess,ToolFailure}::to_canonical_json`
  // (`{"ok":true,"primary_output_id","outputs":[...]}` /
  // `{"ok":false,"error":{"code","message"}}`), plus the transport-level
  // cases that never reach that shape.
  const DISPATCH_RESULT_KIND = Object.freeze({
    SUCCESS: 'success',
    FAILURE: 'failure',
    AUTH_ERROR: 'auth_error',
    HOST_UNAVAILABLE: 'host_unavailable',
    NETWORK_ERROR: 'network_error',
    REQUEST_TOO_LARGE: 'request_too_large',
  });

  /// `encodeURIComponent` already emits exactly the RFC 3986 unreserved
  /// set Rust's `encode_deep_link_value` keeps, so the two agree without a
  /// hand-rolled table on this side.
  function deepLinkValue(value) {
    return encodeURIComponent(String(value).trim());
  }

  function buildDeepLink({ board = null, toolId = null, input = null } = {}) {
    let link = `${DEEP_LINK_ACTION}?surface=${SURFACE_PARAM_VALUE}`;
    for (const [key, value] of [
      ['board', board],
      ['tool', toolId],
      ['input', input],
    ]) {
      if (typeof value !== 'string' || value.trim().length === 0) continue;
      link += `&${key}=${deepLinkValue(value)}`;
    }
    return link;
  }

  function isAuthStatus(status) {
    return status === HTTP_STATUS.UNAUTHORIZED || status === HTTP_STATUS.FORBIDDEN;
  }

  // Endpoint validation failure reasons — popup.js maps each to a
  // localized messages.json key, so this module stays string-free.
  const ENDPOINT_ERROR = Object.freeze({
    NOT_A_URL: 'not_a_url',
    NOT_HTTP: 'not_http',
    CREDENTIALS: 'credentials',
    PATH_OR_QUERY: 'path_or_query',
    NOT_LOOPBACK: 'not_loopback',
    BAD_PORT: 'bad_port',
  });

  // The bearer token rides `Authorization` to this endpoint on every
  // request — the endpoint is therefore restricted to loopback, or a
  // pasted URL would send the token to a remote host. `new URL`
  // normalizes IPv4 spellings ("127.1", "0x7f.0.0.1", "2130706433") to
  // the dotted-quad form and IPv6 to the compressed form before we see
  // the hostname, so the checks below cover the whole loopback space.
  function isLoopbackHostname(hostname) {
    if (hostname === 'localhost' || hostname === '[::1]') return true;
    const octets = hostname.split('.');
    if (octets.length !== 4 || octets[0] !== '127') return false;
    return octets.every(
      (octet) => /^\d{1,3}$/.test(octet) && Number(octet) <= 255,
    );
  }

  const DEFAULT_ENDPOINT = Object.freeze({
    baseUrl: DEFAULT_HTTP_BASE_URL,
    address: DEFAULT_HTTP_ADDRESS,
    host: DEFAULT_HTTP_HOST,
    port: DEFAULT_HTTP_PORT,
    // The `chrome.permissions` origin pattern covering exactly this
    // endpoint — popup.js requests it on save when it is not already
    // granted, so a custom port never widens the granted set silently.
    permissionPattern: `${DEFAULT_HTTP_BASE_URL}/*`,
  });

  /// Parse a user-supplied endpoint into the normalized
  /// `{baseUrl, address, host, port, permissionPattern}` shape used by
  /// every request path, or `{ ok: false, error }` naming which rule
  /// rejected it. Accepted spellings: `http://127.0.0.1:49317`,
  /// `127.0.0.1:49317`, `localhost:8080`, `[::1]:9000` — an omitted port
  /// means the default 7173 so a bare `127.0.0.1` keeps working for
  /// installs paired before ephemeral ports.
  function parseLoopbackEndpoint(input) {
    if (typeof input !== 'string') {
      return { ok: false, error: ENDPOINT_ERROR.NOT_A_URL };
    }
    const trimmed = input.trim();
    if (trimmed.length === 0) {
      return { ok: false, error: ENDPOINT_ERROR.NOT_A_URL };
    }

    // Peel the scheme off by hand: `new URL('127.0.0.1:7173')` reads the
    // host text as a scheme and fails, which would reject exactly the
    // bare `host:port` form `upeg http status --pairing` users type.
    const schemeSep = trimmed.indexOf('://');
    let rest = trimmed;
    if (schemeSep >= 0) {
      if (trimmed.slice(0, schemeSep).toLowerCase() !== 'http') {
        return { ok: false, error: ENDPOINT_ERROR.NOT_HTTP };
      }
      rest = trimmed.slice(schemeSep + 3);
    }

    // Split an explicit port off BEFORE `new URL` — the parser erases the
    // default port (`:80` disappears) and rejects a malformed one by
    // failing the whole URL, so afterwards the port text is
    // unrecoverable and neither mistake can be reported accurately.
    let hostPart = rest;
    let portText = '';
    const lastColon = rest.lastIndexOf(':');
    if (lastColon >= 0) {
      const bracketClose = rest.startsWith('[') ? rest.indexOf(']') : -1;
      const isPortSuffix =
        bracketClose >= 0
          ? lastColon === bracketClose + 1
          : rest.indexOf(':') === lastColon;
      if (isPortSuffix) {
        const candidate = rest.slice(lastColon + 1);
        // A suffix carrying '/', '@', '?' or '#' is a path or
        // credentials tail, not a port — leave it for the URL parse.
        if (!/[/@?#]/.test(candidate)) {
          portText = candidate;
          hostPart = rest.slice(0, lastColon);
        }
      }
    }
    if (portText !== '') {
      const port = Number(portText);
      if (!/^\d+$/.test(portText) || port < 1 || port > 65535) {
        return { ok: false, error: ENDPOINT_ERROR.BAD_PORT };
      }
    }

    let url;
    try {
      url = new URL(`http://${hostPart}${portText === '' ? '' : `:${portText}`}`);
    } catch {
      return { ok: false, error: ENDPOINT_ERROR.NOT_A_URL };
    }
    if (url.username !== '' || url.password !== '') {
      return { ok: false, error: ENDPOINT_ERROR.CREDENTIALS };
    }
    if (url.pathname !== '/' || url.search !== '' || url.hash !== '') {
      return { ok: false, error: ENDPOINT_ERROR.PATH_OR_QUERY };
    }
    if (!isLoopbackHostname(url.hostname)) {
      return { ok: false, error: ENDPOINT_ERROR.NOT_LOOPBACK };
    }

    const host = url.hostname;
    const port = portText === '' ? DEFAULT_HTTP_PORT : Number(portText);
    const baseUrl = `http://${host}:${port}`;
    return {
      ok: true,
      endpoint: Object.freeze({
        baseUrl,
        address: `${host}:${port}`,
        host,
        port,
        permissionPattern: `${baseUrl}/*`,
      }),
    };
  }

  /// Normalize a persisted endpoint value. Anything that fails
  /// validation falls back to the default — a corrupt or hostile stored
  /// value must never aim the bearer token at a remote origin.
  function endpointForStoredValue(stored) {
    const parsed = parseLoopbackEndpoint(typeof stored === 'string' ? stored : '');
    return parsed.ok ? parsed.endpoint : DEFAULT_ENDPOINT;
  }

  /// Read the configured endpoint from a chrome.storage.local-shaped
  /// area. Legacy installs have only a token stored — a missing or
  /// unreadable endpoint migrates to the default instead of discarding
  /// the pairing the user already made.
  async function readHostEndpoint(storageArea) {
    try {
      const stored = await storageArea.get(ENDPOINT_STORAGE_KEY);
      return endpointForStoredValue(
        stored ? stored[ENDPOINT_STORAGE_KEY] : undefined,
      );
    } catch {
      return DEFAULT_ENDPOINT;
    }
  }

  function authHeaders(token, extra) {
    const headers = Object.assign({ Accept: 'application/json' }, extra);
    if (typeof token === 'string' && token.length > 0) {
      headers.Authorization = `Bearer ${token}`;
    }
    return headers;
  }

  /// Classify an already-read `POST /v1/tools/{id}` response. Split out
  /// from [`callTool`] so the whole status matrix is testable without a
  /// fetch double: every status the server can return here
  /// (200/400/401/403/404/422/503) carries a JSON body the caller needs
  /// to render, so classification replaces a throw-on-!ok contract.
  function classifyDispatchResponse(status, body) {
    if (isAuthStatus(status)) {
      return { kind: DISPATCH_RESULT_KIND.AUTH_ERROR };
    }
    if (status === HTTP_STATUS.SERVICE_UNAVAILABLE) {
      return {
        kind: DISPATCH_RESULT_KIND.HOST_UNAVAILABLE,
        hint: body && typeof body.hint === 'string' ? body.hint : null,
      };
    }
    if (body && body.ok === true) {
      return { kind: DISPATCH_RESULT_KIND.SUCCESS, result: body };
    }
    if (body && body.ok === false && body.error) {
      return { kind: DISPATCH_RESULT_KIND.FAILURE, error: body.error };
    }
    return { kind: DISPATCH_RESULT_KIND.FAILURE, error: { status }, status };
  }

  /// POST `args` (already coerced per the tool's `inputSchema`) to
  /// `/v1/tools/{toolId}`. Never throws on a non-2xx response — see
  /// [`classifyDispatchResponse`].
  async function callTool({
    fetchImpl,
    baseUrl = DEFAULT_HTTP_BASE_URL,
    token = null,
    board = null,
    toolId,
    args = {},
  }) {
    let requestBody;
    try {
      requestBody = serializeJsonRequestBody(args);
    } catch (error) {
      if (error instanceof RequestBodyTooLargeError) {
        return {
          kind: DISPATCH_RESULT_KIND.REQUEST_TOO_LARGE,
          actualBytes: error.actualBytes,
          limitBytes: error.limitBytes,
        };
      }
      throw error;
    }

    let response;
    try {
      const path = typeof board === 'string' && board.length > 0
        ? toolCallPath(board, toolId)
        : globalToolCallPath(toolId);
      response = await fetchImpl(`${baseUrl}${path}`, {
        method: 'POST',
        headers: authHeaders(token, { 'Content-Type': 'application/json' }),
        body: requestBody,
      });
    } catch {
      return { kind: DISPATCH_RESULT_KIND.NETWORK_ERROR };
    }

    if (isAuthStatus(response.status)) {
      return { kind: DISPATCH_RESULT_KIND.AUTH_ERROR };
    }
    let body = null;
    try {
      body = await response.json();
    } catch {
      body = null;
    }
    return classifyDispatchResponse(response.status, body);
  }

  /// The primary output row of a `DISPATCH_RESULT_KIND.SUCCESS` body, or
  /// `null` when the tool returned no outputs.
  function primaryOutput(result) {
    const outputs = result && result.outputs;
    if (!Array.isArray(outputs) || outputs.length === 0) return null;
    const primaryId = result.primary_output_id;
    return outputs.find((entry) => entry.id === primaryId) || outputs[0];
  }

  /// Display text for an output row's `value`: strings pass through, other
  /// JSON renders compactly (tooltips have no room for pretty-printing).
  function outputText(value) {
    if (typeof value === 'string') return value;
    if (value === null || value === undefined) return '';
    return JSON.stringify(value);
  }

  return Object.freeze({
    BOARDS_LIST_PATH,
    CONTENT_SCRIPT_BOARD,
    DEEP_LINK_ACTION,
    DEFAULT_ENDPOINT,
    DEFAULT_HTTP_ADDRESS,
    DEFAULT_HTTP_BASE_URL,
    DEFAULT_HTTP_HOST,
    DEFAULT_HTTP_PORT,
    DISPATCH_RESULT_KIND,
    ENDPOINT_ERROR,
    ENDPOINT_STORAGE_KEY,
    HTTP_STATUS,
    PAIRING_COMMAND,
    RUNTIME_MESSAGE,
    RUN_HINT_COMMAND,
    SURFACE_PARAM_VALUE,
    TOKEN_STORAGE_KEY,
    authHeaders,
    boardShowPath,
    buildDeepLink,
    callTool,
    classifyDispatchResponse,
    endpointForStoredValue,
    isAuthStatus,
    outputText,
    parseLoopbackEndpoint,
    primaryOutput,
    readHostEndpoint,
    toolCallPath,
  });
})();

if (typeof module === 'object' && module.exports) {
  module.exports = UpegHostApi;
}
