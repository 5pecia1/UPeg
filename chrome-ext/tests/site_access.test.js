'use strict';

// Per-site enablement (chrome-ext/site_access.js) — the allow-list, the
// permission gate, and the dynamic content-script registration.
//
// The chrome.* adapters take their API object as a parameter, so the mocks
// below are plain objects; nothing here installs a global `chrome`.

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  CONTENT_SCRIPT_FILES,
  CONTENT_SCRIPT_ID,
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
  registrationDiff,
  removeSitePermission,
  requestSitePermission,
  syncContentScripts,
  writeEnabledPatterns,
} = require('../site_access.js');

// --- mocks -------------------------------------------------------------

function mockStorage(initial) {
  const data = initial === undefined ? {} : { ...initial };
  return {
    data,
    writes: 0,
    async get(key) {
      return Object.prototype.hasOwnProperty.call(data, key) ? { [key]: data[key] } : {};
    },
    async set(entries) {
      this.writes += 1;
      Object.assign(data, entries);
    },
  };
}

function mockPermissions({ granted = [], grant = true } = {}) {
  return {
    granted: new Set(granted),
    requested: [],
    removed: [],
    async contains({ origins }) {
      return origins.every((origin) => this.granted.has(origin));
    },
    async request({ origins }) {
      this.requested.push(...origins);
      if (!grant) return false;
      origins.forEach((origin) => this.granted.add(origin));
      return true;
    },
    async remove({ origins }) {
      this.removed.push(...origins);
      origins.forEach((origin) => this.granted.delete(origin));
      return true;
    },
  };
}

function mockScripting(existing = []) {
  return {
    registered: [...existing],
    calls: [],
    async getRegisteredContentScripts() {
      return [...this.registered];
    },
    async registerContentScripts(scripts) {
      this.calls.push(['register', scripts.map((s) => s.id)]);
      this.registered.push(...scripts);
    },
    async updateContentScripts(scripts) {
      this.calls.push(['update', scripts.map((s) => s.id)]);
      this.registered = this.registered.map(
        (existingScript) =>
          scripts.find((script) => script.id === existingScript.id) || existingScript,
      );
    },
    async unregisterContentScripts({ ids }) {
      this.calls.push(['unregister', ids]);
      this.registered = this.registered.filter((script) => !ids.includes(script.id));
    },
  };
}

// --- match patterns ----------------------------------------------------

test('a_tab_url_becomes_a_host_match_pattern_without_the_port', () => {
  // Chrome match patterns address hosts, never ports — the E2E page served
  // on :8000 must be enabled by `http://localhost/*`.
  assert.equal(matchPatternForUrl('http://localhost:8000/page.html'), 'http://localhost/*');
  assert.equal(matchPatternForUrl('https://etherscan.io/tx/0xff?a=b'), 'https://etherscan.io/*');
});

test('urls_a_content_script_cannot_attach_to_produce_no_pattern', () => {
  for (const url of [
    'chrome://extensions',
    'about:blank',
    'file:///tmp/page.html',
    'chrome-extension://abc/popup.html',
    'not a url',
  ]) {
    assert.equal(matchPatternForUrl(url), null, url);
  }
});

test('a_wildcard_host_pattern_covers_the_apex_and_subdomains', () => {
  assert.equal(matchPatternMatchesUrl('https://*.etherscan.io/*', 'https://etherscan.io/x'), true);
  assert.equal(
    matchPatternMatchesUrl('https://*.etherscan.io/*', 'https://api.etherscan.io/x'),
    true,
  );
  assert.equal(
    matchPatternMatchesUrl('https://*.etherscan.io/*', 'https://notetherscan.io/x'),
    false,
  );
  // The scheme is part of the pattern.
  assert.equal(matchPatternMatchesUrl('https://etherscan.io/*', 'http://etherscan.io/x'), false);
});

test('invalid_patterns_do_not_parse_and_are_filtered_out_of_the_list', () => {
  for (const pattern of ['', 'etherscan.io', 'https://', 'https:///*', null, 42]) {
    assert.equal(parseMatchPattern(pattern), null, String(pattern));
  }
  assert.deepEqual(normalizePatterns(['https://a.com/*', 'nope', 'https://a.com/*']), [
    'https://a.com/*',
  ]);
});

// --- allow-list --------------------------------------------------------

test('the_first_run_seeds_etherscan_and_polygonscan_pre_enabled', async () => {
  const storage = mockStorage();
  const patterns = await ensureSeededPatterns(storage);

  assert.deepEqual(patterns, normalizePatterns(SEED_PATTERNS));
  assert.deepEqual(storage.data[ENABLED_PATTERNS_STORAGE_KEY], [...patterns]);
  assert.equal(isUrlEnabled(patterns, 'https://etherscan.io/tx/0xff'), true);
  assert.equal(isUrlEnabled(patterns, 'https://polygonscan.com/tx/0xff'), true);
  assert.equal(isUrlEnabled(patterns, 'https://example.com/'), false);
});

test('an_already_stored_list_is_never_reseeded_even_when_empty', async () => {
  const storage = mockStorage({ [ENABLED_PATTERNS_STORAGE_KEY]: [] });
  assert.deepEqual(await ensureSeededPatterns(storage), []);
  assert.equal(storage.writes, 0, 'seeding an existing empty list would re-enable sites');
});

test('enabling_a_site_adds_its_pattern_and_disabling_drops_every_covering_pattern', () => {
  const enabled = enablePattern(SEED_PATTERNS, 'http://localhost/*');
  assert.equal(isUrlEnabled(enabled, 'http://localhost:8000/x'), true);

  // The seeded wildcard covers the subdomain too, so a naive "remove the
  // exact pattern" would leave the site enabled and the toggle inert.
  const disabled = disableUrl(enabled, 'https://api.etherscan.io/x');
  assert.equal(isUrlEnabled(disabled, 'https://api.etherscan.io/x'), false);
  assert.equal(disabled.includes('https://*.etherscan.io/*'), false);
  assert.equal(disabled.includes('https://etherscan.io/*'), true);
  assert.equal(isUrlEnabled(disabled, 'http://localhost:8000/x'), true);
});

test('saving_writes_only_the_normalized_list', async () => {
  const storage = mockStorage();
  const written = await writeEnabledPatterns(storage, [
    'https://b.com/*',
    'garbage',
    'https://a.com/*',
    'https://a.com/*',
  ]);
  assert.deepEqual(written, ['https://a.com/*', 'https://b.com/*']);
  assert.deepEqual(storage.data[ENABLED_PATTERNS_STORAGE_KEY], ['https://a.com/*', 'https://b.com/*']);
});

// --- permissions -------------------------------------------------------

test('ungranted_patterns_are_left_out_of_the_granted_list', async () => {
  const permissions = mockPermissions({ granted: ['https://a.com/*'] });
  assert.deepEqual(await grantedPatterns(permissions, ['https://a.com/*', 'https://b.com/*']), [
    'https://a.com/*',
  ]);
});

test('a_denied_permission_request_returns_false', async () => {
  const granting = mockPermissions({ grant: true });
  assert.equal(await requestSitePermission(granting, 'http://localhost/*'), true);
  assert.deepEqual(granting.requested, ['http://localhost/*']);

  const denying = mockPermissions({ grant: false });
  assert.equal(await requestSitePermission(denying, 'http://localhost/*'), false);

  // A malformed pattern never reaches Chrome.
  const untouched = mockPermissions();
  assert.equal(await requestSitePermission(untouched, 'nope'), false);
  assert.deepEqual(untouched.requested, []);
});

test('revoking_hands_chrome_only_that_pattern', async () => {
  const permissions = mockPermissions({ granted: ['http://localhost/*'] });
  assert.equal(await removeSitePermission(permissions, 'http://localhost/*'), true);
  assert.deepEqual(permissions.removed, ['http://localhost/*']);
  assert.equal(await removeSitePermission(permissions, 'nope'), false);
});

// --- dynamic registration ----------------------------------------------

test('the_registration_args_carry_every_match_and_content_file_under_one_id', () => {
  const [registration] = contentScriptRegistrations(['https://a.com/*', 'https://b.com/*']);
  assert.equal(registration.id, CONTENT_SCRIPT_ID);
  assert.deepEqual(registration.matches, ['https://a.com/*', 'https://b.com/*']);
  assert.deepEqual(registration.js, [...CONTENT_SCRIPT_FILES]);
  assert.equal(registration.persistAcrossSessions, true);
  assert.deepEqual(contentScriptRegistrations([]), []);
});

test('the_registration_diff_separates_new_updates_and_removals', () => {
  const desired = contentScriptRegistrations(['https://a.com/*']);
  const fresh = registrationDiff([], desired);
  assert.deepEqual(fresh.toRegister.map((s) => s.id), [CONTENT_SCRIPT_ID]);
  assert.deepEqual(fresh.toUpdate, []);
  assert.deepEqual(fresh.toUnregister, []);

  const existing = registrationDiff([{ id: CONTENT_SCRIPT_ID }], desired);
  assert.deepEqual(existing.toRegister, []);
  assert.deepEqual(existing.toUpdate.map((s) => s.id), [CONTENT_SCRIPT_ID]);

  const emptied = registrationDiff([{ id: CONTENT_SCRIPT_ID }], []);
  assert.deepEqual(emptied.toUnregister, [CONTENT_SCRIPT_ID]);
});

test('sync_registers_only_the_patterns_with_permission', async () => {
  const scripting = mockScripting();
  const permissions = mockPermissions({ granted: ['https://a.com/*'] });

  const live = await syncContentScripts(scripting, permissions, [
    'https://a.com/*',
    'https://ungranted.com/*',
  ]);

  assert.deepEqual(live, ['https://a.com/*']);
  assert.deepEqual(scripting.calls, [['register', [CONTENT_SCRIPT_ID]]]);
  assert.deepEqual(scripting.registered[0].matches, ['https://a.com/*']);
});

test('a_second_sync_updates_instead_of_duplicate_registering', async () => {
  const scripting = mockScripting();
  const permissions = mockPermissions({ granted: ['https://a.com/*', 'https://b.com/*'] });

  await syncContentScripts(scripting, permissions, ['https://a.com/*']);
  await syncContentScripts(scripting, permissions, ['https://a.com/*', 'https://b.com/*']);

  assert.deepEqual(scripting.calls, [
    ['register', [CONTENT_SCRIPT_ID]],
    ['update', [CONTENT_SCRIPT_ID]],
  ]);
  assert.equal(scripting.registered.length, 1);
  assert.deepEqual(scripting.registered[0].matches, ['https://a.com/*', 'https://b.com/*']);
});

test('sync_unregisters_once_the_permission_disappears', async () => {
  const scripting = mockScripting();
  const permissions = mockPermissions({ granted: ['https://a.com/*'] });
  await syncContentScripts(scripting, permissions, ['https://a.com/*']);

  permissions.granted.delete('https://a.com/*');
  const live = await syncContentScripts(scripting, permissions, ['https://a.com/*']);

  assert.deepEqual(live, []);
  assert.deepEqual(scripting.calls.at(-1), ['unregister', [CONTENT_SCRIPT_ID]]);
  assert.deepEqual(scripting.registered, []);
});
