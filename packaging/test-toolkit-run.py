#!/usr/bin/env python3
"""Own one Toolkit fixture and the process group using it."""
import os
from pathlib import Path
import secrets
import shutil
import signal
import subprocess
import sys
import tempfile
import time


def active_run():
    root = os.environ.get('UPEG_TOOLKIT_RUN_ROOT')
    owner = os.environ.get('UPEG_TOOLKIT_RUN_OWNER')
    token = os.environ.get('UPEG_TOOLKIT_RUN_TOKEN')
    if not root or not owner or not token:
        return False
    try:
        os.kill(int(owner), 0)
        return (Path(root) / 'owner').read_text() == f'{owner}\n{token}\n'
    except (OSError, ValueError):
        return False


def main():
    if len(sys.argv) < 2:
        print("usage: test-toolkit-run.sh COMMAND [ARG ...]", file=sys.stderr)
        return 2
    if active_run():
        os.execvp(sys.argv[1], sys.argv[1:])
    # A stale inherited root must never be recreated by a late descendant.
    if os.environ.get('UPEG_TOOLKIT_RUN_ROOT'):
        print('test-toolkit-run: inherited fixture owner is no longer active', file=sys.stderr)
        return 1
    root = tempfile.mkdtemp(prefix='upeg-toolkit-test.', dir=os.environ.get('TMPDIR'))
    owner = str(os.getpid())
    token = secrets.token_hex(16)
    Path(root, 'owner').write_text(f'{owner}\n{token}\n')
    env = dict(os.environ, UPEG_TOOLKIT_RUN_ROOT=root, UPEG_TOOLKIT_RUN_OWNER=owner,
               UPEG_TOOLKIT_RUN_TOKEN=token)
    result = 1
    child = None
    received = None
    deadline = None

    def stop(signum, _frame):
        nonlocal received, deadline
        if received is None:
            received = signum
            deadline = time.monotonic() + 1
        if child is not None:
            try:
                os.killpg(child.pid, signum)
            except ProcessLookupError:
                pass

    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(sig, stop)
    try:
        child = subprocess.Popen(sys.argv[1:], env=env, start_new_session=True)
        if received is not None:
            stop(received, None)
        while True:
            try:
                status = child.wait(timeout=0.1)
                break
            except subprocess.TimeoutExpired:
                if deadline is not None and time.monotonic() >= deadline:
                    try:
                        os.killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    status = child.wait()
                    break
        # Descendants may outlive their immediate parent, including on failure.
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        if status == 0 and received is None and Path(root, 'prepared').exists():
            expected = Path(root, 'prepared').read_text().splitlines()[2]
            fingerprint = subprocess.run(
                [sys.executable, str(Path(__file__).with_name('test-toolkit-fingerprint.py'))],
                capture_output=True, text=True)
            if fingerprint.returncode or fingerprint.stdout.strip() != expected:
                print('test-toolkit-run: fixture inputs changed during run', file=sys.stderr)
                status = 1
        result = 128 + received if received is not None else (128 - status if status < 0 else status)
    finally:
        try:
            shutil.rmtree(root)
        except OSError as error:
            print(f'test-toolkit-run: cleanup failed for {root}: {error}', file=sys.stderr)
            result = result or 1
    return result


if __name__ == '__main__':
    sys.exit(main())
