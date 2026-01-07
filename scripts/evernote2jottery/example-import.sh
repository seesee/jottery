#!/bin/bash
# Example: Import all Evernote notebooks into Jottery

set -e

echo "=== Jottery ENEX Import Example ==="
echo

# Check if ENEX files exist
if ! ls *.enex 1> /dev/null 2>&1; then
    echo "No .enex files found in current directory!"
    echo
    echo "To use this script:"
    echo "1. Export your Evernote notebooks (File → Export Notes → ENEX format)"
    echo "2. Place all .enex files in this directory"
    echo "3. Run this script again"
    exit 1
fi

# Count notebooks
NOTEBOOK_COUNT=$(ls -1 *.enex | wc -l | tr -d ' ')
echo "Found $NOTEBOOK_COUNT notebook(s) to import"
echo

# List notebooks
echo "Notebooks:"
ls -1 *.enex | sed 's/^/  - /'
echo

# Ask for confirmation
read -p "Import all notebooks? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Import cancelled."
    exit 0
fi

# Run conversion
echo
echo "Starting conversion..."
echo

# Basic import (fast)
./enex2jottery.pl --output jottery-import.json *.enex

echo
echo "✅ Import complete!"
echo
echo "Output file: jottery-import.json"
echo
echo "Next steps:"
echo "1. Open Jottery web app"
echo "2. Go to Settings → Import/Export"
echo "3. Click Import"
echo "4. Select jottery-import.json"
echo
echo "Optional: For AI-based tagging, run:"
echo "  OPENAI_API_KEY=sk-... ./enex2jottery.pl --ai-tags --output jottery-import-tagged.json *.enex"
