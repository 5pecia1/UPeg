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

test('탭_url은_포트를_뺀_호스트_매치_패턴이_된다', () => {
  // Chrome match patterns address hosts, never ports — the E2E page served
  // on :8000 must be enabled by `http://localhost/*`.
  assert.equal(matchPatternForUrl('http://localhost:8000/page.html'), 'http://localhost/*');
  assert.equal(matchPatternForUrl('https://etherscan.io/tx/0xff?a=b'), 'https://etherscan.io/*');
});

test('content_script가_붙을_수_없는_url은_패턴을_만들지_않는다', () => {
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

test('와일드카드_호스트_패턴은_기본_도메인과_하위_도메인을_모두_덮는다', () => {
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

test('잘못된_패턴은_파싱되지_않고_목록에서_걸러진다', () => {
  for (const pattern of ['', 'etherscan.io', 'https://', 'https:///*', null, 42]) {
    assert.equal(parseMatchPattern(pattern), null, String(pattern));
  }
  assert.deepEqual(normalizePatterns(['https://a.com/*', 'nope', 'https://a.com/*']), [
    'https://a.com/*',
  ]);
});

// --- allow-list --------------------------------------------------------

test('첫_실행은_etherscan과_polygonscan을_미리_켜진_상태로_심는다', async () => {
  const storage = mockStorage();
  const patterns = await ensureSeededPatterns(storage);

  assert.deepEqual(patterns, normalizePatterns(SEED_PATTERNS));
  assert.deepEqual(storage.data[ENABLED_PATTERNS_STORAGE_KEY], [...patterns]);
  assert.equal(isUrlEnabled(patterns, 'https://etherscan.io/tx/0xff'), true);
  assert.equal(isUrlEnabled(patterns, 'https://polygonscan.com/tx/0xff'), true);
  assert.equal(isUrlEnabled(patterns, 'https://example.com/'), false);
});

test('이미_저장된_목록은_빈_목록이어도_다시_심지_않는다', async () => {
  const storage = mockStorage({ [ENABLED_PATTERNS_STORAGE_KEY]: [] });
  assert.deepEqual(await ensureSeededPatterns(storage), []);
  assert.equal(storage.writes, 0, 'seeding an existing empty list would re-enable sites');
});

test('사이트_켜기는_패턴을_더하고_끄기는_그_url을_덮는_모든_패턴을_뺀다', () => {
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

test('저장은_정규화된_목록만_기록한다', async () => {
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

test('허가되지_않은_패턴은_granted_목록에서_빠진다', async () => {
  const permissions = mockPermissions({ granted: ['https://a.com/*'] });
  assert.deepEqual(await grantedPatterns(permissions, ['https://a.com/*', 'https://b.com/*']), [
    'https://a.com/*',
  ]);
});

test('권한_요청은_거부되면_false를_돌려준다', async () => {
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

test('권한_해제는_해당_패턴만_chrome에_넘긴다', async () => {
  const permissions = mockPermissions({ granted: ['http://localhost/*'] });
  assert.equal(await removeSitePermission(permissions, 'http://localhost/*'), true);
  assert.deepEqual(permissions.removed, ['http://localhost/*']);
  assert.equal(await removeSitePermission(permissions, 'nope'), false);
});

// --- dynamic registration ----------------------------------------------

test('등록_인자는_하나의_id로_모든_매치와_content_script_파일을_싣는다', () => {
  const [registration] = contentScriptRegistrations(['https://a.com/*', 'https://b.com/*']);
  assert.equal(registration.id, CONTENT_SCRIPT_ID);
  assert.deepEqual(registration.matches, ['https://a.com/*', 'https://b.com/*']);
  assert.deepEqual(registration.js, [...CONTENT_SCRIPT_FILES]);
  assert.equal(registration.persistAcrossSessions, true);
  assert.deepEqual(contentScriptRegistrations([]), []);
});

test('등록_diff는_새_등록과_갱신과_해제를_구분한다', () => {
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

test('동기화는_권한이_있는_패턴만_등록한다', async () => {
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

test('두_번째_동기화는_중복_등록_대신_갱신한다', async () => {
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

test('권한이_사라지면_동기화가_등록을_해제한다', async () => {
  const scripting = mockScripting();
  const permissions = mockPermissions({ granted: ['https://a.com/*'] });
  await syncContentScripts(scripting, permissions, ['https://a.com/*']);

  permissions.granted.delete('https://a.com/*');
  const live = await syncContentScripts(scripting, permissions, ['https://a.com/*']);

  assert.deepEqual(live, []);
  assert.deepEqual(scripting.calls.at(-1), ['unregister', [CONTENT_SCRIPT_ID]]);
  assert.deepEqual(scripting.registered, []);
});
