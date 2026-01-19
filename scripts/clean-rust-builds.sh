#!/bin/bash
# Clean up Rust build artifacts from server and tui subprojects
# This removes incremental builds, debug artifacts, and old dependency builds

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Rust Build Cleanup Script ==="
echo ""

# Function to get directory size
get_size() {
    if [ -d "$1" ]; then
        du -sh "$1" 2>/dev/null | cut -f1
    else
        echo "0B"
    fi
}

# Show current disk usage
echo "Current disk usage:"
echo "  server/target: $(get_size "$PROJECT_ROOT/server/target")"
echo "  tui/target:    $(get_size "$PROJECT_ROOT/tui/target")"
echo ""

# Calculate total before
TOTAL_BEFORE=$(du -sc "$PROJECT_ROOT/server/target" "$PROJECT_ROOT/tui/target" 2>/dev/null | tail -1 | cut -f1 || echo "0")

# Clean server
if [ -d "$PROJECT_ROOT/server" ]; then
    echo "Cleaning server/target..."
    cd "$PROJECT_ROOT/server"
    if command -v cargo &> /dev/null; then
        cargo clean 2>/dev/null || rm -rf target
    else
        rm -rf target
    fi
    echo "  ✓ server cleaned"
fi

# Clean tui
if [ -d "$PROJECT_ROOT/tui" ]; then
    echo "Cleaning tui/target..."
    cd "$PROJECT_ROOT/tui"
    if command -v cargo &> /dev/null; then
        cargo clean 2>/dev/null || rm -rf target
    else
        rm -rf target
    fi
    echo "  ✓ tui cleaned"
fi

echo ""
echo "=== Cleanup Complete ==="
echo ""
echo "Disk usage after cleanup:"
echo "  server/target: $(get_size "$PROJECT_ROOT/server/target")"
echo "  tui/target:    $(get_size "$PROJECT_ROOT/tui/target")"
echo ""

# Calculate total after
TOTAL_AFTER=$(du -sc "$PROJECT_ROOT/server/target" "$PROJECT_ROOT/tui/target" 2>/dev/null | tail -1 | cut -f1 || echo "0")

# Show savings (approximate)
if [ "$TOTAL_BEFORE" != "0" ]; then
    SAVED_KB=$((TOTAL_BEFORE - TOTAL_AFTER))
    if [ $SAVED_KB -gt 1048576 ]; then
        SAVED_GB=$(echo "scale=2; $SAVED_KB / 1048576" | bc)
        echo "Space freed: ~${SAVED_GB}GB"
    elif [ $SAVED_KB -gt 1024 ]; then
        SAVED_MB=$(echo "scale=2; $SAVED_KB / 1024" | bc)
        echo "Space freed: ~${SAVED_MB}MB"
    else
        echo "Space freed: ~${SAVED_KB}KB"
    fi
fi
