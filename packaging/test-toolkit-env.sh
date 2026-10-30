#!/usr/bin/env bash
# Source inside test-toolkit-run.sh, then prepare a fresh test state.
set -euo pipefail

toolkit_test_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

prepare_test_toolkit_env() {
  local target fixture version state fingerprint
  : "${UPEG_TOOLKIT_RUN_ROOT:?run tests through packaging/test-toolkit-run.sh}"
  if [ ! -f "$UPEG_TOOLKIT_RUN_ROOT/owner" ] ||
    [ "$(sed -n '1p' "$UPEG_TOOLKIT_RUN_ROOT/owner")" != "${UPEG_TOOLKIT_RUN_OWNER:-}" ] ||
    [ "$(sed -n '2p' "$UPEG_TOOLKIT_RUN_ROOT/owner")" != "${UPEG_TOOLKIT_RUN_TOKEN:-}" ] ||
    ! kill -0 "${UPEG_TOOLKIT_RUN_OWNER:-0}" 2>/dev/null; then
    echo 'test-toolkit-env: fixture owner is no longer active' >&2
    return 1
  fi
  fingerprint="$(python3 "$toolkit_test_root/packaging/test-toolkit-fingerprint.py")"
  target="${UPEG_TOOLKIT_TEST_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
  [ -n "$target" ] || { echo 'test-toolkit-env: rustc host target is unavailable' >&2; return 1; }
  fixture="${UPEG_TOOLKIT_TEST_ROOT:-$UPEG_TOOLKIT_RUN_ROOT}"
  mkdir -p "$fixture"
  fixture="$(cd "$fixture" && pwd)"
  version="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$toolkit_test_root/Cargo.toml" | head -n 1)"
  [ -n "$version" ] || { echo 'test-toolkit-env: workspace version is unavailable' >&2; return 1; }
  if [ ! -f "$UPEG_TOOLKIT_RUN_ROOT/prepared" ]; then
    (
      cd "$toolkit_test_root"
      cargo run --locked -p upeg-toolkit-pack -- build-native \
        --target "$target" --out "$fixture/packs" --base-url https://example.invalid
      cargo run --locked -p upeg-toolkit-pack -- verify-native \
        --root "$fixture/packs" --target "$target" --app-version "$version"
    )
    [ "$(python3 "$toolkit_test_root/packaging/test-toolkit-fingerprint.py")" = "$fingerprint" ] || {
      echo 'test-toolkit-env: fixture inputs changed during build' >&2; return 1;
    }
    printf '%s\n%s\n%s\n' "$fixture" "$target" "$fingerprint" > "$UPEG_TOOLKIT_RUN_ROOT/prepared"
  else
    if [ "$(sed -n '1p' "$UPEG_TOOLKIT_RUN_ROOT/prepared")" != "$fixture" ] ||
       [ "$(sed -n '2p' "$UPEG_TOOLKIT_RUN_ROOT/prepared")" != "$target" ] ||
       [ "$(sed -n '3p' "$UPEG_TOOLKIT_RUN_ROOT/prepared")" != "$fingerprint" ]; then
      echo 'test-toolkit-env: fixture inputs changed during run' >&2
      return 1
    fi
  fi
  state="$(mktemp -d "$UPEG_TOOLKIT_RUN_ROOT/state.XXXXXX")"
  mkdir -p "$state/cache" "$state/home"
  export UPEG_TOOLKIT_LOCAL_DIR="$fixture/packs"
  export UPEG_TOOLKIT_CACHE_DIR="$state/cache"
  export UPEG_TOOLKIT_TEST_HOME="$state/home"
  export UPEG_HOME="$UPEG_TOOLKIT_TEST_HOME"
}
