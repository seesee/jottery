#!/usr/bin/env bash

# Demo & Screenshot Generation Script
# This script regenerates all or specific landing page screenshots and demos
#
# Usage from project root:
#   ./demo-generation/generate.sh --lang ja
#   ./demo-generation/generate.sh --all-langs
#
# Usage from demo-generation directory:
#   ./generate.sh --lang ja

set -e  # Exit on error

# Determine project root - handle running from project root or demo-generation/
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ "$SCRIPT_DIR" == */demo-generation ]]; then
  PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
else
  PROJECT_ROOT="$SCRIPT_DIR"
fi
cd "$PROJECT_ROOT"

echo "[DEBUG] Project root: $PROJECT_ROOT"
echo "[DEBUG] Current dir: $(pwd)"

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
    --help)
      echo "Usage: $0 [OPTIONS]"
      echo ""
      echo "Options:"
      echo "  --list              List all available demos"
      echo "  --name DEMO_NAME    Generate specific demo only"
      echo "  --lang LANGUAGE     Set language (default: en-GB)"
      echo "  --all-langs         Generate web screenshots for all languages"
      echo "  --all-langs-tui     Generate TUI demos (VHS) for all languages"
      echo "  --help              Show this help message"
      echo ""
      echo "Available languages: $AVAILABLE_LANGS"
      echo ""
      echo "Examples:"
      echo "  $0                           # Generate all demos for en-GB"
      echo "  $0 --lang ja                 # Generate for Japanese"
      echo "  $0 --all-langs               # Generate web screenshots for all languages"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      echo "Run '$0 --help' for usage information"
      exit 1
      ;;
  esac
done

echo "🎬 Jottery Demo Generation"
echo "=========================="
if [ "$ALL_LANGS" = true ]; then
  echo "Mode: All languages (web screenshots)"
  echo "Languages: $AVAILABLE_LANGS"
elif [ "$ALL_LANGS_TUI" = true ]; then
  echo "Mode: All languages (TUI demos)"
  echo "Languages: $AVAILABLE_LANGS"
else
  echo "Language: $DEMO_LANG"
fi
echo ""

# Check dependencies
echo -e "${BLUE}Checking dependencies...${NC}"
command -v npm >/dev/null 2>&1 || { echo "❌ npm is required but not installed."; exit 1; }
echo -e "${GREEN}✓ All dependencies found${NC}"
echo ""

# Output directories (absolute paths for clarity)
SCREENSHOTS_TEMP="$PROJECT_ROOT/screenshots"
SCREENSHOTS_PUBLIC="$PROJECT_ROOT/public/screenshots"

echo "[DEBUG] Temp screenshot dir: $SCREENSHOTS_TEMP"
echo "[DEBUG] Public screenshot dir: $SCREENSHOTS_PUBLIC"

# Function to generate web screenshots for a single language
generate_lang_screenshots() {
  local lang="$1"

  echo -e "${CYAN}═══════════════════════════════════════${NC}"
  echo -e "${YELLOW}Generating screenshots for: $lang${NC}"
  echo -e "${CYAN}═══════════════════════════════════════${NC}"

  # Check if demo notes file exists for this language
  DEMO_FILE="$PROJECT_ROOT/demo-generation/jottery-demo-notes-${lang}.json"
  if [ ! -f "$DEMO_FILE" ]; then
    echo -e "${RED}❌ Error: $DEMO_FILE not found, skipping${NC}"
    return 1
  fi
  echo "[DEBUG] Using demo file: $DEMO_FILE"

  # Create output directory
  local LANG_OUTPUT_DIR="$SCREENSHOTS_TEMP/$lang"
  mkdir -p "$LANG_OUTPUT_DIR"
  echo "[DEBUG] Output directory: $LANG_OUTPUT_DIR"

  # Run playwright tests with LANG env var
  # The test file reads process.env.LANG to determine language
  echo "[DEBUG] Running: LANG=$lang npx playwright test --config=demo-generation/playwright.config.ts"

  if LANG=$lang npx playwright test --config=demo-generation/playwright.config.ts; then
    echo -e "${GREEN}✓ All tests passed for $lang${NC}"
  else
    local exit_code=$?
    echo -e "${RED}❌ Tests failed for $lang (exit code: $exit_code)${NC}"
    return $exit_code
  fi

  # Check what was generated
  echo "[DEBUG] Checking output directory: $LANG_OUTPUT_DIR"
  if [ -d "$LANG_OUTPUT_DIR" ]; then
    local count=$(ls -1 "$LANG_OUTPUT_DIR"/*.png 2>/dev/null | wc -l | tr -d ' ')
    echo "[DEBUG] Found $count PNG files in $LANG_OUTPUT_DIR"
    if [ "$count" -gt 0 ]; then
      ls -la "$LANG_OUTPUT_DIR"/*.png | head -5
      echo "  ... (showing first 5)"
    fi
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

# Handle --all-langs mode
if [ "$ALL_LANGS" = true ]; then
  echo -e "${BLUE}Generating web screenshots for all languages...${NC}"
  echo ""

  FAILED_LANGS=""
  PASSED_LANGS=""

  for lang in $AVAILABLE_LANGS; do
    if generate_lang_screenshots "$lang"; then
      PASSED_LANGS="$PASSED_LANGS $lang"
    else
      FAILED_LANGS="$FAILED_LANGS $lang"
      echo -e "${YELLOW}⚠ Continuing to next language...${NC}"
      echo ""
    fi
  done

  echo ""
  echo "═══════════════════════════════════════"
  echo "Summary:"
  if [ -n "$PASSED_LANGS" ]; then
    echo -e "${GREEN}✓ Passed:$PASSED_LANGS${NC}"
  fi
  if [ -n "$FAILED_LANGS" ]; then
    echo -e "${RED}❌ Failed:$FAILED_LANGS${NC}"
  fi
  echo ""
  echo "Generated files in: $SCREENSHOTS_PUBLIC/<lang>/"
  exit 0
fi

# Handle single language mode
if [ "$ALL_LANGS" = false ] && [ "$ALL_LANGS_TUI" = false ]; then
  generate_lang_screenshots "$DEMO_LANG"

  echo ""
  echo -e "${GREEN}🎉 Demo generation complete!${NC}"
  echo ""
  echo "Generated files in: $SCREENSHOTS_PUBLIC/$DEMO_LANG/"
fi

# Handle --all-langs-tui mode
if [ "$ALL_LANGS_TUI" = true ]; then
  echo -e "${BLUE}Generating TUI demos for all languages...${NC}"
  echo ""

  command -v vhs >/dev/null 2>&1 || { echo "❌ vhs is required but not installed. Run: brew install vhs"; exit 1; }

  for lang in $AVAILABLE_LANGS; do
    echo -e "${CYAN}═══════════════════════════════════════${NC}"
    echo -e "${YELLOW}Generating TUI demos for: $lang${NC}"
    echo -e "${CYAN}═══════════════════════════════════════${NC}"

    # Ensure output directory exists
    mkdir -p "$SCREENSHOTS_PUBLIC/$lang"

    # Generate all TUI demos for this language
    for demo in tui-cli tui-piping tui-interface tui-sync; do
      tape_file="demo-generation/vhs/${demo}.tape"
      if [ -f "$tape_file" ]; then
        echo -e "${YELLOW}Generating: $demo for $lang...${NC}"
        DEMO_LANG="$lang" vhs "$tape_file" || {
          echo -e "${YELLOW}⚠ Failed to generate $demo for $lang, continuing...${NC}"
        }
        echo -e "${GREEN}✓ $demo generated for $lang${NC}"
      fi
    done

    echo ""
  done

  echo ""
  echo -e "${GREEN}🎉 All language TUI demos generated!${NC}"
  echo ""
  echo "Generated files in: $SCREENSHOTS_PUBLIC/<lang>/"
  exit 0
fi
