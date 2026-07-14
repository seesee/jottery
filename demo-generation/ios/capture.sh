#!/bin/bash
# Capture raw App Store screenshots from iOS simulators.
# Usage: ./capture.sh [--device iphone-69|ipad-13] [--screen NAME]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="$REPO_ROOT/ios-native/Jottery"
SCREENS_JSON="$SCRIPT_DIR/screens.json"
RAW_DIR="$SCRIPT_DIR/raw"
NOTES_PATH="$REPO_ROOT/demo-generation/jottery-demo-notes-en-GB.json"
BUNDLE_ID="com.jottery.ios"
DERIVED="$SCRIPT_DIR/.derived"
SETTLE_SECONDS=6

ONLY_DEVICE=""; ONLY_SCREEN=""
while [ $# -gt 0 ]; do
  case "$1" in
    --device) ONLY_DEVICE="$2"; shift 2 ;;
    --screen) ONLY_SCREEN="$2"; shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }
[ -f "$NOTES_PATH" ] || { echo "Demo pack not found: $NOTES_PATH" >&2; exit 1; }

echo "→ Building Jottery (Debug, generic simulator)..."
# BUILD_DIR is passed explicitly because some machines have a global Xcode
# "Custom" build location preference (Xcode > Settings > Locations) that
# otherwise overrides -derivedDataPath's effect on BUILT_PRODUCTS_DIR,
# silently sending the built app to ~/Library/Developer/Xcode/DerivedData.
xcodebuild -project "$PROJECT_DIR/Jottery.xcodeproj" -scheme Jottery \
  -configuration Debug -destination 'generic/platform=iOS Simulator' \
  -derivedDataPath "$DERIVED" BUILD_DIR="$DERIVED/Build/Products" build | tail -2
APP_PATH="$DERIVED/Build/Products/Debug-iphonesimulator/Jottery.app"
[ -d "$APP_PATH" ] || { echo "App not found at $APP_PATH" >&2; exit 1; }

jq -c '.devices[]' "$SCREENS_JSON" | while read -r device; do
  KEY=$(jq -r '.key' <<<"$device")
  SIM_NAME=$(jq -r '.simulator' <<<"$device")
  [ -n "$ONLY_DEVICE" ] && [ "$KEY" != "$ONLY_DEVICE" ] && continue

  UDID=$(xcrun simctl list devices available | grep -F "$SIM_NAME (" | grep -oE '[0-9A-F-]{36}' | head -1)
  [ -n "$UDID" ] || { echo "Simulator not found: $SIM_NAME" >&2; exit 1; }

  echo "→ [$KEY] booting $SIM_NAME ($UDID)"
  xcrun simctl boot "$UDID" 2>/dev/null || true
  xcrun simctl bootstatus "$UDID" -b

  # Clean status bar (9:41, full signal/wifi/battery)
  xcrun simctl status_bar "$UDID" override \
    --time "09:41" --batteryState charged --batteryLevel 100 \
    --cellularMode active --cellularBars 4 --wifiBars 3 --operatorName ""

  # Best-effort Face ID enrolment so the lock screen shows biometric unlock
  xcrun simctl spawn "$UDID" notifyutil -s com.apple.BiometricKit.enrollmentChanged 1 2>/dev/null || true
  xcrun simctl spawn "$UDID" notifyutil -p com.apple.BiometricKit.enrollmentChanged 2>/dev/null || true

  xcrun simctl install "$UDID" "$APP_PATH"
  mkdir -p "$RAW_DIR/$KEY"

  jq -c '.screens[]' "$SCREENS_JSON" | while read -r screen; do
    NAME=$(jq -r '.name' <<<"$screen")
    [ -n "$ONLY_SCREEN" ] && [ "$NAME" != "$ONLY_SCREEN" ] && continue
    jq -e --arg k "$KEY" '.devices | index($k)' <<<"$screen" >/dev/null || continue

    ARGS=$(jq -r '.args' <<<"$screen")
    THEME=$(jq -r '.theme' <<<"$screen")

    echo "  • $NAME (theme: $THEME)"
    xcrun simctl ui "$UDID" appearance "$THEME"
    xcrun simctl terminate "$UDID" "$BUNDLE_ID" 2>/dev/null || true
    # shellcheck disable=SC2086
    SIMCTL_CHILD_DEMO_NOTES_PATH="$NOTES_PATH" \
      xcrun simctl launch "$UDID" "$BUNDLE_ID" -demo-seed -demo-theme "$THEME" $ARGS >/dev/null
    sleep "$SETTLE_SECONDS"
    xcrun simctl io "$UDID" screenshot "$RAW_DIR/$KEY/$NAME.png" >/dev/null
  done

  xcrun simctl terminate "$UDID" "$BUNDLE_ID" 2>/dev/null || true
  xcrun simctl shutdown "$UDID"
done

echo "✓ Raw screenshots in $RAW_DIR"
