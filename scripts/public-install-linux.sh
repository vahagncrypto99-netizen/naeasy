#!/usr/bin/env bash
# naeasy installer (prebuilt, Linux .deb — Ubuntu/Debian). Installs OR upgrades.
#
# Two ways to run (this file ships in the PUBLIC repo as install-linux.sh):
#   • one-liner, no clone needed — downloads the newest .deb itself:
#       /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/vahagncrypto99-netizen/naeasy/main/install-linux.sh)"
#   • from a clone:  ./install-linux.sh   (uses the newest bin/naeasy_*_<arch>.deb)

set -uo pipefail
cd "$(dirname "$0")"

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

# Newest version for this arch; debs live in bin/.
DEB=$(ls bin/naeasy_*_"$ARCH".deb 2>/dev/null | sort -V | tail -1)

# Standalone (curl) mode — no local deb: fetch the newest one from GitHub.
if [ -z "${DEB:-}" ]; then
  echo "▸ Looking up the newest ${ARCH} version in github.com/${REPO}…"
  NAME=$(curl -fsSL "https://api.github.com/repos/$REPO/contents/bin" 2>/dev/null \
    | grep -o "\"name\" *: *\"naeasy_[^\"]*_${ARCH}\.deb\"" \
    | sed "s/.*\"\(naeasy_[^\"]*_${ARCH}\.deb\)\"/\1/" \
    | sort -V | tail -1)
  if [ -z "${NAME:-}" ]; then
    echo "${RED}Could not find a ${ARCH} .deb in $REPO (bin/).${RESET}" >&2
    exit 1
  fi
  echo "▸ Downloading ${NAME}…"
  if ! curl -fsSL -o "$TMP/$NAME" "https://raw.githubusercontent.com/$REPO/main/bin/$NAME"; then
    echo "${RED}Download failed.${RESET}" >&2
    exit 1
  fi
  DEB="$TMP/$NAME"
fi
echo "${BOLD}naeasy — installing $(basename "$DEB")${RESET}"

# Install / upgrade via apt so dependencies (webkit2gtk, …) are resolved.
SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"
$SUDO apt-get install -y --allow-downgrades "$(realpath "$DEB")" \
  || { echo "${RED}Install failed.${RESET}" >&2; exit 1; }

echo "${GREEN}✓ Installed / updated.${RESET}"
echo "  Launch \"naeasy\" from your app menu — it lives in the system tray."
echo "  Tip: install wmctrl for window focusing:  $SUDO apt-get install -y wmctrl"
