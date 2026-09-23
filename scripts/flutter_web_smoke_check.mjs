// Boot + service worker cache contract check for the Flutter Web (PWA) bundle.
//
// `scripts/flutter_web_smoke.sh` checks the prerequisites (build output,
// chromium, python3) and then calls this script. Here we own the static
// server and headless Chromium lifetimes and assert the contract — the
// offline phase must actually bring the server **down**, so both lifetimes
// live in one place.
//
// Asserted contract (`pwa.service-worker.cache` in the inventory):
//   1. The app shell boots on first load (one flutter-view, the FRB script
//      injected exactly once).
//   2. `navigator.serviceWorker.ready` resolves to `upeg_service_worker.js`
//      activated, and that SW controls this page.
//   3. Cache storage holds exactly one app shell cache containing every
//      entry needed to boot offline.
//   4. A new build goes live **on that load**. Swap the SW version token in
//      the served bootstrap and reload once: the service worker with the new
//      token takes the page on that load and a differently named shell cache
//      appears. This assertion used to fail when the bootstrap was served
//      stale-while-revalidate — the new build arrived one load late.
//   5. With the static server down and the browser offline, a reload still
//      boots the app from cache. Because the server is really dead, a fake
//      "the network answered" pass is impossible.
//
// No npm dependencies — only Node 22+ globals `fetch`/`WebSocket` and
// builtins.

import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import {
  findFreePort,
  launchHeadlessChromium,
  openPageSession,
  waitFor,
} from './lib/cdp_client.mjs';

const LOOPBACK_HOST = '127.0.0.1';
const PORT_SCAN_ATTEMPTS = 50;
const SERVICE_WORKER_FILE_NAME = 'upeg_service_worker.js';
/**
 * The registration URL carries a `?v=<version>` that changes per build (the
 * value becomes the shell cache name). Script identity is therefore compared
 * by path.
 */
const serviceWorkerPath = (url) =>
  new URL(SERVICE_WORKER_FILE_NAME, url).pathname;
const scriptPathOf = (scriptUrl) =>
  scriptUrl ? new URL(scriptUrl).pathname : null;
const APP_SHELL_CACHE_PREFIX = 'upeg-app-shell-';
const ACTIVATED_STATE = 'activated';
/** Build token carried by the registration URL; the shell cache name derives from it. */
const CACHE_VERSION_PARAM = 'v';
const cacheVersionOf = (scriptUrl) =>
  scriptUrl ? new URL(scriptUrl).searchParams.get(CACHE_VERSION_PARAM) : null;
const BOOTSTRAP_FILE_NAME = 'flutter_bootstrap.js';
/** Where `web/flutter_bootstrap.js` receives the token — our own declaration. */
const SERVICE_WORKER_VERSION_DECL = 'const UPEG_SERVICE_WORKER_VERSION = ';
const REBUILT_VERSION_SUFFIX = '-rebuilt';
const FRB_SCRIPT_SELECTOR = 'script[src="pkg/upeg_frb.js"]';
const APP_ROOT_SELECTOR = 'flutter-view';
const PAGE_TITLE = 'upeg';
const OFFLINE_PROBE_QUERY = '?upeg-offline-probe';
const SERVICE_WORKER_READY_PROBE_TIMEOUT_MS = 1000;
const DOM_SNAPSHOT_FILE = 'dom.html';
const CHROMIUM_LOG_FILE = 'chromium.log';
const SERVER_LOG_FILE = 'server.log';
const APP_READY_LOG_FRAGMENT =
  'upeg: appInitProvider — desktop integrations done, painting BoardPage';
const APP_FATAL_LOG_FRAGMENTS = [
  'fail to create WorkerPool',
  'RuntimeError: unreachable',
  'boot failed:',
];
const CONSOLE_CAPTURE_SCRIPT = `(() => {
  const messages = [];
  const unhandledErrors = [];
  const consoleErrors = [];
  Object.defineProperty(window, '__upegBootMessages', { value: messages });
  Object.defineProperty(window, '__upegUnhandledErrors', { value: unhandledErrors });
  Object.defineProperty(window, '__upegConsoleErrors', { value: consoleErrors });
  for (const level of ['debug', 'log', 'info', 'warn', 'error']) {
    const original = console[level].bind(console);
    console[level] = (...args) => {
      const message = args.map((value) => String(value)).join(' ');
      messages.push(message);
      if (level === 'error') consoleErrors.push(message);
      original(...args);
    };
  }
  addEventListener('error', (event) => unhandledErrors.push(String(event.error || event.message)));
  addEventListener('unhandledrejection', (event) => unhandledErrors.push(String(event.reason)));
})();`;

// FRB's web worker pool transfers WebAssembly.Memory between workers. That is
// legal only in a cross-origin-isolated page, so the smoke server must match
// the production deployment header contract instead of merely serving files.
const STATIC_SERVER_SCRIPT = String.raw`
import http.server
import sys

port = int(sys.argv[1])
root = sys.argv[2]

class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=root, **kwargs)

    def end_headers(self):
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        self.send_header('Cross-Origin-Resource-Policy', 'same-origin')
        super().end_headers()

http.server.ThreadingHTTPServer(('127.0.0.1', port), Handler).serve_forever()
`;

/** The shell needed to boot offline — the fixed paths. */
const REQUIRED_SHELL_PATHS = [
  '/',
  '/flutter_bootstrap.js',
  '/manifest.json',
  '/main.dart.js',
  '/pkg/upeg_frb.js',
  '/pkg/upeg_frb_bg.wasm',
];
/**
 * CanvasKit is fetched from a different subdirectory per browser
 * (`canvaskit/canvaskit.js` vs `canvaskit/chromium/canvaskit.js`). We only
 * check that the renderer was cached from our origin — served from the CDN
 * it cannot boot offline.
 */
const REQUIRED_SHELL_SUFFIXES = ['/canvaskit.js', '/canvaskit.wasm'];

/** Contract violation — reported separately from plumbing failures (`CdpError`). */
class ContractViolation extends Error {}

function requireEnv(name) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`missing required environment variable ${name}`);
  }
  return value;
}

function assert(condition, message) {
  if (!condition) {
    throw new ContractViolation(message);
  }
}

function pass(message) {
  console.log(`  ok  ${message}`);
}

/**
 * Wait until the contract holds. A timeout is a contract violation, not a
 * plumbing failure — "the SW did not reach activated within 60s" means the
 * contract is broken.
 */
async function awaitContract(label, timeoutMs, probe) {
  try {
    return await waitFor(label, timeoutMs, probe);
  } catch (error) {
    throw new ContractViolation(error.message);
  }
}

function startStaticServer({ buildDir, port, logPath }) {
  const child = spawn(
    'python3',
    ['-c', STATIC_SERVER_SCRIPT, String(port), buildDir],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );
  const chunks = [];
  child.stdout.on('data', (chunk) => chunks.push(chunk));
  child.stderr.on('data', (chunk) => chunks.push(chunk));
  return {
    child,
    flushLog: () => writeFileSync(logPath, Buffer.concat(chunks)),
  };
}

async function waitForServer(url, timeoutMs) {
  await waitFor('static server to answer', timeoutMs, async () => {
    try {
      const response = await fetch(url, { cache: 'no-store' });
      return { ok: response.status < 500, detail: `HTTP ${response.status}` };
    } catch (error) {
      return { ok: false, detail: error.message };
    }
  });
}

async function stopStaticServer(server, url, timeoutMs) {
  if (server.child.exitCode === null) {
    server.child.kill('SIGTERM');
    await once(server.child, 'exit');
  }
  await waitFor('static server to stop answering', timeoutMs, async () => {
    try {
      await fetch(url, { cache: 'no-store' });
      return { ok: false, detail: 'still answering' };
    } catch {
      return { ok: true };
    }
  });
}

const countSelector = (selector) =>
  `document.querySelectorAll(${JSON.stringify(selector)}).length`;

const SERVICE_WORKER_PROBE = `Promise.race([
  navigator.serviceWorker.ready.then((registration) => ({
    scriptUrl: registration.active && registration.active.scriptURL,
    state: registration.active && registration.active.state,
    scope: registration.scope,
    controller:
      navigator.serviceWorker.controller &&
      navigator.serviceWorker.controller.scriptURL,
  })),
  new Promise((resolve) =>
    setTimeout(() => resolve(null), ${SERVICE_WORKER_READY_PROBE_TIMEOUT_MS}),
  ),
])`;

const CACHE_CONTENTS_PROBE = `caches.keys().then(async (names) => {
  const contents = {};
  for (const name of names) {
    const cache = await caches.open(name);
    contents[name] = (await cache.keys()).map((request) => request.url);
  }
  return contents;
})`;

const offlineProbe = (url) => `fetch(${JSON.stringify(
  url + OFFLINE_PROBE_QUERY,
)}, { cache: 'no-store' }).then(() => 'reachable').catch(() => 'unreachable')`;

async function waitForAppShell(session, label, timeoutMs) {
  await awaitContract(label, timeoutMs, async () => {
    try {
      const count = await session.evaluate(countSelector(APP_ROOT_SELECTOR));
      return { ok: count === 1, detail: `${APP_ROOT_SELECTOR}=${count}` };
    } catch (error) {
      return { ok: false, detail: error.message };
    }
  });
}

async function waitForAppReady(session, label, timeoutMs) {
  await awaitContract(label, timeoutMs, async () => {
    const state = await session.evaluate(`(() => {
      const messages = window.__upegBootMessages || [];
      const unhandledErrors = window.__upegUnhandledErrors || [];
      const consoleErrors = window.__upegConsoleErrors || [];
      return {
        isolated: crossOriginIsolated,
        ready: messages.some((line) => line.includes(${JSON.stringify(
          APP_READY_LOG_FRAGMENT,
        )})),
        fatal:
          unhandledErrors[0] ||
          messages.find((line) => ${JSON.stringify(
            APP_FATAL_LOG_FRAGMENTS,
          )}.some((fragment) => line.includes(fragment))) ||
          null,
        tail: messages.slice(-8),
        consoleErrors: consoleErrors.slice(-8),
      };
    })()`);
    if (!state.isolated) {
      throw new ContractViolation(
        `${label}: page is not cross-origin isolated; deploy COOP/COEP headers`,
      );
    }
    if (state.fatal) {
      throw new ContractViolation(`${label}: ${state.fatal}`);
    }
    return {
      ok: state.ready,
      detail: JSON.stringify({ tail: state.tail, consoleErrors: state.consoleErrors }),
    };
  });
}

async function assertBootedShell(session, phase) {
  const frbScripts = await session.evaluate(countSelector(FRB_SCRIPT_SELECTOR));
  assert(
    frbScripts === 1,
    `${phase}: FRB script (${FRB_SCRIPT_SELECTOR}) count is ${frbScripts} — must be exactly 1`,
  );
  const title = await session.evaluate('document.title');
  assert(
    title === PAGE_TITLE,
    `${phase}: document.title is '${title}' — must be '${PAGE_TITLE}'`,
  );
}

async function assertServiceWorkerActivated(session, url, timeoutMs) {
  const { detail: registration } = await awaitContract(
    'service worker activation',
    timeoutMs,
    async () => {
      try {
        const value = await session.evaluate(SERVICE_WORKER_PROBE);
        return { ok: Boolean(value), detail: value };
      } catch (error) {
        return { ok: false, detail: error.message };
      }
    },
  );

  const expectedPath = serviceWorkerPath(url);
  assert(
    scriptPathOf(registration.scriptUrl) === expectedPath,
    `active service worker is ${registration.scriptUrl} — must be ${expectedPath}`,
  );
  assert(
    registration.state === ACTIVATED_STATE,
    `service worker state is '${registration.state}' — must be '${ACTIVATED_STATE}'`,
  );
  assert(
    registration.scope === url,
    `service worker scope is ${registration.scope} — must be ${url}`,
  );
  assert(
    scriptPathOf(registration.controller) === expectedPath,
    `first load is not controlled by the service worker (controller=${registration.controller})`,
  );
  return registration;
}

function missingShellEntries(cachedUrls) {
  const paths = cachedUrls.map((cached) => new URL(cached).pathname);
  const missing = REQUIRED_SHELL_PATHS.filter((path) => !paths.includes(path));
  const missingSuffixes = REQUIRED_SHELL_SUFFIXES.filter(
    (suffix) => !paths.some((path) => path.endsWith(suffix)),
  );
  return [...missing, ...missingSuffixes.map((suffix) => `*${suffix}`)];
}

async function assertAppShellCached(session, timeoutMs) {
  const { detail: contents } = await awaitContract(
    'app shell cache populated',
    timeoutMs,
    async () => {
      try {
        const value = await session.evaluate(CACHE_CONTENTS_PROBE);
        const names = Object.keys(value);
        const shellCaches = names.filter((name) =>
          name.startsWith(APP_SHELL_CACHE_PREFIX),
        );
        if (shellCaches.length !== 1) {
          return { ok: false, detail: `caches=${JSON.stringify(names)}` };
        }
        const missing = missingShellEntries(value[shellCaches[0]]);
        return missing.length === 0
          ? { ok: true, detail: value }
          : { ok: false, detail: `missing ${missing.join(', ')}` };
      } catch (error) {
        return { ok: false, detail: error.message };
      }
    },
  );

  const names = Object.keys(contents);
  assert(names.length > 0, 'cache storage is empty');
  const shellCaches = names.filter((name) =>
    name.startsWith(APP_SHELL_CACHE_PREFIX),
  );
  assert(
    shellCaches.length === 1,
    `'${APP_SHELL_CACHE_PREFIX}*' caches: ${shellCaches.length} — must be exactly 1 (${names.join(', ')})`,
  );
  return { cacheName: shellCaches[0], urls: contents[shellCaches[0]] };
}

/**
 * Simulate a new build: swap only the SW version token in the served
 * bootstrap.
 *
 * Re-running `flutter build web` mid-smoke costs minutes — more than a
 * contract check can afford. And the token is exactly the one thing a new
 * build changes in this file — Flutter fills
 * `{{flutter_service_worker_version}}` with a fresh random value per build
 * (see the `web/upeg_service_worker.js` header). So replacing the token *is*
 * "a new build was deployed".
 *
 * The caller owns the restore — the server must keep serving the new token
 * until the smoke ends, and the build output must be back to normal
 * afterwards.
 */
function rebuildServedBootstrap(buildDir, currentVersion) {
  const path = join(buildDir, BOOTSTRAP_FILE_NAME);
  const original = readFileSync(path, 'utf8');
  const wanted = `${SERVICE_WORKER_VERSION_DECL}"${currentVersion}"`;
  if (!original.includes(wanted)) {
    throw new ContractViolation(
      `\`${wanted}\` not found in ${BOOTSTRAP_FILE_NAME} — the build did not fill the SW version token ` +
        '(building with `--pwa-strategy none` makes it null and removes SW registration entirely)',
    );
  }
  const version = `${currentVersion}${REBUILT_VERSION_SUFFIX}`;
  writeFileSync(
    path,
    original.replace(wanted, `${SERVICE_WORKER_VERSION_DECL}"${version}"`),
  );
  return { version, restore: () => writeFileSync(path, original) };
}

/**
 * The new build must go live **on this load**. Exactly one reload is
 * allowed — permitting two would pass the very "arrives one load late"
 * failure this asserts against.
 */
async function assertRebuildActivatesOnThisLoad(session, version, timeoutMs) {
  await session.send('Page.reload');
  await waitForAppShell(session, 'app shell after new-build reload', timeoutMs);
  await assertBootedShell(session, 'new-build reload');
  await waitForAppReady(session, 'FRB app initialization after new-build reload', timeoutMs);

  await awaitContract(
    `new build's service worker (?${CACHE_VERSION_PARAM}=${version}) takes the page on this load`,
    timeoutMs,
    async () => {
      try {
        const value = await session.evaluate(SERVICE_WORKER_PROBE);
        if (!value) {
          return { ok: false, detail: 'no registration' };
        }
        const ok =
          value.state === ACTIVATED_STATE &&
          cacheVersionOf(value.scriptUrl) === version &&
          cacheVersionOf(value.controller) === version;
        // On failure the detail must show *what is still on the old token*.
        return { ok, detail: JSON.stringify(value) };
      } catch (error) {
        return { ok: false, detail: error.message };
      }
    },
  );

  const expectedCacheName = APP_SHELL_CACHE_PREFIX + version;
  await awaitContract(`shell cache '${expectedCacheName}'`, timeoutMs, async () => {
    try {
      const names = await session.evaluate('caches.keys()');
      const shellCaches = names.filter((name) =>
        name.startsWith(APP_SHELL_CACHE_PREFIX),
      );
      return shellCaches.length === 1 && shellCaches[0] === expectedCacheName
        ? { ok: true, detail: shellCaches[0] }
        : { ok: false, detail: `caches=${JSON.stringify(names)}` };
    } catch (error) {
      return { ok: false, detail: error.message };
    }
  });
}

async function assertOfflineReloadBoots(session, url, timeoutMs) {
  const reachability = await session.evaluate(offlineProbe(url));
  assert(
    reachability === 'unreachable',
    `offline phase but the network is still alive (${url}${OFFLINE_PROBE_QUERY} → ${reachability})`,
  );
  pass(`static server is down and the browser is offline (${url}${OFFLINE_PROBE_QUERY} unreachable)`);

  await session.send('Page.reload');
  await waitForAppShell(session, 'app shell after offline reload', timeoutMs);
  await assertBootedShell(session, 'offline reload');

  const controller = await session.evaluate(
    'navigator.serviceWorker.controller && navigator.serviceWorker.controller.scriptURL',
  );
  assert(
    scriptPathOf(controller) === serviceWorkerPath(url),
    `offline-reloaded document is not controlled by the service worker (controller=${controller})`,
  );
}

async function dumpDom(session, outDir) {
  try {
    const html = await session.evaluate('document.documentElement.outerHTML');
    const path = join(outDir, DOM_SNAPSHOT_FILE);
    writeFileSync(path, html ?? '');
    return path;
  } catch {
    return null;
  }
}

async function main() {
  const buildDir = requireEnv('UPEG_WEB_SMOKE_BUILD_DIR');
  const outDir = requireEnv('UPEG_WEB_SMOKE_OUT_DIR');
  const chromeExecutable = requireEnv('UPEG_WEB_SMOKE_CHROME');
  const profileDir = requireEnv('UPEG_WEB_SMOKE_PROFILE_DIR');
  const startPort = Number(requireEnv('UPEG_WEB_SMOKE_PORT'));
  const timeoutMs = Number(requireEnv('UPEG_WEB_SMOKE_TIMEOUT_SECONDS')) * 1000;

  mkdirSync(outDir, { recursive: true });
  const port = await findFreePort(startPort, PORT_SCAN_ATTEMPTS);
  const url = `http://${LOOPBACK_HOST}:${port}/`;

  const server = startStaticServer({
    buildDir,
    port,
    logPath: join(outDir, SERVER_LOG_FILE),
  });
  let serverRunning = true;
  let browser;
  let connection;
  let session;
  // Wherever the smoke dies, the build output must be left untouched.
  let restoreBootstrap = null;

  try {
    // Browser launch is inside this try too — so wherever a failure occurs,
    // finally always reaps the static server and Chromium.
    browser = await launchHeadlessChromium({
      executable: chromeExecutable,
      userDataDir: profileDir,
      logPath: join(outDir, CHROMIUM_LOG_FILE),
      startupTimeoutMs: timeoutMs,
    });
    ({ connection, session } = await openPageSession(browser.port));
    await session.send('Page.addScriptToEvaluateOnNewDocument', {
      source: CONSOLE_CAPTURE_SCRIPT,
    });
    await waitForServer(url, timeoutMs);

    // 1) First load — does the app come up.
    await session.send('Page.navigate', { url });
    await waitForAppShell(session, 'first-load app shell', timeoutMs);
    await assertBootedShell(session, 'first load');
    await waitForAppReady(session, 'first-load FRB app initialization', timeoutMs);
    pass(
      `app initializes through a real FRB call on first load ` +
        `(${APP_ROOT_SELECTOR} x1, FRB script injected once)`,
    );

    // 2) service worker registration/activation/control.
    const registration = await assertServiceWorkerActivated(
      session,
      url,
      timeoutMs,
    );
    pass(
      `service worker is activated and controls the page (${registration.scriptUrl})`,
    );

    // 3) app shell in cache storage.
    const cache = await assertAppShellCached(session, timeoutMs);
    pass(
      `'${cache.cacheName}' cache holds ${cache.urls.length} app shell entries ` +
        `(${REQUIRED_SHELL_PATHS.join(' · ')} · *${REQUIRED_SHELL_SUFFIXES.join(' · *')})`,
    );

    // 4) a deployed new build goes live on that load (not one load late).
    const rebuilt = rebuildServedBootstrap(
      buildDir,
      cacheVersionOf(registration.scriptUrl),
    );
    restoreBootstrap = rebuilt.restore;
    await assertRebuildActivatesOnThisLoad(session, rebuilt.version, timeoutMs);
    pass(
      `new build activates on a single reload ` +
        `(?${CACHE_VERSION_PARAM}=${rebuilt.version}, cache '${APP_SHELL_CACHE_PREFIX}${rebuilt.version}')`,
    );

    // 5) Revisit once more so the new shell cache refills. Step 4's reload
    //    started under the old SW's control and the new SW took over
    //    mid-load, so the shell fetched then is deleted along with the old
    //    cache. The offline phase is only honest when it looks at a cache
    //    filled by a real revisit.
    await session.send('Page.reload');
    await waitForAppShell(session, 'app shell on new-build revisit', timeoutMs);
    await waitForAppReady(session, 'FRB app initialization on new-build revisit', timeoutMs);
    const rebuiltCache = await assertAppShellCached(session, timeoutMs);
    pass(
      `on revisit, '${rebuiltCache.cacheName}' cache refilled with ${rebuiltCache.urls.length} app shell entries`,
    );

    // 6) Bring the server down and revisit offline.
    await stopStaticServer(server, url, timeoutMs);
    serverRunning = false;
    await session.send('Network.emulateNetworkConditions', {
      offline: true,
      latency: 0,
      downloadThroughput: -1,
      uploadThroughput: -1,
    });
    await assertOfflineReloadBoots(session, url, timeoutMs);
    await waitForAppReady(session, 'offline FRB app initialization', timeoutMs);
    pass('app shell still boots from cache on an offline reload');

    console.log(`flutter web smoke passed at ${url}`);
  } catch (error) {
    const snapshot = session ? await dumpDom(session, outDir) : null;
    const kind =
      error instanceof ContractViolation ? 'PWA contract violation' : 'smoke failure';
    console.error(`error: ${kind} — ${error.message}`);
    if (snapshot) {
      console.error(`DOM snapshot: ${snapshot}`);
    }
    process.exitCode = 1;
  } finally {
    restoreBootstrap?.();
    connection?.close();
    browser?.process.kill('SIGTERM');
    if (serverRunning && server.child.exitCode === null) {
      server.child.kill('SIGTERM');
    }
    server.flushLog();
  }
}

await main();
