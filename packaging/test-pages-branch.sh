#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$HERE/pages-branch.sh"

[ "$(bash "$SCRIPT" release '0.5.1+rc.2' abcdef0123456789 x86_64-unknown-linux-gnu)" = \
  'release-879ca09e54bc7d385bf3' ]
[ "$(bash "$SCRIPT" release 'v0.5.1+rc.2' abcdef0123456789 x86_64-unknown-linux-gnu)" = \
  'release-879ca09e54bc7d385bf3' ]
[ "$(bash "$SCRIPT" release '0.5.1+rc.2' ABCDEF0123456789 x86_64-unknown-linux-gnu)" = \
  'release-879ca09e54bc7d385bf3' ]
[ "$(bash "$SCRIPT" preview ABCDEF0123456789)" = 'preview-abcdef012345' ]
[ "$(bash "$SCRIPT" pr 42)" = 'pr-42' ]
[ "$(bash "$SCRIPT" catalog-url Example_Pages 0.5.1 abcdef0123456789 x86_64-unknown-linux-gnu)" = \
  'https://release-8b80281b62d7314ee4a3.example-pages.pages.dev/catalog.json' ]
[ "$(bash "$SCRIPT" catalog-url Example_Pages v0.5.1 abcdef0123456789 x86_64-unknown-linux-gnu)" = \
  'https://release-8b80281b62d7314ee4a3.example-pages.pages.dev/catalog.json' ]
seen=()
for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu aarch64-apple-darwin x86_64-apple-darwin x86_64-pc-windows-msvc; do
  branch="$(bash "$SCRIPT" release v0.5.2 193157d77d4864b0175a4e9e6d3ed6514b3de61a "$target")"
  [ "${#branch}" -eq 28 ]
  for existing in "${seen[@]}"; do [ "$existing" != "$branch" ]; done
  seen+=("$branch")
done
long_version='0.5.2+candidate.with.a.long.version.suffix'
long_branch="$(bash "$SCRIPT" release "$long_version" 193157d77d4864b0175a4e9e6d3ed6514b3de61a x86_64-unknown-linux-gnu)"
[ "${#long_branch}" -eq 28 ]
[ "$long_branch" != "$(bash "$SCRIPT" release 0.5.2 193157d77d4864b0175a4e9e6d3ed6514b3de61a x86_64-unknown-linux-gnu)" ]
[ "$(bash "$SCRIPT" release 0.5.2 193157d77d4864b0175a4e9e6d3ed6514b3de61b x86_64-unknown-linux-gnu)" != \
  "$(bash "$SCRIPT" release 0.5.2 193157d77d4864b0175a4e9e6d3ed6514b3de61a x86_64-unknown-linux-gnu)" ]
if bash "$SCRIPT" release 0.5.1 invalid x86_64 >/dev/null 2>&1; then
  printf 'expected an invalid release SHA to fail\n' >&2
  exit 1
fi
printf 'pages branch naming tests passed\n'
