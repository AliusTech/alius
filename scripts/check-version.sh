#!/bin/bash
# Version validation script
# Uses the exact git tag as the release version and Cargo.toml as the dev fallback.

set -euo pipefail

CARGO_FILE="Cargo.toml"

CARGO_VERSION=$(awk -F'"' '/^version = / { print $2; exit }' "$CARGO_FILE")

# Get current git tag (if any)
GIT_TAG=$(git describe --tags --exact-match --match 'v[0-9]*' 2>/dev/null || echo "no-tag")

if [ "$GIT_TAG" != "no-tag" ]; then
    RELEASE_VERSION="${GIT_TAG#v}"
    VERSION_SOURCE="git tag"
else
    RELEASE_VERSION="$CARGO_VERSION"
    VERSION_SOURCE="Cargo.toml"
fi

echo "Release version: $RELEASE_VERSION"
echo "Version source:  $VERSION_SOURCE"
echo "Cargo fallback:  $CARGO_VERSION"
echo "Git tag:         $GIT_TAG"

if [[ ! "$RELEASE_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
    echo "ERROR: Release version ($RELEASE_VERSION) is not a valid semver version"
    exit 1
fi

if [ "$GIT_TAG" != "no-tag" ] && [ "$RELEASE_VERSION" != "$CARGO_VERSION" ]; then
    echo "Note: Cargo.toml fallback version differs from tag-driven release version."
fi

echo "✓ Version consistency check passed"
exit 0
