'use strict';

// Small main-thread facade for Dart.  The worker owns every potentially heavy
// action; this file only correlates requests and releases callers when a
// Worker is terminated or fails.
(function installUpegToolkitLoader() {
  const worker = new Worker('upeg_toolkit_loader_worker.js');
  let nextRequestId = 0;
  const pending = new Map();

  function failAll(message) {
    for (const { reject } of pending.values()) reject(new Error(message));
    pending.clear();
  }

  worker.addEventListener('message', (event) => {
    const message = event.data;
    const request = pending.get(message.requestId);
    if (!request) return;
    if (message.event === 'progress') {
      request.onProgress && request.onProgress(message);
      return;
    }
    pending.delete(message.requestId);
    if (message.event === 'done') request.resolve(message);
    else request.reject(Object.assign(new Error(`${message.code}: ${message.message}`), { code: message.code }));
  });
  worker.addEventListener('error', () => failAll('Toolkit worker stopped.'));
  worker.addEventListener('messageerror', () => failAll('Toolkit worker communication failed.'));

  function request(action, payload, onProgress, timeoutMs = 60000) {
    return new Promise((resolve, reject) => {
      const requestId = ++nextRequestId;
      const timer = setTimeout(() => {
        if (!pending.delete(requestId)) return;
        reject(Object.assign(new Error('Toolkit request timed out.'), { code: 'timeout' }));
      }, timeoutMs);
      pending.set(requestId, {
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
        onProgress,
      });
      worker.postMessage({ requestId, action, ...payload });
    });
  }

  window.UpegToolkitLoader = Object.freeze({
    sync: ({ catalogUrl = '/toolkits/catalog.json', abiDigest = '' } = {}) =>
      request('sync', { catalogUrl, abiDigest }),
    load: ({ catalogDigest, toolkitId }, onProgress) =>
      request('load', { catalogDigest, toolkitId }, onProgress),
    dispatch: ({ catalogDigest, toolkitId, toolId, argsJson }, onProgress) =>
      request('dispatch', { catalogDigest, toolkitId, toolId, argsJson }, onProgress),
    terminate: () => {
      worker.terminate();
      failAll('Toolkit worker stopped.');
    },
  });
})();
