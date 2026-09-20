#!/usr/bin/env python3
"""Hardcoded user-facing string gate for the Flutter surface.

Every user-visible string in `flutter_app/lib/src/{widgets,pages}` and
`flutter_app/lib/src/features/host_attach` is supposed to flow through the
`t()` helper (lib/src/i18n/t.dart) backed by the Rust En/Ko catalog
(upeg-pegboard-ui/src/i18n.rs). This gate detects hardcoded literals that
bypass it.

A perfect Dart parser is out of scope — the detector is a pragmatic
heuristic:

  * `//`, `///` and `/* */` comments are masked before scanning, so quoted
    prose inside commentary never fires;
  * presentation sites: `Text('...')` / `Text("...")`, the named string
    params `labelText:` / `hintText:` / `helperText:` / `tooltip:` /
    `semanticsLabel:`, and `Tooltip(message: '...')` / `Semantics(label:
    '...')` with that named argument first;
  * simple literal assignments (`final|const|var|late [String?] name =
    '...'`) count only when the same file passes that bare name to one of
    those presentation sites (e.g. `Text(name)`). No inter-file or scoped
    data-flow analysis is attempted;
  * ANY string literal containing Hangul counts regardless of site — ko UI
    text belongs to the catalog, so `switch` arms, positional args and
    interpolations are covered too;
  * an English candidate literal counts as user-facing when it contains at
    least two consecutive English words (2+ letters each).

Intentionally not covered: generic `message:` fields (protocol errors and
logging are not inherently UI), unreferenced constants such as user-agent
strings or CLI syntax, and English prose in other expression shapes (e.g.
`x ?? 'fallback'`, `=> 'literal'` returns, nested call arguments). These
need review at the rendering boundary; the check does not certify all UI
copy as localized. Hangul literals still count at any site.

Because the tree predates the gate, existing violations are snapshotted
into a baseline file (fixtures/flutter-i18n-baseline.json, same pattern
as the other fixtures/ drift gates): `check` fails only on NEW
violations; entries that disappear are reported as prunable, not fatal.

There are no per-line suppressions. Migrate UI copy instead of adding new
findings to the baseline.

Usage:
  python3 scripts/flutter_i18n_check.py write --baseline fixtures/flutter-i18n-baseline.json
  python3 scripts/flutter_i18n_check.py check --baseline fixtures/flutter-i18n-baseline.json
  python3 scripts/flutter_i18n_check.py self-test
"""

import argparse
import json
import re
import sys
from pathlib import Path

SCAN_DIRS = (
    "flutter_app/lib/src/widgets",
    "flutter_app/lib/src/pages",
    "flutter_app/lib/src/features/host_attach",
)
DEFAULT_BASELINE = "fixtures/flutter-i18n-baseline.json"
VERSION = 1

# A `${...}` interpolation may itself contain quoted segments
# (`'${map['key']} items'`), so treat it as a unit in the literal body.
_QUOTED = (
    r"(?:'(?P<sq>(?:\$\{[^}]*\}|[^'\\\n]|\\.)*)'"
    r'|"(?P<dq>(?:\$\{[^}]*\}|[^"\\\n]|\\.)*)")'
)
_PRESENTATION_SITE = (
    r"\b(?:Text\s*\(\s*|Tooltip\s*\(\s*message\s*:\s*|Semantics\s*\(\s*label\s*:\s*|"
    r"(?:labelText|hintText|helperText|tooltip|semanticsLabel)\s*:\s*)"
)
CANDIDATE_RE = re.compile(_PRESENTATION_SITE + r"r?" + _QUOTED)
PRESENTED_NAME_RE = re.compile(_PRESENTATION_SITE + r"(?P<name>\w+)\s*[,)]")
ASSIGNED_LITERAL_RE = re.compile(
    r"\b(?:final|const|var|late)\s+(?:String\s*\??\s+)?(?P<name>\w+)\s*=\s*r?"
    + _QUOTED
)
# Hangul literals count wherever they appear — see the module docstring.
# Triple-quoted strings get their own branches (they may span newlines).
ANY_STRING_RE = re.compile(
    r"'''(?P<tsq>.*?)'''|" + r'"""(?P<tdq>.*?)"""|' + _QUOTED, re.S
)

HANGUL_RE = re.compile(r"[가-힣]")
ENGLISH_TWO_WORDS_RE = re.compile(r"[A-Za-z]{2,}\s+[A-Za-z]{2,}")


def repo_root():
    return Path(__file__).resolve().parents[1]


def is_user_facing(literal):
    return bool(HANGUL_RE.search(literal) or ENGLISH_TWO_WORDS_RE.search(literal))


def mask_dart_comments(text):
    """Return `text` with `//`, `///` and `/* */` comments blanked to spaces.

    Offsets and newlines are preserved so diagnostics still map to original
    line numbers. String literals — including `r''` raw strings,
    triple-quoted strings and `${}` interpolation — are kept verbatim, so a
    `//` inside a string is never read as a comment opener and a `'prose'`
    inside a comment is never read as a literal.
    """
    out = list(text)
    n = len(text)
    i = 0
    # Frame stack: "code", or ("str", quote, triple, raw) inside a string.
    # `${` inside a string pushes a code frame; `braces` tracks its '{' '}'
    # balance so the matching '}' pops back to the string.
    stack = ["code"]
    braces = [0]
    while i < n:
        ch = text[i]
        top = stack[-1]
        if top == "code":
            if text.startswith("//", i):
                end = text.find("\n", i)
                if end < 0:
                    end = n
                for k in range(i, end):
                    out[k] = " "
                i = end
            elif text.startswith("/*", i):
                # Dart nests block comments.
                depth = 1
                end = i + 2
                while end < n and depth:
                    if text.startswith("/*", end):
                        depth += 1
                        end += 2
                    elif text.startswith("*/", end):
                        depth -= 1
                        end += 2
                    else:
                        end += 1
                for k in range(i, min(end, n)):
                    if text[k] != "\n":
                        out[k] = " "
                i = end
            elif (
                ch == "r"
                and text[i + 1 : i + 2] in ("'", '"')
                and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] in "_$"))
            ):
                quote = text[i + 1]
                triple = text.startswith(quote * 3, i + 1)
                stack.append(("str", quote, triple, True))
                i += 4 if triple else 2
            elif ch in ("'", '"'):
                triple = text.startswith(ch * 3, i)
                stack.append(("str", ch, triple, False))
                i += 3 if triple else 1
            elif ch == "{":
                braces[-1] += 1
                i += 1
            elif ch == "}":
                if braces[-1]:
                    braces[-1] -= 1
                elif len(braces) > 1:
                    # '}' closing the ${ } hole that opened this code frame.
                    braces.pop()
                    stack.pop()
                i += 1
            else:
                i += 1
        else:
            _, quote, triple, raw = top
            if not raw and ch == "\\":
                i += 2
            elif not raw and ch == "$" and text[i + 1 : i + 2] == "{":
                stack.append("code")
                braces.append(0)
                i += 2
            elif not raw and ch == "$" and (
                text[i + 1 : i + 2].isalpha() or text[i + 1 : i + 2] == "_"
            ):
                i += 1
                while i < n and (text[i].isalnum() or text[i] == "_"):
                    i += 1
            elif triple and text.startswith(quote * 3, i):
                stack.pop()
                i += 3
            elif not triple and ch == quote:
                stack.pop()
                i += 1
            elif not triple and ch == "\n":
                # Unterminated single-line string — recover at the newline.
                stack.pop()
                i += 1
            else:
                i += 1
    return "".join(out)


def scan_text(rel, text):
    """Yield {file, literal} violation records for one Dart source text."""
    masked = mask_dart_comments(text)
    seen = set()

    def emit(m, hangul_anywhere):
        groups = m.groupdict()
        literal = next(
            (groups[k] for k in ("sq", "dq", "tsq", "tdq") if groups.get(k) is not None),
            None,
        )
        if literal is None:
            return
        if hangul_anywhere:
            if not HANGUL_RE.search(literal):
                return
        elif not is_user_facing(literal):
            return
        key = (rel, literal)
        if key in seen:
            return
        seen.add(key)
        return {"file": rel, "literal": literal}

    for m in CANDIDATE_RE.finditer(masked):
        record = emit(m, hangul_anywhere=False)
        if record:
            yield record
    presented_names = {m.group("name") for m in PRESENTED_NAME_RE.finditer(masked)}
    for m in ASSIGNED_LITERAL_RE.finditer(masked):
        if m.group("name") in presented_names:
            record = emit(m, hangul_anywhere=False)
            if record:
                yield record
    for m in ANY_STRING_RE.finditer(masked):
        record = emit(m, hangul_anywhere=True)
        if record:
            yield record


def scan_file(path, root):
    """Yield {file, literal} violation records for one Dart file."""
    rel = path.relative_to(root).as_posix()
    yield from scan_text(rel, path.read_text(encoding="utf-8"))


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
            "flutter_app/lib/src/{widgets,pages,features/host_attach}. New "
            "entries fail `just flutter-i18n-check`; migrate strings to t() + "
            "the Rust En/Ko catalog and regenerate via "
            "scripts/flutter_i18n_check.py write."
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


def cmd_self_test():
    """Fixture-based checks for the detector itself — no filesystem needed."""

    def assert_equal(label, actual, expected):
        if actual != expected:
            raise SystemExit(label + " expected " + repr(expected) + " but got " + repr(actual))

    def literals(text):
        return [r["literal"] for r in scan_text("selftest/a.dart", text)]

    assert_equal(
        "comment masking drops quoted comment prose",
        literals("// the pin shows '설정 필요' here\nfinal x = 1;\n"),
        [],
    )
    assert_equal(
        "doc comment prose ignored",
        literals("/// a \"연결 확인\" action probes /healthz\nconst x = 1;\n"),
        [],
    )
    assert_equal(
        "block comment masked (nested too)",
        literals("/* Text('hidden one') /* nested */ still hidden */\nfinal x = 1;\n"),
        [],
    )
    assert_equal(
        "Text() literal still caught",
        literals("Text('load failed')\n"),
        ["load failed"],
    )
    for parameter in ("labelText", "hintText", "helperText", "tooltip", "semanticsLabel"):
        assert_equal(
            parameter + " literal caught",
            literals(parameter + ": 'load failed',\n"),
            ["load failed"],
        )
    assert_equal(
        "Tooltip message is a presentation site",
        literals("Tooltip(message: 'load failed', child: icon)\n"),
        ["load failed"],
    )
    assert_equal(
        "Semantics label is a presentation site, not every label field",
        literals(
            "const label = 'Running tool';\n"
            "Semantics(label: label, child: icon);\n"
            "Command(label: 'command syntax');\n"
        ),
        ["Running tool"],
    )
    assert_equal(
        "generic protocol error message is not presumed to be UI",
        literals(
            "CanonicalToolError(code: 'x', message: 'host unreachable');\n"
            "const hint = 'host temporarily unavailable';\n"
            "return AttachListUnavailable(hint);\n"
        ),
        [],
    )
    assert_equal(
        "debug events and diagnostic keys are not UI copy",
        literals(
            "observer.emit(message: 'Selector probe failed');\n"
            "diagnosticValueKey('move pin');\n"
            "const String key = 'debug event key';\n"
            "diagnosticValueKey(key);\n"
        ),
        [],
    )
    assert_equal(
        "user-agent and CLI syntax constants are not UI prose",
        literals(
            "const String ua = 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) ';\n"
            "const String verb = 'credential add';\n"
            "WebView(userAgent: ua);\n"
            "final command = 'upeg $verb <name>';\n"
            "return command;\n"
        ),
        [],
    )
    assert_equal(
        "variable-assigned prose passed to Text caught",
        literals("const String hint = 'no pins on this board yet';\nText(hint);\n"),
        ["no pins on this board yet"],
    )
    assert_equal(
        "typed nullable String passed to a presentation param caught",
        literals("final String? msg = 'could not load the board';\nhelperText: msg,\n"),
        ["could not load the board"],
    )
    assert_equal(
        "unreferenced English prose and commented-out Text use ignored",
        literals("const String hint = 'no pins on this board yet';\n// Text(hint);\n"),
        [],
    )
    assert_equal(
        "function and member references are not a bare local variable",
        literals("const hint = 'diagnostic message';\nText(hint());\nText(other.hint);\n"),
        [],
    )
    assert_equal(
        "raw presentation literal caught",
        literals("Text(r'load failed');\n"),
        ["load failed"],
    )
    assert_equal(
        "single English word is not user-facing",
        literals("const String kind = 'embedded_view';\nText('hex')\n"),
        [],
    )
    assert_equal(
        "Hangul literal caught at an unlisted site (switch arm)",
        literals("String get message => switch (c) {\n  Code.bad => '읽을 수 없습니다',\n};\n"),
        ["읽을 수 없습니다"],
    )
    assert_equal(
        "Hangul in interpolation caught",
        literals("Text(x ? a : '${n}개 선택됨')\n"),
        ["${n}개 선택됨"],
    )
    assert_equal(
        "adjacent literals each scanned",
        literals("final s = '응답이 최대 '\n    '$max bytes를 초과';\n"),
        ["응답이 최대 ", "$max bytes를 초과"],
    )
    assert_equal(
        "// inside a string is not a comment",
        literals("const url = 'http://example.com';\nText('server error')\n"),
        ["server error"],
    )
    assert_equal(
        "raw string with escaped-looking content stays in code",
        literals("final re = RegExp(r'\\d+ // x');\nText('could not parse')\n"),
        ["could not parse"],
    )
    assert_equal(
        "nested quotes inside interpolation stay in string",
        literals("Text('${map['key']} items failed')\n"),
        ["${map['key']} items failed"],
    )
    assert_equal(
        "a trailing comment cannot hide UI prose",
        literals("labelText: 'load failed', // technical detail\n"),
        ["load failed"],
    )
    assert_equal(
        "duplicate literal deduped",
        literals("Text('same prose here'); Text('same prose here');\n"),
        ["same prose here"],
    )
    assert_equal(
        "triple-quoted literal scanned once",
        literals("const s = '''\nmulti line 한국어 text\n''';\n"),
        ["\nmulti line 한국어 text\n"],
    )
    print("self-test ok")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("write", "check", "self-test"))
    parser.add_argument("--baseline", default=DEFAULT_BASELINE)
    args = parser.parse_args()

    if args.mode == "self-test":
        return cmd_self_test()

    root = repo_root()
    baseline_path = root / args.baseline
    if args.mode == "write":
        return cmd_write(root, baseline_path)
    return cmd_check(root, baseline_path)


if __name__ == "__main__":
    sys.exit(main())
