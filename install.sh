#!/usr/bin/env bash
# naeasy installer (prebuilt, macOS). Installs OR upgrades. Idempotent.
#
#   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/vahagncrypto99-netizen/naeasy/main/install.sh)"
#
# Downloads the newest release asset for this CPU from GitHub Releases.

set -uo pipefail

REPO="vahagncrypto99-netizen/naeasy"
BOLD=$'\033[1m'; RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RESET=$'\033[0m'

if [ "$(uname)" != "Darwin" ]; then
  echo "${YELLOW}This installer is for macOS. On Linux use install-linux.sh.${RESET}"
  exit 1
fi

ARCH=$([ "$(uname -m)" = "arm64" ] && echo arm64 || echo x64)

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "▸ Looking up the latest release (${ARCH})…"
ASSETS=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
  | grep -o '"browser_download_url" *: *"[^"]*"' | cut -d'"' -f4)
URL=$(echo "$ASSETS" | grep "naeasy-macos-${ARCH}-" | head -1)
# Fallback: a single-arch zip from older releases.
[ -z "${URL:-}" ] && URL=$(echo "$ASSETS" | grep "naeasy-macos-" | head -1)
if [ -z "${URL:-}" ]; then
  echo "${RED}No macOS build found in the latest release of $REPO.${RESET}" >&2
  exit 1
fi

ZIP="$TMP/$(basename "$URL")"
echo "▸ Downloading $(basename "$URL")…"
curl -fsSL -o "$ZIP" "$URL" || { echo "${RED}Download failed.${RESET}" >&2; exit 1; }
echo "${BOLD}naeasy — installing from $(basename "$ZIP")${RESET}"

# 1. Quit a running instance (upgrade case) so the bundle can be replaced.
osascript -e 'tell application "naeasy" to quit' >/dev/null 2>&1 || true
pkill -x naeasy >/dev/null 2>&1 || true
sleep 1

# 2. Unpack.
UNPACK="$TMP/unpack"
mkdir -p "$UNPACK"
unzip -oq "$ZIP" -d "$UNPACK"
APP=$(find "$UNPACK" -maxdepth 2 -name "naeasy.app" -type d | head -1)
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
