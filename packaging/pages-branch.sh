#!/usr/bin/env bash
# Produce the exact Cloudflare Pages branch alias used in catalog URLs and
# Wrangler deployment. Keep this normalization independent of Wrangler: Pages
# normalizes punctuation itself, which otherwise makes a catalog URL differ
# from the deployed alias.
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
  [ "${#branch}" -le 63 ] || die "branch exceeds 63 characters: $branch"
  printf '%s\n' "$branch"
}

case "${1:-}" in
  release)
    [ "$#" -eq 4 ] || die 'usage: release VERSION COMMIT TARGET'
    [[ "$3" =~ ^[0-9a-fA-F]{8,40}$ ]] || die 'release commit must be a hexadecimal SHA'
    finish "release-$(normalize "$2")-$(normalize "${3:0:8}")-$(normalize "$4")"
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
