#!/usr/bin/env bash
# Shared release staging for every upeg package format.
#
#   release-files.sh stage DEST        copy the legal notice set into DEST, write
#                                      DEST/BUILD-INFO, then validate the result
#   release-files.sh check DEST        validate an already-assembled payload dir
#   release-files.sh version           print the resolved release version
#   release-files.sh check-version [V] every versioned surface must agree with V
#                                      (default: the Cargo workspace version):
#                                      flutter pubspec (build +N allowed), the
#                                      msix four-part version (V.N allowed),
#                                      and the Chrome extension manifest.
#
# Every distributed artifact must carry the license texts it ships code or
# fonts under: the root LICENSE/NOTICE plus the full texts NOTICE names
# (OFL font licenses, vendored project licenses). `check` is also used by
# tests against synthetic bundles — it never installs a real OS package.
#
# UPEG_SOURCE_ROOT overrides the source tree root (default: parent dir of
# this script). BUILD-INFO fields come from UPEG_VERSION/UPEG_COMMIT/
# UPEG_REPO/UPEG_RUN_URL, then the GitHub Actions env, then local git.
# UPEG_PUBLIC_RELEASE=1 omits repository and run URLs from distributed metadata.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_ROOT="${UPEG_SOURCE_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"

# src-path-under-root | staged-name | required content marker
NOTICE_SET=(
  "LICENSE|LICENSE|Apache License"
  "NOTICE|NOTICE|5pecia1"
  "flutter_app/fonts/OFL-D2Coding.txt|OFL-D2Coding.txt|SIL Open Font License"
  "flutter_app/fonts/OFL-JetBrainsMono.txt|OFL-JetBrainsMono.txt|SIL Open Font License"
  "vendor/pdf-inspector/LICENSE|pdf-inspector-LICENSE|MIT License"
  "vendor/zxcvbn/LICENSE|zxcvbn-LICENSE|MIT License"
  "flutter_app/rust_builder/cargokit/LICENSE|cargokit-LICENSE|MIT LICENSE"
)

die() { printf 'release-files: %s\n' "$*" >&2; exit 1; }

workspace_version() {
  sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' \
    "$SOURCE_ROOT/Cargo.toml" | head -n 1
}

source_has_own_git() {
  local top
  command -v git >/dev/null 2>&1 || return 1
  top="$(git -C "$SOURCE_ROOT" rev-parse --show-toplevel 2>/dev/null)" || return 1
  [ "$(cd "$top" && pwd -P)" = "$(cd "$SOURCE_ROOT" && pwd -P)" ]
}

resolve_version() {
  if [ -n "${UPEG_VERSION:-}" ]; then
    printf '%s\n' "$UPEG_VERSION"
  elif source_has_own_git; then
    git -C "$SOURCE_ROOT" describe --tags --always --abbrev=8
  else
    ws="$(workspace_version)"
    printf '%s\n' "${ws:-0.0.0+unknown}"
  fi
}

write_build_info() {
  local dest="$1" version commit repo run_url
  version="$(resolve_version)"
  commit="${UPEG_COMMIT:-${GITHUB_SHA:-}}"
  if [ -z "$commit" ] && source_has_own_git; then
    commit="$(git -C "$SOURCE_ROOT" rev-parse HEAD)"
  fi
  if [ "${UPEG_PUBLIC_RELEASE:-0}" = 1 ]; then
    repo="release-source"
    run_url="unpublished"
  else
    repo="${UPEG_REPO:-${GITHUB_REPOSITORY:-unknown}}"
    if [ -n "${GITHUB_RUN_ID:-}" ]; then
      run_url="${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"
    else
      run_url="${UPEG_RUN_URL:-}"
    fi
  fi
  {
    printf 'version=%s\n' "$version"
    printf 'commit=%s\n' "${commit:-unknown}"
    printf 'source=%s\n' "$repo"
    printf 'workflow_run=%s\n' "${run_url:-local}"
    printf 'built_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  } > "$dest/BUILD-INFO"
}

stage() {
  local dest="$1" entry src dst
  [ -n "$dest" ] || die "stage needs a destination directory"
  mkdir -p "$dest"
  for entry in "${NOTICE_SET[@]}"; do
    src="${entry%%|*}"; rest="${entry#*|}"; dst="${rest%%|*}"
    [ -s "$SOURCE_ROOT/$src" ] || die "missing source notice: $src"
    cp "$SOURCE_ROOT/$src" "$dest/$dst"
  done
  write_build_info "$dest"
  check "$dest"
}

check() {
  local dest="$1" entry dst marker src failures=0
  [ -d "$dest" ] || die "check needs an existing directory: $dest"
  for entry in "${NOTICE_SET[@]}"; do
    src="${entry%%|*}"; rest="${entry#*|}"; dst="${rest%%|*}"; marker="${rest#*|}"
    if [ ! -s "$dest/$dst" ]; then
      printf '  fail: missing %s\n' "$dst" >&2; failures=$((failures + 1)); continue
    fi
    if ! grep -qiF "$marker" "$dest/$dst"; then
      printf '  fail: %s lacks marker %s\n' "$dst" "$marker" >&2; failures=$((failures + 1)); continue
    fi
    if [ -f "$SOURCE_ROOT/$src" ] && ! cmp -s "$SOURCE_ROOT/$src" "$dest/$dst"; then
      printf '  fail: %s differs from source %s\n' "$dst" "$src" >&2; failures=$((failures + 1))
    fi
  done
  if [ -f "$dest/BUILD-INFO" ]; then
    if ! grep -q '^version=.' "$dest/BUILD-INFO" \
        || ! grep -q '^commit=.' "$dest/BUILD-INFO"; then
      printf '  fail: BUILD-INFO lacks version=/commit=\n' >&2
      failures=$((failures + 1))
    fi
  fi
  [ "$failures" -eq 0 ] || die "$failures staged-file check(s) failed in $dest"
  printf '  ok: %s notice files staged in %s\n' "${#NOTICE_SET[@]}" "$dest"
}

surface_versions() {
  # name<TAB>version for every independently versioned release surface.
  local pub msix ext
  pub="$(sed -n 's/^version:[[:space:]]*\([^[:space:]]*\).*/\1/p' \
    "$SOURCE_ROOT/flutter_app/pubspec.yaml" | head -n 1)"
  msix="$(sed -n 's/^  msix_version:[[:space:]]*\([^[:space:]]*\).*/\1/p' \
    "$SOURCE_ROOT/flutter_app/pubspec.yaml" | head -n 1)"
  ext="$(sed -n 's/^  "version":[[:space:]]*"\([^"]*\)".*/\1/p' \
    "$SOURCE_ROOT/chrome-ext/manifest.json" | head -n 1)"
  printf 'cargo-workspace\t%s\n' "$(workspace_version)"
  printf 'flutter-pubspec\t%s\n' "${pub%%+*}"
  printf 'flutter-msix\t%s\n' "$msix"
  printf 'chrome-extension\t%s\n' "$ext"
}

check_version() {
  local expected="${1#v}" name value failures=0
  [ -n "$expected" ] || expected="$(workspace_version)"
  [ -n "$expected" ] || die "cannot resolve the workspace version"
  while IFS="$(printf '\t')" read -r name value; do
    case "$name:$value" in
      flutter-msix:"$expected".*) ;;
      *:"$expected") ;;
      *) printf '  fail: %s is %s, want %s\n' "$name" "${value:-missing}" "$expected" >&2
         failures=$((failures + 1)) ;;
    esac
    printf '  %s = %s\n' "$name" "${value:-missing}"
  done < <(surface_versions)
  [ "$failures" -eq 0 ] || die "$failures surface version(s) diverge from $expected"
  printf '  ok: all surface versions agree on %s\n' "$expected"
}

case "${1:-}" in
  stage) stage "${2:-}" ;;
  check) check "${2:-}" ;;
  version) resolve_version ;;
  check-version) check_version "${2:-}" ;;
  *) die "usage: $0 {stage|check DESTDIR|version|check-version [VERSION]}" ;;
esac
