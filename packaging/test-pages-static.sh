#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$HERE/pages-static.sh"
WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT
ROOT="$WORK/web"
mkdir -p "$ROOT/toolkits/sample/1.2.3"
printf '<!doctype html>\n' > "$ROOT/index.html"
printf 'export default function init() {}\n' > "$ROOT/toolkits/sample/1.2.3/upeg_toolkit_sample.js"
printf '\0asmtest' > "$ROOT/toolkits/sample/1.2.3/upeg_toolkit_sample_bg.wasm"

js="$ROOT/toolkits/sample/1.2.3/upeg_toolkit_sample.js"
wasm="$ROOT/toolkits/sample/1.2.3/upeg_toolkit_sample_bg.wasm"
js_size="$(wc -c < "$js" | tr -d '[:space:]')"
wasm_size="$(wc -c < "$wasm" | tr -d '[:space:]')"
js_hash="$(sha256sum "$js" | awk '{print $1}')"
wasm_hash="$(sha256sum "$wasm" | awk '{print $1}')"

jq -n \
  --arg js_hash "$js_hash" --argjson js_size "$js_size" \
  --arg wasm_hash "$wasm_hash" --argjson wasm_size "$wasm_size" \
  '{schema_version:1, app_version:"1.2.3", abi_digest:("a" * 64), catalog_digest:("b" * 64), toolkits:[{id:"sample", version:"1.2.3", web:{js:"/toolkits/sample/1.2.3/upeg_toolkit_sample.js", js_sha256:$js_hash, js_size:$js_size, wasm:"/toolkits/sample/1.2.3/upeg_toolkit_sample_bg.wasm", sha256:$wasm_hash, size:$wasm_size}, native:{}, requires_host:false}]}' \
  > "$ROOT/toolkits/catalog.json"

"$SCRIPT" prepare "$ROOT" 1.2.3 >/dev/null
grep -q 'Cross-Origin-Embedder-Policy: require-corp' "$ROOT/_headers"
grep -q 'Content-Type: application/wasm' "$ROOT/_headers"
test "$(grep -Fc 'Cross-Origin-Resource-Policy: same-origin' "$ROOT/_headers")" -eq 1
test ! -e "$ROOT/_redirects"
test ! -e "$ROOT/404.html"

printf '/* /index.html 200\n' > "$ROOT/_redirects"
if "$SCRIPT" prepare "$ROOT" 1.2.3 >/dev/null 2>&1; then
  printf 'expected _redirects to fail\n' >&2
  exit 1
fi
rm "$ROOT/_redirects"

printf '<!doctype html>\n' > "$ROOT/404.html"
if "$SCRIPT" prepare "$ROOT" 1.2.3 >/dev/null 2>&1; then
  printf 'expected 404.html to fail\n' >&2
  exit 1
fi
rm "$ROOT/404.html"

printf 'changed\n' >> "$js"
if "$SCRIPT" prepare "$ROOT" 1.2.3 >/dev/null 2>&1; then
  printf 'expected changed JS digest to fail\n' >&2
  exit 1
fi

printf 'export { value } from "./helper.js";\n' > "$js"
js_size="$(wc -c < "$js" | tr -d '[:space:]')"
js_hash="$(sha256sum "$js" | awk '{print $1}')"
jq --arg js_hash "$js_hash" --argjson js_size "$js_size" \
  '.toolkits[0].web.js_sha256 = $js_hash | .toolkits[0].web.js_size = $js_size' \
  "$ROOT/toolkits/catalog.json" > "$ROOT/toolkits/catalog.next.json"
mv "$ROOT/toolkits/catalog.next.json" "$ROOT/toolkits/catalog.json"
if "$SCRIPT" prepare "$ROOT" 1.2.3 >/dev/null 2>&1; then
  printf 'expected relative JS import to fail\n' >&2
  exit 1
fi

printf 'pages static packaging tests passed\n'
