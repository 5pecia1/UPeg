#!/usr/bin/env bash
# Verify a public release-build run and stage its artifacts locally.
#
#   verify-build.sh --repo-id ID --run-id N --tag TAG --out DIR
#
# GH_TOKEN must be a read-capable token on the public repository with
# contents:read AND actions:read (artifact download). The script saves the
# raw API responses, downloads every expected artifact, and then lets
# artifacts.py verify-run enforce identity, commit, tag and content.
# On success DIR/verified.json and DIR/files/ are the only hand-off.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="5pecia1/UPeg"
REPO_ID="" RUN_ID="" TAG="" OUT=""

while [ $# -gt 0 ]; do
  case "$1" in
    --repo-id) REPO_ID="$2"; shift 2 ;;
    --run-id) RUN_ID="$2"; shift 2 ;;
    --tag) TAG="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --repo) REPO="$2"; shift 2 ;;
    *) echo "verify-build: unknown argument $1" >&2; exit 2 ;;
  esac
done
if [ -z "$REPO_ID" ] || [ -z "$RUN_ID" ] || [ -z "$TAG" ] || [ -z "$OUT" ]; then
  echo "usage: $0 --repo-id ID --run-id N --tag TAG --out DIR" >&2
  exit 2
fi
command -v gh >/dev/null || { echo "verify-build: gh CLI required" >&2; exit 1; }
command -v python3 >/dev/null || { echo "verify-build: python3 required" >&2; exit 1; }
: "${GH_TOKEN:?GH_TOKEN must authorize the public repository read}"

mkdir -p "$OUT/artifacts" "$OUT/files" "$OUT/manifest"

gh api "repos/$REPO" > "$OUT/repo.json"
gh api "repos/$REPO/actions/runs/$RUN_ID" > "$OUT/run.json"
gh api "repos/$REPO/actions/runs/$RUN_ID/jobs?per_page=100" > "$OUT/jobs.json"
gh api "repos/$REPO/actions/runs/$RUN_ID/artifacts?per_page=100" > "$OUT/artifacts.json"

names="$(python3 -c 'import json,sys
print("\n".join(a["name"] for a in json.load(open(sys.argv[1]))["artifacts"]))' \
  "$OUT/artifacts.json")"
while IFS= read -r name; do
  [ -n "$name" ] || continue
  gh run download "$RUN_ID" --repo "$REPO" --name "$name" \
    --dir "$OUT/artifacts/$name"
done <<< "$names"

# Flatten payloads; the manifest artifact keeps its own directory.
find "$OUT/artifacts" -mindepth 2 -type f -not -path '*/release-manifest/*' \
  -exec cp {} "$OUT/files/" \;
if [ -f "$OUT/artifacts/release-manifest/release-manifest.json" ]; then
  cp "$OUT/artifacts/release-manifest/release-manifest.json" "$OUT/manifest/"
fi

python3 "$HERE/artifacts.py" verify-run "$OUT" \
  --repo-id "$REPO_ID" --run-id "$RUN_ID" --tag "$TAG"

version="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["version"])' \
  "$OUT/verified.json")"
commit="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["commit"])' \
  "$OUT/verified.json")"
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  {
    echo "tag=$TAG"
    echo "version=$version"
    echo "commit=$commit"
    echo "files_dir=$OUT/files"
  } >> "$GITHUB_OUTPUT"
fi
echo "[PASS] verified run $RUN_ID → $TAG at $commit; files in $OUT/files"
