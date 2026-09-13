#!/usr/bin/env bash
set -euo pipefail

# Flutter Web(PWA) 번들의 부팅 + service worker 캐시 계약 스모크.
#
# 이 스크립트는 전제 조건만 본다 — 빌드 산출물이 있는지, 필요한 실행 파일이
# 있는지, 어떤 Chromium을 쓸지. 실제 계약 단언은
# `scripts/flutter_web_smoke_check.mjs`가 CDP로 브라우저를 몰면서 한다.
# 오프라인 재방문 단계에서 정적 서버를 **실제로 내려야** 하므로 서버와
# 브라우저의 수명은 그쪽 한 곳에 모여 있다.
#
# Chromium이 없으면 하드 실패한다. 브라우저가 없다고 조용히 건너뛰면 CI에서
# PWA 검증이 통째로 사라져도 초록으로 보인다 (Justfile의 `ci-smoke` 주석 참고).

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

# python3: 정적 서버. node: CDP 러너 (npm 의존성 없이 builtin만 쓴다).
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
