#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "Usage: $0 <major|minor|patch>"
    exit 1
}

if [[ $# -ne 1 ]]; then
    usage
fi

BUMP_TYPE="$1"

if [[ "$BUMP_TYPE" != "major" && "$BUMP_TYPE" != "minor" && "$BUMP_TYPE" != "patch" ]]; then
    usage
fi

LATEST_TAG=$(git tag -l 'v*' | sort -V | tail -n1)

if [[ -z "$LATEST_TAG" ]]; then
    LATEST_TAG="v0.0.0"
fi

VERSION="${LATEST_TAG#v}"
IFS='.' read -r MAJOR MINOR PATCH <<< "$VERSION"

case "$BUMP_TYPE" in
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
esac

NEW_VERSION="v${MAJOR}.${MINOR}.${PATCH}"

echo "Current version: $LATEST_TAG"
echo "New version: $NEW_VERSION"
read -rp "Tag $NEW_VERSION? [y/N] " CONFIRM

if [[ "$CONFIRM" =~ ^[Yy]$ ]]; then
    git tag -a "$NEW_VERSION" -m "$NEW_VERSION"
    git push origin "$NEW_VERSION"
fi
