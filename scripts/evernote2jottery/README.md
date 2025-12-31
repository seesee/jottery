# evernote2jottery

Convert Evernote `.enex` exports to Jottery JSON format.

## Usage

node evernote2jottery.js notes.enex
# outputs notes.json

Optional:
node evernote2jottery.js notes.enex output.json

## AI Tagging (optional)

USE_AI_TAGGING=true \
OPENROUTER_MODEL=openai/gpt-4o-mini \
OPENROUTER_API_KEY=sk-... \
node evernote2jottery.js notes.enex
