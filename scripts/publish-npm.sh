#!/bin/bash
# Publish all @alius-tech/alius packages to npm
# Prerequisites for manual/bootstrap publishes: npm login (run: npm login)

set -euo pipefail

NPM_DIR="$(cd "$(dirname "$0")/../npm-packages" && pwd)"

echo "Checking npm auth..."
npm whoami || { echo "Error: Run 'npm login' first"; exit 1; }

echo ""
echo "Publishing platform packages..."

PLATFORMS=(
  alius-darwin-arm64
  alius-darwin-x64
  alius-linux-x64
  alius-win32-x64
)

for pkg in "${PLATFORMS[@]}"; do
  echo "  Publishing @alius-tech/$pkg..."
  cd "$NPM_DIR/$pkg"
  npm publish --access public --tag latest
done

echo ""
echo "  Publishing @alius-tech/alius (main wrapper)..."
cd "$NPM_DIR/alius"
npm publish --access public --tag latest

echo ""
echo "All 5 packages published successfully!"
