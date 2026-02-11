#!/usr/bin/env bash

# Demo & Screenshot Generation Script
# This script regenerates all or specific landing page screenshots and demos
#
# Usage from project root:
#   ./demo-generation/generate.sh --lang ja
#   ./demo-generation/generate.sh --all
#
# Usage from demo-generation directory:
#   ./generate.sh --lang ja
#   ./generate.sh --all

set -e  # Exit on error

# Determine project root - handle running from project root or demo-generation/
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ "$SCRIPT_DIR" == */demo-generation ]]; then
  PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
else
  PROJECT_ROOT="$SCRIPT_DIR"
fi
cd "$PROJECT_ROOT"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Default language (use DEMO_LANG to avoid conflict with system LANG)
DEMO_LANG="${DEMO_LANG:-en-GB}"

PATH=tui/target/release/:$PATH

# Available languages for screenshot generation
AVAILABLE_LANGS="de el en-GB en-US es fr it ja ko nl pl pt ru tr zh"

# Parse command line arguments
SPECIFIC_DEMO=""
SHOW_LIST=false
ALL_LANGS=false
ALL_LANGS_TUI=false
ALL_EVERYTHING=false
VERBOSE=false

while [[ $# -gt 0 ]]; do
  case $1 in
    --list)
      SHOW_LIST=true
      shift
      ;;
    --name)
      SPECIFIC_DEMO="$2"
      shift 2
      ;;
    --lang)
      DEMO_LANG="$2"
      shift 2
      ;;
    --all-langs)
      ALL_LANGS=true
      shift
      ;;
    --all-langs-tui)
      ALL_LANGS_TUI=true
      shift
      ;;
    --all)
      ALL_EVERYTHING=true
      shift
      ;;
    --verbose|-v)
      VERBOSE=true
      shift
      ;;
    --help)
      echo "Usage: $0 [OPTIONS]"
      echo ""
      echo "Options:"
      echo "  --all               Generate EVERYTHING (web + TUI for all languages)"
      echo "  --all-langs         Generate web screenshots for all languages"
      echo "  --all-langs-tui     Generate TUI demos (VHS) for all languages"
      echo "  --lang LANGUAGE     Set language for single-language mode (default: en-GB)"
      echo "  --name DEMO_NAME    Generate specific demo only"
      echo "  --list              List all available demos"
      echo "  --verbose, -v       Show debug output"
      echo "  --help              Show this help message"
      echo ""
      echo "Available languages: $AVAILABLE_LANGS"
      echo ""
      echo "Examples:"
      echo "  $0                           # Generate web screenshots for en-GB"
      echo "  $0 --lang ja                 # Generate web screenshots for Japanese"
      echo "  $0 --all-langs               # Generate web screenshots for all languages"
      echo "  $0 --all-langs-tui           # Generate TUI demos for all languages"
      echo "  $0 --all                     # Generate EVERYTHING (web + TUI, all languages)"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      echo "Run '$0 --help' for usage information"
      exit 1
      ;;
  esac
done

# Debug logging function
debug() {
  if [ "$VERBOSE" = true ]; then
    echo "[DEBUG] $1"
  fi
}

debug "Project root: $PROJECT_ROOT"
debug "Current dir: $(pwd)"

echo ""
echo "🎬 Jottery Demo Generation"
echo "=========================="
if [ "$ALL_EVERYTHING" = true ]; then
  echo "Mode: FULL regeneration (web + TUI, all languages)"
  echo "Languages: $AVAILABLE_LANGS"
elif [ "$ALL_LANGS" = true ]; then
  echo "Mode: All languages (web screenshots)"
  echo "Languages: $AVAILABLE_LANGS"
elif [ "$ALL_LANGS_TUI" = true ]; then
  echo "Mode: All languages (TUI demos)"
  echo "Languages: $AVAILABLE_LANGS"
else
  echo "Mode: Single language (web screenshots)"
  echo "Language: $DEMO_LANG"
fi
echo ""

# Check dependencies
echo -e "${BLUE}Checking dependencies...${NC}"
command -v npm >/dev/null 2>&1 || { echo "❌ npm is required but not installed."; exit 1; }
echo -e "${GREEN}✓ npm found${NC}"

if [ "$ALL_LANGS_TUI" = true ] || [ "$ALL_EVERYTHING" = true ]; then
  command -v vhs >/dev/null 2>&1 || { echo "❌ vhs is required for TUI demos. Run: brew install vhs"; exit 1; }
  echo -e "${GREEN}✓ vhs found${NC}"

  if [ ! -x "$PROJECT_ROOT/tui/target/release/jottery" ]; then
    echo -e "${RED}❌ jottery TUI binary not found. Build with: cd tui && cargo build --release${NC}"
    exit 1
  fi
  echo -e "${GREEN}✓ jottery TUI binary found${NC}"
fi
echo ""

# Output directories (absolute paths for clarity)
SCREENSHOTS_TEMP="$PROJECT_ROOT/screenshots"
SCREENSHOTS_PUBLIC="$PROJECT_ROOT/public/screenshots"

debug "Temp screenshot dir: $SCREENSHOTS_TEMP"
debug "Public screenshot dir: $SCREENSHOTS_PUBLIC"

# Function to generate web screenshots for a single language
generate_lang_screenshots() {
  local lang="$1"

  echo -e "${CYAN}═══════════════════════════════════════${NC}"
  echo -e "${YELLOW}📸 Web screenshots: $lang${NC}"
  echo -e "${CYAN}═══════════════════════════════════════${NC}"

  # Check if demo notes file exists for this language
  DEMO_FILE="$PROJECT_ROOT/demo-generation/jottery-demo-notes-${lang}.json"
  if [ ! -f "$DEMO_FILE" ]; then
    echo -e "${RED}❌ Error: $DEMO_FILE not found, skipping${NC}"
    return 1
  fi
  debug "Using demo file: $DEMO_FILE"

  # Create output directory
  local LANG_OUTPUT_DIR="$SCREENSHOTS_TEMP/$lang"
  mkdir -p "$LANG_OUTPUT_DIR"
  debug "Output directory: $LANG_OUTPUT_DIR"

  # Run playwright tests with DEMO_LANG env var (DEMO_LANG avoids conflict with system LANG)
  debug "Running: DEMO_LANG=$lang npx playwright test --config=demo-generation/playwright.config.ts"

  if DEMO_LANG=$lang npx playwright test --config=demo-generation/playwright.config.ts; then
    echo -e "${GREEN}✓ Playwright tests passed for $lang${NC}"
  else
    local exit_code=$?
    echo -e "${RED}❌ Tests failed for $lang (exit code: $exit_code)${NC}"
    return $exit_code
  fi

  # Check what was generated
  debug "Checking output directory: $LANG_OUTPUT_DIR"
  if [ -d "$LANG_OUTPUT_DIR" ]; then
    local count=$(ls -1 "$LANG_OUTPUT_DIR"/*.png 2>/dev/null | wc -l | tr -d ' ')
    debug "Found $count PNG files in $LANG_OUTPUT_DIR"
  else
    echo -e "${RED}❌ Output directory not created: $LANG_OUTPUT_DIR${NC}"
    return 1
  fi

  # Copy screenshots to public directory
  if [ -d "$LANG_OUTPUT_DIR" ] && [ "$(ls -A "$LANG_OUTPUT_DIR" 2>/dev/null)" ]; then
    mkdir -p "$SCREENSHOTS_PUBLIC/$lang"
    cp -r "$LANG_OUTPUT_DIR"/* "$SCREENSHOTS_PUBLIC/$lang/"
    echo -e "${GREEN}✓ Screenshots copied to $SCREENSHOTS_PUBLIC/$lang/${NC}"
  else
    echo -e "${YELLOW}⚠ No screenshots found to copy${NC}"
  fi

  echo ""
}

# Function to generate TUI demos for a single language
generate_lang_tui() {
  local lang="$1"

  echo -e "${CYAN}═══════════════════════════════════════${NC}"
  echo -e "${YELLOW}🎬 TUI demos: $lang${NC}"
  echo -e "${CYAN}═══════════════════════════════════════${NC}"

  # Use the dedicated TUI demo generation script
  if "$PROJECT_ROOT/demo-generation/generate-tui-demo.sh" "$lang"; then
    echo -e "${GREEN}✓ TUI demos generated for $lang${NC}"
  else
    echo -e "${RED}❌ TUI demo generation failed for $lang${NC}"
    return 1
  fi

  echo ""
}

# Track results
FAILED_WEB=""
PASSED_WEB=""
FAILED_TUI=""
PASSED_TUI=""

# Handle --all mode (everything)
if [ "$ALL_EVERYTHING" = true ]; then
  echo -e "${BLUE}═══════════════════════════════════════${NC}"
  echo -e "${BLUE}  FULL REGENERATION: Web + TUI, All Languages${NC}"
  echo -e "${BLUE}═══════════════════════════════════════${NC}"
  echo ""

  # First: Web screenshots for all languages
  echo -e "${CYAN}Phase 1: Web Screenshots${NC}"
  echo ""

  for lang in $AVAILABLE_LANGS; do
    if generate_lang_screenshots "$lang"; then
      PASSED_WEB="$PASSED_WEB $lang"
    else
      FAILED_WEB="$FAILED_WEB $lang"
      echo -e "${YELLOW}⚠ Continuing to next language...${NC}"
      echo ""
    fi
  done

  # Second: TUI demos for all languages
  echo -e "${CYAN}Phase 2: TUI Demos${NC}"
  echo ""

  for lang in $AVAILABLE_LANGS; do
    if generate_lang_tui "$lang"; then
      PASSED_TUI="$PASSED_TUI $lang"
    else
      FAILED_TUI="$FAILED_TUI $lang"
      echo -e "${YELLOW}⚠ Continuing to next language...${NC}"
      echo ""
    fi
  done

  echo ""
  echo "═══════════════════════════════════════"
  echo "Summary:"
  echo ""
  echo "Web Screenshots:"
  if [ -n "$PASSED_WEB" ]; then
    echo -e "${GREEN}  ✓ Passed:$PASSED_WEB${NC}"
  fi
  if [ -n "$FAILED_WEB" ]; then
    echo -e "${RED}  ❌ Failed:$FAILED_WEB${NC}"
  fi
  echo ""
  echo "TUI Demos:"
  if [ -n "$PASSED_TUI" ]; then
    echo -e "${GREEN}  ✓ Passed:$PASSED_TUI${NC}"
  fi
  if [ -n "$FAILED_TUI" ]; then
    echo -e "${RED}  ❌ Failed:$FAILED_TUI${NC}"
  fi
  echo ""
  echo "Generated files in: $SCREENSHOTS_PUBLIC/<lang>/"
  echo -e "${GREEN}🎉 Full regeneration complete!${NC}"
  exit 0
fi

# Handle --all-langs mode (web only)
if [ "$ALL_LANGS" = true ]; then
  echo -e "${BLUE}Generating web screenshots for all languages...${NC}"
  echo ""

  for lang in $AVAILABLE_LANGS; do
    if generate_lang_screenshots "$lang"; then
      PASSED_WEB="$PASSED_WEB $lang"
    else
      FAILED_WEB="$FAILED_WEB $lang"
      echo -e "${YELLOW}⚠ Continuing to next language...${NC}"
      echo ""
    fi
  done

  echo ""
  echo "═══════════════════════════════════════"
  echo "Summary:"
  if [ -n "$PASSED_WEB" ]; then
    echo -e "${GREEN}✓ Passed:$PASSED_WEB${NC}"
  fi
  if [ -n "$FAILED_WEB" ]; then
    echo -e "${RED}❌ Failed:$FAILED_WEB${NC}"
  fi
  echo ""
  echo "Generated files in: $SCREENSHOTS_PUBLIC/<lang>/"
  exit 0
fi

# Handle --all-langs-tui mode
if [ "$ALL_LANGS_TUI" = true ]; then
  echo -e "${BLUE}Generating TUI demos for all languages...${NC}"
  echo ""

  for lang in $AVAILABLE_LANGS; do
    if generate_lang_tui "$lang"; then
      PASSED_TUI="$PASSED_TUI $lang"
    else
      FAILED_TUI="$FAILED_TUI $lang"
      echo -e "${YELLOW}⚠ Continuing to next language...${NC}"
      echo ""
    fi
  done

  echo ""
  echo "═══════════════════════════════════════"
  echo "Summary:"
  if [ -n "$PASSED_TUI" ]; then
    echo -e "${GREEN}✓ Passed:$PASSED_TUI${NC}"
  fi
  if [ -n "$FAILED_TUI" ]; then
    echo -e "${RED}❌ Failed:$FAILED_TUI${NC}"
  fi
  echo ""
  echo "Generated files in: $SCREENSHOTS_PUBLIC/<lang>/"
  echo -e "${GREEN}🎉 TUI demo generation complete!${NC}"
  exit 0
fi

# Handle single language mode (default - web screenshots only)
generate_lang_screenshots "$DEMO_LANG"

echo ""
echo -e "${GREEN}🎉 Demo generation complete!${NC}"
echo ""
echo "Generated files in: $SCREENSHOTS_PUBLIC/$DEMO_LANG/"
