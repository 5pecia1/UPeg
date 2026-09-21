// Minimal client driving headless Chromium over CDP (Chrome DevTools
// Protocol).
//
// Why hand-rolled: this repo's browser verification relies solely on the
// devcontainer's `chromium` and adds no npm dependencies. Node 22+'s global
// `WebSocket` and `fetch` are enough to speak CDP.
//
// Only protocol plumbing lives here. What to assert is the caller's job
// (SoC).

import { spawn } from 'node:child_process';
import { closeSync, existsSync, openSync, readFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const LOOPBACK_HOST = '127.0.0.1';
const DEVTOOLS_PORT_FILE = 'DevToolsActivePort';
const POLL_INTERVAL_MS = 100;

/** Failure from the CDP plumbing — distinct from a contract violation (a failed assertion). */
export class CdpError extends Error {}

/** Scan upward from `start` for one free loopback port. */
export async function findFreePort(start, attempts) {
  for (let port = start; port < start + attempts; port += 1) {
    const free = await new Promise((resolve) => {
      const probe = createServer();
      probe.once('error', () => resolve(false));
      probe.once('listening', () => probe.close(() => resolve(true)));
      probe.listen(port, LOOPBACK_HOST);
    });
    if (free) {
      return port;
    }
  }
  throw new CdpError(
    `no free ${LOOPBACK_HOST} port in [${start}, ${start + attempts})`,
  );
}

/** Poll until the condition holds. On timeout, throw with the last value. */
export async function waitFor(label, timeoutMs, probe) {
  const deadline = Date.now() + timeoutMs;
  let last;
  while (Date.now() < deadline) {
    last = await probe();
    if (last.ok) {
      return last;
    }
    await delay(POLL_INTERVAL_MS);
  }
  throw new CdpError(
    `timed out after ${timeoutMs}ms waiting for ${label}` +
      (last?.detail ? ` (last: ${last.detail})` : ''),
  );
}

/**
 * Launch headless Chromium and return its DevTools port.
 * The port is picked by the kernel via `--remote-debugging-port=0` and read
 * back from the `DevToolsActivePort` file Chromium writes into the user
 * data directory.
 */
export async function launchHeadlessChromium({
  executable,
  userDataDir,
  logPath,
  startupTimeoutMs,
}) {
  const logFd = openSync(logPath, 'a');
  const child = spawn(
    executable,
    [
      '--headless=new',
      '--disable-gpu',
      '--no-sandbox',
      '--disable-dev-shm-usage',
      `--user-data-dir=${userDataDir}`,
      '--remote-debugging-port=0',
      'about:blank',
    ],
    { stdio: ['ignore', logFd, logFd] },
  );

  const portFile = join(userDataDir, DEVTOOLS_PORT_FILE);
  try {
    const { detail: port } = await waitFor(
      'Chromium DevTools port',
      startupTimeoutMs,
      async () => {
        if (child.exitCode !== null) {
          throw new CdpError(
            `Chromium exited early with code ${child.exitCode}`,
          );
        }
        if (!existsSync(portFile)) {
          return { ok: false };
        }
        const [line] = readFileSync(portFile, 'utf8').split('\n');
        return line?.trim() ? { ok: true, detail: line.trim() } : { ok: false };
      },
    );
    return { process: child, port: Number(port) };
  } catch (startupError) {
    // On the failure path this function returns nothing — meaning the
    // caller has no handle to reap. So a Chromium that failed to start is
    // reaped here. Otherwise the process survives and the calling script's
    // cleanup deletes the profile directory out from under it
    // (`rm -rf "$profile_dir"` in scripts/flutter_web_smoke.sh).
    child.kill('SIGKILL');
    closeSync(logFd);
    throw startupError;
  }
}

/** An open CDP socket. A send without a session goes to the browser target. */
class CdpConnection {
  constructor(socket) {
    this.socket = socket;
    this.nextId = 0;
    this.pending = new Map();
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data);
      const settle = this.pending.get(message.id);
      if (!settle) {
        return;
      }
      this.pending.delete(message.id);
      if (message.error) {
        settle.reject(
          new CdpError(`${settle.method}: ${JSON.stringify(message.error)}`),
        );
        return;
      }
      settle.resolve(message.result);
    });
  }

  send(method, params = {}, sessionId = undefined) {
    const id = (this.nextId += 1);
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, method });
      this.socket.send(JSON.stringify({ id, method, params, sessionId }));
    });
  }

  close() {
    this.socket.close();
  }
}

/** Session attached to one page target. All contract assertions run on it. */
export class PageSession {
  constructor(connection, sessionId) {
    this.connection = connection;
    this.sessionId = sessionId;
  }

  send(method, params = {}) {
    return this.connection.send(method, params, this.sessionId);
  }

  /**
   * Evaluate an expression in the page context; await it if it returns a
   * Promise. Exceptions raised in the page surface as [`CdpError`] — they
   * must not quietly become `undefined` and make assertions misfire.
   */
  async evaluate(expression) {
    const result = await this.send('Runtime.evaluate', {
      expression,
      awaitPromise: true,
      returnByValue: true,
    });
    if (result.exceptionDetails) {
      const { text, exception } = result.exceptionDetails;
      throw new CdpError(
        `page evaluate failed: ${text} ${exception?.description ?? ''}`.trim(),
      );
    }
    return result.result.value;
  }
}

/** Attach to the browser, open one new page target, and return its session. */
export async function openPageSession(port) {
  const version = await (
    await fetch(`http://${LOOPBACK_HOST}:${port}/json/version`)
  ).json();
  const socket = new WebSocket(version.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', reject, { once: true });
  });

  const connection = new CdpConnection(socket);
  const { targetId } = await connection.send('Target.createTarget', {
    url: 'about:blank',
  });
  const { sessionId } = await connection.send('Target.attachToTarget', {
    targetId,
    flatten: true,
  });

  const session = new PageSession(connection, sessionId);
  await session.send('Page.enable');
  await session.send('Network.enable');
  await session.send('Runtime.enable');
  return { connection, session };
}
