#!/usr/bin/env bash
# naeasy installer (prebuilt, Linux .deb — Ubuntu/Debian). Installs OR upgrades.
#
#   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/vahagncrypto99-netizen/naeasy/main/install-linux.sh)"
#
# Downloads the newest .deb for this CPU from GitHub Releases; apt resolves
# the dependencies (webkit2gtk, wmctrl, …).

set -uo pipefail

REPO="vahagncrypto99-netizen/naeasy"
BOLD=$'\033[1m'; RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RESET=$'\033[0m'

if [ "$(uname)" != "Linux" ]; then
  echo "${YELLOW}This installer is for Linux. On macOS use install.sh.${RESET}"
  exit 1
fi
if ! command -v dpkg >/dev/null 2>&1; then
  echo "${RED}dpkg not found — this installer supports Debian/Ubuntu (.deb).${RESET}" >&2
  exit 1
fi

ARCH=$(dpkg --print-architecture)   # amd64 / arm64

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "▸ Looking up the latest release (${ARCH})…"
URL=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
  | grep -o '"browser_download_url" *: *"[^"]*"' | cut -d'"' -f4 \
  | grep "_${ARCH}\.deb" | head -1)
if [ -z "${URL:-}" ]; then
  echo "${RED}No ${ARCH} .deb found in the latest release of $REPO.${RESET}" >&2
  exit 1
fi

DEB="$TMP/$(basename "$URL")"
echo "▸ Downloading $(basename "$URL")…"
curl -fsSL -o "$DEB" "$URL" || { echo "${RED}Download failed.${RESET}" >&2; exit 1; }
echo "${BOLD}naeasy — installing $(basename "$DEB")${RESET}"

SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"
$SUDO apt-get install -y --allow-downgrades "$(realpath "$DEB")" \
  || { echo "${RED}Install failed.${RESET}" >&2; exit 1; }

echo "${GREEN}✓ Installed / updated.${RESET}"
echo "  Launch \"naeasy\" from your app menu — it lives in the system tray."
