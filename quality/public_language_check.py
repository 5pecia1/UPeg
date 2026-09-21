#!/usr/bin/env python3
"""Public-surface language guard.

Flags Hangul (Korean) text that would ship in the public repository as
developer-facing identifiers or authored prose. Korean user-facing UI and
catalogs are intentional (upeg-pegboard-ui i18n, chrome-ext/_locales/ko,
README.ko.md) — this guard protects the code/docs surface instead:

  * declaration identifiers and test-name literals in Rust, Dart,
    Python, JS/TS and bats sources;
  * Rust/Dart doc comments (`///`, `//!`) — prose rendered as docs;
  * Markdown outside fenced code blocks, and SVG files (diagram labels).

The only localized Markdown page exempted is root `README.ko.md`;
`docs/LEXICON.md` and any other `*.ko.md` page are checked normally.
A link label targeting the root localized README is allowed, but the rest
of its line is still checked. Runtime locale strings, Unicode test inputs,
and inline `code` spans are subject data, not declaration names or prose.
There is no blanket fixture exemption: test names in fixture sources and
prose in fixture Markdown are checked too.

Vendored third-party originals (`vendor/**`,
`flutter_app/rust_builder/cargokit/**`), legal `LICENSE*`/`NOTICE*` files,
and generated trees (`target`, `build`, `dist`, `node_modules`,
`.dart_tool`) are excluded. Generated FRB output under
`flutter_app/lib/src/rust/**` mirrors Rust doc comments, which are checked
at their source instead.

Boundaries (intentionally not gated): plain `//`/`#` developer comments
are editorial, and data formats (json/toml/yaml) carry subject data.
Declaration matching is line-anchored and deliberately heuristic, not a
full language parser. It avoids ordinary comment/inline-string examples;
multiline embedded source and macro-generated names still need review.

This checker runs only on an exported checkout: every authored path under
ROOT is treated as public. It does not select source-checkout paths or
simulate export transformations. There are no baselines or per-line
suppressions; every finding fails the check.

Usage (run after export, not against the source checkout):
  python3 quality/public_language_check.py check [--root EXPORTED_ROOT]
  python3 quality/public_language_check.py self-test
"""

import argparse
import os
import posixpath
import re
import sys
import tempfile
from pathlib import Path

HANGUL_RE = re.compile(r"[가-힣]")
HANGUL_IDENT = r"\w*[가-힣]\w*"

_RUST_DECL_RE = re.compile(
    r"^\s*(?:(?:pub(?:\s*\([^)]*\))?|async|const|unsafe|extern|default)\s+)*"
    r"(?:fn|mod|struct|enum|trait|type|impl|static|let)\s+" + HANGUL_IDENT,
    re.M,
)
_DART_DECL_RE = re.compile(
    r"^\s*(?:(?:abstract|base|final|sealed|interface)\s+)*"
    r"(?:class|enum|mixin|extension|typedef)\s+" + HANGUL_IDENT,
    re.M,
)
_DART_VAR_RE = re.compile(
    r"^\s*(?:final|const|var|late)\s+(?:[\w<>?]+\s+)?" + HANGUL_IDENT + r"\s*=",
    re.M,
)
_NAME_ARG = r"(?P<name>(?:[^'\"\\\n]|\\.)*[가-힣](?:[^'\"\\\n]|\\.)*)"
_DART_TEST_RE = re.compile(
    r"\b(?:test|testWidgets|group|setUp|setUpAll|tearDown|tearDownAll)"
    r"\s*\(\s*r?['\"]" + _NAME_ARG
)
_PY_DECL_RE = re.compile(r"^\s*(?:async\s+)?(?:def|class)\s+" + HANGUL_IDENT, re.M)
_JS_TEST_RE = re.compile(r"\b(?:test|it|describe)\s*\(\s*['\"`]" + _NAME_ARG)
_JS_DECL_RE = re.compile(
    r"^\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?"
    r"(?:function\*?|class|const|let|var)\s+" + HANGUL_IDENT,
    re.M,
)
_BATS_TEST_RE = re.compile(r"@test\s+[\"'](?P<name>[^\"']*[가-힣][^\"']*)")

IDENTIFIER_RES = {
    ".rs": (_RUST_DECL_RE,),
    ".dart": (_DART_DECL_RE, _DART_VAR_RE, _DART_TEST_RE),
    ".py": (_PY_DECL_RE,),
    ".js": (_JS_TEST_RE, _JS_DECL_RE),
    ".mjs": (_JS_TEST_RE, _JS_DECL_RE),
    ".ts": (_JS_TEST_RE, _JS_DECL_RE),
    ".bats": (_BATS_TEST_RE,),
}
DOC_COMMENT_RE = {
    ".rs": re.compile(r"^\s*//[/!][^\n]*", re.M),
    ".dart": re.compile(r"^\s*///[^\n]*", re.M),
}
FENCE_RE = re.compile(r"^\s*(```|~~~)")
KO_LINK_RE = re.compile(
    r"\[[^\]\n]*\]\((?P<target>(?:\.{1,2}/)*README\.ko\.md)(?:#[^)\s]*)?\)"
)
INLINE_CODE_RE = re.compile(r"`[^`]*`")

GENERATED_DIRS = {".git", "target", "build", "dist", "node_modules", ".dart_tool"}
EXEMPT_PREFIXES = (
    "vendor/",
    "flutter_app/rust_builder/cargokit/",
    # flutter_rust_bridge output — mirrors upeg-frb `///` docs, which are
    # checked at the Rust source instead of the generated copy.
    "flutter_app/lib/src/rust/",
)
EXEMPT_DOC_FILES = {"README.ko.md"}


def is_exempt(rel):
    if any(rel.startswith(prefix) for prefix in EXEMPT_PREFIXES):
        return True
    name = rel.rsplit("/", 1)[-1]
    return name.startswith("LICENSE") or name.startswith("NOTICE")


def strip_fenced_code(text):
    """Blank fenced code blocks, preserving newlines for line numbers."""
    out = []
    fence = None
    for line in text.splitlines(keepends=True):
        m = FENCE_RE.match(line)
        if fence is None and m:
            fence = m.group(1)
        elif fence is not None and m and m.group(1) == fence:
            fence = None
        elif fence is not None:
            line = "\n" * line.count("\n")
        out.append(line)
    return "".join(out)


def find_identifiers(rel, text):
    for pattern in IDENTIFIER_RES.get(Path(rel).suffix, ()):
        for m in pattern.finditer(text):
            yield (m.groupdict().get("name") or m.group(0)).strip()


def find_doc_prose(rel, text):
    suffix = Path(rel).suffix
    if suffix in DOC_COMMENT_RE:
        for m in DOC_COMMENT_RE[suffix].finditer(text):
            if HANGUL_RE.search(INLINE_CODE_RE.sub("", m.group(0))):
                yield m.group(0).strip()
    elif suffix == ".svg":
        for line in text.splitlines():
            if HANGUL_RE.search(line):
                yield line.strip()[:160]
    elif suffix == ".md":
        if rel in EXEMPT_DOC_FILES:
            return

        def mask_readme_link(match):
            target = posixpath.normpath(posixpath.join(posixpath.dirname(rel), match["target"]))
            return "" if target == "README.ko.md" else match[0]

        for line in strip_fenced_code(text).splitlines():
            prose = KO_LINK_RE.sub(mask_readme_link, INLINE_CODE_RE.sub("", line))
            if HANGUL_RE.search(prose):
                yield line.strip()[:160]


def scan_tree(root):
    root = root.resolve()
    findings = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in GENERATED_DIRS)
        for name in sorted(filenames):
            path = Path(dirpath) / name
            rel = path.relative_to(root).as_posix()
            if is_exempt(rel) or path.suffix not in IDENTIFIER_RES.keys() | {".md", ".svg"}:
                continue
            try:
                text = path.read_text(encoding="utf-8")
            except (UnicodeDecodeError, ValueError, OSError):
                continue
            for ident in find_identifiers(rel, text):
                findings.append({"kind": "identifier", "file": rel, "text": ident})
            for prose in find_doc_prose(rel, text):
                findings.append({"kind": "doc", "file": rel, "text": prose})
    findings.sort(key=lambda f: (f["file"], f["kind"], f["text"]))
    return findings


def report(records, stream):
    for r in records:
        print(f"  [{r['kind']}] {r['file']}: {r['text']!r}", file=stream)


def cmd_check(root):
    current = scan_tree(root)
    if current:
        print(
            f"error: {len(current)} Hangul identifier/doc-prose finding(s) "
            "on the public surface:",
            file=sys.stderr,
        )
        report(current, sys.stderr)
        return 1
    print("ok: public surface free of Hangul identifiers and doc prose")
    return 0


def cmd_self_test():
    """Fixture checks for the detectors and the published-tree boundary."""

    def assert_equal(label, actual, expected):
        if actual != expected:
            raise SystemExit(
                label + " expected " + repr(expected) + " but got " + repr(actual)
            )

    assert_equal(
        "rust test fn name caught",
        list(find_identifiers("a_test.rs", "#[test]\nfn 커밋된_도구킷_로드() {}\n")),
        ["fn 커밋된_도구킷_로드"],
    )
    assert_equal(
        "rust string data with fake decl not caught",
        list(find_identifiers("a.rs", 'let s = "fn 한글()";\n// fn 한글\n')),
        [],
    )
    assert_equal(
        "rust qualified fn caught",
        list(find_identifiers("a.rs", "pub(crate) async fn 잠금_획득() {}\n")),
        ["pub(crate) async fn 잠금_획득"],
    )
    assert_equal(
        "dart test name caught",
        list(find_identifiers("t_test.dart", "test('레이아웃 저장', () {});\n")),
        ["레이아웃 저장"],
    )
    assert_equal(
        "dart string data test arg not caught",
        list(find_identifiers("t.dart", "final name = '테스트';\n")),
        [],
    )
    assert_equal(
        "dart class decl caught",
        list(find_identifiers("t.dart", "class 한글위젯 extends Widget {}\n")),
        ["class 한글위젯"],
    )
    assert_equal(
        "python def caught, commented def not",
        list(find_identifiers("t.py", "def 결과_집계():\n    pass\n# def 주석()\n")),
        ["def 결과_집계"],
    )
    assert_equal(
        "js test name caught, subject data not",
        list(
            find_identifiers(
                "t.test.js",
                "test('한국어 파일명 정규화', () => {});\n"
                "const f = fakeFile('첫째.PNG');\n",
            )
        ),
        ["한국어 파일명 정규화"],
    )
    assert_equal(
        "bats test name caught",
        list(find_identifiers("t.bats", '@test "캐시 갱신" { }\n')),
        ["캐시 갱신"],
    )
    md = (
        "# Title\n\nprose 한국어 here\n\n```rust\nfn 한글() {}\n```\n\n"
        "[한국어](README.ko.md)\n"
    )
    assert_equal(
        "markdown prose caught, fence and root README language link allowed",
        list(find_doc_prose("README.md", md)),
        ["prose 한국어 here"],
    )
    assert_equal(
        "a localized README link cannot exempt the rest of its line",
        list(find_doc_prose("README.md", "한국어 prose [한국어](./README.ko.md)\n")),
        ["한국어 prose [한국어](./README.ko.md)"],
    )
    assert_equal(
        "only a link resolving to the root README permits a localized label",
        list(
            find_doc_prose(
                "docs/index.md",
                "[한국어](../README.ko.md)\n"
                "[한국어](README.ko.md)\n"
                "[한국어](guide.ko.md)\n",
            )
        ),
        ["[한국어](README.ko.md)", "[한국어](guide.ko.md)"],
    )
    assert_equal(
        "only root localized README exempt, lexicon and SVG labels checked",
        (
            list(find_doc_prose("README.ko.md", "한국어\n")),
            list(find_doc_prose("docs/README.ko.md", "한국어\n")),
            list(find_doc_prose("docs/guide.ko.md", "한국어\n")),
            list(find_doc_prose("docs/LEXICON.md", "한국어\n")),
            list(find_doc_prose("docs/d/x.svg", "<text>핀</text>\n")),
        ),
        ([], ["한국어"], ["한국어"], ["한국어"], ["<text>핀</text>"]),
    )
    assert_equal(
        "doc comment caught, plain comment ignored",
        list(find_doc_prose("a.rs", "/// 한국어 문서\n// 한국어 메모\n")),
        ["/// 한국어 문서"],
    )
    assert_equal(
        "inline code spans are subject data",
        list(
            find_doc_prose(
                "a.rs",
                '/// `"한글".len()` is 6 bytes\n/// 한국어 서술 `code`\n',
            )
        ),
        ["/// 한국어 서술 `code`"],
    )
    with tempfile.TemporaryDirectory() as tmp:
        troot = Path(tmp)
        (troot / "pub").mkdir()
        (troot / "pub/x.rs").write_text("fn 한글() {}\n", encoding="utf-8")
        (troot / "target").mkdir()
        (troot / "target/y.rs").write_text("fn 생성() {}\n", encoding="utf-8")
        (troot / "fixtures").mkdir()
        (troot / "fixtures/z.rs").write_text("fn 픽스처() {}\n", encoding="utf-8")
        (troot / "fixtures/notes.md").write_text("한국어 prose\n", encoding="utf-8")
        (troot / "fixtures/data.json").write_text('{"input": "한글"}\n', encoding="utf-8")
        (troot / "README.ko.md").write_text("한국어\n", encoding="utf-8")
        (troot / "NOTICE").write_text("한국어 legal text\n", encoding="utf-8")
        (troot / "pub/locale.rs").write_text('const LABEL: &str = "설정";\n', encoding="utf-8")
        (troot / "pub/unicode_test.dart").write_text(
            "test('normalizes unicode', () { expect(normalize('한글'), '한글'); });\n",
            encoding="utf-8",
        )
        found = {(f["file"], f["kind"]) for f in scan_tree(troot)}
        assert_equal(
            "published tree checks fixture names/prose, preserves locale/Unicode/legal data",
            found,
            {("pub/x.rs", "identifier"), ("fixtures/z.rs", "identifier"), ("fixtures/notes.md", "doc")},
        )
    print("self-test ok")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "self-test"))
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="exported checkout root")
    args = parser.parse_args()

    if args.command == "self-test":
        return cmd_self_test()
    root = args.root.resolve()
    if not root.is_dir():
        parser.error(f"root {root} is not a directory")
    return cmd_check(root)


if __name__ == "__main__":
    sys.exit(main())
