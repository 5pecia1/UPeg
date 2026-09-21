#!/usr/bin/env python3
"""Read Git branches as JSON for the git_refs Toolkit example.

This wrapper deliberately exposes only read operations.  It accepts a
repository path and an optional selected branch, then runs Git with argument
arrays (never a shell) so names containing spaces remain ordinary values.
"""

import argparse
import json
import subprocess
import sys


def git(repo: str, arguments: list[str]) -> str:
    completed = subprocess.run(
        ["git", "-C", repo, *arguments],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return completed.stdout


def list_refs(repo: str) -> dict[str, object]:
    # `--end-of-options` is significant: ref patterns are data, not options.
    output = git(
        repo,
        [
            "for-each-ref",
            "--format=%(refname:short)%00%(objectname)%00%(subject)",
            "--end-of-options",
            "refs/heads",
        ],
    )
    refs = []
    for line in output.splitlines():
        ref, object_id, subject = line.split("\x00", 2)
        refs.append({"ref": ref, "object": object_id, "subject": subject})
    return {"refs": refs}


def selected_ref(repo: str, ref: str) -> dict[str, str]:
    # Prefixing with refs/heads makes a branch called "--x" unambiguous too.
    object_id = git(
        repo,
        ["rev-parse", "--verify", "--end-of-options", f"refs/heads/{ref}^{{commit}}"],
    ).strip()
    return {"repo": repo, "ref": ref, "object": object_id}


def main() -> int:
    parser = argparse.ArgumentParser(description="Read Git branch references as JSON")
    parser.add_argument("operation", choices=("list", "show"))
    parser.add_argument("--repo", required=True)
    parser.add_argument("--ref")
    args = parser.parse_args()
    if args.operation == "show" and args.ref is None:
        parser.error("show requires --ref")
    value = list_refs(args.repo) if args.operation == "list" else selected_ref(args.repo, args.ref)
    print(json.dumps(value, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as error:
        sys.stderr.write(error.stderr)
        raise SystemExit(error.returncode)
