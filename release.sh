#!/usr/bin/env bash
# Cut a release: bump the version everywhere, commit and tag.
# CI (.github/workflows/release.yml) builds macOS + Linux artifacts and
# publishes the GitHub Release when the tag is pushed.
#
#   ./release.sh 1.2.0
#   git push --follow-tags

set -euo pipefail
cd "$(dirname "$0")"

GREEN=$'\033[32m'; BOLD=$'\033[1m'; RESET=$'\033[0m'

NEW="${1:?usage: ./release.sh X.Y.Z}"
echo "${BOLD}▸ Bumping version → $NEW${RESET}"
node -e "const f='src-tauri/tauri.conf.json',j=require('fs');const c=JSON.parse(j.readFileSync(f));c.version='$NEW';j.writeFileSync(f,JSON.stringify(c,null,2)+'\n')"
node -e "const f='package.json',j=require('fs');const c=JSON.parse(j.readFileSync(f));c.version='$NEW';j.writeFileSync(f,JSON.stringify(c,null,2)+'\n')"
sed -i.bak -E "s/^version = \".*\"/version = \"$NEW\"/" src-tauri/Cargo.toml && rm -f src-tauri/Cargo.toml.bak
# Refresh Cargo.lock with the new version.
(cd src-tauri && cargo update -p naeasy --precise "$NEW" 2>/dev/null || cargo check -q 2>/dev/null || true)

git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock 2>/dev/null || true
git commit -m "release v$NEW"
git tag "v$NEW"

echo
echo "${GREEN}✓ v$NEW committed and tagged.${RESET} Publish with:"
echo "    git push --follow-tags"
