#!/usr/bin/env bash
# Exercise the public, Linux, and full v2 profiles with literal asset fixtures.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT
TAG=v0.1.2
VERSION=0.1.2
COMMIT=d173ad2452c2df052afb54749d42677593252998

public=(
  "upeg-$TAG-x86_64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-aarch64-apple-darwin.tar.gz"
  "upeg-$TAG-x86_64-pc-windows-msvc.zip"
  "upeg_${VERSION}_amd64.deb"
  "upeg-$TAG-x86_64.AppImage"
  "upeg-$TAG-web.tar.gz"
)
linux=(
  "upeg-$TAG-x86_64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-aarch64-unknown-linux-gnu.tar.gz"
  "upeg-$TAG-web.tar.gz"
  "upeg_${VERSION}_amd64.deb"
  "upeg-$TAG-x86_64.AppImage"
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

build_profile() {
  local profile="$1" payload
  local -a names
  case "$profile" in
    public) names=("${public[@]}") ;;
    linux) names=("${linux[@]}") ;;
    full) names=("${full[@]}") ;;
  esac
  local files="$WORK/$profile/files"
  mkdir -p "$files"
  for payload in "${names[@]}"; do
    printf 'payload %s\n' "$payload" > "$files/$payload"
    ( cd "$files" && sha256sum -b "$payload" > "$payload.sha256" )
  done
  python3 "$HERE/artifacts.py" manifest-v2 "$files" "$WORK/$profile/manifest.json" \
    --tag "$TAG" --commit "$COMMIT" --profile "$profile" >/dev/null
  python3 "$HERE/artifacts.py" verify-mirror "$files" "$WORK/$profile/manifest.json" --tag "$TAG" >/dev/null
  PROFILE="$profile" COUNT="$((${#names[@]} * 2))" MANIFEST="$WORK/$profile/manifest.json" \
    python3 - <<'PY'
import json, os
doc = json.load(open(os.environ['MANIFEST']))
assert doc['schema'] == 'upeg-release-manifest/v2'
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
