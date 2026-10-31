#!/usr/bin/env bash
# Exercise the public, Linux, Linux/macOS, and full v2 profiles with literal asset fixtures.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT
TAG=v0.1.2
VERSION=0.1.2
COMMIT=d173ad2452c2df052afb54749d42677593252998

public=(
  "upeg-$TAG-x86_64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-aarch64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-aarch64-apple-darwin.tar.gz"
  "upeg-$TAG-x86_64-pc-windows-msvc.zip"
  "upeg_${VERSION}_amd64.deb"
  "upeg-$TAG-x86_64.AppImage"
  "upeg-$TAG-web.tar.gz"
  "upeg-$TAG.dmg"
  "upeg-$TAG-x86_64.msix"
)
linux=(
  "upeg-$TAG-x86_64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-aarch64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-web.tar.gz"
  "upeg_${VERSION}_amd64.deb"
  "upeg-$TAG-x86_64.AppImage"
)
linux_macos=(
  "upeg-$TAG-x86_64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-aarch64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-aarch64-apple-darwin.tar.gz"
  "upeg_${VERSION}_amd64.deb"
  "upeg-$TAG-x86_64.AppImage"
  "upeg-$TAG-web.tar.gz"
  "upeg-$TAG.dmg"
)
full=(
  "upeg-$TAG-x86_64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-aarch64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-aarch64-apple-darwin.tar.gz"
  "upeg-$TAG-x86_64-pc-windows-msvc.zip"
  "upeg_${VERSION}_amd64.deb"
  "upeg-$TAG-x86_64.AppImage"
  "upeg-$TAG-web.tar.gz"
  "upeg-$TAG.dmg"
  "upeg-$TAG-x86_64.msix"
)

toolkit_ids=(color convert csv devcontainer eth hash id media net num qr security text time weather)
add_toolkit_assets() {
  local array_name="$1" target id
  shift
  local -n assets="$array_name"
  for target in "$@"; do
    local extension=""
    if [[ "$target" == "x86_64-pc-windows-msvc" ]]; then
      extension=".exe"
    fi
    assets+=("toolkits-$target-catalog.json")
    for id in "${toolkit_ids[@]}"; do
      assets+=("toolkits-$target-$id-$VERSION$extension.gz")
    done
  done
}
add_toolkit_assets public x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu aarch64-apple-darwin x86_64-apple-darwin x86_64-pc-windows-msvc
[[ ${#public[@]} -eq 89 ]]
[[ " ${public[*]} " == *" toolkits-x86_64-pc-windows-msvc-color-$VERSION.exe.gz "* ]]
[[ " ${public[*]} " != *" toolkits-x86_64-pc-windows-msvc-color-$VERSION.gz "* ]]

build_profile() {
  local profile="$1" include_rpm="${2:-false}" payload variant="$1"
  local -a names
  local -a rpm_flag=()
  case "$profile" in
    public) names=("${public[@]}") ;;
    linux) names=("${linux[@]}") ;;
    linux-macos) names=("${linux_macos[@]}") ;;
    full) names=("${full[@]}") ;;
  esac
  if [ "$include_rpm" = true ]; then
    variant="$profile-rpm"
    rpm_flag=(--include-rpm)
    names+=("upeg-${VERSION}-1.x86_64.rpm")
  fi
  local files="$WORK/$variant/files"
  mkdir -p "$files"
  for payload in "${names[@]}"; do
    printf 'payload %s\n' "$payload" > "$files/$payload"
    ( cd "$files" && sha256sum -b "$payload" > "$payload.sha256" )
  done
  python3 "$HERE/artifacts.py" manifest-v2 "$files" "$WORK/$variant/manifest.json" \
    --tag "$TAG" --commit "$COMMIT" --profile "$profile" "${rpm_flag[@]}" >/dev/null
  python3 "$HERE/artifacts.py" verify-mirror "$files" "$WORK/$variant/manifest.json" --tag "$TAG" >/dev/null
  PROFILE="$profile" COUNT="$((${#names[@]} * 2))" MANIFEST="$WORK/$variant/manifest.json" \
    python3 - <<'PY'
import json, os
doc = json.load(open(os.environ['MANIFEST']))
assert doc['schema'] == 'upeg-release-manifest/v2'
assert set(doc) == {'schema', 'tag', 'version', 'profile', 'build_commit', 'files'}
assert doc['profile'] == os.environ['PROFILE']
assert doc['build_commit'] == 'd173ad2452c2df052afb54749d42677593252998'
assert len(doc['files']) == int(os.environ['COUNT'])
assert len({item['name'] for item in doc['files']}) == len(doc['files'])
PY
}

expect_failure() {
  if "$@" >/dev/null 2>&1; then
    printf 'expected command to fail: %s\n' "$*" >&2
    exit 1
  fi
}

build_profile public
build_profile linux
build_profile full
build_profile linux-macos
build_profile public true
build_profile linux true
build_profile linux-macos true
build_profile full true

# Old manifests remain valid, while new release profiles require the RPM.
rpm="upeg-${VERSION}-1.x86_64.rpm"
mv "$WORK/linux-rpm/files/$rpm" "$WORK/missing-rpm"
expect_failure python3 "$HERE/artifacts.py" manifest-v2 "$WORK/linux-rpm/files" \
  "$WORK/linux-rpm/missing.json" --tag "$TAG" --commit "$COMMIT" --profile linux --include-rpm
mv "$WORK/missing-rpm" "$WORK/linux-rpm/files/$rpm"
expect_failure python3 "$HERE/artifacts.py" manifest-v2 "$WORK/linux-rpm/files" \
  "$WORK/linux-rpm/old-profile.json" --tag "$TAG" --commit "$COMMIT" --profile linux
mkdir "$WORK/partial-rpm"
cp "$WORK/linux-rpm/files/"* "$WORK/partial-rpm/"
rm "$WORK/partial-rpm/$rpm.sha256"
python3 - "$WORK/linux-rpm/manifest.json" "$WORK/partial-rpm.json" "$rpm.sha256" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1]))
doc['files'] = [item for item in doc['files'] if item['name'] != sys.argv[3]]
json.dump(doc, open(sys.argv[2], 'w'))
PY
expect_failure python3 "$HERE/artifacts.py" verify-mirror "$WORK/partial-rpm" \
  "$WORK/partial-rpm.json" --tag "$TAG"

# Private releases must still fail closed on missing macOS assets.
mv "$WORK/linux-macos/files/upeg-$TAG.dmg" "$WORK/private.dmg"
expect_failure python3 "$HERE/artifacts.py" manifest-v2 "$WORK/linux-macos/files" \
  "$WORK/private-missing.json" --tag "$TAG" --commit "$COMMIT" --profile linux-macos
mv "$WORK/private.dmg" "$WORK/linux-macos/files/upeg-$TAG.dmg"
# Windows artifacts cannot silently enter the reduced private profile.
printf unexpected > "$WORK/linux-macos/files/upeg-$TAG-x86_64.msix"
expect_failure python3 "$HERE/artifacts.py" manifest-v2 "$WORK/linux-macos/files" \
  "$WORK/private-extra.json" --tag "$TAG" --commit "$COMMIT" --profile linux-macos
rm "$WORK/linux-macos/files/upeg-$TAG-x86_64.msix"

# The public profile must fail closed if either desktop package is missing.
for package in "upeg-$TAG.dmg" "upeg-$TAG-x86_64.msix"; do
  mv "$WORK/public/files/$package" "$WORK/$package"
  expect_failure python3 "$HERE/artifacts.py" manifest-v2 "$WORK/public/files" \
    "$WORK/missing-package.json" --tag "$TAG" --commit "$COMMIT" --profile public
  mv "$WORK/$package" "$WORK/public/files/$package"
done

files="$WORK/public/files"
manifest="$WORK/public/manifest.json"
asset="${public[0]}"
printf changed > "$files/$asset"
expect_failure python3 "$HERE/artifacts.py" verify-mirror "$files" "$manifest" --tag "$TAG"
printf 'payload %s\n' "$asset" > "$files/$asset"
expect_failure python3 "$HERE/artifacts.py" verify-mirror "$files" "$manifest" --tag v9.9.9
mv "$files/$asset" "$WORK/missing"
expect_failure python3 "$HERE/artifacts.py" verify-mirror "$files" "$manifest" --tag "$TAG"
mv "$WORK/missing" "$files/$asset"
printf extra > "$files/extra"
expect_failure python3 "$HERE/artifacts.py" verify-mirror "$files" "$manifest" --tag "$TAG"
rm "$files/extra"
MANIFEST="$manifest" OUT="$WORK/bad.json" python3 - <<'PY'
import json, os
doc = json.load(open(os.environ['MANIFEST']))
doc['files'].append(dict(doc['files'][0]))
json.dump(doc, open(os.environ['OUT'], 'w'))
PY
expect_failure python3 "$HERE/artifacts.py" verify-mirror "$files" "$WORK/bad.json" --tag "$TAG"
ln -s "$files/$asset" "$files/extra-link"
expect_failure python3 "$HERE/artifacts.py" manifest-v2 "$files" "$WORK/unsafe.json" \
  --tag "$TAG" --commit "$COMMIT" --profile public
rm "$files/extra-link"

python3 -m unittest discover -s "$HERE" -p 'test_github_release.py' -v
printf 'v2 release manifest and GitHub release protocol tests passed\n'
