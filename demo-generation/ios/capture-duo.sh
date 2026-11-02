#!/bin/bash
# Capture screenshots of Jottery on the iPhone Duo simulator, in both poses.
#
# Usage: ./capture-duo.sh [--pose closed|open|both] [--wait-minutes N] [--skip-build]
#
# The closed pose (outer display) is the simulator's default. The open pose
# (inner display) can't be set from the command line: open the iPhone Duo in
# Xcode's Device Hub (Xcode > Open Developer Tool > Device Hub, or
# DeviceHub.app inside Xcode) and press its "open" control. The script waits
# until the inner display starts drawing, then captures it.
#
# Output: demo-generation/ios/raw/iphone-duo-closed/ and iphone-duo-open/
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="$REPO_ROOT/ios-native/Jottery"
RAW_DIR="$SCRIPT_DIR/raw"
NOTES_PATH="$REPO_ROOT/demo-generation/jottery-demo-notes-en-GB.json"
BUNDLE_ID="com.jottery.ios"
DERIVED="$SCRIPT_DIR/.derived"
SIM_NAME="iPhone Duo"
OUTER="primary"     # outer display
INNER="primary-1"   # inner display
SETTLE_SECONDS=7

POSE="both"; WAIT_MINUTES=30; SKIP_BUILD=""
while [ $# -gt 0 ]; do
  case "$1" in
    --pose) POSE="$2"; shift 2 ;;
    --wait-minutes) WAIT_MINUTES="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

# name | theme | -demo-screen value
SCREENS=(
  "01-list|dark|list"
  "02-editor|dark|note:Welcome"
  "03-search|dark|search:#recipe"
  "04-calc|dark|note:Holiday"
  "05-lock|dark|lock"
  "06-settings|dark|sync"
  "07-list-light|light|list"
)

APP_PATH="$DERIVED/Build/Products/Debug-iphonesimulator/Jottery.app"
if [ -z "$SKIP_BUILD" ]; then
  echo "→ Building Jottery (Debug, simulator)..."
  xcodebuild -project "$PROJECT_DIR/Jottery.xcodeproj" -scheme Jottery \
    -configuration Debug -destination 'generic/platform=iOS Simulator' \
    -derivedDataPath "$DERIVED" BUILD_DIR="$DERIVED/Build/Products" build | tail -2
fi
[ -d "$APP_PATH" ] || { echo "App not found at $APP_PATH" >&2; exit 1; }

UDID=$(xcrun simctl list devices available | grep -F "$SIM_NAME (" | grep -oE '[0-9A-F-]{36}' | head -1)
[ -n "$UDID" ] || { echo "Simulator not found: $SIM_NAME" >&2; exit 1; }
echo "→ $SIM_NAME ($UDID)"
xcrun simctl boot "$UDID" 2>/dev/null || true
xcrun simctl bootstatus "$UDID" -b >/dev/null
xcrun simctl status_bar "$UDID" override \
  --time "09:41" --batteryState charged --batteryLevel 100 --wifiBars 3 --cellularBars 4
xcrun simctl install "$UDID" "$APP_PATH"

# A display that isn't drawing (the closed device's inner display, say)
# screenshots as solid black, which compresses to a few kilobytes.
display_is_lit() {
  local probe
  probe="$(mktemp -t duo-probe).png"
  xcrun simctl io "$UDID" screenshot --display="$1" "$probe" >/dev/null 2>&1 || { rm -f "$probe"; return 1; }
  local bytes
  bytes=$(stat -f%z "$probe")
  rm -f "$probe"
  [ "$bytes" -gt 40000 ]
}

capture_set() {
  local display="$1" out="$RAW_DIR/$2"
  mkdir -p "$out"
  for entry in "${SCREENS[@]}"; do
    IFS='|' read -r name theme screen <<<"$entry"
    echo "  • $2/$name"
    xcrun simctl ui "$UDID" appearance "$theme"
    xcrun simctl terminate "$UDID" "$BUNDLE_ID" 2>/dev/null || true
    SIMCTL_CHILD_DEMO_NOTES_PATH="$NOTES_PATH" \
      xcrun simctl launch "$UDID" "$BUNDLE_ID" -demo-seed -demo-theme "$theme" -demo-screen "$screen" >/dev/null
    sleep "$SETTLE_SECONDS"
    xcrun simctl io "$UDID" screenshot --display="$display" "$out/$name.png" >/dev/null
  done
  xcrun simctl ui "$UDID" appearance dark
}

if [ "$POSE" = "closed" ] || [ "$POSE" = "both" ]; then
  if display_is_lit "$OUTER"; then
    echo "→ Closed pose (outer display)"
    capture_set "$OUTER" iphone-duo-closed
  else
    echo "! The outer display isn't drawing; close the device in Device Hub for the closed pose." >&2
  fi
fi

if [ "$POSE" = "open" ] || [ "$POSE" = "both" ]; then
  echo "→ Open pose: open the iPhone Duo in Device Hub. Waiting up to $WAIT_MINUTES min..."
  deadline=$(( $(date +%s) + WAIT_MINUTES * 60 ))
  until display_is_lit "$INNER"; do
    [ "$(date +%s)" -lt "$deadline" ] || { echo "! Timed out waiting for the inner display." >&2; exit 2; }
    sleep 5
  done
  sleep 3
  echo "→ Open pose (inner display)"
  capture_set "$INNER" iphone-duo-open
fi

xcrun simctl terminate "$UDID" "$BUNDLE_ID" 2>/dev/null || true
echo "✓ Screenshots in $RAW_DIR"
