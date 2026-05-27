#!/bin/bash
# Version validation script
# Ensures .version file matches Cargo.toml workspace version

set -e

VERSION_FILE=".version"
CARGO_FILE="Cargo.toml"

# Read version from .version file
FILE_VERSION=$(cat "$VERSION_FILE" | tr -d '\n')

# Read version from Cargo.toml (workspace.package.version)
CARGO_VERSION=$(grep -A 1 "workspace.package]" "$CARGO_FILE" | grep "version" | head -1 | sed 's/version = "//' | sed 's/"//' | tr -d '\n')

# Alternative: extract version using simpler grep
CARGO_VERSION=$(grep "^version = " "$CARGO_FILE" | head -1 | sed 's/version = "//' | sed 's/"//' | tr -d '\n')

# Get current git tag (if any)
GIT_TAG=$(git describe --tags --exact-match 2>/dev/null || echo "no-tag")
TAG_VERSION="${GIT_TAG#v}"

echo "File version:  $FILE_VERSION"
echo "Cargo version: $CARGO_VERSION"
echo "Git tag:       $GIT_TAG"
echo "Tag version:   $TAG_VERSION"

# Validate consistency
if [ "$FILE_VERSION" != "$CARGO_VERSION" ]; then
    echo "ERROR: .version file ($FILE_VERSION) does not match Cargo.toml ($CARGO_VERSION)"
    exit 1
fi

# If there's a git tag, ensure it matches
if [ "$GIT_TAG" != "no-tag" ] && [ "$TAG_VERSION" != "$FILE_VERSION" ]; then
    echo "ERROR: Git tag version ($TAG_VERSION) does not match .version file ($FILE_VERSION)"
    exit 1
fi

echo "✓ Version consistency check passed"
exit 0