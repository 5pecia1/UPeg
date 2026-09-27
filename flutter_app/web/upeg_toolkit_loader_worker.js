'use strict';

// Toolkit packs execute outside Flutter's FRB worker pool.  A pack is a
// wasm-bindgen guest: the verified glue module receives verified wasm bytes
// and returns the existing canonical ToolResult JSON wire.
//
// This worker owns network I/O, retrying, integrity checks, CacheStorage, and
// one-instance-per-toolkit execution.  Keeping these concerns here prevents a
// downloaded pack from widening the browser's host capabilities.

const PACK_CACHE_NAME = 'upeg-toolkit-packs-v1';
const CATALOG_CACHE_NAME = 'upeg-toolkit-catalog-v1';
const MAX_RETRIES = 2;
const RETRY_DELAY_MS = 250;
const HEX_SHA256 = /^[0-9a-f]{64}$/;
const TOOLKIT_PATH = '/toolkits/';
const MAX_TOOLKIT_BYTES = 25 * 1024 * 1024;

const pendingLoads = new Map();
const loadedToolkits = new Map();
const catalogByDigest = new Map();

const pause = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

function reply(requestId, event, value = {}) {
  self.postMessage({ requestId, event, ...value });
}

function userError(code, message) {
  const error = new Error(message);
  error.code = code;
  return error;
}

function sameOriginToolkitUrl(value) {
  if (typeof value !== 'string') return null;
  const url = new URL(value, self.location.origin);
  if (url.origin !== self.location.origin || !url.pathname.startsWith(TOOLKIT_PATH)) {
    return null;
  }
  return url.href;
}

function validateArtifact(url, sha256, size, name) {
  const absoluteUrl = sameOriginToolkitUrl(url);
  if (!absoluteUrl || typeof sha256 !== 'string' || !HEX_SHA256.test(sha256) ||
      !Number.isSafeInteger(size) || size <= 0 || size > MAX_TOOLKIT_BYTES) {
    throw userError('invalid_catalog', `Invalid ${name} toolkit artifact.`);
  }
  return { url: absoluteUrl, sha256, size };
}

function canonicalizeJcs(value) {
  if (value === null || typeof value === 'boolean' || typeof value === 'string') {
    return JSON.stringify(value);
  }
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw userError('invalid_catalog', 'Toolkit catalog is invalid.');
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(canonicalizeJcs).join(',')}]`;
  }
  if (typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) =>
      `${JSON.stringify(key)}:${canonicalizeJcs(value[key])}`).join(',')}}`;
  }
  throw userError('invalid_catalog', 'Toolkit catalog is invalid.');
}

async function validateCatalog(raw, expectedAbiDigest) {
  if (!raw || raw.schema_version !== 1 || !Array.isArray(raw.toolkits) ||
      typeof raw.catalog_digest !== 'string' || !HEX_SHA256.test(raw.catalog_digest) ||
      typeof raw.abi_digest !== 'string' || !HEX_SHA256.test(raw.abi_digest)) {
    throw userError('invalid_catalog', 'Toolkit catalog is invalid.');
  }
  if (expectedAbiDigest && raw.abi_digest !== expectedAbiDigest) {
    throw userError('incompatible_catalog', 'Toolkit update requires a newer app.');
  }
  const digestInput = { ...raw };
  delete digestInput.catalog_digest;
  const digest = await sha256(new TextEncoder().encode(canonicalizeJcs(digestInput)));
  if (digest !== raw.catalog_digest) {
    throw userError('invalid_catalog', 'Toolkit catalog could not be verified.');
  }

  const ids = new Set();
  const toolIds = new Set();
  const toolkits = raw.toolkits.map((toolkit) => {
    if (!toolkit || typeof toolkit.id !== 'string' || !toolkit.id ||
        typeof toolkit.version !== 'string' || !toolkit.version || ids.has(toolkit.id) ||
        !Array.isArray(toolkit.tools)) {
      throw userError('invalid_catalog', 'Toolkit catalog is invalid.');
    }
    ids.add(toolkit.id);
    for (const tool of toolkit.tools) {
      if (!tool || typeof tool.id !== 'string' || !tool.id || toolIds.has(tool.id)) {
        throw userError('invalid_catalog', 'Toolkit catalog has duplicate tools.');
      }
      toolIds.add(tool.id);
    }
    if (toolkit.web == null) {
      return { ...toolkit, web: null };
    }
    const web = toolkit.web;
    const js = validateArtifact(web.js, web.js_sha256, web.js_size, 'JavaScript');
    const wasm = validateArtifact(web.wasm, web.sha256, web.size, 'WebAssembly');
    return {
      ...toolkit,
      web: {
        js,
        wasm,
      },
    };
  });
  return { ...raw, toolkits };
}

async function sha256(bytes) {
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(digest)]
    .map((value) => value.toString(16).padStart(2, '0'))
    .join('');
}

function cacheKey(artifact) {
  const url = new URL(artifact.url);
  url.searchParams.set('upeg_sha256', artifact.sha256);
  return url.href;
}

async function readVerifiedCached(cache, key, artifact) {
  const cached = await cache.match(key);
  if (!cached) return null;
  const bytes = await cached.arrayBuffer();
  if (bytes.byteLength !== artifact.size || await sha256(bytes) !== artifact.sha256) {
    await cache.delete(key);
    return null;
  }
  return bytes;
}

async function fetchVerifiedArtifact(artifact, requestId, label) {
  const cache = await caches.open(PACK_CACHE_NAME);
  const key = cacheKey(artifact);
  const cached = await readVerifiedCached(cache, key, artifact);
  if (cached) {
    reply(requestId, 'progress', { label, state: 'ready', loaded: artifact.size, total: artifact.size });
    return cached;
  }

  let lastError;
  for (let attempt = 0; attempt <= MAX_RETRIES; attempt += 1) {
    try {
      reply(requestId, 'progress', {
        label,
        state: attempt === 0 ? 'downloading' : 'retrying',
        attempt,
        loaded: 0,
        total: artifact.size,
      });
      const response = await fetch(artifact.url, { cache: 'no-store' });
      if (!response.ok || !response.body) {
        throw userError('download_failed', 'Toolkit download failed.');
      }
      const reader = response.body.getReader();
      const chunks = [];
      let loaded = 0;
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        chunks.push(value);
        loaded += value.byteLength;
        if (loaded > artifact.size) {
          throw userError('integrity_failed', 'Toolkit download could not be verified.');
        }
        reply(requestId, 'progress', { label, state: 'downloading', loaded, total: artifact.size });
      }
      if (loaded !== artifact.size) {
        throw userError('integrity_failed', 'Toolkit download could not be verified.');
      }
      const bytes = new Uint8Array(loaded);
      let offset = 0;
      for (const chunk of chunks) {
        bytes.set(chunk, offset);
        offset += chunk.byteLength;
      }
      if (await sha256(bytes) !== artifact.sha256) {
        throw userError('integrity_failed', 'Toolkit download could not be verified.');
      }
      await cache.put(key, new Response(bytes, {
        headers: { 'content-type': 'application/octet-stream' },
      }));
      reply(requestId, 'progress', { label, state: 'ready', loaded, total: artifact.size });
      return bytes.buffer;
    } catch (error) {
      lastError = error;
      if (attempt < MAX_RETRIES) await pause(RETRY_DELAY_MS * (2 ** attempt));
    }
  }
  if (lastError && lastError.code) throw lastError;
  if (self.navigator && self.navigator.onLine === false) {
    throw userError(
      'offline_download_required',
      'This toolkit needs a download. Connect to the internet and try again.',
    );
  }
  throw userError(
    'download_failed',
    'Toolkit download failed. Try again.',
  );
}

async function fetchCatalog(catalogUrl, expectedAbiDigest) {
  const cache = await caches.open(CATALOG_CACHE_NAME);
  const request = new Request(catalogUrl);
  let payload;
  try {
    const response = await fetch(request, { cache: 'no-store' });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    payload = await response.text();
    const catalog = await parseAndValidateCatalog(payload, expectedAbiDigest);
    await cache.put(request, new Response(payload, {
      headers: { 'content-type': 'application/json; charset=utf-8' },
    }));
    return catalog;
  } catch (error) {
    if (error && error.code) throw error;
    const cached = await cache.match(request);
    if (!cached) {
      throw userError('offline_catalog_required', 'Toolkit updates need an internet connection.');
    }
    payload = await cached.text();
  }
  return parseAndValidateCatalog(payload, expectedAbiDigest);
}

async function parseAndValidateCatalog(payload, expectedAbiDigest) {
  let raw;
  try {
    raw = JSON.parse(payload);
  } catch (_) {
    throw userError('invalid_catalog', 'Toolkit catalog is invalid.');
  }
  return validateCatalog(raw, expectedAbiDigest);
}

async function loadToolkit(catalog, toolkit, requestId) {
  if (!toolkit.web) {
    throw userError('host_required', 'This toolkit needs a connected host.');
  }
  const loaded = loadedToolkits.get(toolkit.id);
  if (loaded && loaded.catalogDigest === catalog.catalog_digest &&
      loaded.version === toolkit.version) {
    return loaded;
  }
  const pendingKey = `${catalog.catalog_digest}:${toolkit.id}:${toolkit.version}`;
  const pending = pendingLoads.get(pendingKey);
  if (pending) return pending;

  const work = (async () => {
    const [glueBytes, wasmBytes] = await Promise.all([
      fetchVerifiedArtifact(toolkit.web.js, requestId, 'toolkit'),
      fetchVerifiedArtifact(toolkit.web.wasm, requestId, 'toolkit'),
    ]);
    const glueUrl = URL.createObjectURL(new Blob([glueBytes], { type: 'text/javascript' }));
    try {
      const module = await import(glueUrl);
      if (typeof module.default !== 'function' || typeof module.dispatch !== 'function') {
        throw userError('invalid_toolkit', 'Toolkit could not be loaded.');
      }
      await module.default(wasmBytes);
      const instance = {
        catalogDigest: catalog.catalog_digest,
        version: toolkit.version,
        dispatch: module.dispatch,
      };
      loadedToolkits.set(toolkit.id, instance);
      return instance;
    } finally {
      URL.revokeObjectURL(glueUrl);
    }
  })().finally(() => pendingLoads.delete(pendingKey));
  pendingLoads.set(pendingKey, work);
  return work;
}

async function handle(message) {
  const { requestId, action } = message;
  try {
    if (action === 'sync') {
      const catalog = await fetchCatalog(message.catalogUrl, message.abiDigest || '');
      catalogByDigest.set(catalog.catalog_digest, catalog);
      reply(requestId, 'done', { catalog });
      return;
    }
    const catalog = catalogByDigest.get(message.catalogDigest);
    if (!catalog) throw userError('catalog_not_loaded', 'Toolkit catalog is not ready.');
    const toolkit = catalog.toolkits.find((entry) => entry.id === message.toolkitId);
    if (!toolkit) throw userError('unknown_toolkit', 'Toolkit is unavailable.');
    const instance = await loadToolkit(catalog, toolkit, requestId);
    if (action === 'load') {
      reply(requestId, 'done', { toolkitId: toolkit.id, version: toolkit.version });
      return;
    }
    if (action === 'dispatch') {
      const resultJson = await instance.dispatch(message.toolId, message.argsJson);
      if (typeof resultJson !== 'string') {
        throw userError('invalid_toolkit_result', 'Toolkit returned an invalid result.');
      }
      reply(requestId, 'done', { resultJson });
      return;
    }
    throw userError('invalid_request', 'Toolkit request is invalid.');
  } catch (error) {
    reply(requestId, 'error', {
      code: error && error.code ? error.code : 'toolkit_failed',
      message: error && error.message ? error.message : 'Toolkit could not be loaded.',
    });
  }
}

self.addEventListener('message', (event) => handle(event.data));
