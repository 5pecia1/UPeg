#!/usr/bin/env bash
# Lexicon v2.3 drift check — fails if retired identifiers reappear.
#
# Scope:
#   - WidgetKind (enum)  → PinKind
#   - widget_kind (TOML/Rust field) → pin
#
# Allowed locations:
#   - docs/LEXICON.md (the deprecation table itself)
#   - docs/rules/decisions.md (the rename decision and its rationale)
#
# Git workspaces scan tracked files and non-ignored untracked files. This keeps
# ignored generated recovery copies out while still checking new source files.
# Source archives without .git scan the workspace and prune generated trees.

set -euo pipefail

cd "$(dirname "$0")/.."

EXIT=0

source_files() {
    local git_root
    git_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
    if [ -n "$git_root" ] && [ "$(cd "$git_root" && pwd -P)" = "$(pwd -P)" ]; then
        git ls-files -z --cached --others --exclude-standard
    else
        find . \
            -path './target' -prune -o \
            -path './node_modules' -prune -o \
            -path './tmp' -prune -o \
            -path './.sisyphus' -prune -o \
            -path './.git' -prune -o \
            -path './.omx' -prune -o \
            -path './vendor' -prune -o \
            -type f -print0
    fi
}

file_list="$(mktemp)"
trap 'rm -f "$file_list"' EXIT
if ! source_files > "$file_list"; then
    echo "lexicon-check: failed to enumerate source files" >&2
    exit 1
fi

scan() {
    local pattern="$1"
    local description="$2"
    local file
    local matches=""
    while IFS= read -r -d '' file; do
        case "$file" in
            *.rs|*.toml|*.json|*.md) ;;
            *) continue ;;
        esac
        case "$file" in
            vendor/*|./vendor/*) continue ;;
        esac
        case "$file" in
            docs/LEXICON.md|./docs/LEXICON.md|docs/rules/decisions.md|./docs/rules/decisions.md) continue ;;
        esac
        # Deleted tracked paths and symlinks are not regular source files.
        if [ ! -f "$file" ] || [ -L "$file" ]; then
            continue
        fi
        if grep -q -- "$pattern" "$file" 2>/dev/null; then
            matches+="${matches:+$'\n'}${file#./}"
        fi
    done < "$file_list"

    if [ -n "$matches" ]; then
        echo "FAIL: retired identifier \`$pattern\` ($description) found in:"
        echo "  ${matches//$'\n'/$'\n'  }"
        EXIT=1
    fi
}

scan 'WidgetKind' 'enum renamed to PinKind'
scan 'widget_kind' 'field renamed to pin'

if [ "$EXIT" -eq 0 ]; then
    echo "lexicon-check: ok (no retired identifiers in workspace)"
fi

exit "$EXIT"
