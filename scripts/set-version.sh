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

# Update Cargo.toml (server)
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" server/Cargo.toml && rm server/Cargo.toml.bak
echo "✓ Updated server/Cargo.toml"

# Update package.json (admin dashboard)
if command -v jq &> /dev/null; then
    # Use jq if available for safer JSON editing
    jq ".version = \"$VERSION\"" admin/package.json > admin/package.json.tmp && mv admin/package.json.tmp admin/package.json
    echo "✓ Updated admin/package.json"
else
    # Fallback to sed (less safe but works)
    sed -i.bak "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" admin/package.json && rm admin/package.json.bak
    echo "✓ Updated admin/package.json (using sed)"
fi

echo ""
echo "Version updated to $VERSION in:"
echo "  - package.json (web client)"
echo "  - tui/Cargo.toml (TUI client)"
echo "  - server/Cargo.toml (server)"
echo "  - admin/package.json (admin dashboard)"
echo ""
echo "Review changes:"
echo "  git diff"
echo ""
echo "To commit, tag, and push (copy and paste this):"
echo ""
echo "git add -A && git commit -m \"release v$VERSION\" && git tag v$VERSION && git push && git push --tags"
echo ""
