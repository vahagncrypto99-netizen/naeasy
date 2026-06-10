#!/usr/bin/env bash
# naeasy installer (prebuilt, macOS). Installs OR upgrades from the bundled zip.
# Idempotent: works whether a previous version is installed or not.
#
# This file ships in the PUBLIC repo next to naeasy-macos-<version>.zip.

set -uo pipefail
cd "$(dirname "$0")"

BOLD=$'\033[1m'; RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RESET=$'\033[0m'

if [ "$(uname)" != "Darwin" ]; then
  echo "${YELLOW}This installer is for macOS. On Linux use the .deb / .AppImage.${RESET}"
  exit 1
fi

ZIP=$(ls naeasy-macos-*.zip 2>/dev/null | sort -V | tail -1)
if [ -z "${ZIP:-}" ]; then
  echo "${RED}No naeasy-macos-*.zip found next to this script.${RESET}" >&2
  exit 1
fi
echo "${BOLD}naeasy — installing from ${ZIP}${RESET}"

# 1. Quit a running instance (upgrade case) so the bundle can be replaced.
osascript -e 'tell application "naeasy" to quit' >/dev/null 2>&1 || true
pkill -x naeasy >/dev/null 2>&1 || true
sleep 1

# 2. Unpack to a temp dir.
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
unzip -oq "$ZIP" -d "$TMP"
APP=$(find "$TMP" -maxdepth 2 -name "naeasy.app" -type d | head -1)
[ -n "$APP" ] || { echo "${RED}naeasy.app not found inside the zip${RESET}" >&2; exit 1; }

# 3. Install — replace any existing copy (in /Applications or ~/Applications).
rm -rf "$HOME/Applications/naeasy.app" 2>/dev/null || true
DEST="/Applications/naeasy.app"
if rm -rf "$DEST" 2>/dev/null && cp -R "$APP" "$DEST" 2>/dev/null; then
  :
else
  echo "${YELLOW}No write access to /Applications — installing to ~/Applications.${RESET}"
  mkdir -p "$HOME/Applications"
  DEST="$HOME/Applications/naeasy.app"
  rm -rf "$DEST"
  cp -R "$APP" "$DEST"
fi

# 4. Drop the quarantine flag so the (unsigned) app opens without Gatekeeper nags.
xattr -dr com.apple.quarantine "$DEST" 2>/dev/null || true

# 5. Launch.
open "$DEST"
echo "${GREEN}✓ Installed / updated:${RESET} $DEST"
echo "  naeasy lives in the menu bar — toggle with Cmd+Shift+M."
