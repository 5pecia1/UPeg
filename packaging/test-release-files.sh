#!/usr/bin/env bash
# Tests for packaging/release-files.sh against synthetic temp bundles.
# No real package tools or OS installs — copies text files only.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$HERE/release-files.sh"
WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT
PASS=0

# Synthetic source root: same relative paths, marker-carrying fake content.
SRC="$WORK/src"
mkdir -p "$SRC/flutter_app/fonts" "$SRC/vendor/pdf-inspector" \
  "$SRC/flutter_app/rust_builder/cargokit" "$SRC/chrome-ext"
printf 'Apache License\nVersion 2.0\n' > "$SRC/LICENSE"
printf 'UPeg\nCopyright 2026 5pecia1\n' > "$SRC/NOTICE"
printf 'SIL Open Font License\n' > "$SRC/flutter_app/fonts/OFL-D2Coding.txt"
printf 'SIL Open Font License\n' > "$SRC/flutter_app/fonts/OFL-JetBrainsMono.txt"
printf 'MIT License\nFirecrawl\n' > "$SRC/vendor/pdf-inspector/LICENSE"
printf 'MIT LICENSE\nApache LICENSE\n' > "$SRC/flutter_app/rust_builder/cargokit/LICENSE"
printf '[workspace.package]\nversion = "9.9.9"\n' > "$SRC/Cargo.toml"
printf 'version: 9.9.9+1\nmsix_config:\n  msix_version: 9.9.9.0\n' \
  > "$SRC/flutter_app/pubspec.yaml"
printf '{\n  "manifest_version": 3,\n  "version": "9.9.9"\n}\n' \
  > "$SRC/chrome-ext/manifest.json"

ok() { PASS=$((PASS + 1)); printf 'ok %d - %s\n' "$PASS" "$1"; }
fail() { printf 'FAIL - %s\n' "$1" >&2; exit 1; }

# 1. version: UPEG_VERSION wins over everything.
got="$(UPEG_SOURCE_ROOT="$SRC" UPEG_VERSION=v1.2.3 bash "$SCRIPT" version)"
[ "$got" = "v1.2.3" ] || fail "UPEG_VERSION override: got $got"
ok "version honours UPEG_VERSION"

# 2. version: no git repo and no override falls back to workspace version.
got="$(UPEG_SOURCE_ROOT="$SRC" bash -c 'cd / && PATH=/usr/bin:/bin bash "$0" version' "$SCRIPT" \
  2>/dev/null || true)"
# $SRC has no .git, so git describe fails inside; expect workspace fallback.
[ "$got" = "9.9.9" ] || fail "workspace version fallback: got '$got'"
ok "version falls back to Cargo.toml workspace version"

# 3. stage: a synthetic bundle dir gains every required notice + BUILD-INFO.
DEST="$WORK/bundle/opt/upeg"
mkdir -p "$DEST"
printf 'fake-binary' > "$DEST/upeg"
UPEG_SOURCE_ROOT="$SRC" UPEG_VERSION=v9.9.9 UPEG_COMMIT=deadbeef \
  bash "$SCRIPT" stage "$DEST" >/dev/null
for f in LICENSE NOTICE OFL-D2Coding.txt OFL-JetBrainsMono.txt \
         pdf-inspector-LICENSE cargokit-LICENSE BUILD-INFO; do
  [ -s "$DEST/$f" ] || fail "staged file missing: $f"
done
cmp -s "$SRC/LICENSE" "$DEST/LICENSE" || fail "LICENSE content drifted"
grep -q '^version=v9.9.9$' "$DEST/BUILD-INFO" || fail "BUILD-INFO version"
grep -q '^commit=deadbeef$' "$DEST/BUILD-INFO" || fail "BUILD-INFO commit"
ok "stage copies the full notice set and BUILD-INFO"

# 4. check: an assembled payload passes; existing payload files are untouched.
[ -f "$DEST/upeg" ] || fail "payload file lost"
UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" check "$DEST" >/dev/null
ok "check accepts a complete payload"

# 5. check: a missing notice fails.
rm "$DEST/NOTICE"
if UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" check "$DEST" >/dev/null 2>&1; then
  fail "check passed with NOTICE missing"
fi
ok "check refuses a payload missing NOTICE"

# 6. check: corrupted content (marker absent) fails.
printf 'garbage\n' > "$DEST/NOTICE"
if UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" check "$DEST" >/dev/null 2>&1; then
  fail "check passed with corrupted NOTICE"
fi
ok "check refuses a payload with corrupted NOTICE"

# 7. check: a notice that differs from the source text fails.
printf 'Apache License\nVersion 2.0 BUT EDITED\n' > "$DEST/LICENSE"
if UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" check "$DEST" >/dev/null 2>&1; then
  fail "check passed with edited LICENSE"
fi
ok "check refuses a payload with edited LICENSE"

# 8. stage: missing source file refuses instead of copying nothing.
rm "$SRC/vendor/pdf-inspector/LICENSE"
if UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" stage "$WORK/empty" >/dev/null 2>&1; then
  fail "stage passed with missing vendored license"
fi
ok "stage refuses when a source notice is absent"

# 9. check-version: every surface aligned on the workspace version passes.
UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" check-version 9.9.9 >/dev/null
ok "check-version accepts aligned surfaces"

# 10. check-version: a drifted extension version fails.
printf '{\n  "manifest_version": 3,\n  "version": "1.2.3"\n}\n' \
  > "$SRC/chrome-ext/manifest.json"
if UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" check-version 9.9.9 >/dev/null 2>&1; then
  fail "check-version passed with a drifted extension version"
fi
ok "check-version refuses a diverged surface version"

# 11. check-version: a leading v on the expected version is normalised.
printf '{\n  "manifest_version": 3,\n  "version": "9.9.9"\n}\n' \
  > "$SRC/chrome-ext/manifest.json"
UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" check-version v9.9.9 >/dev/null
ok "check-version normalises a v-prefixed argument"

printf '%d tests passed\n' "$PASS"
