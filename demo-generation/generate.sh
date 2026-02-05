#!/usr/bin/env bash

# Demo & Screenshot Generation Script
# This script regenerates all or specific landing page screenshots and demos

set -e  # Exit on error

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Default language (use DEMO_LANG to avoid conflict with system LANG)
DEMO_LANG="${DEMO_LANG:-en-GB}"

PATH=tui/target/release/:$PATH

# Available demos/screenshots
declare -A DEMOS
DEMOS[web-carousel-light]="Playwright carousel screenshots (light mode)"
DEMOS[web-carousel-dark]="Playwright carousel screenshots (dark mode)"
DEMOS[web-python-light]="Playwright Python syntax screenshot (light mode)"
DEMOS[web-python-dark]="Playwright Python syntax screenshot (dark mode)"
DEMOS[web-multiselect-light]="Playwright multi-select screenshot (light mode)"
DEMOS[web-multiselect-dark]="Playwright multi-select screenshot (dark mode)"
DEMOS[web-version-history-light]="Playwright version history screenshot (light mode)"
DEMOS[web-version-history-dark]="Playwright version history screenshot (dark mode)"
DEMOS[web-calculator-light]="Playwright calculator screenshot (light mode)"
DEMOS[web-calculator-dark]="Playwright calculator screenshot (dark mode)"
DEMOS[web-mobile-list-light]="Playwright mobile list view screenshot (light mode)"
DEMOS[web-mobile-list-dark]="Playwright mobile list view screenshot (dark mode)"
DEMOS[web-mobile-japan-light]="Playwright mobile Japan note screenshot (light mode)"
DEMOS[web-mobile-japan-dark]="Playwright mobile Japan note screenshot (dark mode)"
DEMOS[web-mobile-calculator-light]="Playwright mobile calculator screenshot (light mode)"
DEMOS[web-mobile-calculator-dark]="Playwright mobile calculator screenshot (dark mode)"
DEMOS[web-outliner-light]="Playwright outliner mode screenshot (light mode)"
DEMOS[web-outliner-dark]="Playwright outliner mode screenshot (dark mode)"
DEMOS[web-greek-ui-light]="Playwright Greek UI screenshot (light mode)"
DEMOS[web-greek-ui-dark]="Playwright Greek UI screenshot (dark mode)"
DEMOS[tui-cli]="VHS TUI CLI commands demo"
DEMOS[tui-interface]="VHS TUI interactive interface demo"
DEMOS[tui-piping]="VHS TUI piping content demo"
DEMOS[tui-sync]="VHS TUI sync commands demo"

# Available languages for screenshot generation
AVAILABLE_LANGS="de el en-GB en-US es fr it ja ko nl pl pt ru tr zh"

# Parse command line arguments
SPECIFIC_DEMO=""
SHOW_LIST=false
ALL_LANGS=false

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
    --help)
      echo "Usage: $0 [OPTIONS]"
      echo ""
      echo "Options:"
      echo "  --list              List all available demos"
      echo "  --name DEMO_NAME    Generate specific demo only"
      echo "  --lang LANGUAGE     Set language (default: en-GB)"
      echo "  --all-langs         Generate web screenshots for all languages"
      echo "  --help              Show this help message"
      echo ""
      echo "Available languages: $AVAILABLE_LANGS"
      echo ""
      echo "Examples:"
      echo "  $0                           # Generate all demos for en-GB"
      echo "  $0 --list                    # List available demos"
      echo "  $0 --name tui-cli            # Generate only TUI CLI demo"
      echo "  $0 --name web-carousel-light # Generate only light carousel"
      echo "  $0 --lang en-US              # Generate for en-US"
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

# Handle --list
if [ "$SHOW_LIST" = true ]; then
  echo -e "${CYAN}Available Demos:${NC}"
  echo ""
  for demo in "${!DEMOS[@]}"; do
    echo -e "  ${YELLOW}$demo${NC} - ${DEMOS[$demo]}"
  done | sort
  echo ""
  echo "Usage: $0 --name <demo-name>"
  exit 0
fi

# Validate specific demo if provided
if [ -n "$SPECIFIC_DEMO" ] && [ -z "${DEMOS[$SPECIFIC_DEMO]}" ]; then
  echo -e "${YELLOW}Error: Unknown demo '$SPECIFIC_DEMO'${NC}"
  echo ""
  echo "Run '$0 --list' to see available demos"
  exit 1
fi

echo "🎬 Jottery Demo Generation"
echo "=========================="
if [ "$ALL_LANGS" = true ]; then
  echo "Mode: All languages"
  echo "Languages: $AVAILABLE_LANGS"
else
  echo "Language: $DEMO_LANG"
fi
if [ -n "$SPECIFIC_DEMO" ]; then
  echo "Generating: $SPECIFIC_DEMO"
fi
echo ""

# Check dependencies
echo -e "${BLUE}Checking dependencies...${NC}"
command -v npm >/dev/null 2>&1 || { echo "❌ npm is required but not installed."; exit 1; }

if [ -z "$SPECIFIC_DEMO" ] || [[ "$SPECIFIC_DEMO" == tui-* ]]; then
  if [ "$ALL_LANGS" = false ]; then
    command -v vhs >/dev/null 2>&1 || { echo "❌ vhs is required but not installed. Run: brew install vhs"; exit 1; }
  fi
fi

echo -e "${GREEN}✓ All dependencies found${NC}"
echo ""

# Handle --all-langs mode
if [ "$ALL_LANGS" = true ]; then
  echo -e "${BLUE}Generating web screenshots for all languages...${NC}"
  echo ""

  for lang in $AVAILABLE_LANGS; do
    echo -e "${CYAN}═══════════════════════════════════════${NC}"
    echo -e "${YELLOW}Generating screenshots for: $lang${NC}"
    echo -e "${CYAN}═══════════════════════════════════════${NC}"

    # Check if demo notes file exists for this language
    DEMO_FILE="demo-generation/jottery-demo-notes-${lang}.json"
    if [ ! -f "$DEMO_FILE" ]; then
      echo -e "${YELLOW}⚠ Warning: $DEMO_FILE not found, skipping${NC}"
      continue
    fi

    # Generate web screenshots for this language
    LANG=$lang npx playwright test --config=demo-generation/playwright.config.ts || {
      echo -e "${YELLOW}⚠ Some tests failed for $lang, continuing...${NC}"
    }

    # Copy screenshots to public directory
    if [ -d "screenshots/$lang" ]; then
      mkdir -p "public/screenshots/$lang"
      cp -r screenshots/$lang/* "public/screenshots/$lang/"
      echo -e "${GREEN}✓ Screenshots copied to public/screenshots/$lang/${NC}"
    fi

    echo ""
  done

  # Clean up intermediate screenshots directory
  if [ -d "screenshots" ]; then
    echo -e "${BLUE}Cleaning up intermediate screenshots...${NC}"
    rm -rf screenshots
    echo -e "${GREEN}✓ Intermediate files removed${NC}"
  fi

  echo ""
  echo -e "${GREEN}🎉 All language screenshots generated!${NC}"
  echo ""
  echo "Generated files in: public/screenshots/<lang>/"
  exit 0
fi

# Function to generate web screenshots
generate_web_screenshots() {
  local test_filter="$1"
  echo -e "${BLUE}Generating web screenshots...${NC}"
  if [ -n "$test_filter" ]; then
    LANG=$DEMO_LANG npx playwright test --config=demo-generation/playwright.config.ts -g "$test_filter"
  else
    LANG=$DEMO_LANG npx playwright test --config=demo-generation/playwright.config.ts
  fi

  # Copy from screenshots/en-GB to public/screenshots/en-GB
  mkdir -p "public/screenshots/$DEMO_LANG"
  cp -r screenshots/$DEMO_LANG/* "public/screenshots/$DEMO_LANG/"
  echo -e "${GREEN}✓ Web screenshots copied to public/screenshots/$DEMO_LANG/${NC}"
}

# Function to generate VHS demo
generate_vhs_demo() {
  local demo_name="$1"
  local tape_file="demo-generation/vhs/${demo_name}.tape"

  if [ ! -f "$tape_file" ]; then
    echo -e "${YELLOW}⚠ Warning: $tape_file not found${NC}"
    return 1
  fi

  echo -e "${YELLOW}Generating: $demo_name...${NC}"
  vhs "$tape_file"
  echo -e "${GREEN}✓ $demo_name generated${NC}"
}

# Generate based on selection
if [ -z "$SPECIFIC_DEMO" ]; then
  # Generate all demos
  echo -e "${BLUE}Step 1: Generating all web screenshots...${NC}"
  generate_web_screenshots ""
  echo ""

  echo -e "${BLUE}Step 2: Generating all TUI demos...${NC}"
  for demo in tui-cli tui-piping tui-interface tui-sync; do
    generate_vhs_demo "$demo"
  done
  echo ""

else
  # Generate specific demo
  case "$SPECIFIC_DEMO" in
    web-carousel-light)
      generate_web_screenshots "01-light"
      ;;
    web-carousel-dark)
      generate_web_screenshots "01-dark"
      ;;
    web-python-light)
      generate_web_screenshots "02.*light.*Python"
      ;;
    web-python-dark)
      generate_web_screenshots "02.*dark.*Python"
      ;;
    web-multiselect-light)
      generate_web_screenshots "03.*light.*Multi"
      ;;
    web-multiselect-dark)
      generate_web_screenshots "03.*dark.*Multi"
      ;;
    web-version-history-light)
      generate_web_screenshots "04.*light.*Version"
      ;;
    web-version-history-dark)
      generate_web_screenshots "04.*dark.*Version"
      ;;
    web-calculator-light)
      generate_web_screenshots "05.*light.*Calculator"
      ;;
    web-calculator-dark)
      generate_web_screenshots "05.*dark.*Calculator"
      ;;
    web-mobile-list-light)
      generate_web_screenshots "06-mobile-light.*List"
      ;;
    web-mobile-list-dark)
      generate_web_screenshots "06-mobile-dark.*List"
      ;;
    web-mobile-japan-light)
      generate_web_screenshots "07-mobile-light.*Japan"
      ;;
    web-mobile-japan-dark)
      generate_web_screenshots "07-mobile-dark.*Japan"
      ;;
    web-mobile-calculator-light)
      generate_web_screenshots "08-mobile-light.*Calculator"
      ;;
    web-mobile-calculator-dark)
      generate_web_screenshots "08-mobile-dark.*Calculator"
      ;;
    web-outliner-light)
      generate_web_screenshots "09-light.*Outliner"
      ;;
    web-outliner-dark)
      generate_web_screenshots "09-dark.*Outliner"
      ;;
    web-greek-ui-light)
      generate_web_screenshots "10-light.*Greek"
      ;;
    web-greek-ui-dark)
      generate_web_screenshots "10-dark.*Greek"
      ;;
    tui-*)
      generate_vhs_demo "$SPECIFIC_DEMO"
      ;;
  esac
fi

echo ""
echo -e "${GREEN}🎉 Demo generation complete!${NC}"
echo ""
echo "Generated files in: public/screenshots/$DEMO_LANG/"

# Clean up intermediate screenshots directory
if [ -d "screenshots" ]; then
  echo -e "${BLUE}Cleaning up intermediate screenshots...${NC}"
  rm -rf screenshots
  echo -e "${GREEN}✓ Intermediate files removed${NC}"
fi
