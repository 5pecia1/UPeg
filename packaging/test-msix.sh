#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE="$(cd "$HERE/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT
SRC="$WORK/source"
mkdir -p "$SRC/flutter_app/fonts" "$SRC/flutter_app/rust_builder/cargokit" \
  "$SRC/vendor/pdf-inspector" "$SRC/vendor/zxcvbn" "$SRC/flutter_app/build/windows/x64/runner/Release" \
  "$WORK/bin" "$WORK/packages/windows"
for path in LICENSE NOTICE flutter_app/fonts/OFL-D2Coding.txt \
  flutter_app/fonts/OFL-JetBrainsMono.txt vendor/pdf-inspector/LICENSE \
  vendor/zxcvbn/LICENSE flutter_app/rust_builder/cargokit/LICENSE; do
  cp "$SOURCE/$path" "$SRC/$path"
done
printf '  msix: ^3.16.7\n' > "$SRC/flutter_app/pubspec.yaml"

cat > "$WORK/bin/flutter" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "$1" = pub ] && [ "$2" = run ] && [ "$3" = msix:create ] || exit 90
[ "$4" = --output-path ] || exit 91
out="$5"
[ -d "$out" ] || exit 92
case "$out" in "$UPEG_SOURCE_ROOT/flutter_app/build/windows/x64/runner/Release"/*) exit 93 ;; esac
case "$MSIX_TEST_MODE" in
  zero) ;;
  one) printf 'new package' > "$out/current.msix" ;;
  multiple) printf 'one' > "$out/a.msix"; printf 'two' > "$out/b.msix" ;;
  empty) : > "$out/empty.msix" ;;
  fail) exit 17 ;;
esac
EOF
chmod +x "$WORK/bin/flutter"

run_case() {
  PATH="$WORK/bin:$PATH" UPEG_SOURCE_ROOT="$SRC" UPEG_PACKAGE_OUT="$WORK/packages" \
    MSIX_TEST_MODE="$1" bash "$HERE/package-windows-msix.sh" >/dev/null 2>&1
}
fail() { printf 'FAIL - %s\n' "$1" >&2; exit 1; }

printf 'old package' > "$WORK/packages/windows/old.msix"
for mode in zero multiple empty fail; do
  if run_case "$mode"; then fail "$mode unexpectedly passed"; fi
  [ "$(cat "$WORK/packages/windows/old.msix")" = 'old package' ] || fail "$mode removed old package"
done
printf 'ok - zero, multiple, empty and failed generation reject stale output\n'

run_case one || fail 'one package failed'
[ "$(cat "$WORK/packages/windows/current.msix")" = 'new package' ] || fail 'current package not copied'
if run_case one; then fail 'existing destination overwritten'; fi
[ "$(cat "$WORK/packages/windows/current.msix")" = 'new package' ] || fail 'existing destination changed'
printf 'ok - one new package succeeds and existing packages remain\n'

mkdir "$WORK/packages/windows/blocked.msix"
sed -i 's@current.msix@blocked.msix@' "$WORK/bin/flutter"
if run_case one; then fail 'failed copy passed'; fi
[ -d "$WORK/packages/windows/blocked.msix" ] || fail 'failed copy changed target'
printf 'ok - copy failure propagates\n'

ln -s "$WORK/missing.msix" "$WORK/packages/windows/symlink.msix"
sed -i 's@blocked.msix@symlink.msix@' "$WORK/bin/flutter"
if run_case one; then fail 'dangling symlink destination accepted'; fi
[ -L "$WORK/packages/windows/symlink.msix" ] || fail 'symlink destination changed'
printf 'ok - dangling symlink destination is refused\n'
