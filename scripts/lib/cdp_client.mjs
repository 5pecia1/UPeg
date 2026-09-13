// 헤드리스 Chromium을 CDP(Chrome DevTools Protocol)로 모는 최소 클라이언트.
//
// 왜 직접 쓰는가: 이 저장소의 브라우저 검증은 devcontainer의 `chromium`
// 하나에만 기대고, npm 의존성을 새로 들이지 않는다. Node 22+의 전역
// `WebSocket`과 `fetch`만으로 CDP를 말하기에 충분하다.
//
// 여기에는 프로토콜 배관만 둔다. 무엇을 단언할지는 호출자의 몫이다(SoC).

import { spawn } from 'node:child_process';
import { closeSync, existsSync, openSync, readFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const LOOPBACK_HOST = '127.0.0.1';
const DEVTOOLS_PORT_FILE = 'DevToolsActivePort';
const POLL_INTERVAL_MS = 100;

/** CDP 배관에서 난 실패 — 계약 위반(단언 실패)과 구분된다. */
export class CdpError extends Error {}

/** `start`부터 위로 훑어 비어 있는 loopback 포트를 하나 찾는다. */
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

/** 조건이 참이 될 때까지 폴링한다. 시간 초과는 마지막 값과 함께 던진다. */
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
 * 헤드리스 Chromium을 띄우고 DevTools 포트를 돌려준다.
 * 포트는 `--remote-debugging-port=0`으로 커널이 고르고, Chromium이
 * 사용자 데이터 디렉터리에 적어 준 `DevToolsActivePort`에서 읽는다.
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
    // 실패로 빠져나가면 이 함수는 아무것도 돌려주지 않는다 — 호출자에게는
    // 거둘 핸들이 없다는 뜻이다. 그래서 시작에 실패한 Chromium은 여기서
    // 우리가 거둔다. 그러지 않으면 프로세스는 살아남고, 호출 스크립트의
    // cleanup이 그 프로세스가 쓰고 있는 프로필 디렉터리를 밑에서 지워
    // 버린다(scripts/flutter_web_smoke.sh의 `rm -rf "$profile_dir"`).
    child.kill('SIGKILL');
    closeSync(logFd);
    throw startupError;
  }
}

/** 열린 CDP 소켓. 세션 없이 보내면 브라우저 타깃으로 간다. */
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

/** 한 페이지 타깃에 붙은 세션. 계약 단언은 전부 이 위에서 돈다. */
export class PageSession {
  constructor(connection, sessionId) {
    this.connection = connection;
    this.sessionId = sessionId;
  }

  send(method, params = {}) {
    return this.connection.send(method, params, this.sessionId);
  }

  /**
   * 페이지 컨텍스트에서 식을 평가한다. Promise를 돌려주는 식이면 기다린다.
   * 페이지에서 난 예외는 [`CdpError`]로 올라온다 — 조용히 `undefined`가
   * 되어 단언이 헛돌지 않게.
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

/** 브라우저에 붙고 새 페이지 타깃을 하나 열어 세션을 준다. */
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
