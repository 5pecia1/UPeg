#!/usr/bin/env bash
# Produce the exact Cloudflare Pages branch alias used in catalog URLs and
# Wrangler deployment. Keep this normalization independent of Wrangler: Pages
# normalizes punctuation itself, which otherwise makes a catalog URL differ
# from the deployed alias. A release run shortened a longer alias, so release
# branches stay at 28 characters and deployment checks the resulting URL.
set -euo pipefail

die() { printf 'pages-branch: %s\n' "$*" >&2; exit 1; }

normalize() {
  local raw="$1" normalized
  normalized="$(printf '%s' "$raw" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//')"
  [ -n "$normalized" ] || die 'branch has no usable characters'
  printf '%s\n' "$normalized"
}

finish() {
  local branch="$1"
  [ "${#branch}" -le 28 ] || die "branch exceeds 28 characters: $branch"
  printf '%s\n' "$branch"
}

sha256_hex() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | cut -d ' ' -f1
  else
    shasum -a 256 | cut -d ' ' -f1
  fi
}

case "${1:-}" in
  release)
    [ "$#" -eq 4 ] || die 'usage: release VERSION COMMIT TARGET'
    [[ "$3" =~ ^[0-9a-fA-F]{8,40}$ ]] || die 'release commit must be a hexadecimal SHA'
    normalize "${2#v}" >/dev/null
    normalize "$4" >/dev/null
    commit="$(printf '%s' "$3" | tr '[:upper:]' '[:lower:]')"
    digest="$(printf '%s\0%s\0%s' "${2#v}" "$commit" "$4" | sha256_hex)"
    finish "release-${digest:0:20}"
    ;;
  preview)
    [ "$#" -eq 2 ] || die 'usage: preview COMMIT'
    [[ "$2" =~ ^[0-9a-fA-F]{8,40}$ ]] || die 'preview commit must be a hexadecimal SHA'
    finish "preview-$(normalize "${2:0:12}")"
    ;;
  pr)
    [ "$#" -eq 2 ] || die 'usage: pr NUMBER'
    [[ "$2" =~ ^[1-9][0-9]*$ ]] || die 'pull request number must be positive'
    finish "pr-$2"
    ;;
  catalog-url)
    [ "$#" -eq 5 ] || die 'usage: catalog-url PROJECT VERSION COMMIT TARGET'
    project="$(normalize "$2")"
    branch="$($0 release "$3" "$4" "$5")"
    printf 'https://%s.%s.pages.dev/catalog.json\n' "$branch" "$project"
    ;;
  *) die 'usage: {release VERSION COMMIT TARGET|preview COMMIT|pr NUMBER|catalog-url PROJECT VERSION COMMIT TARGET}' ;;
esac
