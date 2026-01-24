#!/bin/bash

# Demo & Screenshot Generation Script
# This script regenerates all landing page screenshots and demos

set -e  # Exit on error

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo "🎬 Jottery Demo Generation"
echo "=========================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check dependencies
echo -e "${BLUE}Checking dependencies...${NC}"
command -v npm >/dev/null 2>&1 || { echo "❌ npm is required but not installed."; exit 1; }
command -v vhs >/dev/null 2>&1 || { echo "❌ vhs is required but not installed. Run: brew install vhs"; exit 1; }

echo -e "${GREEN}✓ All dependencies found${NC}"
echo ""

# Step 1: Generate Web Screenshots with Playwright
echo -e "${BLUE}Step 1: Generating web screenshots with Playwright...${NC}"
echo "This will start the dev server and capture screenshots"
echo ""

npx playwright test --config=demo-generation/playwright.config.ts

echo -e "${GREEN}✓ Web screenshots generated${NC}"
echo ""

# Step 2: Generate TUI Demos with VHS
echo -e "${BLUE}Step 2: Generating TUI demos with VHS...${NC}"
echo ""

VHS_SCRIPTS=(
  "demo-generation/vhs/tui-cli.tape"
  "demo-generation/vhs/tui-piping.tape"
  "demo-generation/vhs/tui-interface.tape"
  "demo-generation/vhs/tui-sync.tape"
)

for script in "${VHS_SCRIPTS[@]}"; do
  if [ -f "$script" ]; then
    echo -e "${YELLOW}Generating: $(basename $script .tape)...${NC}"
    vhs "$script"
  else
    echo -e "${YELLOW}⚠ Warning: $script not found, skipping${NC}"
  fi
done

echo -e "${GREEN}✓ TUI demos generated${NC}"
echo ""

# Step 3: Copy screenshots to public directory
echo -e "${BLUE}Step 3: Copying screenshots to public/screenshots/...${NC}"

# Web screenshots are already in public/screenshots/ from Playwright tests
# Just verify they exist
WEB_SCREENSHOTS=(
  "01-main-interface-light.png"
  "01-main-interface-light-preview.png"
  "01-main-interface-light-japan-preview.png"
  "01-main-interface-light-japan.png"
  "01-main-interface-dark.png"
  "01-main-interface-dark-preview.png"
  "01-main-interface-dark-japan-preview.png"
  "01-main-interface-dark-japan.png"
  "02-python-syntax-light.png"
  "02-python-syntax-dark.png"
  "03-multi-select-light.png"
  "03-multi-select-dark.png"
  "04-version-history-light.png"
  "04-version-history-dark.png"
  "05-calculator-light.png"
  "05-calculator-dark.png"
)

TUI_GIFS=(
  "tui-cli.gif"
  "tui-piping.gif"
  "tui-interface.gif"
  "tui-sync.gif"
)

# Verify web screenshots
echo "Verifying web screenshots..."
missing=0
for screenshot in "${WEB_SCREENSHOTS[@]}"; do
  if [ ! -f "public/screenshots/$screenshot" ]; then
    echo -e "${YELLOW}⚠ Missing: $screenshot${NC}"
    ((missing++))
  fi
done

# Verify TUI GIFs
echo "Verifying TUI demos..."
for gif in "${TUI_GIFS[@]}"; do
  if [ ! -f "public/screenshots/$gif" ]; then
    echo -e "${YELLOW}⚠ Missing: $gif${NC}"
    ((missing++))
  fi
done

if [ $missing -eq 0 ]; then
  echo -e "${GREEN}✓ All screenshots and demos present${NC}"
else
  echo -e "${YELLOW}⚠ $missing files missing or not generated${NC}"
fi

echo ""
echo -e "${GREEN}🎉 Demo generation complete!${NC}"
echo ""
echo "Generated files:"
echo "  - 16 web screenshots (light + dark mode)"
echo "  - 4 TUI demo GIFs"
echo ""
echo "All files are in: public/screenshots/"
