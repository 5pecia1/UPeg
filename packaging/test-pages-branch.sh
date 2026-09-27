#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$HERE/pages-branch.sh"

[ "$(bash "$SCRIPT" release '0.5.1+rc.2' abcdef0123456789 x86_64-unknown-linux-gnu)" = \
  'release-0-5-1-rc-2-abcdef01-x86-64-unknown-linux-gnu' ]
[ "$(bash "$SCRIPT" preview ABCDEF0123456789)" = 'preview-abcdef012345' ]
[ "$(bash "$SCRIPT" pr 42)" = 'pr-42' ]
[ "$(bash "$SCRIPT" catalog-url Example_Pages 0.5.1 abcdef0123456789 x86_64-unknown-linux-gnu)" = \
  'https://release-0-5-1-abcdef01-x86-64-unknown-linux-gnu.example-pages.pages.dev/catalog.json' ]
if bash "$SCRIPT" release 0.5.1 invalid x86_64 >/dev/null 2>&1; then
  printf 'expected an invalid release SHA to fail\n' >&2
  exit 1
fi
printf 'pages branch naming tests passed\n'
