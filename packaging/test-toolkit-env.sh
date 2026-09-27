#!/usr/bin/env bash
# Set an isolated, verified native Toolkit fixture for Rust test processes.
# Source this file, then call `prepare_test_toolkit_env` before cargo tests.

set -euo pipefail

toolkit_test_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

prepare_test_toolkit_env() {
  local target fixture version
  target="${UPEG_TOOLKIT_TEST_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
  [ -n "$target" ] || { echo 'test-toolkit-env: rustc host target is unavailable' >&2; return 1; }
  if [ -n "${UPEG_TOOLKIT_TEST_ROOT:-}" ]; then
    fixture="$UPEG_TOOLKIT_TEST_ROOT"
  else
    fixture="$(mktemp -d "${TMPDIR:-/tmp}/upeg-toolkit-test.XXXXXX")"
  fi
  mkdir -p "$fixture"
  fixture="$(cd "$fixture" && pwd)"
  version="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$toolkit_test_root/Cargo.toml" | head -n 1)"
  [ -n "$version" ] || { echo 'test-toolkit-env: workspace version is unavailable' >&2; return 1; }

  (
    cd "$toolkit_test_root"
    cargo run --locked -p upeg-toolkit-pack -- build-native \
      --target "$target" --out "$fixture/packs" --base-url https://example.invalid
    cargo run --locked -p upeg-toolkit-pack -- verify-native \
      --root "$fixture/packs" --target "$target" --app-version "$version"
  )
  mkdir -p "$fixture/cache"
  mkdir -p "$fixture/home"
  export UPEG_TOOLKIT_LOCAL_DIR="$fixture/packs"
  export UPEG_TOOLKIT_CACHE_DIR="$fixture/cache"
  export UPEG_TOOLKIT_TEST_HOME="$fixture/home"
  export UPEG_HOME="$UPEG_TOOLKIT_TEST_HOME"
}
