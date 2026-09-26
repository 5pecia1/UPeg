'use strict';

// upeg MV3 service worker — the extension's only privileged actor.
//
// It exists for two things a content script cannot do for itself:
//
//   1. **Dynamic content-script registration.** The manifest no longer
//      hard-codes which sites get the in-page detectors; the enabled list
//      lives in storage (site_access.js) and is compiled into a
//      `chrome.scripting` registration here, on install, on browser start,
//      whenever the popup toggles a site, and whenever Chrome tells us a
//      host permission went away.
//   2. **Host dispatch.** A content script's `fetch` is subject to the
//      *page's* CORS, not the extension's host permissions, so the page
//      side asks for a tool run by message and this worker performs it.
//      The bearer token therefore never enters a web page's world.
//
// Extension JavaScript stays dependency-free; rich execution remains in
// the Desktop/PWA/host surfaces.

importScripts('wire.js', 'host_api.js', 'site_access.js');

(() => {
  const {
    DEFAULT_ENDPOINT,
    DISPATCH_RESULT_KIND,
    ENDPOINT_STORAGE_KEY,
    RUNTIME_MESSAGE,
    TOKEN_STORAGE_KEY,
    callTool,
    endpointForStoredValue,
  } = UpegHostApi;
  const { ensureSeededPatterns, syncContentScripts } = UpegSiteAccess;

  // Token + endpoint are read together on every dispatch — the worker
  // caches neither, so a pairing change in the popup takes effect on the
  // next request without a restart. The endpoint is re-validated on read
  // (endpointForStoredValue): a corrupt or hostile stored value falls
  // back to the default rather than aiming the token elsewhere.
  async function readPairing() {
    try {
      const stored = await chrome.storage.local.get([
        TOKEN_STORAGE_KEY,
        ENDPOINT_STORAGE_KEY,
      ]);
      const value = stored[TOKEN_STORAGE_KEY];
      return {
        token: typeof value === 'string' && value.length > 0 ? value : null,
        endpoint: endpointForStoredValue(stored[ENDPOINT_STORAGE_KEY]),
      };
    } catch {
      return { token: null, endpoint: DEFAULT_ENDPOINT };
    }
  }

  async function syncSites() {
    const patterns = await ensureSeededPatterns(chrome.storage.local);
    return syncContentScripts(chrome.scripting, chrome.permissions, patterns);
  }

  async function dispatchTool(message) {
    const toolId = typeof message.toolId === 'string' ? message.toolId : null;
    if (toolId === null) {
      return { kind: DISPATCH_RESULT_KIND.FAILURE, error: { message: 'missing toolId' } };
    }
    const pairing = await readPairing();
    return callTool({
      fetchImpl: fetch,
      baseUrl: pairing.endpoint.baseUrl,
      token: pairing.token,
      toolId,
      args: message.args && typeof message.args === 'object' ? message.args : {},
    });
  }

  // Every branch resolves to a value the sender can render; a thrown
  // rejection would surface as an opaque `chrome.runtime.lastError`.
  async function handleMessage(message) {
    switch (message && message.type) {
      case RUNTIME_MESSAGE.DISPATCH_TOOL:
        return dispatchTool(message);
      case RUNTIME_MESSAGE.SYNC_SITES:
        return { matches: await syncSites() };
      default:
        return null;
    }
  }

  chrome.runtime.onInstalled.addListener(() => {
    syncSites();
  });
  chrome.runtime.onStartup.addListener(() => {
    syncSites();
  });
  // A revoked host permission must drop the matching registration, or
  // Chrome keeps a registration it will refuse to inject.
  chrome.permissions.onRemoved.addListener(() => {
    syncSites();
  });

  chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
    handleMessage(message).then(sendResponse);
    // Keep the message channel open for the async reply above.
    return true;
  });
})();
