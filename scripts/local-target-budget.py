#!/usr/bin/env python3
"""Guard the allocated size of Cargo's target directory around a local command.

CI owns its own target lifecycle and bypasses this local guard. The default
budget is 150 GiB; UPEG_LOCAL_TARGET_MAX_GIB accepts a positive decimal.
This is a check at command boundaries, not a filesystem quota.
"""
import decimal
import json
import os
import signal
import subprocess
import sys


def fail(message):
    print(f"local target budget: {message}", file=sys.stderr)


def main():
    if len(sys.argv) < 3 or sys.argv[1] != "--":
        fail("usage: local-target-budget.py -- COMMAND [ARG...]")
        return 2
    command = sys.argv[2:]
    if os.environ.get("CI"):
        os.execvp(command[0], command)

    try:
        limit = decimal.Decimal(os.environ.get("UPEG_LOCAL_TARGET_MAX_GIB", "150"))
        if not limit.is_finite() or limit <= 0:
            raise ValueError("must be a positive finite number")
        limit_bytes = int(limit * (1024 ** 3))
        if limit_bytes < 1:
            raise ValueError("must represent at least one byte")
    except (decimal.InvalidOperation, ValueError, OverflowError) as error:
        fail(f"invalid UPEG_LOCAL_TARGET_MAX_GIB: {error}")
        return 2

    try:
        metadata = subprocess.run(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
            check=True, stdout=subprocess.PIPE, text=True,
        )
        target = json.loads(metadata.stdout)["target_directory"]
        if not isinstance(target, str) or not os.path.isabs(target):
            raise ValueError("Cargo returned an invalid target_directory")
    except (OSError, subprocess.CalledProcessError, ValueError, KeyError) as error:
        fail(f"cannot determine Cargo target directory: {error}")
        return 2

    def check():
        if not os.path.lexists(target):
            return True
        try:
            result = subprocess.run(["du", "-sk", os.path.realpath(target)], check=True, stdout=subprocess.PIPE, text=True)
            size = int(result.stdout.split()[0]) * 1024
        except (OSError, subprocess.CalledProcessError, ValueError, IndexError) as error:
            fail(f"cannot measure {target!r}: {error}")
            return False
        if size > limit_bytes:
            fail(f"{target!r} uses {size} allocated bytes, exceeding the {limit_bytes}-byte budget; free space manually before retrying")
            return False
        return True

    if not check():
        return 1
    try:
        child = subprocess.Popen(command)
    except OSError as error:
        fail(f"cannot start command: {error}")
        return 127

    def forward(signum, _frame):
        try:
            child.send_signal(signum)
        except ProcessLookupError:
            pass

    for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(signum, forward)
    status = child.wait()
    within_budget = check()
    if status:
        return 128 - status if status < 0 else status
    return 0 if within_budget else 1


if __name__ == "__main__":
    sys.exit(main())
