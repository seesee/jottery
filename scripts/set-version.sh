#!/bin/bash

# Script to set version across all components
# Usage: ./scripts/set-version.sh <version>
# Example: ./scripts/set-version.sh 0.5.0

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.5.0"
    exit 1
fi

VERSION="$1"

# Validate version format (x.y.z)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: Version must be in format x.y.z (e.g., 0.5.0)"
    exit 1
fi

echo "Setting version to $VERSION across all components..."

# Update package.json (web client)
if command -v jq &> /dev/null; then
    # Use jq if available for safer JSON editing
    jq ".version = \"$VERSION\"" package.json > package.json.tmp && mv package.json.tmp package.json
    echo "✓ Updated package.json"
else
    # Fallback to sed (less safe but works)
    sed -i.bak "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" package.json && rm package.json.bak
    echo "✓ Updated package.json (using sed)"
fi

# Update Cargo.toml (TUI client)
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" tui/Cargo.toml && rm tui/Cargo.toml.bak
echo "✓ Updated tui/Cargo.toml"

echo ""
echo "Version updated to $VERSION in:"
echo "  - package.json"
echo "  - tui/Cargo.toml"
echo ""
echo "Next steps:"
echo "  1. Review changes: git diff"
echo "  2. Commit: git add -A && git commit -m \"Bump version to v$VERSION\""
echo "  3. Tag: git tag v$VERSION"
echo "  4. Push: git push && git push --tags"
