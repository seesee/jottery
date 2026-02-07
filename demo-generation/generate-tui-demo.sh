#!/usr/bin/env bash
#
# Generate TUI demo GIFs for multi-language screenshots
#
# This script runs VHS tapes with language-specific demo databases.
#
# Usage:
#   ./generate-tui-demo.sh [language] [tape]
#
# Examples:
#   ./generate-tui-demo.sh en-GB                    # All tapes for English
#   ./generate-tui-demo.sh en-GB tui-interface      # Specific tape
#   ./generate-tui-demo.sh                          # All languages, all tapes

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DEMO_DIR="$SCRIPT_DIR/tui-demo"
VHS_DIR="$SCRIPT_DIR/vhs"
OUTPUT_DIR="$PROJECT_ROOT/public/screenshots"

# All supported languages
ALL_LANGUAGES=(
    "en-GB" "en-US" "de" "fr" "es" "pt" "it" "nl" "pl" "ru" "tr" "el" "ja" "ko" "zh"
)

# Available tapes
ALL_TAPES=(
    "tui-interface"
    "tui-cli"
    "tui-piping"
    "tui-sync"
)

# Colours for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

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

# Check if VHS is installed
check_vhs() {
    if ! command -v vhs &> /dev/null; then
        log_error "VHS is not installed. Install with: brew install vhs"
        exit 1
    fi
}

# Generate demo for a single language and tape
generate_tape() {
    local lang="$1"
    local tape_name="$2"
    local db_path="$DEMO_DIR/$lang/jottery.db"
    local tape_file="$VHS_DIR/${tape_name}.tape"
    local output_file="$OUTPUT_DIR/$lang/${tape_name}.gif"

    # Check demo database exists
    if [[ ! -f "$db_path" ]]; then
        log_warn "Demo database not found: $db_path (run setup-tui-demo.sh first)"
        return 1
    fi

    # Check tape file exists
    if [[ ! -f "$tape_file" ]]; then
        log_error "Tape file not found: $tape_file"
        return 1
    fi

    # Create output directory
    mkdir -p "$(dirname "$output_file")"

    log_info "Generating: $lang / $tape_name"

    # Run VHS with environment variables
    # DB_PATH is used by the tape file
    export DB_PATH="$db_path"

    if vhs "$tape_file" --output "$output_file" 2>&1; then
        log_success "  Created: $output_file"
        return 0
    else
        log_error "  Failed: $tape_name for $lang"
        return 1
    fi
}

# Generate all tapes for a language
generate_language() {
    local lang="$1"
    shift
    local tapes=("$@")

    if [[ ${#tapes[@]} -eq 0 ]]; then
        tapes=("${ALL_TAPES[@]}")
    fi

    for tape in "${tapes[@]}"; do
        generate_tape "$lang" "$tape" || true
    done
}

# Main
main() {
    echo ""
    echo "========================================"
    echo "  Jottery TUI Demo Generation"
    echo "========================================"
    echo ""

    check_vhs

    # Parse arguments
    local languages=()
    local tapes=()

    if [[ $# -eq 0 ]]; then
        languages=("${ALL_LANGUAGES[@]}")
    elif [[ $# -eq 1 ]]; then
        languages=("$1")
    else
        languages=("$1")
        shift
        tapes=("$@")
    fi

    for lang in "${languages[@]}"; do
        generate_language "$lang" "${tapes[@]}"
    done

    echo ""
    log_success "Generation complete"
    echo ""
}

main "$@"
