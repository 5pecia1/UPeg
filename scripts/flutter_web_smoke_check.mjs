// Flutter Web(PWA) 번들의 부팅 + service worker 캐시 계약 검사.
//
// `scripts/flutter_web_smoke.sh`가 전제 조건(빌드 산출물 · chromium · python3)을
// 확인한 뒤 이 스크립트를 부른다. 여기서는 정적 서버와 헤드리스 Chromium의
// 수명을 쥐고 계약을 단언한다 — 오프라인 단계에서 **서버를 실제로 내려야**
// 하기 때문에 둘의 수명이 한 곳에 있어야 한다.
//
// 단언하는 계약 (인벤토리의 `pwa.service-worker.cache`):
//   1. 첫 로드에서 앱 셸이 뜬다 (flutter-view 1개, FRB 스크립트 1회 주입).
//   2. `navigator.serviceWorker.ready`가 `upeg_service_worker.js`를 activated로
//      돌려주고, 그 SW가 이 페이지를 제어한다.
//   3. 캐시 스토리지에 app shell 캐시가 정확히 하나 있고 그 안에 오프라인
//      부팅에 필요한 셸이 다 들어 있다.
//   4. 새 빌드가 **그 로드에서** 산다. 서빙 중인 부트스트랩의 SW 버전 토큰을
//      갈아 끼우고 한 번만 reload 하면, 새 토큰의 service worker가 그 로드에서
//      페이지를 잡고 새 이름의 셸 캐시가 선다. 부트스트랩을
//      stale-while-revalidate로 주던 시절에는 이 단언이 실패했다 — 새 빌드가
//      한 로드 늦게 도착했다.
//   5. 정적 서버를 내리고 브라우저를 오프라인으로 만든 뒤 reload 해도 앱이
//      캐시에서 뜬다. 서버를 실제로 죽이므로 "네트워크가 답해 준" 가짜 통과가
//      성립하지 않는다.
//
// npm 의존성 없음 — Node 22+의 전역 `fetch`/`WebSocket`과 builtin만 쓴다.

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
 * 등록 URL은 빌드마다 달라지는 `?v=<version>`을 달고 다닌다(그 값이 셸 캐시
 * 이름이 된다). 그래서 스크립트 동일성은 경로로 본다.
 */
const serviceWorkerPath = (url) =>
  new URL(SERVICE_WORKER_FILE_NAME, url).pathname;
const scriptPathOf = (scriptUrl) =>
  scriptUrl ? new URL(scriptUrl).pathname : null;
const APP_SHELL_CACHE_PREFIX = 'upeg-app-shell-';
const ACTIVATED_STATE = 'activated';
/** 등록 URL이 실어 오는 빌드 토큰. 셸 캐시 이름이 이 값에서 나온다. */
const CACHE_VERSION_PARAM = 'v';
const cacheVersionOf = (scriptUrl) =>
  scriptUrl ? new URL(scriptUrl).searchParams.get(CACHE_VERSION_PARAM) : null;
const BOOTSTRAP_FILE_NAME = 'flutter_bootstrap.js';
/** `web/flutter_bootstrap.js`가 토큰을 받는 자리. 우리가 쓴 선언이다. */
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

/** 오프라인 부팅에 필요한 셸 — 경로가 고정된 것들. */
const REQUIRED_SHELL_PATHS = [
  '/',
  '/flutter_bootstrap.js',
  '/manifest.json',
  '/main.dart.js',
  '/pkg/upeg_frb.js',
  '/pkg/upeg_frb_bg.wasm',
];
/**
 * CanvasKit은 브라우저마다 다른 하위 디렉터리에서 받는다
 * (`canvaskit/canvaskit.js` vs `canvaskit/chromium/canvaskit.js`). 렌더러가
 * 우리 origin에서 캐시되었는지만 본다 — CDN에서 받으면 오프라인에서 못 뜬다.
 */
const REQUIRED_SHELL_SUFFIXES = ['/canvaskit.js', '/canvaskit.wasm'];

/** 계약 위반. 배관 실패(`CdpError`)와 구분해서 보고한다. */
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
 * 계약이 참이 될 때까지 기다린다. 시간 초과는 배관 실패가 아니라 계약
 * 위반이다 — "SW가 60초 안에 activated 되지 않았다"는 계약이 깨진 것이다.
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
    [
      '-m',
      'http.server',
      String(port),
      '--bind',
      LOOPBACK_HOST,
      '--directory',
      buildDir,
    ],
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

async function assertBootedShell(session, phase) {
  const frbScripts = await session.evaluate(countSelector(FRB_SCRIPT_SELECTOR));
  assert(
    frbScripts === 1,
    `${phase}: FRB 스크립트(${FRB_SCRIPT_SELECTOR})가 ${frbScripts}개 — 정확히 1개여야 한다`,
  );
  const title = await session.evaluate('document.title');
  assert(
    title === PAGE_TITLE,
    `${phase}: document.title이 '${title}' — '${PAGE_TITLE}'이어야 한다`,
  );
}

async function assertServiceWorkerActivated(session, url, timeoutMs) {
  const { detail: registration } = await awaitContract(
    'service worker 활성화',
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
    `활성 service worker가 ${registration.scriptUrl} — ${expectedPath}이어야 한다`,
  );
  assert(
    registration.state === ACTIVATED_STATE,
    `service worker 상태가 '${registration.state}' — '${ACTIVATED_STATE}'이어야 한다`,
  );
  assert(
    registration.scope === url,
    `service worker scope가 ${registration.scope} — ${url}이어야 한다`,
  );
  assert(
    scriptPathOf(registration.controller) === expectedPath,
    `첫 로드가 service worker의 제어를 받지 않는다 (controller=${registration.controller})`,
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
    'app shell 캐시 채워짐',
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
  assert(names.length > 0, '캐시 스토리지가 비어 있다');
  const shellCaches = names.filter((name) =>
    name.startsWith(APP_SHELL_CACHE_PREFIX),
  );
  assert(
    shellCaches.length === 1,
    `'${APP_SHELL_CACHE_PREFIX}*' 캐시가 ${shellCaches.length}개 — 정확히 1개여야 한다 (${names.join(', ')})`,
  );
  return { cacheName: shellCaches[0], urls: contents[shellCaches[0]] };
}

/**
 * 새 빌드를 흉내 낸다: 서빙 중인 부트스트랩의 SW 버전 토큰만 갈아 끼운다.
 *
 * 스모크 한가운데서 `flutter build web`을 한 번 더 도는 것은 분 단위라
 * 계약 검사가 감당할 비용이 아니다. 그리고 새 빌드가 이 파일에서 바꾸는
 * 것은 정확히 이 토큰 하나다 — Flutter는 `{{flutter_service_worker_version}}`에
 * 빌드마다 새로 뽑는 난수를 넣는다(`web/upeg_service_worker.js` 헤더 참고).
 * 그래서 토큰 치환이 곧 "새 빌드가 배포됐다"와 같은 상황이다.
 *
 * 되돌리기는 호출자가 쥔다 — 스모크가 끝날 때까지 서버는 새 토큰을 서빙해야
 * 하고, 빌드 산출물은 스모크가 끝난 뒤 원래대로 남아야 한다.
 */
function rebuildServedBootstrap(buildDir, currentVersion) {
  const path = join(buildDir, BOOTSTRAP_FILE_NAME);
  const original = readFileSync(path, 'utf8');
  const wanted = `${SERVICE_WORKER_VERSION_DECL}"${currentVersion}"`;
  if (!original.includes(wanted)) {
    throw new ContractViolation(
      `${BOOTSTRAP_FILE_NAME}에서 \`${wanted}\`를 찾지 못했다 — 빌드가 SW 버전 토큰을 채우지 않았다 ` +
        '(`--pwa-strategy none`으로 빌드하면 이 값이 null이 되고 SW 등록 자체가 사라진다)',
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
 * 새 빌드가 **이 로드에서** 살아야 한다. reload는 딱 한 번이다 — 두 번
 * 허용하면 "한 로드 늦게 도착한다"는 바로 그 실패를 통과시킨다.
 */
async function assertRebuildActivatesOnThisLoad(session, version, timeoutMs) {
  await session.send('Page.reload');
  await waitForAppShell(session, '새 빌드 reload 후 앱 셸', timeoutMs);
  await assertBootedShell(session, '새 빌드 reload');

  await awaitContract(
    `새 빌드의 service worker(?${CACHE_VERSION_PARAM}=${version})가 이 로드에서 페이지를 잡음`,
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
        // 실패했을 때 '무엇이 아직 옛 토큰인지'가 보여야 한다.
        return { ok, detail: JSON.stringify(value) };
      } catch (error) {
        return { ok: false, detail: error.message };
      }
    },
  );

  const expectedCacheName = APP_SHELL_CACHE_PREFIX + version;
  await awaitContract(`셸 캐시 '${expectedCacheName}'`, timeoutMs, async () => {
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
    `오프라인 단계인데 네트워크가 아직 살아 있다 (${url}${OFFLINE_PROBE_QUERY} → ${reachability})`,
  );
  pass(`정적 서버가 내려갔고 브라우저도 오프라인이다 (${url}${OFFLINE_PROBE_QUERY} 도달 불가)`);

  await session.send('Page.reload');
  await waitForAppShell(session, '오프라인 reload 후 앱 셸', timeoutMs);
  await assertBootedShell(session, '오프라인 reload');

  const controller = await session.evaluate(
    'navigator.serviceWorker.controller && navigator.serviceWorker.controller.scriptURL',
  );
  assert(
    scriptPathOf(controller) === serviceWorkerPath(url),
    `오프라인 reload된 문서를 service worker가 제어하지 않는다 (controller=${controller})`,
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
  // 스모크가 어디서 죽든 빌드 산출물은 원래대로 남아야 한다.
  let restoreBootstrap = null;

  try {
    // 브라우저를 띄우는 것까지 이 try 안이다 — 그래야 어디서 실패하든
    // finally가 정적 서버와 Chromium을 반드시 거둔다.
    browser = await launchHeadlessChromium({
      executable: chromeExecutable,
      userDataDir: profileDir,
      logPath: join(outDir, CHROMIUM_LOG_FILE),
      startupTimeoutMs: timeoutMs,
    });
    ({ connection, session } = await openPageSession(browser.port));
    await waitForServer(url, timeoutMs);

    // 1) 첫 로드 — 앱이 뜨는지.
    await session.send('Page.navigate', { url });
    await waitForAppShell(session, '첫 로드 앱 셸', timeoutMs);
    await assertBootedShell(session, '첫 로드');
    pass(`첫 로드에서 앱 셸이 뜬다 (${APP_ROOT_SELECTOR} 1개, FRB 스크립트 1회 주입)`);

    // 2) service worker 등록/활성화/제어.
    const registration = await assertServiceWorkerActivated(
      session,
      url,
      timeoutMs,
    );
    pass(
      `service worker가 activated이고 페이지를 제어한다 (${registration.scriptUrl})`,
    );

    // 3) 캐시 스토리지에 app shell.
    const cache = await assertAppShellCached(session, timeoutMs);
    pass(
      `'${cache.cacheName}' 캐시에 app shell ${cache.urls.length}개가 들어 있다 ` +
        `(${REQUIRED_SHELL_PATHS.join(' · ')} · *${REQUIRED_SHELL_SUFFIXES.join(' · *')})`,
    );

    // 4) 새 빌드가 배포되면 그 로드에서 산다 (한 로드 늦게가 아니라).
    const rebuilt = rebuildServedBootstrap(
      buildDir,
      cacheVersionOf(registration.scriptUrl),
    );
    restoreBootstrap = rebuilt.restore;
    await assertRebuildActivatesOnThisLoad(session, rebuilt.version, timeoutMs);
    pass(
      `새 빌드가 한 번의 reload에서 활성화된다 ` +
        `(?${CACHE_VERSION_PARAM}=${rebuilt.version}, 캐시 '${APP_SHELL_CACHE_PREFIX}${rebuilt.version}')`,
    );

    // 5) 새 셸 캐시가 다시 찰 때까지 한 번 더 방문한다. 4)의 reload는 옛 SW의
    //    제어 아래에서 시작해 새 SW가 도중에 이어받으므로, 그때 받아 둔 셸은
    //    옛 캐시와 함께 지워진다. 오프라인 단계는 실제 재방문에서 채워진
    //    캐시를 봐야 정직하다.
    await session.send('Page.reload');
    await waitForAppShell(session, '새 빌드 재방문 앱 셸', timeoutMs);
    const rebuiltCache = await assertAppShellCached(session, timeoutMs);
    pass(
      `재방문에서 '${rebuiltCache.cacheName}' 캐시가 app shell ${rebuiltCache.urls.length}개로 다시 찼다`,
    );

    // 6) 서버를 내리고 오프라인에서 재방문.
    await stopStaticServer(server, url, timeoutMs);
    serverRunning = false;
    await session.send('Network.emulateNetworkConditions', {
      offline: true,
      latency: 0,
      downloadThroughput: -1,
      uploadThroughput: -1,
    });
    await assertOfflineReloadBoots(session, url, timeoutMs);
    pass('오프라인 reload에서도 앱 셸이 캐시에서 뜬다');

    console.log(`flutter web smoke passed at ${url}`);
  } catch (error) {
    const snapshot = session ? await dumpDom(session, outDir) : null;
    const kind =
      error instanceof ContractViolation ? 'PWA 계약 위반' : 'smoke 실패';
    console.error(`error: ${kind} — ${error.message}`);
    if (snapshot) {
      console.error(`DOM 스냅샷: ${snapshot}`);
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
