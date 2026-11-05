#!/bin/bash

# Script to set version across all components
# Usage: ./scripts/set-version.sh <version> [--skip-e2e]
# Example: ./scripts/set-version.sh 0.5.0
# Example: ./scripts/set-version.sh 0.5.0 --skip-e2e

set -e

SKIP_E2E=false
VERSION=""

# Parse arguments
for arg in "$@"; do
    case $arg in
        --skip-e2e)
            SKIP_E2E=true
            ;;
        *)
            if [ -z "$VERSION" ]; then
                VERSION="$arg"
            fi
            ;;
    esac
done

if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version> [--skip-e2e]"
    echo "Example: $0 0.5.0"
    echo "Example: $0 0.5.0 --skip-e2e"
    exit 1
fi

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

# Regenerate SQLx query cache for offline Docker builds (pre-version update)
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
if [ "$SKIP_E2E" = true ]; then
    echo "→ Skipping E2E tests (--skip-e2e flag set)"
    echo ""
else
    echo "→ Running E2E tests (this may take 10-15 minutes)..."
    if ! npm run test:e2e; then
        echo "✗ E2E tests failed"
        exit 1
    fi
    echo "✓ E2E tests passed"
    echo ""
fi

echo "All tests passed! Setting version to $VERSION across all components..."
echo ""

# One build number for both mobile stores, which need it to rise with every release:
# major×10000 + minor×100 + patch
IFS='.' read -r V_MAJOR V_MINOR V_PATCH <<< "$VERSION"
BUILD_NUMBER=$(( V_MAJOR * 10000 + V_MINOR * 100 + V_PATCH ))

# Update package.json and package-lock.json (web client). This writes only those two
# files, and --allow-same-version lets a failed run be repeated.
npm version "$VERSION" --no-git-tag-version --allow-same-version --ignore-scripts > /dev/null
echo "✓ Updated package.json and package-lock.json"

# Update Cargo.toml (TUI client)
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" tui/Cargo.toml && rm tui/Cargo.toml.bak
echo "✓ Updated tui/Cargo.toml"

# Update Cargo.lock (TUI client). Nothing else builds the TUI after this point,
# and CI's --locked test run fails if the lock file is out of date.
if ! (cd tui && cargo update --package jottery-tui --offline); then
    echo "✗ Failed to update tui/Cargo.lock"
    exit 1
fi
echo "✓ Updated tui/Cargo.lock"

# Update Cargo.toml (server)
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" server/Cargo.toml && rm server/Cargo.toml.bak
echo "✓ Updated server/Cargo.toml"

# Update package.json and package-lock.json (admin dashboard)
(cd admin && npm version "$VERSION" --no-git-tag-version --allow-same-version --ignore-scripts > /dev/null)
echo "✓ Updated admin/package.json and admin/package-lock.json"

# Update the image Compose users get. The tag only exists once the release is built.
sed -i.bak "s|ghcr.io/seesee/jottery:v[0-9][^[:space:]]*|ghcr.io/seesee/jottery:v$VERSION|" docker-compose.yml && rm docker-compose.yml.bak
echo "✓ Updated docker-compose.yml"

# Update Xcode project (iOS client)
IOS_PBXPROJ="ios-native/Jottery/Jottery.xcodeproj/project.pbxproj"
if [ -f "$IOS_PBXPROJ" ]; then
    sed -i.bak "s/MARKETING_VERSION = [^;]*/MARKETING_VERSION = $VERSION/" "$IOS_PBXPROJ" && rm "$IOS_PBXPROJ.bak"
    sed -i.bak "s/CURRENT_PROJECT_VERSION = [^;]*/CURRENT_PROJECT_VERSION = $BUILD_NUMBER/" "$IOS_PBXPROJ" && rm "$IOS_PBXPROJ.bak"
    echo "✓ Updated $IOS_PBXPROJ (version=$VERSION, build=$BUILD_NUMBER)"
else
    echo "⚠ iOS project not found at $IOS_PBXPROJ — skipping"
fi

# Update the Android build
ANDROID_GRADLE="android-native/app/build.gradle.kts"
if [ -f "$ANDROID_GRADLE" ]; then
    sed -i.bak -E \
        -e "s/^([[:space:]]*versionCode = )[0-9]+/\1$BUILD_NUMBER/" \
        -e "s/^([[:space:]]*versionName = )\"[^\"]*\"/\1\"$VERSION\"/" \
        "$ANDROID_GRADLE" && rm "$ANDROID_GRADLE.bak"
    echo "✓ Updated $ANDROID_GRADLE (version=$VERSION, build=$BUILD_NUMBER)"
else
    echo "⚠ Android build not found at $ANDROID_GRADLE — skipping"
fi

echo ""
echo "Regenerating SQLx query cache after version update..."
if ! (cd server && cargo sqlx prepare); then
    echo "✗ SQLx cache regeneration failed after version update"
    exit 1
fi
echo "✓ SQLx query cache regenerated with new version"
echo ""

echo "Version updated to $VERSION in:"
echo "  - package.json and package-lock.json (web client)"
echo "  - tui/Cargo.toml and tui/Cargo.lock (TUI client)"
echo "  - server/Cargo.toml (server)"
echo "  - admin/package.json and admin/package-lock.json (admin dashboard)"
echo "  - docker-compose.yml (image tag)"
echo "  - ios-native/Jottery/Jottery.xcodeproj (iOS client)"
echo "  - android-native/app/build.gradle.kts (Android client)"
echo ""
echo "SQLx query cache updated for Docker builds"
echo ""
echo "Review changes:"
echo "  git diff"
echo ""
# Named files rather than git add -A, which has already published a database and
# would publish anything else lying in the working copy. The repo is public, and
# main only receives merged PRs.
echo "To commit and open a pull request (copy and paste this):"
echo ""
echo "git checkout -b release/v$VERSION"
echo "git add package.json package-lock.json admin/package.json admin/package-lock.json docker-compose.yml tui/Cargo.toml tui/Cargo.lock server/Cargo.toml server/Cargo.lock server/.sqlx $IOS_PBXPROJ $ANDROID_GRADLE"
echo "git commit -m \"jottery release v$VERSION\""
echo "git push -u origin release/v$VERSION"
echo "gh pr create --fill"
echo ""
echo "Once that's merged, tag the merge commit, which starts the release build:"
echo ""
echo "git checkout main && git pull && git tag v$VERSION && git push origin v$VERSION"
echo ""
