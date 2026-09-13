#!/usr/bin/env python3
"""Hardcoded user-facing string gate for the Flutter surface.

Every user-visible string in `flutter_app/lib/src/{widgets,pages}` is
supposed to flow through the `t()` helper (lib/src/i18n/t.dart) backed by
the Rust En/Ko catalog (upeg-pegboard-ui/src/i18n.rs). This gate detects
hardcoded literals that bypass it.

A perfect Dart parser is out of scope — the detector is a pragmatic
heuristic:

  * candidate sites: `Text('...')` / `Text("...")` and the user-facing
    named string params `labelText:` / `hintText:` / `helperText:` /
    `tooltip:` / `semanticsLabel:`;
  * a candidate literal counts as user-facing when it contains Hangul,
    or at least two consecutive English words (2+ letters each).

Because the tree predates the gate, existing violations are snapshotted
into a baseline file (fixtures/flutter-i18n-baseline.json, same pattern
as the other fixtures/ drift gates): `check` fails only on NEW
violations; entries that disappear are reported as prunable, not fatal.

Escape hatch: a `// i18n-exempt: <reason>` comment on the same line as
the literal skips it (e.g. brand wordmarks, CLI syntax).

Usage:
  python3 scripts/flutter_i18n_check.py write --baseline fixtures/flutter-i18n-baseline.json
  python3 scripts/flutter_i18n_check.py check --baseline fixtures/flutter-i18n-baseline.json
"""

import argparse
import json
import re
import sys
from pathlib import Path

SCAN_DIRS = ("flutter_app/lib/src/widgets", "flutter_app/lib/src/pages")
DEFAULT_BASELINE = "fixtures/flutter-i18n-baseline.json"
VERSION = 1
EXEMPT_MARKER = "i18n-exempt"

# Candidate extraction. `Text(` with an optional line break before the
# literal; named params must be directly followed by the literal.
_QUOTED = r"(?:'(?P<sq>(?:[^'\\\n]|\\.)*)'|\"(?P<dq>(?:[^\"\\\n]|\\.)*)\")"
CANDIDATE_RES = (
    re.compile(r"\bText\(\s*" + _QUOTED),
    re.compile(
        r"\b(?:labelText|hintText|helperText|tooltip|semanticsLabel)\s*:\s*"
        + _QUOTED
    ),
)

HANGUL_RE = re.compile(r"[가-힣]")
ENGLISH_TWO_WORDS_RE = re.compile(r"[A-Za-z]{2,}\s+[A-Za-z]{2,}")


def repo_root():
    return Path(__file__).resolve().parents[1]


def is_user_facing(literal):
    return bool(HANGUL_RE.search(literal) or ENGLISH_TWO_WORDS_RE.search(literal))


def scan_file(path, root):
    """Yield {file, literal} violation records for one Dart file."""
    rel = path.relative_to(root).as_posix()
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    seen = set()
    for pattern in CANDIDATE_RES:
        for m in pattern.finditer(text):
            literal = m.group("sq") if m.group("sq") is not None else m.group("dq")
            if not is_user_facing(literal):
                continue
            # Line-level exemption marker.
            line_no = text.count("\n", 0, m.start())
            window = lines[line_no : line_no + 2]
            if any(EXEMPT_MARKER in ln for ln in window):
                continue
            key = (rel, literal)
            if key in seen:
                continue
            seen.add(key)
            yield {"file": rel, "literal": literal}


def collect_violations(root):
    records = []
    for scan_dir in SCAN_DIRS:
        base = root / scan_dir
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*.dart")):
            records.extend(scan_file(path, root))
    records.sort(key=lambda r: (r["file"], r["literal"]))
    return records


def load_baseline(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    return {(r["file"], r["literal"]) for r in data["violations"]}


def cmd_write(root, baseline_path):
    records = collect_violations(root)
    payload = {
        "project": "upeg",
        "version": VERSION,
        "note": (
            "Snapshot of pre-existing hardcoded user-facing strings in "
            "flutter_app/lib/src/{widgets,pages}. New entries fail "
            "`just flutter-i18n-check`; migrate strings to t() + the Rust "
            "En/Ko catalog and regenerate via scripts/flutter_i18n_check.py write."
        ),
        "violations": records,
    }
    baseline_path.parent.mkdir(parents=True, exist_ok=True)
    baseline_path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"wrote {len(records)} baseline violation(s) to {baseline_path}")
    return 0


def cmd_check(root, baseline_path):
    if not baseline_path.is_file():
        print(
            f"error: baseline {baseline_path} not found; generate it with\n"
            f"  python3 scripts/flutter_i18n_check.py write --baseline {baseline_path}",
            file=sys.stderr,
        )
        return 1
    baseline = load_baseline(baseline_path)
    current = collect_violations(root)
    current_keys = {(r["file"], r["literal"]) for r in current}

    new = sorted(current_keys - baseline)
    resolved = sorted(baseline - current_keys)

    if resolved:
        print(f"info: {len(resolved)} baseline entr(y/ies) no longer present —")
        print("      prune via: python3 scripts/flutter_i18n_check.py write")
        for file, literal in resolved:
            print(f"      resolved: {file}: {literal!r}")

    if new:
        print(
            "error: new hardcoded user-facing string(s) — route them through "
            "t() (lib/src/i18n/t.dart) with En/Ko keys in "
            "upeg-pegboard-ui/src/i18n.rs:",
            file=sys.stderr,
        )
        for file, literal in new:
            print(f"  {file}: {literal!r}", file=sys.stderr)
        return 1

    print(
        f"ok: no new hardcoded user-facing strings "
        f"({len(current)} baselined violation(s) remain to migrate)"
    )
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("write", "check"))
    parser.add_argument("--baseline", default=DEFAULT_BASELINE)
    args = parser.parse_args()

    root = repo_root()
    baseline_path = root / args.baseline
    if args.mode == "write":
        return cmd_write(root, baseline_path)
    return cmd_check(root, baseline_path)


if __name__ == "__main__":
    sys.exit(main())
