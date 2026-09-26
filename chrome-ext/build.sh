#!/usr/bin/env bash
# Stage the static Chrome MV3 extension.
#
# Usage:
#   ./chrome-ext/build.sh
#
# Output: chrome-ext/dist/ (load via chrome://extensions "Load unpacked").
set -euo pipefail

EXT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="$EXT_DIR/dist"
FILES=(
  manifest.json popup.html
  wire.js host_api.js file_input.js site_access.js tool_routing.js form_values.js popup_state.js
  detectors.js selector_adapter.js
  background.js popup.js content.js
)
LOCALE_FILES=(_locales/en/messages.json _locales/ko/messages.json)

echo "[1/3] validate static extension inputs"
for file in "${FILES[@]}" "${LOCALE_FILES[@]}"; do
  if [ ! -f "$EXT_DIR/$file" ]; then
    echo "error: missing chrome-ext/$file" >&2
    exit 1
  fi
done

echo "[2/3] stage dist/"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"
for file in "${FILES[@]}"; do
  cp "$EXT_DIR/$file" "$DIST_DIR/$file"
done
for file in "${LOCALE_FILES[@]}"; do
  mkdir -p "$DIST_DIR/$(dirname "$file")"
  cp "$EXT_DIR/$file" "$DIST_DIR/$file"
done

echo "[3/3] extension ready — load $DIST_DIR in chrome://extensions"
echo "     · click 'Load unpacked' and select that directory"
echo "     · popup renders Board tabs + pinned tools from 127.0.0.1:7173,"
echo "       or falls back to upeg://open?surface=ext if upeg isn't running"
echo "     · content scripts are registered dynamically per enabled site;"
echo "       etherscan/polygonscan are seeded on install, any other site is"
echo "       added from the popup's 'Enable on this site' toggle"
echo
echo "note: manifest.json ships without an \`icons\` block — Chrome will use"
echo "      its default puzzle-piece icon. Add icon-{16,48,128}.png and"
echo "      reference them from manifest before publishing to the Web Store."
