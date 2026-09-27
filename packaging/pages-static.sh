#!/usr/bin/env bash
# Prepare and verify the static UPeg web bundle for Cloudflare Pages.
#
# This deliberately has no Cloudflare credentials or network calls. The CI
# workflow owns deployment; this script makes the deploy directory verifiable
# on a developer machine and in release jobs.
set -euo pipefail

die() { printf 'pages-static: %s\n' "$*" >&2; exit 1; }

usage() {
  die "usage: $0 {prepare BUNDLE_DIR VERSION|limits BUNDLE_DIR}"
}

check_limits() {
  local root="$1" count path bytes
  count="$(find "$root" -type f -print | wc -l | tr -d '[:space:]')"
  [ "$count" -le 20000 ] || die "bundle has $count files; Cloudflare Pages Free allows at most 20000"
  while IFS= read -r -d '' path; do
    bytes="$(wc -c < "$path" | tr -d '[:space:]')"
    [ "$bytes" -le $((25 * 1024 * 1024)) ] \
      || die "${path#"$root"/} is $bytes bytes; Cloudflare Pages Free allows 25 MiB per file"
  done < <(find "$root" -type f -print0)
}

check_toolkits() {
  local root="$1" version="$2"
  ROOT="$root" VERSION="$version" python3 - <<'PY'
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import sys

root = Path(os.environ['ROOT']).resolve()
version = os.environ['VERSION']
catalog_path = root / 'toolkits' / 'catalog.json'

def fail(message: str) -> None:
    raise SystemExit(f'pages-static: {message}')

try:
    catalog = json.loads(catalog_path.read_text(encoding='utf-8'))
except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
    fail(f'invalid toolkit catalog: {error}')

if not isinstance(catalog, dict):
    fail('toolkit catalog must be a JSON object')
if catalog.get('schema_version') != 1:
    fail('toolkit catalog schema_version must be 1')
if catalog.get('app_version') != version:
    fail(f"toolkit catalog app_version is {catalog.get('app_version')!r}, want {version!r}")
for field in ('abi_digest', 'catalog_digest'):
    if not isinstance(catalog.get(field), str) or not re.fullmatch(r'[0-9a-f]{64}', catalog[field]):
        fail(f'toolkit catalog {field} must be a lowercase SHA-256 digest')
toolkits = catalog.get('toolkits')
if not isinstance(toolkits, list):
    fail('toolkit catalog toolkits must be an array')

seen = set()
selected = 0
digest_pattern = re.compile(r'[0-9a-f]{64}')
id_pattern = re.compile(r'[A-Za-z0-9][A-Za-z0-9._-]*')

def artifact_path(value: object, toolkit_id: str, toolkit_version: str, field: str) -> Path:
    if not isinstance(value, str):
        fail(f'{toolkit_id}.{toolkit_version} web.{field} must be a string path')
    prefix = f'/toolkits/{toolkit_id}/{toolkit_version}/'
    if not value.startswith(prefix) or '?' in value or '#' in value:
        fail(f'{toolkit_id}.{toolkit_version} web.{field} must be a versioned same-origin path')
    relative = PurePosixPath(value.removeprefix('/'))
    if relative.is_absolute() or '..' in relative.parts or relative.parts[:1] != ('toolkits',):
        fail(f'{toolkit_id}.{toolkit_version} web.{field} has an unsafe path')
    path = root.joinpath(*relative.parts)
    if not path.is_file() or path.is_symlink():
        fail(f'{toolkit_id}.{toolkit_version} web.{field} is missing or not a regular file')
    return path

for entry in toolkits:
    if not isinstance(entry, dict):
        fail('toolkit catalog entries must be objects')
    toolkit_id, toolkit_version = entry.get('id'), entry.get('version')
    if not isinstance(toolkit_id, str) or not id_pattern.fullmatch(toolkit_id):
        fail('toolkit catalog entry has an invalid id')
    if not isinstance(toolkit_version, str) or not toolkit_version:
        fail(f'{toolkit_id} has an invalid version')
    key = (toolkit_id, toolkit_version)
    if key in seen:
        fail(f'toolkit catalog repeats {toolkit_id}@{toolkit_version}')
    seen.add(key)
    native = entry.get('native')
    if not isinstance(native, dict) or native:
        fail(f'{toolkit_id}.{toolkit_version} Pages catalog must use an empty native map')
    web = entry.get('web')
    if web is None:
        continue
    if not isinstance(web, dict):
        fail(f'{toolkit_id}.{toolkit_version} web must be an object or null')
    selected += 1
    js = artifact_path(web.get('js'), toolkit_id, toolkit_version, 'js')
    wasm = artifact_path(web.get('wasm'), toolkit_id, toolkit_version, 'wasm')
    if js.suffix != '.js' or wasm.suffix != '.wasm':
        fail(f'{toolkit_id}.{toolkit_version} web artifact extensions must be .js and .wasm')
    for label, path, hash_key, size_key in (
        ('JS', js, 'js_sha256', 'js_size'),
        ('WASM', wasm, 'sha256', 'size'),
    ):
        expected_hash, expected_size = web.get(hash_key), web.get(size_key)
        if not isinstance(expected_hash, str) or not digest_pattern.fullmatch(expected_hash):
            fail(f'{toolkit_id}.{toolkit_version} {label} digest is invalid')
        if type(expected_size) is not int or expected_size <= 0:
            fail(f'{toolkit_id}.{toolkit_version} {label} size is invalid')
        data = path.read_bytes()
        if len(data) != expected_size or hashlib.sha256(data).hexdigest() != expected_hash:
            fail(f'{toolkit_id}.{toolkit_version} {label} bytes do not match catalog')
    if not wasm.read_bytes().startswith(b'\x00asm'):
        fail(f'{toolkit_id}.{toolkit_version} WASM artifact has no WebAssembly header')
    try:
        source = js.read_text(encoding='utf-8')
    except UnicodeDecodeError:
        fail(f'{toolkit_id}.{toolkit_version} JS glue is not UTF-8')
    relative_import = re.compile(
        r"(?:^|[;\n])\s*(?:import|export)\s+(?:[^;\n]*?\s+from\s+)?['\"](?:\./|\.\./)",
        re.MULTILINE,
    )
    if relative_import.search(source):
        fail(f'{toolkit_id}.{toolkit_version} JS glue has a forbidden relative import')

if selected == 0:
    fail('toolkit catalog selects no web toolkit; Pages artifacts must be complete')
PY
}

write_hosting_files() {
  local root="$1"
  cat > "$root/_headers" <<'EOF'
/*
  Cross-Origin-Opener-Policy: same-origin
  Cross-Origin-Embedder-Policy: require-corp
  Cross-Origin-Resource-Policy: same-origin
  X-Content-Type-Options: nosniff

/index.html
  Cache-Control: no-cache, no-store, must-revalidate

/flutter_bootstrap.js
  Content-Type: text/javascript; charset=utf-8
  Cache-Control: no-cache, must-revalidate

/main.dart.js
  Content-Type: text/javascript; charset=utf-8
  Cache-Control: no-cache, must-revalidate

/flutter.js
  Content-Type: text/javascript; charset=utf-8
  Cache-Control: no-cache, must-revalidate

/upeg_service_worker.js
  Content-Type: text/javascript; charset=utf-8
  Cache-Control: no-cache, must-revalidate

/manifest.json
  Cache-Control: no-cache

/toolkits/catalog.json
  Content-Type: application/json; charset=utf-8
  Cache-Control: no-cache, must-revalidate

/toolkits/*.js
  Content-Type: text/javascript; charset=utf-8
  Cache-Control: public, max-age=31536000, immutable

/toolkits/*.wasm
  Content-Type: application/wasm
  Cache-Control: public, max-age=31536000, immutable

/pkg/*.wasm
  Content-Type: application/wasm
  Cache-Control: no-cache, must-revalidate
EOF
}

prepare() {
  local root="$1" version="$2"
  if [ -z "$root" ] || [ -z "$version" ]; then
    usage
  fi
  [ -d "$root" ] || die "bundle directory does not exist: $root"
  [ -f "$root/index.html" ] || die "bundle has no index.html"
  [ ! -e "$root/_redirects" ] || die 'bundle must not contain _redirects; Pages provides the SPA fallback'
  [ ! -e "$root/404.html" ] || die 'bundle must not contain 404.html; Pages provides the SPA fallback'
  if find "$root" -type l -print -quit | grep -q .; then
    die 'bundle contains a symlink'
  fi
  check_toolkits "$root" "$version"
  write_hosting_files "$root"
  check_limits "$root"
  printf 'pages-static: verified %s for version %s\n' "$root" "$version"
}

case "${1:-}" in
  prepare) prepare "${2:-}" "${3:-}" ;;
  limits)
    [ -d "${2:-}" ] || die "bundle directory does not exist: ${2:-}"
    if find "$2" -type l -print -quit | grep -q .; then
      die 'bundle contains a symlink'
    fi
    check_limits "$2"
    ;;
  *) usage ;;
esac
