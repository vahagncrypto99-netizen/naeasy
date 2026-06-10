#!/usr/bin/env bash
# naeasy installer (prebuilt, macOS). Installs OR upgrades. Idempotent.
#
# Two ways to run (this file ships in the PUBLIC repo):
#   • one-liner, no clone needed — downloads the newest zip itself:
#       /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/vahagncrypto99-netizen/naeasy/main/install.sh)"
#   • from a clone:  ./install.sh   (uses the newest bin/naeasy-macos-*.zip)

set -uo pipefail
cd "$(dirname "$0")"

REPO="vahagncrypto99-netizen/naeasy"
BOLD=$'\033[1m'; RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RESET=$'\033[0m'

if [ "$(uname)" != "Darwin" ]; then
  echo "${YELLOW}This installer is for macOS.${RESET}"
  exit 1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Newest version wins; zips live in bin/ (fallback: next to this script).
ZIP=$(ls bin/naeasy-macos-*.zip naeasy-macos-*.zip 2>/dev/null | sort -V | tail -1)

# Standalone (curl) mode — no local zip: fetch the newest one from GitHub.
if [ -z "${ZIP:-}" ]; then
  echo "▸ Looking up the newest version in github.com/${REPO}…"
  NAME=$(curl -fsSL "https://api.github.com/repos/$REPO/contents/bin" 2>/dev/null \
    | grep -o '"name" *: *"naeasy-macos-[^"]*\.zip"' \
    | sed 's/.*"\(naeasy-macos-[^"]*\.zip\)"/\1/' \
    | sort -V | tail -1)
  if [ -z "${NAME:-}" ]; then
    echo "${RED}Could not find a release zip in $REPO (bin/).${RESET}" >&2
    exit 1
  fi
  echo "▸ Downloading ${NAME}…"
  if ! curl -fsSL -o "$TMP/$NAME" "https://raw.githubusercontent.com/$REPO/main/bin/$NAME"; then
    echo "${RED}Download failed.${RESET}" >&2
    exit 1
  fi
  ZIP="$TMP/$NAME"
fi
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
