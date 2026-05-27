#!/bin/bash
# Publish all @alius/alius packages to npm
# Prerequisites: npm login (run: npm login)

set -e

NPM_DIR="$(cd "$(dirname "$0")/../npm-packages" && pwd)"

echo "Checking npm auth..."
npm whoami || { echo "Error: Run 'npm login' first"; exit 1; }

echo ""
echo "Publishing platform packages..."

PLATFORMS=(
  alius-darwin-arm64
  alius-darwin-x64
  alius-linux-arm64
  alius-linux-x64
  alius-win32-arm64
  alius-win32-x64
)

for pkg in "${PLATFORMS[@]}"; do
  echo "  Publishing @alius/$pkg..."
  cd "$NPM_DIR/$pkg"
  npm publish --access public
done

echo ""
echo "  Publishing @alius/alius (main wrapper)..."
cd "$NPM_DIR/alius"
npm publish --access public

echo ""
echo "All 7 packages published successfully!"
