'use strict';

// upeg per-site enablement — which pages the content script runs on.
//
// The extension used to hard-code two blockchain explorers in the
// manifest's `content_scripts` block. That is the wrong shape for a
// capability whose whole point is "works on the page you are already on":
// every new site meant a new release. Instead the manifest declares
// `optional_host_permissions: ["<all_urls>"]` and this module owns
//
//   * the enabled match-pattern list (chrome.storage.local),
//   * the first-run seed that keeps etherscan/polygonscan working with no
//     user action (their patterns stay in `host_permissions`, so the seed
//     is already granted and registers without a prompt),
//   * turning a tab URL into a match pattern and back,
//   * the desired `chrome.scripting.registerContentScripts` state and the
//     diff against what is registered right now.
//
// Rules:
//   * Only granted patterns are ever registered — the user can revoke a
//     host permission in `chrome://extensions` behind the extension's
//     back, so every sync filters the allow-list through
//     `chrome.permissions.contains` first.
//   * Disabling a URL drops every pattern that covers it, not just the
//     exact one the toggle added — otherwise a seeded
//     `https://*.etherscan.io/*` would keep a subdomain enabled and the
//     toggle would look inert.
//   * Enabling injects into the current tab immediately
//     (`chrome.scripting.executeScript`) — a dynamic registration only
//     affects future loads.
//
// Everything above `// === chrome adapters ===` is pure. The adapters below
// it take the chrome API object as a parameter instead of reaching for the
// global, so chrome-ext/tests/site_access.test.js drives them with plain
// objects.

const UpegSiteAccess = (() => {
  const ENABLED_PATTERNS_STORAGE_KEY = 'upegEnabledSites';

  // Registration id for the single dynamic content-script registration.
  const CONTENT_SCRIPT_ID = 'upeg-in-page';
  // Load order matters: host_api.js reads `UpegWire` at evaluation time and
  // content.js reads all three helpers.
  const CONTENT_SCRIPT_FILES = Object.freeze([
    'wire.js',
    'host_api.js',
    'detectors.js',
    'selector_adapter.js',
    'content.js',
  ]);
  const CONTENT_SCRIPT_RUN_AT = 'document_idle';

  // Migration seed: the two explorers the hard-coded `content_scripts`
  // block used to match. They stay in `host_permissions`, so seeding the
  // list is enough to keep them working exactly as before an upgrade.
  const SEED_PATTERNS = Object.freeze([
    'https://etherscan.io/*',
    'https://*.etherscan.io/*',
    'https://polygonscan.com/*',
    'https://*.polygonscan.com/*',
  ]);

  const MATCH_PATTERN_PATH = '/*';
  const MATCH_PATTERN_SCHEME_SEPARATOR = '://';
  const HOST_WILDCARD_PREFIX = '*.';
  // Chrome match patterns address hosts, never ports: `http://localhost/*`
  // covers `http://localhost:8000/page.html`.
  const SUPPORTED_SCHEMES = Object.freeze(['http:', 'https:']);

  function parseMatchPattern(pattern) {
    if (typeof pattern !== 'string') return null;
    const separatorIndex = pattern.indexOf(MATCH_PATTERN_SCHEME_SEPARATOR);
    if (separatorIndex <= 0) return null;
    const scheme = `${pattern.slice(0, separatorIndex)}:`;
    const rest = pattern.slice(separatorIndex + MATCH_PATTERN_SCHEME_SEPARATOR.length);
    const pathIndex = rest.indexOf('/');
    if (pathIndex < 0) return null;
    const host = rest.slice(0, pathIndex);
    if (host.length === 0) return null;
    return { scheme, host };
  }

  /// The match pattern that enables `url`'s site, or `null` when the URL is
  /// not a page a content script can run on (`chrome://`, `about:`, a file
  /// URL, the extension's own pages).
  function matchPatternForUrl(url) {
    let parsed;
    try {
      parsed = new URL(url);
    } catch {
      return null;
    }
    if (!SUPPORTED_SCHEMES.includes(parsed.protocol)) return null;
    if (parsed.hostname.length === 0) return null;
    return `${parsed.protocol}//${parsed.hostname}${MATCH_PATTERN_PATH}`;
  }

  function matchPatternMatchesUrl(pattern, url) {
    const parsedPattern = parseMatchPattern(pattern);
    if (parsedPattern === null) return false;
    let parsedUrl;
    try {
      parsedUrl = new URL(url);
    } catch {
      return false;
    }
    if (parsedPattern.scheme !== parsedUrl.protocol) return false;
    if (!parsedPattern.host.startsWith(HOST_WILDCARD_PREFIX)) {
      return parsedPattern.host === parsedUrl.hostname;
    }
    // Chrome's `*.example.com` matches `example.com` itself as well as any
    // subdomain of it.
    const suffix = parsedPattern.host.slice(HOST_WILDCARD_PREFIX.length - 1);
    const bareHost = suffix.slice(1);
    return parsedUrl.hostname === bareHost || parsedUrl.hostname.endsWith(suffix);
  }

  function normalizePatterns(patterns) {
    if (!Array.isArray(patterns)) return [];
    const unique = new Set();
    for (const pattern of patterns) {
      if (parseMatchPattern(pattern) !== null) unique.add(pattern);
    }
    return Object.freeze([...unique].sort());
  }

  /// First-run migration: an absent list becomes the seed, an existing one
  /// (even an empty one — the user may have turned everything off) is kept.
  function seedEnabledPatterns(stored) {
    return Array.isArray(stored) ? normalizePatterns(stored) : normalizePatterns(SEED_PATTERNS);
  }

  function isUrlEnabled(patterns, url) {
    return normalizePatterns(patterns).some((pattern) => matchPatternMatchesUrl(pattern, url));
  }

  function enablePattern(patterns, pattern) {
    if (parseMatchPattern(pattern) === null) return normalizePatterns(patterns);
    return normalizePatterns([...normalizePatterns(patterns), pattern]);
  }

  /// Disabling a URL drops EVERY pattern that covers it, not just the exact
  /// one the toggle would add — otherwise turning `https://api.etherscan.io`
  /// off would leave the seeded `https://*.etherscan.io/*` still matching it
  /// and the toggle would appear to do nothing.
  function disableUrl(patterns, url) {
    return normalizePatterns(
      normalizePatterns(patterns).filter((pattern) => !matchPatternMatchesUrl(pattern, url)),
    );
  }

  /// The desired `chrome.scripting.registerContentScripts` argument for a
  /// set of enabled patterns. Empty when nothing is enabled — Chrome
  /// rejects a registration with no matches.
  function contentScriptRegistrations(patterns) {
    const matches = normalizePatterns(patterns);
    if (matches.length === 0) return [];
    return [
      {
        id: CONTENT_SCRIPT_ID,
        matches: [...matches],
        js: [...CONTENT_SCRIPT_FILES],
        runAt: CONTENT_SCRIPT_RUN_AT,
        persistAcrossSessions: true,
        allFrames: false,
      },
    ];
  }

  /// Split the desired registrations against what Chrome already has, so a
  /// sync is one `register`, one `update`, or one `unregister` — never a
  /// register that throws "duplicate id".
  function registrationDiff(existing, desired) {
    const existingIds = new Set(
      (Array.isArray(existing) ? existing : []).map((script) => script.id),
    );
    const toRegister = desired.filter((script) => !existingIds.has(script.id));
    const toUpdate = desired.filter((script) => existingIds.has(script.id));
    const desiredIds = new Set(desired.map((script) => script.id));
    const toUnregister = [...existingIds].filter((id) => !desiredIds.has(id));
    return { toRegister, toUpdate, toUnregister };
  }

  // === chrome adapters ===

  async function readEnabledPatterns(storageArea) {
    try {
      const stored = await storageArea.get(ENABLED_PATTERNS_STORAGE_KEY);
      return seedEnabledPatterns(stored ? stored[ENABLED_PATTERNS_STORAGE_KEY] : undefined);
    } catch {
      return normalizePatterns(SEED_PATTERNS);
    }
  }

  /// Read the enabled list, writing the migration seed on first run. The
  /// seed is written (not just returned) so the popup toggle has a real
  /// list to remove from — and so a later release changing `SEED_PATTERNS`
  /// never silently re-enables a site the user turned off.
  async function ensureSeededPatterns(storageArea) {
    let stored = null;
    try {
      stored = await storageArea.get(ENABLED_PATTERNS_STORAGE_KEY);
    } catch {
      stored = null;
    }
    const raw = stored ? stored[ENABLED_PATTERNS_STORAGE_KEY] : undefined;
    if (Array.isArray(raw)) return normalizePatterns(raw);
    return writeEnabledPatterns(storageArea, SEED_PATTERNS);
  }

  async function writeEnabledPatterns(storageArea, patterns) {
    const normalized = normalizePatterns(patterns);
    await storageArea.set({ [ENABLED_PATTERNS_STORAGE_KEY]: [...normalized] });
    return normalized;
  }

  /// The subset of `patterns` Chrome currently holds a host permission for.
  /// Registering a match without its permission throws, and a permission
  /// can disappear behind the extension's back (the user revokes it in
  /// chrome://extensions), so registration always goes through this filter.
  async function grantedPatterns(permissionsApi, patterns) {
    const normalized = normalizePatterns(patterns);
    const verdicts = await Promise.all(
      normalized.map(async (pattern) => {
        try {
          return await permissionsApi.contains({ origins: [pattern] });
        } catch {
          return false;
        }
      }),
    );
    return Object.freeze(normalized.filter((_, index) => verdicts[index]));
  }

  async function requestSitePermission(permissionsApi, pattern) {
    if (parseMatchPattern(pattern) === null) return false;
    try {
      return (await permissionsApi.request({ origins: [pattern] })) === true;
    } catch {
      return false;
    }
  }

  async function removeSitePermission(permissionsApi, pattern) {
    if (parseMatchPattern(pattern) === null) return false;
    try {
      return (await permissionsApi.remove({ origins: [pattern] })) === true;
    } catch {
      return false;
    }
  }

  /// Make the registered content scripts equal `contentScriptRegistrations`
  /// of the granted subset of `patterns`. Returns the matches now live.
  async function syncContentScripts(scriptingApi, permissionsApi, patterns) {
    const granted = await grantedPatterns(permissionsApi, patterns);
    const desired = contentScriptRegistrations(granted);
    const existing = await scriptingApi.getRegisteredContentScripts();
    const { toRegister, toUpdate, toUnregister } = registrationDiff(existing, desired);
    if (toUnregister.length > 0) {
      await scriptingApi.unregisterContentScripts({ ids: toUnregister });
    }
    if (toUpdate.length > 0) {
      await scriptingApi.updateContentScripts(toUpdate);
    }
    if (toRegister.length > 0) {
      await scriptingApi.registerContentScripts(toRegister);
    }
    return granted;
  }

  return Object.freeze({
    CONTENT_SCRIPT_FILES,
    CONTENT_SCRIPT_ID,
    CONTENT_SCRIPT_RUN_AT,
    ENABLED_PATTERNS_STORAGE_KEY,
    SEED_PATTERNS,
    contentScriptRegistrations,
    disableUrl,
    enablePattern,
    ensureSeededPatterns,
    grantedPatterns,
    isUrlEnabled,
    matchPatternForUrl,
    matchPatternMatchesUrl,
    normalizePatterns,
    parseMatchPattern,
    readEnabledPatterns,
    registrationDiff,
    removeSitePermission,
    requestSitePermission,
    seedEnabledPatterns,
    syncContentScripts,
    writeEnabledPatterns,
  });
})();

if (typeof module === 'object' && module.exports) {
  module.exports = UpegSiteAccess;
}
