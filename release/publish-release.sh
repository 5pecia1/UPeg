#!/usr/bin/env bash
# Create the public tag + GitHub Release from a verified build bundle.
#
#   publish-release.sh --bundle DIR
#
# DIR is the --out directory produced by verify-build.sh; it must contain
# verified.json and files/. GH_TOKEN needs contents:write on the public
# repository — tag creation on it is restricted to the release App anyway.
# Refuses to touch an existing tag or a release whose contents differ;
# re-running an identical publish is a no-op.
set -euo pipefail

REPO="5pecia1/UPeg"
BUNDLE=""

while [ $# -gt 0 ]; do
  case "$1" in
    --bundle) BUNDLE="$2"; shift 2 ;;
    --repo) REPO="$2"; shift 2 ;;
    *) echo "publish-release: unknown argument $1" >&2; exit 2 ;;
  esac
done
[ -n "$BUNDLE" ] || { echo "usage: $0 --bundle DIR" >&2; exit 2; }
command -v gh >/dev/null || { echo "publish-release: gh CLI required" >&2; exit 1; }
command -v python3 >/dev/null || { echo "publish-release: python3 required" >&2; exit 1; }
: "${GH_TOKEN:?GH_TOKEN must authorize contents:write on the public repository}"

VERIFIED="$BUNDLE/verified.json"
FILES_DIR="$BUNDLE/files"
if [ ! -f "$VERIFIED" ] || [ ! -d "$FILES_DIR" ]; then
  echo "publish-release: run verify-build.sh first (missing $VERIFIED/$FILES_DIR)" >&2
  exit 1
fi

read -r TAG COMMIT RUN_ID RUN_ATTEMPT < <(python3 -c '
import json, sys
doc = json.load(open(sys.argv[1]))
print(doc["tag"], doc["commit"], doc["run_id"], doc["run_attempt"])' "$VERIFIED")
EXPECTED="$(python3 -c 'import json,sys
print("\n".join(json.load(open(sys.argv[1]))["files"]))' "$VERIFIED")"
ACTUAL="$(cd "$FILES_DIR" && find . -maxdepth 1 -type f -printf '%f\n' | LC_ALL=C sort)"
[ "$EXPECTED" = "$ACTUAL" ] \
  || { echo "publish-release: files/ differs from verified.json" >&2; exit 1; }

release_state() {
  # Prints: tag_target<TAB>release_assets (empty tag_target when no tag).
  python3 - "$1" "$2" <<'PYEOF'
import json, subprocess, sys
repo, tag = sys.argv[1], sys.argv[2]
def api(path):
    try:
        return subprocess.run(['gh', 'api', path, '-H', 'Accept: application/vnd.github+json'],
                              capture_output=True, text=True, check=True).stdout
    except subprocess.CalledProcessError:
        return ''
raw = api(f'repos/{repo}/git/ref/tags/{tag}')
target = json.loads(raw)['object']['sha'] if raw else ''
rel = api(f'repos/{repo}/releases/tags/{tag}')
assets = sorted(a['name'] for a in json.loads(rel)['assets']) if rel else []
print(target, '\t'.join(assets), sep='\t')
PYEOF
}

state="$(release_state "$REPO" "$TAG")"
TAG_TARGET="${state%%$'\t'*}"
REL_ASSETS="${state#*$'\t'}"

if [ -n "$TAG_TARGET" ]; then
  [ "$TAG_TARGET" = "$COMMIT" ] \
    || { echo "publish-release: tag $TAG already points at $TAG_TARGET" >&2; exit 1; }
  [ -n "$REL_ASSETS" ] \
    || { echo "publish-release: tag $TAG exists without a release" >&2; exit 1; }
  if [ "$REL_ASSETS" = "$(printf '%s' "$EXPECTED" | tr '\n' '\t' | sed 's/\t$//')" ]; then
    echo "[PASS] $TAG already published with the verified assets; nothing to do"
    exit 0
  fi
  echo "publish-release: release $TAG exists with different assets; refusing" >&2
  exit 1
fi
if [ -n "$REL_ASSETS" ]; then
  echo "publish-release: a release named $TAG exists without its tag; refusing" >&2
  exit 1
fi

notes="$(mktemp)"
trap 'rm -f "$notes"' EXIT
cat > "$notes" <<EOF
upeg $TAG — built from public source.

Source: $REPO commit $COMMIT (main)
Build: release workflow run $RUN_ID attempt $RUN_ATTEMPT
Artifacts are unsigned; verify each download against its .sha256 sidecar or
the run's release-manifest artifact.
EOF

mapfile -t file_list < <(printf '%s\n' "$EXPECTED")
paths=()
for name in "${file_list[@]}"; do paths+=("$FILES_DIR/$name"); done
gh release create "$TAG" --repo "$REPO" --target "$COMMIT" \
  --title "$TAG" --notes-file "$notes" "${paths[@]}"

state="$(release_state "$REPO" "$TAG")"
REL_ASSETS="${state#*$'\t'}"
[ "$REL_ASSETS" = "$(printf '%s' "$EXPECTED" | tr '\n' '\t' | sed 's/\t$//')" ] \
  || { echo "publish-release: published release assets differ" >&2; exit 1; }
echo "[PASS] published $TAG at $COMMIT with ${#file_list[@]} assets"
