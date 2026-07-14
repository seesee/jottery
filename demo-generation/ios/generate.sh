#!/bin/bash
# Generate App Store screenshot assets: capture from simulators, then compose.
# Usage: ./generate.sh [--skip-capture]
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [ "${1:-}" != "--skip-capture" ]; then
  "$SCRIPT_DIR/capture.sh"
fi

cd "$REPO_ROOT"
npx playwright test --config demo-generation/ios/compose/playwright.config.ts
echo "✓ Finished assets in demo-generation/screenshots/appstore/"
