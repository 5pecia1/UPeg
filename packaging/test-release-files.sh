#!/usr/bin/env bash
# Tests for packaging/release-files.sh against synthetic temp bundles.
# No real package tools or OS installs — copies text files only.
set -euo pipefail
# Build metadata in the runner must not alter synthetic source expectations.
unset UPEG_VERSION UPEG_COMMIT UPEG_REPO UPEG_RUN_URL GITHUB_SHA \
  GITHUB_REPOSITORY GITHUB_RUN_ID GITHUB_SERVER_URL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$HERE/release-files.sh"
WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT
PASS=0

# Synthetic source root: same relative paths, marker-carrying fake content.
SRC="$WORK/src"
mkdir -p "$SRC/flutter_app/fonts" "$SRC/vendor/pdf-inspector" "$SRC/vendor/zxcvbn" \
  "$SRC/flutter_app/rust_builder/cargokit" "$SRC/chrome-ext"
printf 'Apache License\nVersion 2.0\n' > "$SRC/LICENSE"
printf 'UPeg\nCopyright 2026 5pecia1\n' > "$SRC/NOTICE"
printf 'SIL Open Font License\n' > "$SRC/flutter_app/fonts/OFL-D2Coding.txt"
printf 'SIL Open Font License\n' > "$SRC/flutter_app/fonts/OFL-JetBrainsMono.txt"
printf 'MIT License\nFirecrawl\n' > "$SRC/vendor/pdf-inspector/LICENSE"
printf 'MIT License\nJoshua Holmer\n' > "$SRC/vendor/zxcvbn/LICENSE"
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

# A copied source archive can live below an unrelated tagged repository.
git -C "$WORK" init -q
git -C "$WORK" -c user.name=Test -c user.email=test@example.invalid \
  commit -q --allow-empty -m unrelated
git -C "$WORK" tag unrelated-v7
got="$(UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" version)"
[ "$got" = "9.9.9" ] || fail "nested archive inherited unrelated git tag: $got"
UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" stage "$WORK/nested-bundle" >/dev/null
grep -q '^commit=unknown$' "$WORK/nested-bundle/BUILD-INFO" || fail "nested archive inherited unrelated git commit"
ok "nested archive ignores unrelated parent git metadata"

git -C "$SRC" init -q
git -C "$SRC" -c user.name=Test -c user.email=test@example.invalid \
  commit -q --allow-empty -m source
git -C "$SRC" tag v9.9.9
got="$(UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" version)"
[ "$got" = "v9.9.9" ] || fail "source root git version: $got"
ln -s "$SRC" "$WORK/src-link"
got="$(UPEG_SOURCE_ROOT="$WORK/src-link" bash "$SCRIPT" version)"
[ "$got" = "v9.9.9" ] || fail "symlinked source root git version: $got"
UPEG_SOURCE_ROOT="$WORK/src-link" bash "$SCRIPT" stage "$WORK/git-bundle" >/dev/null
expected_commit="$(git -C "$SRC" rev-parse HEAD)"
grep -q "^commit=$expected_commit$" "$WORK/git-bundle/BUILD-INFO" || fail "symlinked root commit"
ok "git metadata is accepted for source roots and symlinked roots"

git -C "$SRC" worktree add -q --detach "$WORK/source-worktree" HEAD
got="$(UPEG_SOURCE_ROOT="$WORK/source-worktree" bash "$SCRIPT" version)"
[ "$got" = "v9.9.9" ] || fail "worktree git version: $got"
ok "git metadata is accepted for an independent worktree"

UPEG_SOURCE_ROOT="$SRC" UPEG_VERSION=override UPEG_COMMIT=chosen \
  bash "$SCRIPT" stage "$WORK/override-bundle" >/dev/null
grep -q '^version=override$' "$WORK/override-bundle/BUILD-INFO" || fail "version override precedence"
grep -q '^commit=chosen$' "$WORK/override-bundle/BUILD-INFO" || fail "commit override precedence"
ok "explicit metadata overrides git"

GITHUB_SHA=ci-commit UPEG_SOURCE_ROOT="$SRC" bash "$SCRIPT" stage "$WORK/ci-bundle" >/dev/null
grep -q '^commit=ci-commit$' "$WORK/ci-bundle/BUILD-INFO" || fail "CI commit override precedence"
ok "CI commit override wins over local git"

GITHUB_RUN_ID=123 GITHUB_REPOSITORY=private/repo GITHUB_SERVER_URL=https://github.example \
  UPEG_SOURCE_ROOT="$SRC" UPEG_PUBLIC_RELEASE=1 UPEG_COMMIT=honest-commit \
  bash "$SCRIPT" stage "$WORK/public-bundle" >/dev/null
grep -q '^commit=honest-commit$' "$WORK/public-bundle/BUILD-INFO" || fail "public metadata commit"
grep -q '^source=release-source$' "$WORK/public-bundle/BUILD-INFO" || fail "public metadata source"
grep -q '^workflow_run=unpublished$' "$WORK/public-bundle/BUILD-INFO" || fail "public metadata workflow"
if grep -qE 'private/repo|github\.example|123' "$WORK/public-bundle/BUILD-INFO"; then
  fail "public metadata leaked CI provenance"
fi
ok "public metadata omits CI repository and run URL"

# 3. stage: a synthetic bundle dir gains every required notice + BUILD-INFO.
DEST="$WORK/bundle/opt/upeg"
mkdir -p "$DEST"
printf 'fake-binary' > "$DEST/upeg"
UPEG_SOURCE_ROOT="$SRC" UPEG_VERSION=v9.9.9 UPEG_COMMIT=deadbeef \
  bash "$SCRIPT" stage "$DEST" >/dev/null
for f in LICENSE NOTICE OFL-D2Coding.txt OFL-JetBrainsMono.txt \
         pdf-inspector-LICENSE zxcvbn-LICENSE cargokit-LICENSE BUILD-INFO; do
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
