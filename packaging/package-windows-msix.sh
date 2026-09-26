#!/usr/bin/env bash
# Called after a Windows release build; output_path is supported by msix 3.16.13.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_ROOT="${UPEG_SOURCE_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
APP="$SOURCE_ROOT/flutter_app"
RELEASE="$APP/build/windows/x64/runner/Release"
grep -q '^  msix:' "$APP/pubspec.yaml" || {
  printf 'error: msix package not in pubspec.yaml\n' >&2
  exit 1
}
[ -d "$RELEASE" ] || { printf 'error: Windows Release build is missing\n' >&2; exit 1; }

bash "$SCRIPT_DIR/release-files.sh" stage "$RELEASE/licenses"
run_dir="$(mktemp -d "${TMPDIR:-/tmp}/upeg-msix.XXXXXX")"
copy_tmp=''
trap 'rm -rf -- "$run_dir"; if [ -n "$copy_tmp" ]; then rm -f -- "$copy_tmp"; fi' EXIT
msix_output="$run_dir"
if command -v cygpath >/dev/null 2>&1; then
  msix_output="$(cygpath -w "$run_dir")"
fi
(cd "$APP" && flutter pub run msix:create --output-path "$msix_output")

mapfile -d '' packages < <(find "$run_dir" -maxdepth 1 -type f -name '*.msix' -print0)
if [ "${#packages[@]}" -ne 1 ] || [ ! -s "${packages[0]:-}" ]; then
  printf 'error: expected exactly one new nonempty .msix, found %s\n' "${#packages[@]}" >&2
  exit 1
fi

out="${UPEG_PACKAGE_OUT:-$SOURCE_ROOT/target/packages}/windows"
mkdir -p "$out"
dest="$out/$(basename "${packages[0]}")"
if [ -e "$dest" ] || [ -L "$dest" ]; then
  printf 'error: package already exists: %s\n' "$dest" >&2
  exit 1
fi
copy_tmp="$(mktemp "$out/.upeg-copy.XXXXXX")"
cp -- "${packages[0]}" "$copy_tmp"
# A same-directory hard link publishes the completed copy only if dest is free.
ln -- "$copy_tmp" "$dest"
rm -f -- "$copy_tmp"
copy_tmp=''
printf 'windows package: %s\n' "$dest"
