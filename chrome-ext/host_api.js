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

  const HTTP_HOST = '127.0.0.1';
  const HTTP_PORT = 7173;
  const HTTP_BASE_URL = `http://${HTTP_HOST}:${HTTP_PORT}`;
  const HTTP_ADDRESS = `${HTTP_HOST}:${HTTP_PORT}`;
  const RUN_HINT_COMMAND = `upeg http --addr ${HTTP_ADDRESS}`;

  const BOARDS_LIST_PATH = '/v1/boards';
  const boardShowPath = (board) => `/v1/boards/${encodeURIComponent(board)}`;
  const toolCallPath = (toolId) => `/v1/tools/${encodeURIComponent(toolId)}`;

  // Non-/healthz routes require `Authorization: Bearer <token>`. The token is
  // user-supplied (pasted from the host's config / start-up log) and lives
  // only in this extension's local storage — never logged, never sent
  // anywhere but HTTP_BASE_URL above.
  const TOKEN_STORAGE_KEY = 'upegDaemonToken';

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
    baseUrl = HTTP_BASE_URL,
    token = null,
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
      response = await fetchImpl(`${baseUrl}${toolCallPath(toolId)}`, {
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
    DISPATCH_RESULT_KIND,
    HTTP_ADDRESS,
    HTTP_BASE_URL,
    HTTP_HOST,
    HTTP_PORT,
    HTTP_STATUS,
    RUNTIME_MESSAGE,
    RUN_HINT_COMMAND,
    SURFACE_PARAM_VALUE,
    TOKEN_STORAGE_KEY,
    authHeaders,
    boardShowPath,
    buildDeepLink,
    callTool,
    classifyDispatchResponse,
    isAuthStatus,
    outputText,
    primaryOutput,
    toolCallPath,
  });
})();

if (typeof module === 'object' && module.exports) {
  module.exports = UpegHostApi;
}
