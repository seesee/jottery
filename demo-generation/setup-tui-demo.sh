#!/usr/bin/env bash
#
# Setup TUI demo databases for multi-language screenshot generation
#
# This script creates isolated demo databases for each language with:
# - Demo notes imported from JSON files
# - Password stored for auto-unlock (no manual entry needed)
#
# Usage:
#   ./setup-tui-demo.sh [language]
#
# Examples:
#   ./setup-tui-demo.sh en-GB    # Setup English demo only
#   ./setup-tui-demo.sh          # Setup all languages

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TUI_DIR="$PROJECT_ROOT/tui"
DEMO_DIR="$SCRIPT_DIR/tui-demo"
DEMO_PASSWORD="demo-password-2026"

# All supported languages
ALL_LANGUAGES=(
    "en-GB" "en-US" "de" "fr" "es" "pt" "it" "nl" "pl" "ru" "tr" "el" "ja" "ko" "zh"
)

# Colours for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Colour

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Build TUI if needed
build_tui() {
    log_info "Building TUI..."
    cd "$TUI_DIR"
    cargo build --release 2>/dev/null || {
        log_error "Failed to build TUI"
        exit 1
    }
    log_success "TUI built successfully"
}

# Setup demo for a single language
setup_language() {
    local lang="$1"
    local lang_dir="$DEMO_DIR/$lang"
    local db_path="$lang_dir/jottery.db"
    local demo_notes="$SCRIPT_DIR/jottery-demo-notes-${lang}.json"
    local jottery="$TUI_DIR/target/release/jottery"

    log_info "Setting up demo for: $lang"

    # Check demo notes file exists
    if [[ ! -f "$demo_notes" ]]; then
        log_warn "Demo notes not found: $demo_notes (skipping)"
        return 1
    fi

    # Create language directory
    mkdir -p "$lang_dir"

    # Remove existing database if present
    if [[ -f "$db_path" ]]; then
        rm -f "$db_path" "$db_path-wal" "$db_path-shm" "$lang_dir/.jottery_remember"
        log_info "  Removed existing database"
    fi

    # Import demo notes
    log_info "  Importing demo notes..."
    "$jottery" --database "$db_path" import --input "$demo_notes" --password "$DEMO_PASSWORD" || {
        log_error "  Failed to import notes for $lang"
        return 1
    }

    # Store password for auto-unlock
    log_info "  Storing password for auto-unlock..."
    "$jottery" --database "$db_path" store-password --password "$DEMO_PASSWORD" || {
        log_error "  Failed to store password for $lang"
        return 1
    }

    log_success "  Demo ready: $lang_dir"
    return 0
}

# Main
main() {
    echo ""
    echo "========================================"
    echo "  Jottery TUI Demo Setup"
    echo "========================================"
    echo ""

    # Build TUI first
    build_tui

    # Create demo directory
    mkdir -p "$DEMO_DIR"

    # Determine which languages to setup
    local languages=()
    if [[ $# -gt 0 ]]; then
        languages=("$@")
    else
        languages=("${ALL_LANGUAGES[@]}")
    fi

    local success_count=0
    local fail_count=0

    for lang in "${languages[@]}"; do
        if setup_language "$lang"; then
            success_count=$((success_count + 1))
        else
            fail_count=$((fail_count + 1))
        fi
    done

    echo ""
    echo "========================================"
    echo "  Summary"
    echo "========================================"
    echo ""
    log_info "Demo directory: $DEMO_DIR"
    log_success "Languages ready: $success_count"
    if [[ $fail_count -gt 0 ]]; then
        log_warn "Languages failed: $fail_count"
    fi
    echo ""
    echo "To run TUI with a demo database:"
    echo "  $TUI_DIR/target/release/jottery --database $DEMO_DIR/en-GB/jottery.db"
    echo ""
}

main "$@"
