set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# ─── Two-tier local loop ────────────────────────────────────────────
#
#   just check    pre-commit tier — fast, read-only, Rust-only.
#   just verify   full verification closure (NFR-06 canonical entry point).
#   just default  `fix` (mutates the tree) then `verify`.
#
# `verify` is the exact union of what .github/workflows/ci.yml runs, and
# CI calls the very same `ci-*` lane recipes — so "green locally" and
# "green in CI" cannot drift apart. `fix` is never a `verify` dependency:
# verify is read-only by contract.
#
# Auto-fix the tree, then run the full verification closure.
default: fix verify
	@printf '\n\033[1;36m[done]\033[0m ✓ all\n'

all: default

# Apply rustc/clippy suggestions, then rustfmt.
fix:
	@printf '\n\033[1;36m[fix 1/3]\033[0m cargo fix (rustc compiler suggestions)\n'
	cargo fix --workspace --all-targets --allow-dirty --allow-staged
	@printf '\n\033[1;36m[fix 2/3]\033[0m cargo clippy --fix (lint suggestions)\n'
	cargo clippy --workspace --all-targets --fix --allow-dirty --allow-staged
	@printf '\n\033[1;36m[fix 3/3]\033[0m cargo fmt --all\n'
	cargo fmt --all
	@printf '\033[1;32m  ok\033[0m\n'

# ─── Pre-commit tier ────────────────────────────────────────────────
# Read-only, Rust-only, no Flutter, no drift fixtures, unit tests only —
# the cheapest gate that still catches what a skipped local loop actually
# leaks into CI (formatting, lints, a broken unit test). Target: ~2–3
# minutes on a warm target dir. The full closure is `just verify`.
#
# Pre-commit gate: fast, read-only, Rust-only. Run before every commit.
check: fmt-check clippy-native file-size-budget lexicon-check test-unit
	@printf '\033[1;32m  ok (check)\033[0m\n'

# ─── Full verification closure ──────────────────────────────────────
# The union of the four CI lanes and nothing else. Every lane below is
# called by .github/workflows/ci.yml verbatim, so this recipe list is the
# single source of truth for "what CI checks".
#
# Deliberately NOT in the closure, because they would run the same tests
# a second time:
#   test-workspace / test-docs / flutter-test — `test-baseline-check`
#     already runs all three suites exactly once via
#     scripts/test_baseline.py; they stay as standalone dev recipes.
#   ui-parity-check — the golden regression suite is part of the
#     baseline's `flutter` suite; the recipe stays for local
#     `--update-goldens` work.
#
# Full verification closure: the four CI lanes. The canonical entry point.
verify:
	bash scripts/verify_public.sh all
	@printf '\033[1;32m  ok (verify)\033[0m\n'

# No Rust compilation, no test run; everything here fails in seconds.
# `require-wasm-target` rides along so a missing rustup target fails in
# seconds instead of ~15 minutes later, when `ci-wasm` finally needs it
# (just runs a dependency at most once per invocation, so `ci-wasm`
# keeping it too costs nothing and stays correct when run standalone).
#
# CI lane 1/4 — cheap repository gates (format, lint scripts, budgets).
ci-preflight: require-wasm-target fmt-check flutter-fmt-check actionlint license-check file-size-budget lexicon-check flutter-theme-token-check flutter-i18n-check test-baseline-self-test
	@printf '\033[1;32m  ok (ci-preflight)\033[0m\n'

# The whole test corpus runs exactly once here, inside
# `test-baseline-check` (cargo workspace + cargo doc + flutter), next to
# the generated-artifact drift gates and native clippy.
#
# CI lane 2/4 — the whole test corpus (once) + drift gates + native clippy.
ci-core: test-baseline-check interface-inventory-check toolkit-schema-check frb-codegen-check test-chrome-ext-file-input test-controlled-embed flutter-analyze clippy-native
	@printf '\033[1;32m  ok (ci-core)\033[0m\n'

# CI lane 3/4 — wasm32 clippy for every crate that ships into the PWA.
ci-wasm: require-wasm-target clippy-tools-wasm clippy-core-wasm clippy-frb-wasm
	@printf '\033[1;32m  ok (ci-wasm)\033[0m\n'

# `flutter-web-smoke` runs UNCONDITIONALLY here. It is the ONLY PWA
# build+boot check in the required Linux lane, and a probe that skipped
# it when no browser binary was found could make that check vanish from
# CI silently — a missing Chromium would read as a green lane. The lane
# runs inside the devcontainer, which installs `chromium` and exports
# `CHROME_EXECUTABLE` (.devcontainer/Dockerfile), so the binary is
# always present; if it ever isn't, scripts/flutter_web_smoke.sh errors
# and the lane goes red, which is the point. Hosted macOS/Windows
# runners never reach here — they run `just smoke`
# (.github/workflows/ci.yml, secondary-hosted-platforms).
#
# The only escape hatch is explicit: `UPEG_SKIP_WEB_SMOKE=1 just
# ci-smoke` for a local machine that genuinely can't build web. Absence
# of a binary is not an escape hatch.
#
# CI lane 4/4 — release build, CLI smoke, and the headless web boot check.
ci-smoke:
	#!/usr/bin/env bash
	set -euo pipefail
	just smoke
	just mcp-import-real-smoke
	if [ "${UPEG_SKIP_WEB_SMOKE:-0}" = "1" ]; then
	  printf '\033[1;33m  skip\033[0m flutter-web-smoke (UPEG_SKIP_WEB_SMOKE=1)\n'
	else
	  just flutter-web-smoke
	fi
	printf '\033[1;32m  ok (ci-smoke)\033[0m\n'

# MCP import against a REAL official-SDK server
# (`@modelcontextprotocol/server-filesystem` via `npx`), not one of the
# suite's fake stdio servers. E-5 shipped a handshake both ends of an
# all-upeg test agreed on and the official SDK rejected, so this lane
# exists to keep one non-upeg implementation in the loop.
#
# `#[ignore]` by default — a plain `cargo test` must not need a Node
# toolchain or the npm registry. On a DEV machine the three tests SKIP
# (printing why) when `npx` is missing or the package cannot be
# fetched, so an offline laptop is not a red lane.
#
# In CI that leniency is the bug: a runner that quietly lost its Node
# install would report three green tests that never ran, and this lane
# exists precisely to keep one non-upeg MCP implementation in the loop.
# So when `CI` is set, `UPEG_REQUIRE_REAL_MCP=1` turns every
# not-ready reason into a failure with that reason attached.
#
# Runtime: the first run on a cold `npx` cache pays a one-time package
# download (tens of seconds, network-bound), shared by all three tests
# through a process-wide pre-warm; afterwards they are a couple of
# seconds together. CI's per-job cache volumes cover cargo only, so the
# runner re-downloads the package once per job.
#
# MCP import smoke against the official SDK filesystem server (skips without npx; required in CI).
mcp-import-real-smoke:
	#!/usr/bin/env bash
	set -euo pipefail
	printf '\n\033[1;36m[smoke]\033[0m cargo test -p upeg-cli --test mcp_import_real_server -- --ignored\n'
	if [ -n "${CI:-}" ]; then
	  export UPEG_REQUIRE_REAL_MCP=1
	  printf '\033[1;33m  CI\033[0m UPEG_REQUIRE_REAL_MCP=1 — a skip is a failure here\n'
	fi
	cargo test -p upeg-cli --test mcp_import_real_server -- --ignored --nocapture

lexicon-check:
	@printf '\n\033[1;36m[verify+]\033[0m scripts/lexicon_check.sh\n'
	scripts/lexicon_check.sh

actionlint:
	@printf '\n\033[1;36m[verify]\033[0m actionlint -color -shellcheck= -pyflakes= .github/workflows/*.yml\n'
	actionlint -color -shellcheck= -pyflakes= .github/workflows/*.yml

# Advisory, licence and source policy gate. `deny.toml` at the repository
# root defines the policy; NOTICE carries the required attribution. No
# compilation: the shared public lane checks the pinned cargo-deny version,
# runs against the locked graph, and rejects lockfile drift.
license-check:
	@printf '\n\033[1;36m[verify]\033[0m cargo deny (advisories, licenses, sources)\n'
	bash scripts/verify_public.sh licenses

# Release build plus CLI smoke assertions.
smoke:
	@printf '\n\033[1;36m[smoke 1/2]\033[0m cargo build --release -p upeg-cli --bin upeg\n'
	cargo build --release -p upeg-cli --bin upeg
	@printf '\n\033[1;36m[smoke 2/2]\033[0m CLI smoke (tool list / hex-to-decimal / uuid-v7)\n'
	bin="./target/release/upeg"; [ -x "$bin" ] || bin="./target/release/upeg.exe"; just smoke-bin "$bin"
	@printf '\033[1;32m  ok\033[0m\n'

# Run CLI smoke assertions against an already-built binary.
smoke-bin bin:
	{{bin}} tool list | grep -q 'num.hex_to_decimal' || { echo "  fail: 'num.hex_to_decimal' not in tool list" >&2; exit 1; }
	{{bin}} tool list | grep -q 'media.image_to_pdf' || { echo "  fail: 'media.image_to_pdf' not in tool list" >&2; exit 1; }
	{{bin}} tool list | grep -q 'media.pdf_to_images' || { echo "  fail: 'media.pdf_to_images' not in tool list" >&2; exit 1; }
	result="$({{bin}} num hex-to-decimal 0xff)"; [ "$result" = "255" ] || { echo "  fail: hex-to-decimal 0xff = '$result' (want '255')" >&2; exit 1; }
	uuid="$({{bin}} id uuid-v7)"; [ "${#uuid}" -eq 36 ] || { echo "  fail: uuid-v7 length ${#uuid} (want 36): $uuid" >&2; exit 1; }

require-wasm-target:
	@if ! rustup target list --installed 2>/dev/null | grep -q '^wasm32-unknown-unknown$'; then echo "error: wasm32-unknown-unknown target not installed" >&2; echo "  fix: rustup target add wasm32-unknown-unknown" >&2; exit 1; fi

# Rebuild the two standalone WASM guest fixtures/examples (they sit outside
# the workspace since they target wasm32-unknown-unknown) and refresh the
# committed `upeg-wasm/tests/fixtures/test_plugin.wasm` binary from
# `upeg-wasm/tests/fixture-src`. `examples/plugins/greet` is rebuilt to
# prove it still compiles under the macro contract, but it ships no
# committed binary artifact of its own. Opt-in — not part of `verify`/`check`
# because it needs the wasm32 target and mutates a checked-in binary.
wasm-fixtures: require-wasm-target
	@printf '\n\033[1;36m[wasm-fixtures 1/3]\033[0m build upeg-wasm/tests/fixture-src\n'
	cd upeg-wasm/tests/fixture-src && cargo build --target wasm32-unknown-unknown --release
	@printf '\n\033[1;36m[wasm-fixtures 2/3]\033[0m copy fixture-src artifact over upeg-wasm/tests/fixtures/test_plugin.wasm\n'
	cp upeg-wasm/tests/fixture-src/target/wasm32-unknown-unknown/release/upeg_wasm_test_plugin.wasm \
		upeg-wasm/tests/fixtures/test_plugin.wasm
	@printf '\n\033[1;36m[wasm-fixtures 3/3]\033[0m build examples/plugins/greet\n'
	cd examples/plugins/greet && cargo build --target wasm32-unknown-unknown --release
	@printf '\033[1;32m  ok\033[0m\n'

fmt-check:
	@printf '\n\033[1;36m[verify]\033[0m cargo fmt --all -- --check\n'
	cargo fmt --all -- --check

file-size-budget:
	#!/usr/bin/env bash
	set -euo pipefail
	rust_limit=1000
	printf '\n\033[1;36m[verify]\033[0m Rust file-size budget (<=%s lines)\n' "$rust_limit"
	# Exclude flutter_rust_bridge_codegen-emitted dispatch tables; they are
	# machine-generated and re-generated on every `just frb-codegen-check`.
	too_large="$({ git ls-files '*.rs'; git ls-files -o --exclude-standard '*.rs'; } | sort -u | while IFS= read -r file; do
	  [ -f "$file" ] || continue
	  case "$file" in
	    upeg-frb/src/frb_generated.rs | vendor/*) continue ;;
	  esac
	  lines="$(wc -l <"$file" | tr -d ' ')"
	  if [ "$lines" -gt "$rust_limit" ]; then
	    printf '%s %s\n' "$lines" "$file"
	  fi
	done)"
	if [ -n "$too_large" ]; then
	  echo "error: Rust source/test files must stay at or below $rust_limit lines" >&2
	  echo "$too_large" >&2
	  exit 1
	fi

	# Dart budget: hand-written files only. Excludes flutter_rust_bridge's
	# generated API bindings (flutter_app/lib/src/rust/**) and build_runner
	# output (*.freezed.dart, *.g.dart) — neither is something a contributor
	# edits by hand, so neither should count against the budget.
	#
	# Same ceiling as Rust: the eight files that used to carry debt past
	# 1000 lines were split into cohesive parts (widgets/board_canvas/**,
	# widgets/pin/**, pages/board_page/**, and per-feature test files
	# sharing test/test_helpers/*_harness.dart), so there is no longer a
	# reason for Dart to be held to a looser budget than Rust.
	dart_limit=1000
	printf '\n\033[1;36m[verify]\033[0m Dart file-size budget (<=%s lines, hand-written only)\n' "$dart_limit"
	too_large_dart="$({ git ls-files '*.dart'; git ls-files -o --exclude-standard '*.dart'; } | sort -u | while IFS= read -r file; do
	  [ -f "$file" ] || continue
	  case "$file" in
	    flutter_app/lib/src/rust/*) continue ;;
	    *.freezed.dart|*.g.dart) continue ;;
	  esac
	  lines="$(wc -l <"$file" | tr -d ' ')"
	  if [ "$lines" -gt "$dart_limit" ]; then
	    printf '%s %s\n' "$lines" "$file"
	  fi
	done)"
	if [ -n "$too_large_dart" ]; then
	  echo "error: hand-written Dart files must stay at or below $dart_limit lines" >&2
	  echo "$too_large_dart" >&2
	  exit 1
	fi

# Unit tests only — the pre-commit tier's test lane. No integration or
# bin targets, no doc tests; those live in `test-workspace` / `test-docs`
# and run once inside `test-baseline-check` during `just verify`.
#
# cargo test --workspace --lib — the `check` tier's test lane.
test-unit:
	@printf '\n\033[1;36m[check]\033[0m cargo test --workspace --lib\n'
	{{hermetic_project_manifest}} cargo test --workspace --lib

# Standalone dev recipes. NOT part of `verify`: `test-baseline-check`
# already runs both suites once (see scripts/test_baseline.py SUITES).
# Use these to iterate on a failure without the baseline machinery.
#
# cargo test --workspace --all-targets (standalone; not in `verify`).
test-workspace:
	@printf '\n\033[1;36m[dev]\033[0m cargo test --workspace --all-targets\n'
	{{hermetic_project_manifest}} cargo test --workspace --all-targets

test-docs:
	@printf '\n\033[1;36m[dev]\033[0m cargo test --workspace --doc\n'
	{{hermetic_project_manifest}} cargo test --workspace --doc

test-chrome-ext-file-input:
	@printf '\n\033[1;36m[verify]\033[0m node --test chrome-ext/tests/*.test.js\n'
	node --test chrome-ext/tests/*.test.js

# Compile + run the upeg-cli test suite with the chromiumoxide-backed
# Controlled Embed dispatcher enabled. Verifies the headless backend
# wiring compiles and the dispatcher chain is intact. The actual
# CDP round-trip is exercised by `--ignored` integration tests that
# spin up real chromium (require `CHROME_EXECUTABLE` to point at one).
test-controlled-embed:
	@printf '\n\033[1;36m[verify]\033[0m cargo test -p upeg-cli --features controlled-embed\n'
	{{hermetic_project_manifest}} cargo test -p upeg-cli --features controlled-embed

# Parser/diff self-checks for the baseline runner itself. Sub-second, no
# cargo — the drift gate below is only as trustworthy as this.
#
# Parser/diff self-checks for scripts/test_baseline.py.
test-baseline-self-test:
	@printf '\n\033[1;36m[preflight]\033[0m python3 scripts/test_baseline.py self-test\n'
	python3 scripts/test_baseline.py self-test

test-baseline:
	@printf '\n\033[1;36m[test-baseline]\033[0m python3 scripts/test_baseline.py write --output fixtures/test-baseline.json\n'
	python3 scripts/test_baseline.py write --output fixtures/test-baseline.json

test-baseline-check:
	@printf '\n\033[1;36m[verify]\033[0m python3 scripts/test_baseline.py compare --baseline fixtures/test-baseline.json --current-output target/test-baseline/current.json --diff-output target/test-baseline/diff.json\n'
	python3 scripts/test_baseline.py compare --baseline fixtures/test-baseline.json --current-output target/test-baseline/current.json --diff-output target/test-baseline/diff.json

# The inventory/schema drift gates compare generated output against a
# committed baseline, so the generating `upeg` run must be hermetic
# against per-user runtime sources. With `wasm-plugin` now a *default*
# upeg-cli feature, a plugin the developer installed into
# `~/.upeg/wasm/` (via `upeg plugin install`) would otherwise auto-load
# into `cargo run -p upeg-cli` and surface as phantom inventory drift —
# same for `~/.upeg/toolkits/` TOML. Point every source dir at a
# nonexistent path (the loader treats missing dirs as empty) so these
# lanes only ever see link-time built-ins, matching the committed
# fixtures.
#
# The repo dogfoods itself: `/workspaces/upeg/upeg.toml` is a real
# Project Manifest declaring the `dev.*` toolkit. Every lane below runs
# `cargo run` from the repo root, so without `UPEG_PROJECT_MANIFEST_PATH=off`
# those 15 tools would land in the generated inventory/schema output and
# blow up the drift gates. `off` disables Project Manifest detection
# entirely (docs/architecture/project-manifest.md).
hermetic_project_manifest := "UPEG_PROJECT_MANIFEST_PATH=off"
hermetic_sources := "UPEG_TOOLKITS_DIR=target/hermetic-sources/toolkits UPEG_WASM_DIR=target/hermetic-sources/wasm UPEG_MCP_IMPORTS_DIR=target/hermetic-sources/mcp-imports " + hermetic_project_manifest

interface-inventory:
	@printf '\n\033[1;36m[interface-inventory]\033[0m cargo run -p upeg-cli -- interface inventory generate --json fixtures/interface-inventory.json --markdown target/interface-inventory/interfaces.md\n'
	{{hermetic_sources}} cargo run -p upeg-cli -- interface inventory generate --json fixtures/interface-inventory.json --markdown target/interface-inventory/interfaces.md

interface-inventory-check:
	@printf '\n\033[1;36m[verify]\033[0m cargo run -p upeg-cli -- interface inventory check --baseline fixtures/interface-inventory.json --docs target/interface-inventory/interfaces.md --current-output target/interface-inventory/current.json --diff-output target/interface-inventory/diff.json --comment-output target/interface-inventory/pr-comment.md\n'
	{{hermetic_sources}} cargo run -p upeg-cli -- interface inventory check --baseline fixtures/interface-inventory.json --docs target/interface-inventory/interfaces.md --current-output target/interface-inventory/current.json --diff-output target/interface-inventory/diff.json --comment-output target/interface-inventory/pr-comment.md

toolkit-schema:
	@printf '\n\033[1;36m[toolkit-schema]\033[0m cargo run -p upeg-cli -- interface toolkit-schema generate --json fixtures/toolkit.schema.json --markdown docs/TOOL_MANIFEST.md\n'
	{{hermetic_sources}} cargo run -p upeg-cli -- interface toolkit-schema generate --json fixtures/toolkit.schema.json --markdown docs/TOOL_MANIFEST.md

toolkit-schema-check:
	@printf '\n\033[1;36m[verify]\033[0m cargo run -p upeg-cli -- interface toolkit-schema check --baseline-json fixtures/toolkit.schema.json --baseline-markdown docs/TOOL_MANIFEST.md --current-json target/toolkit-schema/current.schema.json --current-markdown target/toolkit-schema/TOOL_MANIFEST.md --schema-diff-output target/toolkit-schema/schema.diff --docs-diff-output target/toolkit-schema/docs.diff\n'
	{{hermetic_sources}} cargo run -p upeg-cli -- interface toolkit-schema check --baseline-json fixtures/toolkit.schema.json --baseline-markdown docs/TOOL_MANIFEST.md --current-json target/toolkit-schema/current.schema.json --current-markdown target/toolkit-schema/TOOL_MANIFEST.md --schema-diff-output target/toolkit-schema/schema.diff --docs-diff-output target/toolkit-schema/docs.diff

clippy-native:
	@printf '\n\033[1;36m[verify]\033[0m cargo clippy (workspace, --all-targets) -D warnings\n'
	cargo clippy --workspace --all-targets -- -D warnings

clippy-tools-wasm:
	@printf '\n\033[1;36m[verify]\033[0m cargo clippy --target wasm32 -p upeg-tools -D warnings\n'
	cargo clippy --target wasm32-unknown-unknown -p upeg-tools -- -D warnings

clippy-core-wasm:
	@printf '\n\033[1;36m[verify]\033[0m cargo clippy --target wasm32 -p upeg-core -D warnings\n'
	cargo clippy --target wasm32-unknown-unknown -p upeg-core -- -D warnings

# upeg_frb (the flutter_rust_bridge cdylib) is the crate `just build-frb-wasm`
# / wasm-pack actually compiles for the PWA. clippy-tools-wasm/clippy-core-wasm
# above cover its dependencies but never compiled upeg_frb itself on
# wasm32, so a native-only symbol (upeg_runtime::persistence, gated
# `cfg(not(target_arch = "wasm32"))`) rotted here unnoticed. This lane closes
# that coverage gap.
clippy-frb-wasm:
	@printf '\n\033[1;36m[verify]\033[0m cargo clippy --target wasm32 -p upeg_frb -D warnings\n'
	cargo clippy --target wasm32-unknown-unknown -p upeg_frb -- -D warnings

# ─── Flutter lanes ──────────────────────────────────────────────────
# Post-Phase-10 cutover (2026-05-24): Flutter is the only active desktop/PWA
# surface. These lanes are part of `default` / `verify`'s gate.

flutter-pub-get:
	@printf '\n\033[1;36m[flutter]\033[0m flutter pub get\n'
	cd flutter_app && flutter pub get

flutter-analyze: flutter-pub-get
	@printf '\n\033[1;36m[flutter]\033[0m flutter analyze\n'
	cd flutter_app && flutter analyze

flutter-fmt:
	@printf '\n\033[1;36m[flutter]\033[0m dart format flutter_app/\n'
	dart format flutter_app/lib flutter_app/test

flutter-fmt-check:
	@printf '\n\033[1;36m[flutter]\033[0m dart format --output=none --set-exit-if-changed\n'
	dart format --output=none --set-exit-if-changed flutter_app/lib flutter_app/test

# Standalone dev recipe. NOT part of `verify`: the Flutter suite runs
# once inside `test-baseline-check` (scripts/test_baseline.py `flutter`
# suite, which includes the golden tests).
#
# flutter test (standalone; not in `verify`).
flutter-test: flutter-pub-get
	@printf '\n\033[1;36m[flutter]\033[0m flutter test\n'
	cd flutter_app && flutter test

# Browser-platform Flutter tests. Runs `flutter test --platform=chrome`
# inside an Xvfb display so CanvasKit/Skwasm WebGL renders headlessly
# via swiftshader. CHROME_EXECUTABLE is pinned by the devcontainer
# Dockerfile to `/usr/bin/chromium`. Not part of `default` — the host
# `flutter test` lane already covers VM-platform widget tests, this
# recipe is for verifying web-only divergence. Golden PNG regression
# stays in `ui-parity-check`; browser rendering differs enough that
# mixing those concerns makes the web gate noisy.
flutter-test-chrome: flutter-pub-get
	@printf '\n\033[1;36m[flutter]\033[0m flutter test --platform=chrome --exclude-tags=golden (xvfb)\n'
	cd flutter_app && xvfb-run --auto-servernum --server-args="-screen 0 1920x1080x24" flutter test --platform=chrome --exclude-tags=golden

# Theme-token lint. Dark-only foreground/palette inks must come from
# `UpegTokens` (flutter_app/lib/src/theme/), never as `Color(0x...)`
# literals inside widgets — the accent ink #07120A and the dark
# accent/warn palette regressed this way once and broke light-mode
# contrast (WCAG AA). Greps the banned hexes back out of the tree.
flutter-theme-token-check:
	#!/usr/bin/env bash
	set -euo pipefail
	printf '\n\033[1;36m[flutter]\033[0m flutter-theme-token-check (no hardcoded theme hex outside lib/src/theme)\n'
	banned='0xFF07120A|0xFF5BD16C|0xFFD9B14C|0xFFE08260|0xFF2E8C44|0xFFB07832|0xFFBF5230'
	hits="$(grep -rniE "$banned" flutter_app/lib --include='*.dart' | grep -v '^flutter_app/lib/src/theme/' || true)"
	if [ -n "$hits" ]; then
	  echo "error: theme palette hex hardcoded outside flutter_app/lib/src/theme/" >&2
	  echo "  fix: read the colour from UpegTokens instead (context.upeg.onAccent / onWarn / accent / warn / ...)" >&2
	  echo "$hits" >&2
	  exit 1
	fi
	printf '\033[1;32m  ok\033[0m\n'

# i18n hardcode lint for flutter_app/lib/src/{widgets,pages,features/host_attach}.
# UI copy belongs in t() + the Rust En/Ko catalog (upeg-pegboard-ui/src/i18n.rs).
# Heuristic: Text literals/simple same-file variables, established presentation
# named parameters, and Hangul literals anywhere. Generic protocol/error/debug
# messages and cross-file data flow are not inferred to be UI. See the script
# docstring for exact bounds. No per-line suppressions. Only NEW findings fail
# against fixtures/flutter-i18n-baseline.json; do not expand it to hide new UI
# copy. After migrating a baselined string, prune the snapshot via:
#   python3 scripts/flutter_i18n_check.py write --baseline fixtures/flutter-i18n-baseline.json
flutter-i18n-check:
	@printf '\n\033[1;36m[flutter]\033[0m flutter-i18n-check (no new hardcoded user-facing strings)\n'
	python3 scripts/flutter_i18n_check.py check --baseline fixtures/flutter-i18n-baseline.json

# UI parity guard. Re-runs the golden regression suite which compares
# every major widget against `flutter_app/test/goldens/*.png`. On
# divergence Flutter writes `*-actual.png` next to the failing test.
# NOT a `verify` lane of its own: the goldens are `flutter test` tests, so
# `test-baseline-check`'s `flutter` suite already runs them. Kept for
# local golden work — run it, look at the `*-actual.png`, then re-baseline
# intentional UI edits via `just ui-parity`.
#
# Golden regression suite on its own (standalone; not in `verify`).
ui-parity-check: flutter-pub-get
	@printf '\n\033[1;36m[flutter]\033[0m ui-parity-check (golden regression)\n'
	cd flutter_app && flutter test --tags=golden

# Phase 9 integration tests. NOT part of `default`/`verify` because the
# devcontainer is headless and most widget-driven integration tests need
# a real display + bundled dylib. Runs locally on a desktop host via
# `just flutter-integration-test` once the dispatch dylib is available.
flutter-integration-test: flutter-pub-get
	@printf '\n\033[1;36m[flutter]\033[0m flutter test integration_test/\n'
	cd flutter_app && flutter test integration_test/

# Real WebKitGTK session/debug ownership and CLI -> Desktop HTTP acceptance.
# The CLI case opts in only here so ordinary integration runs never use a
# developer's personal UPeg configuration or an unrelated running host.
flutter-controlled-embed-linux-test: flutter-pub-get
	#!/usr/bin/env bash
	set -euo pipefail
	cargo build -p upeg-cli --bin upeg
	cli_path="$PWD/target/debug/upeg"
	for variable in "${!UPEG_@}"; do
	  if [[ -n "$variable" ]]; then unset "$variable"; fi
	done
	cd flutter_app
	for test_file in \
	  integration_test/controlled_embed_shared_session_integration_test.dart \
	  integration_test/controlled_embed_cli_session_integration_test.dart; do
	  test_config_dir="$(mktemp -d "${TMPDIR:-/tmp}/upeg-webview-native.XXXXXXXX")"
	  trap 'rm -rf -- "$test_config_dir"' EXIT
	  env UPEG_HOME="$test_config_dir" UPEG_PROJECT_MANIFEST_PATH=off \
	    UPEG_TEST_CLI_PATH="$cli_path" \
	    xvfb-run -a flutter test "$test_file" -d linux
	  rm -rf -- "$test_config_dir"
	  trap - EXIT
	done

# frb-codegen-check: re-runs flutter_rust_bridge_codegen and fails if it
# would change generated files from their current working-tree content.
# Phase 0 has no `upeg-frb/src/api/` yet, so the recipe explicitly
# short-circuits to a no-op until that directory exists (Phase 2 creates it).
frb-codegen-check:
	#!/usr/bin/env bash
	set -euo pipefail
	printf '\n\033[1;36m[flutter]\033[0m flutter_rust_bridge_codegen check\n'
	if ! command -v flutter_rust_bridge_codegen >/dev/null 2>&1; then
	  echo "error: flutter_rust_bridge_codegen not installed" >&2
	  echo "  fix: cargo install flutter_rust_bridge_codegen --locked --version 2.12.0" >&2
	  exit 1
	fi
	if [ ! -d upeg-frb/src/api ]; then
	  printf '\033[1;33m  skip\033[0m (upeg-frb/src/api/ does not exist yet — Phase 2 will create it)\n'
	  exit 0
	fi
	before="$(mktemp -d)"
	trap 'rm -rf "$before"' EXIT
	mkdir -p "$before/upeg-frb/src" "$before/flutter_app/lib/src"
	cp -a upeg-frb/src/frb_generated.rs "$before/upeg-frb/src/frb_generated.rs"
	cp -a flutter_app/lib/src/rust "$before/flutter_app/lib/src/rust"
	# RUST_LOG is set explicitly because flutter_rust_bridge_codegen 2.12.0
	# panics on an empty / missing env var instead of defaulting to `info`.
	cd flutter_app && RUST_LOG=info flutter_rust_bridge_codegen generate
	cd ..
	# Codegen emits unformatted imports; rustfmt normalizes them. The
	# committed version is the formatted one, so the drift check below
	# compares like-for-like.
	cargo fmt -p upeg_frb
	# Codegen can also emit trailing whitespace in generated Dart switch
	# tables. Keep generated Dart clean so `git diff --check` remains a
	# reliable repository-wide gate.
	find flutter_app/lib/src/rust -name '*.dart' -type f -exec perl -pi -e 's/[ \t]+$//' {} +
	if ! diff -qr "$before/upeg-frb/src/frb_generated.rs" upeg-frb/src/frb_generated.rs >/dev/null 2>&1 \
	  || ! diff -qr "$before/flutter_app/lib/src/rust" flutter_app/lib/src/rust >/dev/null 2>&1; then
	  echo "error: flutter_rust_bridge_codegen would change generated files" >&2
	  echo "  fix: run \`cd flutter_app && flutter_rust_bridge_codegen generate\` and commit the result" >&2
	  diff -qr "$before/upeg-frb/src/frb_generated.rs" upeg-frb/src/frb_generated.rs >&2 || true
	  diff -qr "$before/flutter_app/lib/src/rust" flutter_app/lib/src/rust >&2 || true
	  exit 1
	fi
	printf '\033[1;32m  ok (no drift)\033[0m\n'

# Regenerate flutter_rust_bridge bindings. Re-runs flutter_rust_bridge_codegen,
# applies rustfmt, and cleans trailing whitespace to match the committed state.
# This is the regeneration step that `frb-codegen-check` verifies against.
frb-codegen:
	#!/usr/bin/env bash
	set -euo pipefail
	printf '\n\033[1;36m[flutter]\033[0m flutter_rust_bridge_codegen regenerate\n'
	if ! command -v flutter_rust_bridge_codegen >/dev/null 2>&1; then
	  echo "error: flutter_rust_bridge_codegen not installed" >&2
	  echo "  fix: cargo install flutter_rust_bridge_codegen --locked --version 2.12.0" >&2
	  exit 1
	fi
	if [ ! -d upeg-frb/src/api ]; then
	  printf '\033[1;33m  skip\033[0m (upeg-frb/src/api/ does not exist yet)\n'
	  exit 0
	fi
	cd flutter_app && RUST_LOG=info flutter_rust_bridge_codegen generate
	cd ..
	cargo fmt -p upeg_frb
	find flutter_app/lib/src/rust -name '*.dart' -type f -exec perl -pi -e 's/[ \t]+$//' {} +
	printf '\033[1;32m  ok (regenerated)\033[0m\n'

# Regenerate UI parity baselines. Re-runs the golden regression suite
# with --update-goldens to capture intentional UI changes. Run this after
# intentional widget/layout modifications, then commit the result.
ui-parity: flutter-pub-get
	@printf '\n\033[1;36m[flutter]\033[0m ui-parity update-goldens\n'
	cd flutter_app && flutter test --update-goldens --tags=golden
	@printf '\033[1;32m  ok (goldens updated)\033[0m\n'

flutter-build-linux: flutter-pub-get
	@printf '\n\033[1;36m[flutter]\033[0m flutter build linux --release\n'
	cd flutter_app && flutter build linux --release

flutter-build-macos: flutter-pub-get
	@printf '\n\033[1;36m[flutter]\033[0m flutter build macos --release\n'
	cd flutter_app && flutter build macos --release

# macOS runtime smoke for real desktop startup/parity checks. Host-only.
# Override the Flutter command on machines managed by FVM:
#   UPEG_FLUTTER='fvm flutter' just flutter-run-macos
flutter-run-macos:
	#!/usr/bin/env bash
	set -euo pipefail
	flutter_display="${UPEG_FLUTTER:-flutter}"
	read -r -a flutter_cmd <<< "$flutter_display"
	printf '\n\033[1;36m[flutter]\033[0m %s run -d macos\n' "$flutter_display"
	cd flutter_app
	"${flutter_cmd[@]}" pub get
	"${flutter_cmd[@]}" run -d macos

flutter-build-windows: flutter-pub-get
	@printf '\n\033[1;36m[flutter]\033[0m flutter build windows --release\n'
	cd flutter_app && flutter build windows --release

# NEVER add `--pwa-strategy` here — not even `--pwa-strategy none`.
#
# The flag is hidden and deprecated (flutter/flutter#156910) but still
# live, and it is the ONLY thing that decides whether Flutter substitutes
# `{{flutter_service_worker_version}}`. With the flag absent the default
# `offline-first` fills it with a per-build random number, which is what
# `web/flutter_bootstrap.js` registers our own service worker under
# (`upeg_service_worker.js?v=<token>`) and what names the app-shell cache.
# `--pwa-strategy none` substitutes `null` instead, the bootstrap then
# skips registration entirely, and the PWA offline contract
# (`pwa.service-worker.cache`) disappears with no build error — a green
# lane that ships a non-offline PWA. The header of
# flutter_app/web/upeg_service_worker.js carries the same warning.
#
# Release web bundle. Leave the service-worker flags alone (see above).
flutter-build-web: build-frb-wasm
	@printf '\n\033[1;36m[flutter]\033[0m flutter build web --release --no-wasm-dry-run\n'
	cd flutter_app && flutter build web --release --no-wasm-dry-run

# Boots the web bundle AND asserts the PWA service-worker cache contract
# (`pwa.service-worker.cache` in the interface inventory): the worker
# activates and controls the page, the app shell cache holds the boot set,
# and a reload with the static server STOPPED still boots from cache. The
# assertions live in scripts/flutter_web_smoke_check.mjs, which drives
# headless Chromium over CDP (node builtins only, no npm deps). Missing
# Chromium is a hard failure, never a skip — see the ci-smoke comment.
flutter-web-smoke: flutter-build-web
	@printf '\n\033[1;36m[flutter]\033[0m flutter web smoke + PWA service worker contract (headless Chromium)\n'
	scripts/flutter_web_smoke.sh

# FRB web helper. cargokit drives Linux/macOS/Windows cdylib builds
# from inside the Flutter native pipeline (see flutter_app/rust_builder/),
# but it does NOT cover web — Flutter Web is JS+wasm, not CMake/Xcode.
# FRB's supported `build-web` command wraps wasm-pack with a nightly std build
# and atomics/bulk-memory/mutable-globals. A plain stable wasm-pack build emits
# non-shared WebAssembly.Memory, which cannot be transferred to FRB workers.
build-frb-wasm: flutter-pub-get
	#!/usr/bin/env bash
	set -euo pipefail
	toolchain="nightly-2025-12-08"
	wasm_pack_version="0.13.1"
	wasm_rustflags='--cfg getrandom_backend="wasm_js" -C target-feature=+atomics,+bulk-memory,+mutable-globals -C link-arg=--shared-memory -C link-arg=--import-memory -C link-arg=--max-memory=4294967296 -C link-arg=--export=__wasm_init_tls -C link-arg=--export=__tls_size -C link-arg=--export=__tls_align -C link-arg=--export=__tls_base'
	printf '\n\033[1;36m[flutter]\033[0m FRB shared-memory web build (upeg-frb → flutter_app/web/pkg)\n'
	if ! rustup run "$toolchain" rustc --version >/dev/null 2>&1; then
	  rustup toolchain install "$toolchain" --profile minimal --component rust-src --target wasm32-unknown-unknown
	else
	  rustup component add rust-src --toolchain "$toolchain"
	  rustup target add wasm32-unknown-unknown --toolchain "$toolchain"
	fi
	if ! wasm-pack --version 2>/dev/null | grep -Fx "wasm-pack $wasm_pack_version" >/dev/null; then
	  cargo install wasm-pack --version "$wasm_pack_version" --locked
	fi
	cd flutter_app
	rust_root="$(cd ../upeg-frb && pwd)"
	web_root="$PWD/web"
	dart run flutter_rust_bridge build-web \
	  --rust-root "$rust_root" \
	  --output "$web_root" \
	  --release \
	  --wasm-pack-rustup-toolchain "$toolchain" \
	  --wasm-pack-rustflags "$wasm_rustflags"
	test -s "$web_root/pkg/upeg_frb.js"
	test -s "$web_root/pkg/upeg_frb_bg.wasm"

# Flutter Web has no native pre-build hook for arbitrary commands, so
# bundle the wasm-pack step into a single composite recipe. Web parallel
# to how cargokit auto-invokes cargo on the desktop platforms.
flutter-run-web: build-frb-wasm
	@printf '\n\033[1;36m[flutter]\033[0m flutter run -d chrome\n'
	cd flutter_app && flutter run -d chrome

flutter-run-web-server: build-frb-wasm
	@printf '\n\033[1;36m[flutter]\033[0m flutter run -d web-server\n'
	cd flutter_app && flutter run -d web-server --web-port=3000

# ─── Phase 8 packaging ─────────────────────────────────────────────
# Build + wrap Flutter outputs into installable artifacts. Recipes
# assume the Flutter build for the target platform has already run
# (or run flutter-build-X if not). Code signing is out of scope —
# producing UNSIGNED artifacts. Phase 9+ can layer signing.
# UPEG_PACKAGE_OUT redirects the `target/packages` root, e.g.
# `UPEG_PACKAGE_OUT="$(mktemp -d)" just package-linux` for a fresh-dir run.

# Linux .deb via fpm. Requires: just flutter-build-linux already ran.
# The desktop app does NOT get a `upeg` PATH entry — that name belongs to the
# CLI (`cargo build -p upeg-cli --bin upeg`, see README "Build and run"). The
# GUI launches via the .desktop file at the absolute /opt path, or `upeg-app`.
package-linux-deb: flutter-build-linux
	#!/usr/bin/env bash
	set -euo pipefail
	printf '\n\033[1;36m[package]\033[0m linux .deb (fpm)\n'
	if ! command -v fpm >/dev/null 2>&1; then
	  echo "error: fpm not installed (gem install fpm)" >&2
	  exit 1
	fi
	out="${UPEG_PACKAGE_OUT:-target/packages}/linux"
	mkdir -p "$out"
	rm -f "$out"/upeg_*.deb
	stage="$(mktemp -d)"
	trap 'rm -rf "$stage"' EXIT
	mkdir -p \
	    "$stage/opt/upeg" \
	    "$stage/usr/bin" \
	    "$stage/usr/share/applications" \
	    "$stage/usr/share/icons/hicolor/512x512/apps" \
	    "$stage/usr/share/metainfo"
	cp -a flutter_app/build/linux/x64/release/bundle/. "$stage/opt/upeg/"
	ln -s /opt/upeg/upeg "$stage/usr/bin/upeg-app"
	sed 's|^Exec=upeg %u$|Exec=/opt/upeg/upeg %u|' packaging/linux/upeg.desktop \
	    > "$stage/usr/share/applications/upeg.desktop"
	cp packaging/linux/io.github._5pecia1.upeg.metainfo.xml "$stage/usr/share/metainfo/io.github._5pecia1.upeg.metainfo.xml"
	cp flutter_app/web/icons/Icon-512.png "$stage/usr/share/icons/hicolor/512x512/apps/upeg.png"
	bash packaging/release-files.sh stage "$stage/usr/share/doc/upeg"
	version="$(bash packaging/release-files.sh version)"
	version="${version#v}"
	case "$version" in
	  [0-9]*) ;;
	  *) version="0.0.0+$version" ;;
	esac
	fpm -s dir -t deb \
	    -n upeg -v "$version" --architecture amd64 \
	    --depends libgtk-3-0 \
	    --depends libkeybinder-3.0-0 \
	    --depends libayatana-appindicator3-1 \
	    --depends libwebkit2gtk-4.1-0 \
	    --description "Universal Pegboard" \
	    --license "Apache-2.0" \
	    --maintainer "upeg contributors" \
	    --url "https://github.com/5pecia1/UPeg" \
	    -C "$stage" \
	    --package "$out/" \
	    .
	deb="$out/upeg_${version}_amd64.deb"
	if command -v dpkg-deb >/dev/null 2>&1; then
	  dpkg-deb -c "$deb" | grep 'usr/share/doc/upeg/NOTICE' \
	    || { echo "error: $deb lacks staged notices" >&2; exit 1; }
	fi
	ls -la "$out"

# Linux .AppImage via appimagetool. Requires: just flutter-build-linux.
# `appimagetool` itself is shipped as an AppImage; run it in extract-and-run
# mode so headless devcontainers/CI do not need FUSE.
package-linux-appimage: flutter-build-linux
	#!/usr/bin/env bash
	set -euo pipefail
	printf '\n\033[1;36m[package]\033[0m linux .AppImage\n'
	if ! command -v appimagetool >/dev/null 2>&1; then
	  echo "error: appimagetool not installed" >&2
	  echo "  fix: download from https://github.com/AppImage/appimagetool/releases" >&2
	  exit 1
	fi
	if ! command -v appstreamcli >/dev/null 2>&1; then
	  echo "error: appstreamcli not installed (install the appstream package)" >&2
	  exit 1
	fi
	# --no-net: the homepage check needs the public repo reachable, which a
	# pre-release private mirror is not; structural validation stays local.
	appstreamcli validate --no-net packaging/linux/io.github._5pecia1.upeg.metainfo.xml
	out="${UPEG_PACKAGE_OUT:-target/packages}/linux"
	mkdir -p "$out"
	out="$(cd "$out" && pwd)"
	work="$(mktemp -d)"
	trap 'rm -rf "$work"' EXIT
	tmp="$work/upeg.AppDir"
	mkdir -p "$tmp"
	cp -a flutter_app/build/linux/x64/release/bundle/. "$tmp/"
	mkdir -p "$tmp/usr/share/applications" "$tmp/usr/share/metainfo"
	cp packaging/linux/upeg.desktop "$tmp/upeg.desktop"
	cp packaging/linux/upeg.desktop "$tmp/usr/share/applications/upeg.desktop"
	cp packaging/linux/io.github._5pecia1.upeg.metainfo.xml "$tmp/usr/share/metainfo/io.github._5pecia1.upeg.metainfo.xml"
	cp flutter_app/web/icons/Icon-512.png "$tmp/upeg.png"
	bash packaging/release-files.sh stage "$tmp/usr/share/doc/upeg"
	cat > "$tmp/AppRun" <<'EOF'
	#!/bin/sh
	HERE=$(dirname "$(readlink -f "$0")")
	exec "$HERE/upeg" "$@"
	EOF
	chmod +x "$tmp/AppRun"
	version="$(bash packaging/release-files.sh version)"
	# Pin the AppImage runtime when the caller provides one; without it
	# appimagetool downloads the moving `continuous` release at build time.
	runtime_args=()
	if [ -n "${UPEG_APPIMAGE_RUNTIME:-}" ]; then
	  runtime_args=(--runtime-file "$UPEG_APPIMAGE_RUNTIME")
	fi
	APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 appimagetool --no-appstream "${runtime_args[@]}" \
	    "$tmp" "$out/upeg-${version}-x86_64.AppImage"
	# Self-check: extract the produced image and re-validate its payload.
	probe="$work/extract"
	mkdir -p "$probe"
	( cd "$probe" && "$out/upeg-${version}-x86_64.AppImage" --appimage-extract >/dev/null )
	bash packaging/release-files.sh check "$probe/squashfs-root/usr/share/doc/upeg"

# macOS .dmg via create-dmg. Requires: just flutter-build-macos on macOS host.
# cargokit wires the cdylib build via flutter_app/rust_builder/macos/upeg_frb.podspec,
# so `flutter build macos` auto-builds + embeds upeg_frb without any extra steps.
package-macos-dmg: flutter-build-macos
	#!/usr/bin/env bash
	set -euo pipefail
	printf '\n\033[1;36m[package]\033[0m macos .dmg\n'
	if ! command -v create-dmg >/dev/null 2>&1; then
	  echo "error: create-dmg not installed (brew install create-dmg)" >&2
	  exit 1
	fi
	out="${UPEG_PACKAGE_OUT:-target/packages}/macos"
	mkdir -p "$out"
	work="$(mktemp -d)"
	trap 'rm -rf "$work"' EXIT
	# Stage the .app plus the legal set side by side in the .dmg window.
	cp -a "flutter_app/build/macos/Build/Products/Release/." "$work/dmg/"
	bash packaging/release-files.sh stage "$work/dmg/licenses"
	version="$(bash packaging/release-files.sh version)"
	create-dmg --overwrite \
	    --volname "upeg" \
	    "$out/upeg-${version}.dmg" \
	    "$work/dmg"

# Windows MSIX via msix Dart pub global. Requires: just flutter-build-windows on Windows host.
package-windows-msix: flutter-build-windows
	#!/usr/bin/env bash
	set -euo pipefail
	printf '\n\033[1;36m[package]\033[0m windows .msix\n'
	# `msix` package provides `flutter pub run msix:create`.
	cd flutter_app
	if ! grep -q '^  msix:' pubspec.yaml; then
	  echo "error: msix package not in pubspec.yaml" >&2
	  exit 1
	fi
	# Everything under Release/ is packed into the MSIX install dir.
	bash ../packaging/release-files.sh stage "build/windows/x64/runner/Release/licenses"
	flutter pub run msix:create
	out="${UPEG_PACKAGE_OUT:-../target/packages}/windows"
	mkdir -p "$out"
	cp build/windows/x64/runner/Release/*.msix "$out/" 2>/dev/null || true

# Web PWA bundle. Just renames the directory + gzip-prepares static assets.
package-web: flutter-build-web
	#!/usr/bin/env bash
	set -euo pipefail
	printf '\n\033[1;36m[package]\033[0m web pwa bundle\n'
	out="${UPEG_PACKAGE_OUT:-target/packages}/web"
	mkdir -p "$out"
	rm -rf "$out"/*
	cp -a flutter_app/build/web/. "$out/"
	bash packaging/release-files.sh stage "$out/licenses"
	# pre-gzip the main JS for static-host deployment
	if command -v gzip >/dev/null 2>&1; then
	  find "$out" -name '*.js' -exec gzip -k -9 {} \;
	  find "$out" -name '*.css' -exec gzip -k -9 {} \;
	fi
	echo "web bundle: $out"

# Build all linux artifacts on this host. macOS/Windows skipped (need the OS).
package-linux: package-linux-deb package-linux-appimage
package: package-linux package-web
