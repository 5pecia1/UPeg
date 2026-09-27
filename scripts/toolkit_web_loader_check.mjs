// Browser contract test for independently downloaded web toolkits.
//
// Runs only against a locally built bundle. It never contacts a release URL:
// the HTTP server below serves `flutter_app/build/web` and deliberately
// mutates responses to prove the loader verifies artifacts before execution.

import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { mkdirSync, readFileSync, rmSync } from 'node:fs';
import { join, normalize, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import {
  findFreePort,
  launchHeadlessChromium,
  openPageSession,
  waitFor,
} from './lib/cdp_client.mjs';

const root = process.env.UPEG_TOOLKIT_WEB_BUILD_DIR;
const chrome = process.env.UPEG_TOOLKIT_WEB_CHROME || 'chromium';
if (!root) throw new Error('UPEG_TOOLKIT_WEB_BUILD_DIR is required');

const sha256 = (text) => createHash('sha256').update(text).digest('hex');
const jcs = (value) => {
  if (value === null || ['boolean', 'number', 'string'].includes(typeof value)) {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) return `[${value.map(jcs).join(',')}]`;
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${jcs(value[key])}`).join(',')}}`;
};
const sleep = (ms) => new Promise((resolveDelay) => setTimeout(resolveDelay, ms));

const catalogPath = join(root, 'toolkits/catalog.json');
const catalog = JSON.parse(readFileSync(catalogPath, 'utf8'));
const num = catalog.toolkits.find((toolkit) => toolkit.id === 'num');
const offlineToolkit = catalog.toolkits.find(
  (toolkit) => toolkit.web && toolkit.id !== 'num',
);
if (!num?.web || !offlineToolkit?.web) {
  throw new Error('web catalog needs num plus one other downloadable toolkit');
}

let mode = 'normal';
const requests = new Map();
const count = (path) => requests.get(path) || 0;
const increment = (path) => requests.set(path, count(path) + 1);
const local = (url) => resolve(root, `.${url}`);
const safeFile = (url) => {
  const path = normalize(local(url));
  if (!path.startsWith(resolve(root))) throw new Error('unsafe path');
  return path;
};
const catalogJson = () => {
  const copy = structuredClone(catalog);
  if (mode === 'same-version-catalog-update') {
    delete copy.catalog_digest;
    copy.catalog_digest = sha256(jcs(copy));
  }
  return JSON.stringify(copy);
};

const server = createServer((request, response) => {
  const path = new URL(request.url, 'http://localhost').pathname;
  if (path === '/') {
    response.writeHead(200, { 'content-type': 'text/html' });
    response.end('<script src="/upeg_toolkit_loader.js"></script>');
    return;
  }
  if (path === '/toolkits/catalog.json') {
    response.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
    response.end(catalogJson());
    return;
  }
  try {
    const bytes = readFileSync(safeFile(path));
    if (path.startsWith('/toolkits/')) increment(path);
    if (mode === 'tamper' && path === num.web.wasm) {
      const tampered = Buffer.from(bytes);
      tampered[0] ^= 1;
      response.writeHead(200, { 'content-type': 'application/wasm' });
      response.end(tampered);
      return;
    }
    response.writeHead(200, {
      'content-type': path.endsWith('.wasm') ? 'application/wasm' : 'text/javascript',
    });
    response.end(bytes);
  } catch {
    response.writeHead(404).end();
  }
});

const freshLoader = `(async () => {
  UpegToolkitLoader.terminate();
  await new Promise((resolve) => {
    const script = document.createElement('script');
    script.src = '/upeg_toolkit_loader.js?fresh=' + Math.random();
    script.onload = resolve;
    document.head.append(script);
  });
})()`;
const syncAndDispatch = (toolkit, toolId, argsJson) => `
  (async () => {
    const fetched = await fetch('/toolkits/catalog.json');
    const raw = await fetched.json();
    const synced = await UpegToolkitLoader.sync({ abiDigest: raw.abi_digest });
    return UpegToolkitLoader.dispatch({
      catalogDigest: synced.catalog.catalog_digest,
      toolkitId: ${JSON.stringify(toolkit)},
      toolId: ${JSON.stringify(toolId)},
      argsJson: ${JSON.stringify(argsJson)},
    });
  })()
`;

const profile = join(tmpdir(), `upeg-toolkit-browser-${process.pid}`);
mkdirSync(profile, { recursive: true });
const port = await findFreePort(8190, 30);
const url = `http://127.0.0.1:${port}/`;
let browser;
let connection;
let session;
try {
  await new Promise((resolveListen) => server.listen(port, '127.0.0.1', resolveListen));
  browser = await launchHeadlessChromium({
    executable: chrome,
    userDataDir: profile,
    logPath: join(profile, 'chromium.log'),
    startupTimeoutMs: 30000,
  });
  ({ connection, session } = await openPageSession(browser.port));
  await session.send('Page.navigate', { url });
  await waitFor('toolkit loader', 10000, async () => ({
    ok: await session.evaluate('typeof UpegToolkitLoader === "object"'),
  }));
  await session.evaluate(`
    (async () => {
      await navigator.serviceWorker.register('/upeg_service_worker.js?toolkit-check');
      await navigator.serviceWorker.ready;
    })()
  `);
  await session.send('Page.reload');
  await waitFor('service-worker control', 10000, async () => ({
    ok: await session.evaluate('Boolean(navigator.serviceWorker.controller)'),
  }));
  await waitFor('toolkit loader after service-worker control', 10000, async () => ({
    ok: await session.evaluate('typeof UpegToolkitLoader === "object"'),
  }));

  const initialJs = count(num.web.js);
  const initialWasm = count(num.web.wasm);
  await session.evaluate(`UpegToolkitLoader.sync({ abiDigest: ${JSON.stringify(catalog.abi_digest)} })`);
  if (count(num.web.js) !== initialJs || count(num.web.wasm) !== initialWasm) {
    throw new Error('catalog sync eagerly downloaded a toolkit');
  }

  const first = await session.evaluate(syncAndDispatch('num', 'num.hex_to_decimal', '{"input":"0xff"}'));
  if (JSON.parse(first.resultJson).outputs[0].value !== 255) throw new Error('num pack did not return 255');
  const downloadedJs = count(num.web.js);
  const downloadedWasm = count(num.web.wasm);

  mode = 'same-version-catalog-update';
  await session.evaluate(freshLoader);
  await session.evaluate(syncAndDispatch('num', 'num.hex_to_decimal', '{"input":"0xff"}'));
  if (count(num.web.js) !== downloadedJs || count(num.web.wasm) !== downloadedWasm) {
    throw new Error('unchanged pack was redownloaded after catalog update');
  }

  await session.evaluate('caches.delete("upeg-toolkit-packs-v1")');
  await session.evaluate(freshLoader);
  mode = 'tamper';
  const tampered = await session.evaluate(`
    ${syncAndDispatch('num', 'num.hex_to_decimal', '{"input":"0xff"}')}
      .then(() => 'accepted').catch((error) => String(error));
  `);
  if (!tampered.includes('integrity_failed')) throw new Error(`tampered pack was not rejected: ${tampered}`);

  mode = 'normal';
  const retry = await session.evaluate(syncAndDispatch('num', 'num.hex_to_decimal', '{"input":"0xff"}'));
  if (JSON.parse(retry.resultJson).outputs[0].value !== 255) throw new Error('retry after tamper failed');

  await session.evaluate(freshLoader);
  await session.send('Network.emulateNetworkConditions', {
    offline: true,
    latency: 0,
    downloadThroughput: 0,
    uploadThroughput: 0,
  });
  const warmOffline = await session.evaluate(`
    (async () => {
      const synced = await UpegToolkitLoader.sync({ abiDigest: ${JSON.stringify(catalog.abi_digest)} });
      return UpegToolkitLoader.dispatch({
        catalogDigest: synced.catalog.catalog_digest,
        toolkitId: 'num', toolId: 'num.hex_to_decimal', argsJson: '{"input":"0xff"}',
      });
    })()
  `);
  if (JSON.parse(warmOffline.resultJson).outputs[0].value !== 255) {
    throw new Error('cached toolkit did not run after Worker restart while offline');
  }
  await session.send('Network.emulateNetworkConditions', {
    offline: false,
    latency: 0,
    downloadThroughput: -1,
    uploadThroughput: -1,
  });

  await session.evaluate('caches.delete("upeg-toolkit-packs-v1")');
  await session.evaluate(freshLoader);
  const beforeConcurrent = [count(num.web.js), count(num.web.wasm)];
  await session.evaluate(`
    (async () => {
      const raw = await (await fetch('/toolkits/catalog.json')).json();
      const synced = await UpegToolkitLoader.sync({ abiDigest: raw.abi_digest });
      const request = { catalogDigest: synced.catalog.catalog_digest, toolkitId: 'num' };
      await Promise.all([UpegToolkitLoader.load(request), UpegToolkitLoader.load(request)]);
    })()
  `);
  if (count(num.web.js) !== beforeConcurrent[0] + 1 || count(num.web.wasm) !== beforeConcurrent[1] + 1) {
    throw new Error('concurrent toolkit loads did not share one download');
  }

  await session.evaluate(`caches.delete('upeg-toolkit-packs-v1')`);
  await session.evaluate(freshLoader);
  await session.send('Network.emulateNetworkConditions', {
    offline: true,
    latency: 0,
    downloadThroughput: 0,
    uploadThroughput: 0,
  });
  await new Promise((resolveClose) => server.close(resolveClose));
  await sleep(50);
  const offline = await session.evaluate(`
    (async () => {
      const synced = await UpegToolkitLoader.sync({ abiDigest: ${JSON.stringify(catalog.abi_digest)} });
      return UpegToolkitLoader.dispatch({
        catalogDigest: synced.catalog.catalog_digest,
        toolkitId: ${JSON.stringify(offlineToolkit.id)},
        toolId: ${JSON.stringify(offlineToolkit.tools[0].id)},
        argsJson: '{}',
      });
    })().then(() => 'accepted').catch((error) => String(error));
  `);
  if (!offline.includes('offline_download_required')) throw new Error(`offline uncached pack did not explain download need: ${offline}`);
  const shellCaches = await session.evaluate(`
    caches.keys().then(async (names) => {
      const entries = await Promise.all(names
        .filter((name) => name.startsWith('upeg-app-shell-'))
        .map(async (name) => ({ name, keys: await (await caches.open(name)).keys() })));
      return entries.flatMap((entry) => entry.keys.map((request) => request.url));
    })
  `);
  if (shellCaches.some((url) => new URL(url).pathname.startsWith('/toolkits/'))) {
    throw new Error('app-shell service-worker cache captured a toolkit artifact');
  }
  console.log('ok toolkit loader: service-worker isolation, lazy, hash, update cache, retry, concurrent, offline');
} finally {
  if (connection) connection.close();
  if (browser?.process?.exitCode === null) {
    browser.process.kill('SIGTERM');
    await new Promise((resolveExit) => browser.process.once('exit', resolveExit));
  }
  if (server.listening) server.close();
  rmSync(profile, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
}
