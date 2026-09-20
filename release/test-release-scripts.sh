#!/usr/bin/env bash
# Tests for verify-build.sh / publish-release.sh / artifacts.py using a
# stub `gh` on PATH and synthetic artifact bundles. No network, no releases.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT
PASS=0

REPO_ID=1368191530
RUN_ID=424242
COMMIT=d173ad2452c2df052afb54749d42677593252998
TAG=v0.1.2
VERSION=0.1.2

BIN="$WORK/bin"; mkdir -p "$BIN"
cat > "$BIN/gh" <<'EOF'
#!/usr/bin/env bash
# Stub gh: serves canned API JSON and artifact dirs from $FAKE_GH_DATA.
set -euo pipefail
data="${FAKE_GH_DATA:?}"
cmd="$1"; shift
case "$cmd" in
  api)
    path="$1"; shift || true
    case "$path" in
      repos/*/actions/runs/*/jobs*)    cat "$data/jobs.json" ;;
      repos/*/actions/runs/*/artifacts*) cat "$data/artifacts.json" ;;
      repos/*/actions/runs/*)          cat "$data/run.json" ;;
      repos/*/git/ref/tags/*)
        tag="${path##*/}"
        if [ -f "$data/tags/$tag.sha" ]; then
          printf '{"object":{"sha":"%s"}}\n' "$(cat "$data/tags/$tag.sha")"
        else exit 1; fi ;;
      repos/*/releases/tags/*)
        tag="${path##*/}"
        if [ -f "$data/releases/$tag.json" ]; then
          cat "$data/releases/$tag.json"
        else exit 1; fi ;;
      repos/*) cat "$data/repo.json" ;;
      *) echo "stub gh: unhandled api $path" >&2; exit 1 ;;
    esac ;;
  run)
    sub="$1"; shift
    [ "$sub" = download ] || { echo "stub gh: unhandled run $sub" >&2; exit 1; }
    name="" dir=""
    while [ $# -gt 0 ]; do
      case "$1" in
        --name|-n) name="$2"; shift 2 ;;
        --dir|-D) dir="$2"; shift 2 ;;
        --repo|-R) shift 2 ;;
        *) shift ;;
      esac
    done
    [ -d "$data/artifacts/$name" ] || { echo "stub gh: no artifact $name" >&2; exit 1; }
    mkdir -p "$dir"; cp -a "$data/artifacts/$name/." "$dir/" ;;
  release)
    sub="$1"; shift
    case "$sub" in
      create)
        tag="$1"; shift
        echo "release create $tag $*" >> "$data/release-create.log"
        commit="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["head_sha"])' "$data/run.json")"
        printf '%s' "$commit" > "$data/tags/$tag.sha"
        python3 - "$data/releases/$tag.json" "$tag" "$commit" "$@" <<'PYEOF'
import json, os, sys
flags_with_values = {'--repo', '-R', '--target', '--title', '--notes-file',
                     '-F', '-n', '-t'}
args, skip, files = sys.argv[4:], False, []
for a in args:
    if skip:
        skip = False
    elif a in flags_with_values:
        skip = True
    elif a.startswith('-'):
        continue
    elif os.path.isfile(a):
        files.append(a)
rel = {"tagName": sys.argv[2], "targetCommitish": sys.argv[3],
       "assets": [{"name": os.path.basename(a)} for a in files]}
json.dump(rel, open(sys.argv[1], 'w'))
PYEOF
        ;;
      *) echo "stub gh: unhandled release $sub" >&2; exit 1 ;;
    esac ;;
  *) echo "stub gh: unhandled $cmd" >&2; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"

DATA="$WORK/ghdata"
mkdir -p "$DATA/artifacts" "$DATA/tags" "$DATA/releases"

# Fixture: a complete good run + artifacts set.
build_fixture() {
  rm -rf "$DATA/artifacts" "$DATA/tags" "$DATA/releases"
  mkdir -p "$DATA/artifacts" "$DATA/tags" "$DATA/releases"
  printf '{"id":%s,"full_name":"5pecia1/UPeg"}' "$REPO_ID" > "$DATA/repo.json"
  cat > "$DATA/run.json" <<EOF
{"id":$RUN_ID,"path":".github/workflows/release.yml","event":"workflow_dispatch",
 "head_branch":"main","head_sha":"$COMMIT","status":"completed","conclusion":"success",
 "run_attempt":1,"repository":{"id":$REPO_ID},"head_repository":{"id":$REPO_ID}}
EOF
  printf '{"jobs":[' > "$DATA/jobs.json"
  sep=""; for j in preflight cli-linux cli-macos cli-windows flutter-linux manifest; do
    printf '%s{"name":"%s","conclusion":"success"}' "$sep" "$j" >> "$DATA/jobs.json"; sep=","
  done; printf ']}' >> "$DATA/jobs.json"
  printf '{"artifacts":[' > "$DATA/artifacts.json"
  sep=""; for a in cli-linux cli-macos cli-windows flutter-linux flutter-web release-manifest; do
    printf '%s{"name":"%s","expired":false}' "$sep" "$a" >> "$DATA/artifacts.json"; sep=","
  done; printf ']}' >> "$DATA/artifacts.json"

  # Payload files per artifact (small synthetic content).
  local dist="$WORK/dist"; rm -rf "$dist"; mkdir -p "$dist"
  python3 "$HERE/artifacts.py" expected-files "$VERSION" "$TAG" | while read -r f; do
    printf 'payload %s\n' "$f" > "$dist/$f"
  done
  UPEG_VERSION="$VERSION" UPEG_TAG="$TAG" UPEG_COMMIT="$COMMIT" \
    UPEG_RUN_ID="$RUN_ID" UPEG_RUN_ATTEMPT=1 UPEG_REPO=5pecia1/UPeg \
    python3 "$HERE/artifacts.py" manifest "$dist" "$WORK/release-manifest.json" >/dev/null
  mkdir -p "$DATA/artifacts/release-manifest"
  cp "$WORK/release-manifest.json" "$DATA/artifacts/release-manifest/"
  for a in cli-linux cli-macos cli-windows flutter-linux flutter-web; do
    mkdir -p "$DATA/artifacts/$a"
  done
  for p in "$dist"/*; do
    f="$(basename "$p")"
    case "$f" in
      *x86_64-unknown*) cp "$p" "$DATA/artifacts/cli-linux/" ;;
      *aarch64*)        cp "$p" "$DATA/artifacts/cli-macos/" ;;
      *windows*)        cp "$p" "$DATA/artifacts/cli-windows/" ;;
      *.deb*|*.AppImage*) cp "$p" "$DATA/artifacts/flutter-linux/" ;;
      *-web.tar.gz*)    cp "$p" "$DATA/artifacts/flutter-web/" ;;
    esac
  done
}

ok() { PASS=$((PASS + 1)); printf 'ok %d - %s\n' "$PASS" "$1"; }
fail() { printf 'FAIL - %s\n' "$1" >&2; exit 1; }

build_fixture
export PATH="$BIN:$PATH" GH_TOKEN=fake

# 1. happy path: verify-build verifies and publish-release creates.
OUT="$WORK/out1"
FAKE_GH_DATA="$DATA" bash "$HERE/verify-build.sh" \
  --repo-id "$REPO_ID" --run-id "$RUN_ID" --tag "$TAG" --out "$OUT" >/dev/null
[ -f "$OUT/verified.json" ] || fail "verified.json not written"
FAKE_GH_DATA="$DATA" bash "$HERE/publish-release.sh" --bundle "$OUT" >/dev/null
grep -q "release create $TAG" "$DATA/release-create.log" \
  || fail "gh release create not called"
ok "happy path: verify + publish"

# 2. wrong repository ID.
if FAKE_GH_DATA="$DATA" bash "$HERE/verify-build.sh" \
    --repo-id 999 --run-id "$RUN_ID" --tag "$TAG" --out "$WORK/out2" >/dev/null 2>&1; then
  fail "accepted wrong repo id"
fi
ok "refuses wrong repository ID"

# 3. wrong event/branch.
python3 - "$DATA/run.json" <<'EOF'
import json,sys
d=json.load(open(sys.argv[1])); d['event']='push'; json.dump(d,open(sys.argv[1],'w'))
EOF
if FAKE_GH_DATA="$DATA" bash "$HERE/verify-build.sh" \
    --repo-id "$REPO_ID" --run-id "$RUN_ID" --tag "$TAG" --out "$WORK/out3" >/dev/null 2>&1; then
  fail "accepted push-event run"
fi
ok "refuses non-dispatch event"

# 4. tag mismatch.
build_fixture
if FAKE_GH_DATA="$DATA" bash "$HERE/verify-build.sh" \
    --repo-id "$REPO_ID" --run-id "$RUN_ID" --tag v9.9.9 --out "$WORK/out4" >/dev/null 2>&1; then
  fail "accepted mismatched tag"
fi
ok "refuses tag that differs from the manifest"

# 5. unexpected artifact name.
build_fixture
printf 'x' > "$DATA/artifacts/cli-linux/extra.txt"
if FAKE_GH_DATA="$DATA" bash "$HERE/verify-build.sh" \
    --repo-id "$REPO_ID" --run-id "$RUN_ID" --tag "$TAG" --out "$WORK/out5" >/dev/null 2>&1; then
  fail "accepted unexpected file"
fi
ok "refuses unexpected artifact content"

# 6. corrupted file content.
build_fixture
printf 'tampered' > "$DATA/artifacts/cli-linux/upeg-${TAG}-x86_64-unknown-linux-gnu.tar.gz"
if FAKE_GH_DATA="$DATA" bash "$HERE/verify-build.sh" \
    --repo-id "$REPO_ID" --run-id "$RUN_ID" --tag "$TAG" --out "$WORK/out6" >/dev/null 2>&1; then
  fail "accepted corrupted artifact"
fi
ok "refuses corrupted artifact content"

# 7. tag already exists at a different commit → publish refuses.
build_fixture
OUT="$WORK/out7"
FAKE_GH_DATA="$DATA" bash "$HERE/verify-build.sh" \
  --repo-id "$REPO_ID" --run-id "$RUN_ID" --tag "$TAG" --out "$OUT" >/dev/null
printf 'aaaaaa2452c2df052afb54749d42677593252998' > "$DATA/tags/$TAG.sha"
if FAKE_GH_DATA="$DATA" bash "$HERE/publish-release.sh" --bundle "$OUT" >/dev/null 2>&1; then
  fail "published over a moved tag"
fi
ok "refuses a tag that points at another commit"

# 8. identical existing release → idempotent no-op.
printf '%s' "$COMMIT" > "$DATA/tags/$TAG.sha"
python3 - "$DATA/releases/$TAG.json" "$TAG" "$COMMIT" "$OUT/verified.json" <<'EOF'
import json,sys
v=json.load(open(sys.argv[4]))
rel={"tagName":sys.argv[2],"targetCommitish":sys.argv[3],
     "assets":[{"name":n} for n in v["files"]]}
json.dump(rel,open(sys.argv[1],'w'))
EOF
FAKE_GH_DATA="$DATA" bash "$HERE/publish-release.sh" --bundle "$OUT" >/dev/null
ok "identical existing release is a no-op"

printf '%d tests passed\n' "$PASS"
