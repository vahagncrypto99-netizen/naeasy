#!/usr/bin/env bash
# naeasy — uninstaller. Quits the running app, disables login-item autostart,
# and removes the bundle from /Applications and ~/Applications.

set -uo pipefail

echo "▸ Quitting naeasy…"
osascript -e 'tell application "naeasy" to quit' >/dev/null 2>&1 || true
pkill -x naeasy >/dev/null 2>&1 || true
sleep 1

echo "▸ Removing app…"
removed=0
for p in "/Applications/naeasy.app" "$HOME/Applications/naeasy.app"; do
  if [ -d "$p" ]; then
    rm -rf "$p" && echo "  removed $p" && removed=1
  fi
done
[ "$removed" = "0" ] && echo "  (no installed bundle found)"

# Disable login-item autostart, if registered (tauri-plugin-autostart names
# the LaunchAgent after the app: naeasy.plist).
launchctl bootout "gui/$(id -u)/naeasy" >/dev/null 2>&1 || true
rm -f "$HOME/Library/LaunchAgents/naeasy.plist" \
      "$HOME/Library/LaunchAgents/com.vahagn.naeasy.plist" >/dev/null 2>&1 || true

echo
echo "✓ Uninstalled."
echo "  Settings/config kept at: ~/Library/Application Support/com.vahagn.naeasy"
echo "  Delete it too with:  rm -rf \"\$HOME/Library/Application Support/com.vahagn.naeasy\""
