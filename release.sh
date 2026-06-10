#!/usr/bin/env bash
# Build a distributable macOS release into ./dist (run from the PRIVATE repo).
# Then publish ./dist to the PUBLIC repo (or via `gh release`).
#
#   ./release.sh            build current version
#   ./release.sh 0.2.0      bump version everywhere, then build

set -euo pipefail
cd "$(dirname "$0")"

GREEN=$'\033[32m'; BOLD=$'\033[1m'; RESET=$'\033[0m'

# Optional version bump (keeps tauri.conf.json / Cargo.toml / package.json in sync).
if [ "${1:-}" != "" ]; then
  NEW="$1"
  echo "▸ Bumping version → $NEW"
  node -e "const f='src-tauri/tauri.conf.json',j=require('fs');const c=JSON.parse(j.readFileSync(f));c.version='$NEW';j.writeFileSync(f,JSON.stringify(c,null,2)+'\n')"
  node -e "const f='package.json',j=require('fs');const c=JSON.parse(j.readFileSync(f));c.version='$NEW';j.writeFileSync(f,JSON.stringify(c,null,2)+'\n')"
  sed -i.bak -E "s/^version = \".*\"/version = \"$NEW\"/" src-tauri/Cargo.toml && rm -f src-tauri/Cargo.toml.bak
fi

VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
echo "${BOLD}▸ Releasing naeasy v$VERSION${RESET}"

echo "▸ Building release…"
npm install
npm run build

APP="src-tauri/target/release/bundle/macos/naeasy.app"
[ -d "$APP" ] || { echo "Build did not produce $APP" >&2; exit 1; }

mkdir -p dist
rm -f dist/naeasy-macos-*.zip
ZIP_ABS="$PWD/dist/naeasy-macos-$VERSION.zip"
# ditto preserves the .app bundle structure (symlinks, resources).
( cd "$(dirname "$APP")" && ditto -c -k --sequesterRsrc --keepParent "naeasy.app" "$ZIP_ABS" )

cp scripts/public-install.sh dist/install.sh
chmod +x dist/install.sh

cat > dist/README.md <<EOF
# naeasy — install (macOS)

\`\`\`bash
./install.sh
\`\`\`

Installs or upgrades naeasy. Safe whether a previous version is installed or not.
Version: $VERSION
EOF

echo
echo "${GREEN}✓ dist/ ready:${RESET}"
ls -1 dist

# Publish into the local checkout of the PUBLIC binary repo (bin/ layout).
# Old versions are kept — each release just adds a new zip to bin/.
# Guard against pointing at this (dev) repo itself.
PUBLIC_DIR="${PUBLIC_DIR:-$(dirname "$PWD")/naeasy}"
if [ -d "$PUBLIC_DIR/.git" ] && [ "$(cd "$PUBLIC_DIR" && pwd)" != "$PWD" ]; then
  echo
  echo "▸ Publishing v$VERSION into $PUBLIC_DIR"
  mkdir -p "$PUBLIC_DIR/bin"
  cp "dist/naeasy-macos-$VERSION.zip" "$PUBLIC_DIR/bin/"
  cp dist/install.sh "$PUBLIC_DIR/install.sh"
  chmod +x "$PUBLIC_DIR/install.sh"
  [ -f "$PUBLIC_DIR/README.md" ] || cp dist/README.md "$PUBLIC_DIR/README.md"
  git -C "$PUBLIC_DIR" add bin install.sh README.md
  if git -C "$PUBLIC_DIR" commit -m "release v$VERSION" >/dev/null; then
    echo "${GREEN}✓ Committed in public repo.${RESET} Push it:"
    echo "    git -C \"$PUBLIC_DIR\" push"
  else
    echo "Nothing new to commit in the public repo."
  fi
else
  echo
  echo "Publish (no public checkout at $PUBLIC_DIR — manual options):"
  echo "  • Public repo:  copy dist/naeasy-macos-$VERSION.zip → <public>/bin/, dist/install.sh → <public>/, commit & push"
  echo "  • GitHub release:  gh release create v$VERSION dist/naeasy-macos-$VERSION.zip dist/install.sh"
fi
