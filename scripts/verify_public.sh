#!/usr/bin/env bash
# Shared by the public CI and the private export's candidate verification jobs.
set -euo pipefail
readonly RUST_VERSION=1.92.0
readonly FLUTTER_VERSION=3.44.0
readonly CARGO_DENY_VERSION=0.18.9
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
      # This lane runs in the exported checkout, where quality/ is at root.
      python3 quality/public_language_check.py self-test
      python3 quality/public_language_check.py check
      bash packaging/test-release-files.sh
      bash packaging/test-msix.sh
      bash release/test-release-scripts.sh
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
      # shellcheck source=/dev/null
      source packaging/test-toolkit-env.sh
      prepare_test_toolkit_env
      cargo test --workspace --locked
      just interface-inventory-check
      just toolkit-schema-check
      just toolkit-metadata-check
      just test-chrome-ext-file-input
      ;;
    wasm)
      for package in upeg-tools upeg-core upeg_frb; do
        if [ "$package" = upeg-tools ]; then
          cargo clippy --target "$WASM_TARGET" -p "$package" --features all-toolkits --locked -- -D warnings
        else
          cargo clippy --target "$WASM_TARGET" -p "$package" --locked -- -D warnings
        fi
      done
      ;;
    flutter)
      python3 scripts/flutter_i18n_check.py self-test
      python3 scripts/flutter_i18n_check.py check --baseline fixtures/flutter-i18n-baseline.json
      flutter --version | grep -F "Flutter $FLUTTER_VERSION " >/dev/null
      (cd flutter_app && flutter pub get --enforce-lockfile)
      just frb-codegen-check
      (cd flutter_app && flutter analyze && flutter test)
      just flutter-build-linux
      just flutter-build-web
      ;;
    licenses)
      if ! cargo deny --version | grep -Fx "cargo-deny $CARGO_DENY_VERSION" >/dev/null; then
        printf 'Verification requires cargo-deny %s; install with cargo install cargo-deny --locked --version %s.\n' \
          "$CARGO_DENY_VERSION" "$CARGO_DENY_VERSION" >&2
        return 1
      fi
      cargo deny --locked --all-features check advisories licenses sources
      ;;
    *) printf 'Unknown verification lane: %s\n' "$1" >&2; return 1 ;;
  esac
  # Enforce the dependency graph even in an archive or linked worktree.
  printf '%s\n' "$LOCK_HASHES" | sha256sum -c - >/dev/null

}

if [ "${1:-all}" = all ]; then
  for lane in "${DEFAULT_LANES[@]}"; do verify_lane "$lane"; done
else
  verify_lane "$1"
fi
