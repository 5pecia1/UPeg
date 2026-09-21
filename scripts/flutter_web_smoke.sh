#!/usr/bin/env bash
set -euo pipefail

# Boot + service worker cache contract smoke for the Flutter Web (PWA) bundle.
#
# This script checks prerequisites only — that the build output exists, the
# required executables are installed, and which Chromium to use. The actual
# contract assertions live in `scripts/flutter_web_smoke_check.mjs`, which
# drives the browser over CDP. Because the offline-revisit phase must stop
# the static server *for real*, the server and browser lifetimes are owned
# in that one place.
#
# Missing Chromium is a hard failure. Silently skipping when no browser is
# present would let CI report green while the whole PWA verification is gone
# (see the `ci-smoke` comment in the Justfile).

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
build_dir="$repo_root/flutter_app/build/web"
start_port="${UPEG_FLUTTER_WEB_SMOKE_PORT:-8097}"
timeout_seconds="${UPEG_FLUTTER_WEB_SMOKE_TIMEOUT:-60}"
out_dir="$repo_root/target/flutter-web-smoke"
server_log="$out_dir/server.log"
chrome_log="$out_dir/chromium.log"
profile_dir="$(mktemp -d)"

cleanup() {
  status=$?
  if [ "$status" -ne 0 ] && [ -f "$server_log" ]; then
    echo "flutter web smoke failed; server log:" >&2
    tail -200 "$server_log" >&2 || true
  fi
  if [ "$status" -ne 0 ] && [ -f "$chrome_log" ]; then
    echo "flutter web smoke failed; Chromium log:" >&2
    tail -200 "$chrome_log" >&2 || true
  fi
  rm -rf "$profile_dir"
  exit "$status"
}
trap cleanup EXIT

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: required command '$1' is not installed" >&2
    exit 1
  fi
}

choose_chrome() {
  if [ -n "${CHROME_EXECUTABLE:-}" ]; then
    printf '%s\n' "$CHROME_EXECUTABLE"
    return 0
  fi
  for candidate in chromium google-chrome chrome; do
    if command -v "$candidate" >/dev/null 2>&1; then
      command -v "$candidate"
      return 0
    fi
  done
  echo "error: no Chromium/Chrome executable found" >&2
  exit 1
}

if [ ! -f "$build_dir/index.html" ]; then
  echo "error: missing $build_dir/index.html; run 'just flutter-build-web' first" >&2
  exit 1
fi

# python3: static server. node: CDP runner (builtins only, no npm deps).
require_command python3
require_command node
chrome="$(choose_chrome)"

mkdir -p "$out_dir"
rm -f "$server_log" "$chrome_log"

UPEG_WEB_SMOKE_BUILD_DIR="$build_dir" \
UPEG_WEB_SMOKE_OUT_DIR="$out_dir" \
UPEG_WEB_SMOKE_CHROME="$chrome" \
UPEG_WEB_SMOKE_PROFILE_DIR="$profile_dir" \
UPEG_WEB_SMOKE_PORT="$start_port" \
UPEG_WEB_SMOKE_TIMEOUT_SECONDS="$timeout_seconds" \
  node "$repo_root/scripts/flutter_web_smoke_check.mjs"
