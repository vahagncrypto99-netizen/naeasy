#!/usr/bin/env bash
# naeasy — one-step installer.
# Checks every required tool. For anything missing it prints the exact fix
# steps for THAT tool and stops. When everything is present it builds the app
# (only if it isn't built yet) and installs it into /Applications.
#
#   ./install.sh            build if needed, then install
#   ./install.sh --rebuild  force a clean recompile

set -uo pipefail
cd "$(dirname "$0")"

BOLD=$'\033[1m'; RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RESET=$'\033[0m'

missing=0
ok() { printf "%s✓ %s%s\n" "$GREEN" "$1" "$RESET"; }

# need_step <label> <step1> [step2 ...]
need_step() {
  missing=$((missing + 1))
  printf "%s✗ %s — not installed%s\n" "$RED" "$1" "$RESET"
  shift
  for line in "$@"; do printf "      %s\n" "$line"; done
  echo
}

echo "${BOLD}naeasy — environment check${RESET}"
echo

if [ "$(uname)" != "Darwin" ]; then
  echo "${YELLOW}! naeasy targets macOS. On this OS the /Applications install step will be skipped.${RESET}"
  echo
fi

# 1. Xcode Command Line Tools (needed to link Rust on macOS)
if xcode-select -p >/dev/null 2>&1; then
  ok "Xcode Command Line Tools"
else
  need_step "Xcode Command Line Tools" \
    "Run:  xcode-select --install" \
    "A system dialog opens — click Install, wait for it to finish, then run ./install.sh again."
fi

# 2. Node.js (>= 18)
if command -v node >/dev/null 2>&1; then
  ver=$(node -v | sed 's/v//'); major=${ver%%.*}
  if [ "${major:-0}" -ge 18 ]; then
    ok "Node.js $ver"
  else
    need_step "Node.js >= 18 (found $ver)" \
      "Update via Homebrew:  brew install node" \
      "or download the LTS build:  https://nodejs.org/en/download"
  fi
else
  need_step "Node.js" \
    "Via Homebrew:  brew install node" \
    "or download the LTS installer:  https://nodejs.org/en/download" \
    'No Homebrew? install it:  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"'
fi

# 3. npm (ships with Node)
if command -v npm >/dev/null 2>&1; then
  ok "npm $(npm -v)"
else
  need_step "npm" "Usually installed together with Node.js — reinstall Node (see above)."
fi

# 4. Rust / cargo
if command -v cargo >/dev/null 2>&1; then
  ok "Rust $(rustc --version 2>/dev/null | awk '{print $2}')"
else
  need_step "Rust (cargo)" \
    "Install rustup:  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" \
    'Load it in this shell:  source "$HOME/.cargo/env"' \
    "Then run ./install.sh again."
fi

if [ "$missing" -gt 0 ]; then
  echo "${BOLD}${RED}Missing tools: $missing.${RESET} Fix the items above and run ${BOLD}./install.sh${RESET} again."
  exit 1
fi

echo
echo "${GREEN}${BOLD}All tools present.${RESET}"
echo

APP_BUILD="src-tauri/target/release/bundle/macos/naeasy.app"
REBUILD=0
[ "${1:-}" = "--rebuild" ] && REBUILD=1

set -e
if [ "$REBUILD" = "0" ] && [ -d "$APP_BUILD" ]; then
  echo "▸ Using existing build (pass --rebuild to recompile)."
else
  echo "▸ npm install…"
  npm install
  echo "▸ Building release (first build compiles Rust — a few minutes; later builds are incremental)…"
  npm run build
fi

if [ ! -d "$APP_BUILD" ]; then
  echo "${RED}✗ Build did not produce $APP_BUILD${RESET}" >&2
  exit 1
fi

# Quit any running instance so the bundle can be replaced.
osascript -e 'tell application "naeasy" to quit' >/dev/null 2>&1 || true
pkill -x naeasy >/dev/null 2>&1 || true
sleep 1

DEST="/Applications/naeasy.app"
echo "▸ Installing to /Applications…"
if rm -rf "$DEST" 2>/dev/null && cp -R "$APP_BUILD" "$DEST" 2>/dev/null; then
  :
else
  echo "${YELLOW}  No write access to /Applications — installing to ~/Applications instead.${RESET}"
  mkdir -p "$HOME/Applications"
  rm -rf "$HOME/Applications/naeasy.app"
  cp -R "$APP_BUILD" "$HOME/Applications/naeasy.app"
  DEST="$HOME/Applications/naeasy.app"
fi

echo "▸ Launching…"
open "$DEST"

echo
echo "${GREEN}✓ Installed:${RESET} $DEST"
