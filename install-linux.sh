#!/usr/bin/env bash
# naeasy — Linux installer. Checks build deps, then builds .deb + .AppImage.
# For anything missing it prints the exact fix steps and stops.
# (On macOS use ./install.sh instead.)

set -uo pipefail
cd "$(dirname "$0")"

BOLD=$'\033[1m'; RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RESET=$'\033[0m'
missing=0
ok() { printf "%s✓ %s%s\n" "$GREEN" "$1" "$RESET"; }
need() {
  missing=$((missing + 1))
  printf "%s✗ %s%s\n" "$RED" "$1" "$RESET"
  shift
  for l in "$@"; do printf "      %s\n" "$l"; done
  echo
}

echo "${BOLD}naeasy (Linux) — environment check${RESET}"
echo
if [ "$(uname)" != "Linux" ]; then
  echo "${YELLOW}This script is for Linux. On macOS run ./install.sh${RESET}"
  exit 1
fi

# Distro-specific package install command (GTK/WebKit + helpers).
if command -v apt >/dev/null 2>&1; then
  PKG="sudo apt update && sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev wmctrl"
elif command -v dnf >/dev/null 2>&1; then
  PKG="sudo dnf install -y webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel wmctrl @development-tools"
elif command -v pacman >/dev/null 2>&1; then
  PKG="sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl libappindicator-gtk3 librsvg libxdo wmctrl"
else
  PKG="install GTK/WebKit dev deps: webkit2gtk-4.1, build tools, libssl, libayatana-appindicator3, librsvg2, libxdo, wmctrl"
fi

# Node.js >= 18
if command -v node >/dev/null 2>&1; then
  ver=$(node -v | sed 's/v//'); maj=${ver%%.*}
  if [ "${maj:-0}" -ge 18 ]; then ok "Node.js $ver"; else
    need "Node.js >= 18 (found $ver)" "Install Node 18+ via your package manager or https://nodejs.org"
  fi
else
  need "Node.js" "Install Node 18+ (https://nodejs.org) or via your package manager"
fi
command -v npm >/dev/null 2>&1 && ok "npm $(npm -v)" || need "npm" "Ships with Node.js — install Node (see above)"

# Rust
if command -v cargo >/dev/null 2>&1; then
  ok "Rust $(rustc --version 2>/dev/null | awk '{print $2}')"
else
  need "Rust (cargo)" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" 'then: source "$HOME/.cargo/env"'
fi

# WebKitGTK build dependency
if pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  ok "webkit2gtk-4.1 + GTK deps"
else
  need "webkit2gtk-4.1 + GTK build deps" "$PKG"
fi

# wmctrl — optional (enables 'open now' badges + focus-existing-window)
if command -v wmctrl >/dev/null 2>&1; then
  ok "wmctrl (open-now / window focus)"
else
  echo "${YELLOW}! wmctrl not found — 'open now' badges and switch-to-window are disabled (optional).${RESET}"
  echo "      $PKG"
  echo
fi

if [ "$missing" -gt 0 ]; then
  echo "${BOLD}${RED}Missing $missing requirement(s).${RESET} Fix the items above and re-run ./install-linux.sh"
  exit 1
fi

echo "${GREEN}${BOLD}All required tools present.${RESET}"
echo

set -e
echo "▸ npm install…"
npm install
echo "▸ Building .deb + .AppImage (first build compiles Rust — a few minutes)…"
npm run tauri -- build --bundles deb,appimage

BUNDLE="src-tauri/target/release/bundle"
echo
echo "${GREEN}✓ Built.${RESET} Artifacts:"
ls -1 "$BUNDLE"/deb/*.deb "$BUNDLE"/appimage/*.AppImage 2>/dev/null || true
echo
echo "Install one of:"
echo "  • .deb (Debian/Ubuntu):  sudo dpkg -i $BUNDLE/deb/*.deb"
echo "  • AppImage (portable):   chmod +x $BUNDLE/appimage/*.AppImage && run it"
echo
echo "naeasy runs in the system tray; toggle the window with Ctrl+Shift+M (changeable in ⚙)."
