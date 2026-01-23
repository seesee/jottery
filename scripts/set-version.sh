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

echo "Running tests before setting version to $VERSION..."
echo ""

# Run web client tests
echo "→ Testing web client..."
if ! npm run test:run; then
    echo "✗ Web client tests failed"
    exit 1
fi
echo "✓ Web client tests passed"
echo ""

# Run web client type check
echo "→ Type checking web client..."
if ! npm run check; then
    echo "✗ Web client type check failed"
    exit 1
fi
echo "✓ Web client type check passed"
echo ""

# Run TUI tests
echo "→ Testing TUI client..."
if ! (cd tui && cargo test); then
    echo "✗ TUI tests failed"
    exit 1
fi
echo "✓ TUI tests passed"
echo ""

# Run server tests
echo "→ Testing server..."
if ! (cd server && cargo test); then
    echo "✗ Server tests failed"
    exit 1
fi
echo "✓ Server tests passed"
echo ""

# Regenerate SQLx query cache for offline Docker builds
echo "→ Regenerating SQLx query cache..."
if ! (cd server && cargo sqlx prepare); then
    echo "✗ SQLx cache regeneration failed"
    exit 1
fi
echo "✓ SQLx query cache regenerated"
echo ""

# Run admin dashboard tests
echo "→ Testing admin dashboard..."
if ! (cd admin && npm test); then
    echo "✗ Admin dashboard tests failed"
    exit 1
fi
echo "✓ Admin dashboard tests passed"
echo ""

# Run admin dashboard type check
echo "→ Type checking admin dashboard..."
if ! (cd admin && npm run check); then
    echo "✗ Admin dashboard type check failed"
    exit 1
fi
echo "✓ Admin dashboard type check passed"
echo ""

# Run web client E2E tests (slow - run last)
echo "→ Running E2E tests (this may take 10-15 minutes)..."
if ! npm run test:e2e; then
    echo "✗ E2E tests failed"
    exit 1
fi
echo "✓ E2E tests passed"
echo ""

echo "All tests passed! Setting version to $VERSION across all components..."
echo ""

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
