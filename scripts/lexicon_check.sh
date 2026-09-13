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
# Skipped trees: target/, node_modules/, .sisyphus/, .git/, .omx/, vendor/
#
# .omx/ holds historical setup backups (e.g. older AGENTS.md snapshots)
# that legitimately contain retired identifiers — backups are frozen in
# time and should not block the lexicon gate on current code.
#
# Note: bare "widget" / "registry" are NOT checked — too broad. They match
# ratatui's widget API, Flutter widget classes, i18n keys, and the
# dispatcher/trigger/embed "X registry poisoned" panic messages, none of
# which are the lexicon's PinKind / Toolbox concept.

set -euo pipefail

cd "$(dirname "$0")/.."

EXIT=0

scan() {
    local pattern="$1"
    local description="$2"
    local matches
    matches=$(grep -rln "$pattern" . \
        --include="*.rs" --include="*.toml" --include="*.json" --include="*.md" \
        2>/dev/null \
        | grep -v '^\./target/' \
        | grep -v '^\./node_modules/' \
        | grep -v '^\./\.sisyphus/' \
        | grep -v '^\./\.git/' \
        | grep -v '^\./\.omx/' \
        | grep -v '^\./vendor/' \
        | grep -v '^\./docs/LEXICON\.md$' \
        | grep -v '^\./docs/rules/decisions\.md$' \
        || true)
    if [ -n "$matches" ]; then
        echo "FAIL: retired identifier \`$pattern\` ($description) found in:"
        # Indent every hit two spaces. Bash parameter expansion rather
        # than `sed` so shellcheck stays clean (SC2001) and the check
        # spawns nothing per finding.
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
