#!/usr/bin/env bash
# Build Linux .deb packages in Docker (per arch) → ./releases/.
#
#   ./scripts/build-linux.sh              # amd64 + arm64
#   ARCHES=arm64 ./scripts/build-linux.sh # one arch
#
# Named volumes keep cargo/npm/target caches per arch, so re-builds are fast.
set -euo pipefail
cd "$(dirname "$0")/.."

ARCHES=${ARCHES:-"amd64 arm64"}
mkdir -p releases

for arch in $ARCHES; do
  echo "▸ [$arch] building image…"
  docker build --platform "linux/$arch" -t "naeasy-linux-build:$arch" \
    -f docker/linux.Dockerfile docker

  echo "▸ [$arch] building .deb…"
  docker run --rm --platform "linux/$arch" \
    -v "$PWD:/app" \
    -v "naeasy-cargo-$arch:/usr/local/cargo/registry" \
    -v "naeasy-target-$arch:/app/src-tauri/target" \
    -v "naeasy-node-$arch:/app/node_modules" \
    "naeasy-linux-build:$arch" \
    bash -c "npm install --no-audit --no-fund && npm run build -- --bundles deb && cp src-tauri/target/release/bundle/deb/*.deb /app/releases/"
done

echo
echo "✓ releases/ ready:"
ls -1 releases
