#!/usr/bin/env bash
# Shared by the public CI and the private export's candidate verification jobs.
set -euo pipefail
readonly RUST_VERSION=1.92.0
readonly FLUTTER_VERSION=3.44.0
readonly CARGO_DENY_VERSION=0.18.2
readonly WASM_TARGET=wasm32-unknown-unknown
readonly DEFAULT_LANES=(rust wasm flutter licenses)
export UPEG_PROJECT_MANIFEST_PATH=off
# Existing tests share runtime state and Linux process-scope cleanup. Keep
# independent tests serial; explicit concurrency tests still exercise threads.
export RUST_TEST_THREADS=1
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export CARGO_TERM_COLOR=always
LOCK_HASHES="$(sha256sum Cargo.lock flutter_app/pubspec.lock)"
readonly LOCK_HASHES
# chromiumoxide otherwise shares one process-global profile under /tmp.
UPEG_OSS_TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/upeg-oss-verify.XXXXXX")"
readonly UPEG_OSS_TMP_ROOT
trap 'rm -rf -- "$UPEG_OSS_TMP_ROOT"' EXIT
export TMPDIR="$UPEG_OSS_TMP_ROOT"
export UPEG_HOME="$UPEG_OSS_TMP_ROOT/state"
export UPEG_TOOLKITS_DIR="$UPEG_HOME/toolkits"
export UPEG_WASM_DIR="$UPEG_HOME/wasm"
export UPEG_MCP_IMPORTS_DIR="$UPEG_HOME/mcp-imports"
unset UPEG_LOG_PATH UPEG_CREDENTIALS_PATH GH_TOKEN GITHUB_TOKEN

verify_lane() {
  case "$1" in
    rust)
      browser="${UPEG_BROWSER_PATH:-${CHROME_EXECUTABLE:-}}"
      if [ -z "$browser" ]; then
        browser="$(command -v google-chrome || command -v chromium || true)"
      fi
      if [ -z "$browser" ] || [ ! -x "$browser" ]; then
        printf 'Rust E2E requires Chrome/Chromium; set UPEG_BROWSER_PATH.\n' >&2
        return 1
      fi
      export UPEG_BROWSER_PATH="$browser"
      rustc --version | grep -F "rustc $RUST_VERSION " >/dev/null
      cargo metadata --locked --format-version 1 >/dev/null
      cargo fmt --all -- --check
      cargo build --workspace --locked
      cargo clippy --workspace --all-targets --locked -- -D warnings
      cargo test --workspace --locked
      ;;
    wasm)
      for package in upeg-tools upeg-core upeg_frb; do
        cargo clippy --target "$WASM_TARGET" -p "$package" --locked -- -D warnings
      done
      ;;
    flutter)
      flutter --version | grep -F "Flutter $FLUTTER_VERSION " >/dev/null
      (cd flutter_app && flutter pub get --enforce-lockfile && flutter analyze && flutter test)
      just flutter-build-linux
      just flutter-build-web
      ;;
    licenses)
      cargo deny --version | grep -F "cargo-deny $CARGO_DENY_VERSION" >/dev/null
      cargo deny --locked --all-features check licenses sources
      ;;
    *) printf 'Unknown verification lane: %s\n' "$1" >&2; return 1 ;;
  esac
  # Enforce the dependency graph even in an archive or linked worktree.
  printf '%s\n' "$LOCK_HASHES" | sha256sum --check --status

}

if [ "${1:-all}" = all ]; then
  for lane in "${DEFAULT_LANES[@]}"; do verify_lane "$lane"; done
else
  verify_lane "$1"
fi
