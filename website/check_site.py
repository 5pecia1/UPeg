#!/usr/bin/env python3
"""Offline link/anchor check for the generated UPeg documentation site.

Walks every .html file under SITE_DIR, resolves each internal href/src
(including <meta http-equiv="refresh"> targets emitted by the redirects
plugin) against the deploy base path, and fails if a target file or
fragment id is missing. External links (https:, mailto:, ...) are counted
but not fetched — the check is hermetic and safe to run anywhere.

Usage:
    python3 website/check_site.py site --base-path /UPeg/ \
        [--site-url https://5pecia1.github.io/UPeg/]

Exit code is 0 when no broken internal link or anchor is found, else 1.
"""

from __future__ import annotations

import argparse
import os
import posixpath
import sys
from html.parser import HTMLParser
from urllib.parse import unquote, urlparse

# Attributes that can reference another resource.
REF_ATTRS = {
    "a": ("href",),
    "area": ("href",),
    "link": ("href",),
    "script": ("src",),
    "img": ("src",),
    "iframe": ("src",),
    "embed": ("src",),
    "audio": ("src",),
    "video": ("src", "poster"),
    "source": ("src",),
    "track": ("src",),
}

# Schemes that never map to a file in the site tree.
SKIP_SCHEMES = ("mailto:", "tel:", "data:", "javascript:", "chrome-extension:")


class PageScan(HTMLParser):
    """Collect outbound references and anchor ids of one HTML document."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.refs: list[tuple[int, str]] = []
        self.ids: set[str] = set()

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        line = self.getpos()[0]
        attr = dict(attrs)
        element_id = attr.get("id")
        if element_id:
            self.ids.add(element_id)
        if tag == "a" and attr.get("name"):
            self.ids.add(attr["name"] or "")
        for key in REF_ATTRS.get(tag, ()):
            value = attr.get(key)
            if value:
                self.refs.append((line, value))
        if tag == "meta" and (attr.get("http-equiv") or "").lower() == "refresh":
            content = attr.get("content") or ""
            marker = content.lower().find("url=")
            if marker >= 0:
                self.refs.append((line, content[marker + 4:].strip()))

    handle_startendtag = handle_starttag


def normalize_base(base_path: str) -> str:
    base = "/" + base_path.strip("/")
    return "/" if base == "/" else base + "/"


def resolve_target(page_rel: str, raw: str, base: str, site_url: str | None):
    """Map a reference to a site-relative path, or classify it.

    Returns (kind, rel_path, fragment) where kind is
    'internal' | 'external' | 'skip' | 'broken'.
    """
    ref = unquote(raw.strip())
    if not ref:
        return "skip", "", ""
    if ref.startswith("#"):
        return "internal", page_rel, ref[1:]
    lowered = ref.lower()
    if lowered.startswith(SKIP_SCHEMES):
        return "skip", "", ""
    if lowered.startswith("//"):
        return "external", "", ""
    parsed = urlparse(ref)
    from_site_root = False
    if parsed.scheme:
        if site_url and ref.startswith(site_url):
            ref = ref[len(site_url):]
            from_site_root = True
        else:
            return "external", "", ""
    path, _, fragment = ref.partition("#")
    path = path.split("?", 1)[0]
    if from_site_root:
        # site_url already carries the base path; the remainder resolves
        # against the site root directly.
        return "internal", path, fragment
    if path.startswith("/"):
        if base != "/" and path == base[:-1]:
            # "/UPeg" without the trailing slash redirects to the site root.
            return "internal", "index.html", fragment
        if base != "/" and not path.startswith(base):
            return "broken", path, fragment
        path = path[len(base):] if base != "/" else path[1:]
        if path == "":
            return "internal", "index.html", fragment
    else:
        page_dir = posixpath.dirname(page_rel)
        path = posixpath.normpath(posixpath.join(page_dir, path))
        if path.startswith(".."):
            return "broken", path, fragment
    return "internal", path, fragment


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("site_dir", help="generated site directory")
    parser.add_argument("--base-path", default="/",
                        help="deploy base path, e.g. /UPeg/ (default /)")
    parser.add_argument("--site-url", default=None,
                        help="absolute site URL; same-origin links are "
                             "validated as internal")
    args = parser.parse_args()

    site_dir = os.path.abspath(args.site_dir)
    base = normalize_base(args.base_path)
    site_url = args.site_url.rstrip("/") + "/" if args.site_url else None

    if not os.path.isdir(site_dir):
        print(f"error: not a directory: {site_dir}", file=sys.stderr)
        return 1

    pages: dict[str, PageScan] = {}

    def scan(rel: str) -> PageScan:
        if rel not in pages:
            result = PageScan()
            with open(os.path.join(site_dir, rel), encoding="utf-8") as handle:
                result.feed(handle.read())
            pages[rel] = result
        return pages[rel]

    errors: list[str] = []
    checked = external = 0

    for root, _dirs, files in os.walk(site_dir):
        for name in files:
            if not name.endswith(".html"):
                continue
            path = os.path.join(root, name)
            rel = os.path.relpath(path, site_dir).replace(os.sep, "/")
            for line, raw in scan(rel).refs:
                kind, target, fragment = resolve_target(rel, raw, base, site_url)
                if kind == "skip":
                    continue
                if kind == "external":
                    external += 1
                    continue
                if kind == "broken":
                    errors.append(
                        f"{rel}:{line}: '{raw}' escapes the {base} base path")
                    continue
                checked += 1
                if target == "" or target.endswith("/"):
                    target += "index.html"
                elif not os.path.splitext(target)[1]:
                    target += "/index.html"
                full = os.path.join(site_dir, target)
                if not os.path.isfile(full):
                    errors.append(
                        f"{rel}:{line}: '{raw}' -> missing '{target}'")
                    continue
                if fragment and target.endswith(".html"):
                    target_rel = os.path.relpath(full, site_dir).replace(os.sep, "/")
                    if fragment not in scan(target_rel).ids:
                        errors.append(
                            f"{rel}:{line}: '{raw}' -> no '#{fragment}' in "
                            f"'{target}'")

    for error in errors:
        print(error)
    print(f"checked {checked} internal refs, {external} external skipped, "
          f"{len(errors)} broken")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
