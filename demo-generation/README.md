# Demo Generation

This directory contains all scripts and assets for generating landing page screenshots and demos.

## Directory Structure

```
demo-generation/
├── generate.sh              # Main generation script (run this!)
├── playwright/              # Playwright E2E screenshot tests
│   └── screenshots.spec.ts  # Web screenshot generation
├── vhs/                     # VHS terminal recording scripts
│   ├── tui-cli.tape         # CLI commands help screen
│   ├── tui-interface.tape   # Interactive TUI with vim
│   ├── tui-piping.tape      # Piping examples
│   └── tui-sync.tape        # Sync commands demo
├── playwright.config.ts     # Playwright configuration
└── README.md                # This file

public/screenshots/
└── en-GB/                   # Language-specific screenshots
    ├── 01-main-interface-light.png
    ├── 01-main-interface-light-preview.png
    ├── ...
    ├── tui-cli.gif
    ├── tui-interface.gif
    ├── tui-piping.gif
    └── tui-sync.gif
```

## Quick Start

From the project root:

```bash
# Generate all demos
./demo-generation/generate.sh

# List available demos
./demo-generation/generate.sh --list

# Generate specific demo
./demo-generation/generate.sh --name tui-cli

# Get help
./demo-generation/generate.sh --help
```

## Command Line Options

```bash
./demo-generation/generate.sh [OPTIONS]

Options:
  --list              List all available demos
  --name DEMO_NAME    Generate specific demo only
  --lang LANGUAGE     Set language (default: en-GB)
  --help              Show help message
```

## Available Demos

Run `./demo-generation/generate.sh --list` to see all available demos.

### Web Demos (Playwright)
- `web-carousel-light` - Carousel screenshots (light mode, 4 images)
- `web-carousel-dark` - Carousel screenshots (dark mode, 4 images)
- `web-python-light` - Python syntax highlighting (light mode)
- `web-python-dark` - Python syntax highlighting (dark mode)
- `web-multiselect-light` - Multi-select feature (light mode)
- `web-multiselect-dark` - Multi-select feature (dark mode)
- `web-version-history-light` - Version history (light mode)
- `web-version-history-dark` - Version history (dark mode)
- `web-calculator-light` - REPL calculator (light mode)
- `web-calculator-dark` - REPL calculator (dark mode)
- `web-mobile-list-light` - Mobile note list view (light mode)
- `web-mobile-list-dark` - Mobile note list view (dark mode)
- `web-mobile-japan-light` - Mobile Japan itinerary note (light mode)
- `web-mobile-japan-dark` - Mobile Japan itinerary note (dark mode)
- `web-mobile-calculator-light` - Mobile calculator note (light mode)
- `web-mobile-calculator-dark` - Mobile calculator note (dark mode)
- `web-outliner-light` - Outliner mode (light mode)
- `web-outliner-dark` - Outliner mode (dark mode)
- `web-greek-ui-light` - Greek multi-lingual UI (light mode)
- `web-greek-ui-dark` - Greek multi-lingual UI (dark mode)

### TUI Demos (VHS)
- `tui-cli` - CLI commands help screen
- `tui-interface` - Interactive TUI with vim integration
- `tui-piping` - Piping content examples
- `tui-sync` - Cross-platform sync commands

## Examples

```bash
# Generate all demos for default language (en-GB)
./demo-generation/generate.sh

# List all available demos
./demo-generation/generate.sh --list

# Generate only the TUI CLI demo
./demo-generation/generate.sh --name tui-cli

# Generate only light mode carousel
./demo-generation/generate.sh --name web-carousel-light

# Generate for specific language (future)
./demo-generation/generate.sh --lang en-US
```

## Prerequisites

- **Node.js** - For running Playwright tests
- **Playwright** - Installed via `npm install`
- **VHS** - Terminal recorder by Charm
  ```bash
  brew install vhs
  ```

## Language Support

Screenshots and demos are organized by language in `public/screenshots/LANG/`:
- `en-GB/` - British English (default)
- `en-US/` - American English (future)
- `fr/` - French (future)
- etc.

To generate for a specific language:
```bash
./demo-generation/generate.sh --lang en-US
```

The language can also be set via environment variable:
```bash
LANG=en-US ./demo-generation/generate.sh
```

## Modifying Demos

### Web Screenshots

Edit `demo-generation/playwright/screenshots.spec.ts`:
- Change which notes to screenshot
- Update tag colors in test setup
- Modify editor content

The test file uses the `LANG` environment variable to determine output directory:
```typescript
const LANG = process.env.LANG || 'en-GB';
const SCREENSHOT_DIR = `screenshots/${LANG}`;
```

### TUI Demos

Edit VHS tape files in `demo-generation/vhs/`:
- **tui-cli.tape** - Change help commands shown
- **tui-interface.tape** - Modify TUI interaction flow
- **tui-piping.tape** - Update piping examples
- **tui-sync.tape** - Change sync demo commands

VHS Tape Syntax:
```tape
Type "command here"
Enter
Sleep 2s
Down
Ctrl+C
```

Output path in each tape file:
```tape
Output public/screenshots/en-GB/tui-cli.gif
```

See [VHS documentation](https://github.com/charmbracelet/vhs) for more.

## Regenerating Individual Assets

### Just Web Screenshots
```bash
npx playwright test --config=demo-generation/playwright.config.ts
```

### Just TUI Demos
```bash
vhs demo-generation/vhs/tui-cli.tape
vhs demo-generation/vhs/tui-interface.tape
vhs demo-generation/vhs/tui-piping.tape
vhs demo-generation/vhs/tui-sync.tape
```

### Specific Screenshot by Filter
```bash
# Light mode carousel only
./demo-generation/generate.sh --name web-carousel-light

# Dark mode version history only
./demo-generation/generate.sh --name web-version-history-dark

# Specific TUI demo
./demo-generation/generate.sh --name tui-interface
```

## How It Works

1. **Playwright Tests**: Generate web screenshots in `screenshots/LANG/`
2. **Copy Step**: Script copies from `screenshots/LANG/` to `public/screenshots/LANG/`
3. **VHS Tapes**: Generate TUI GIFs directly to `public/screenshots/LANG/`
4. **LandingPage.svelte**: References screenshots from `/screenshots/LANG/`

## CI/CD Integration

The generation script can be integrated into CI/CD pipelines:

```yaml
# Example GitHub Actions
- name: Generate demos
  run: |
    npm install
    ./demo-generation/generate.sh

- name: Generate specific demo
  run: |
    ./demo-generation/generate.sh --name web-carousel-light
```

## Troubleshooting

### VHS shows "cd tui/target/release" in output

This is expected. VHS records from the project root and must navigate to the TUI binary location. The `cd` command is part of the demo.

### Playwright tests fail

Ensure the dev server config in `demo-generation/playwright.config.ts` is correct. The script automatically starts the dev server.

### TUI demos show wrong content

The TUI demos use the default database in `tui/target/release/jottery.db`. Ensure this database has appropriate demo content.

### Missing VHS dependency

Install VHS:
```bash
brew install vhs  # macOS
# or
go install github.com/charmbracelet/vhs@latest
```

### --list shows duplicate or missing demos

The list is defined in `generate.sh` in the `DEMOS` associative array. Add or remove entries there.

## Output Files

All generated files go to `public/screenshots/LANG/`:

```
public/screenshots/en-GB/
├── 01-main-interface-light.png
├── 01-main-interface-light-preview.png
├── 01-main-interface-light-japan-preview.png
├── 01-main-interface-light-japan.png
├── 01-main-interface-dark.png
├── 01-main-interface-dark-preview.png
├── 01-main-interface-dark-japan-preview.png
├── 01-main-interface-dark-japan.png
├── 02-rich-editor-light.png
├── 02-rich-editor-dark.png
├── 03-multi-select-light.png
├── 03-multi-select-dark.png
├── 04-version-history-light.png
├── 04-version-history-dark.png
├── 05-calculator-light.png
├── 05-calculator-dark.png
├── 06-mobile-list-light.png
├── 06-mobile-list-dark.png
├── 07-mobile-japan-light.png
├── 07-mobile-japan-dark.png
├── 08-mobile-calculator-light.png
├── 08-mobile-calculator-dark.png
├── 09-outliner-light.png
├── 09-outliner-dark.png
├── 10-greek-ui-light.png
├── 10-greek-ui-dark.png
├── tui-cli.gif
├── tui-piping.gif
├── tui-interface.gif
└── tui-sync.gif
```

These files are referenced by `src/lib/components/LandingPage.svelte`.

## App Store Screenshots (iOS)

Marketing-style App Store assets, seeded from the demo pack. Requires Xcode
(with the iPhone 17 Pro Max and iPad Pro 13-inch simulators), `jq`, and the
repo's Playwright install.

```bash
./demo-generation/ios/generate.sh                # capture + compose
./demo-generation/ios/generate.sh --skip-capture # recompose only
./demo-generation/ios/capture.sh --device iphone-69 --screen 01-list  # one shot
```

Screens, launch arguments, and captions live in `demo-generation/ios/screens.json`.
Raw captures land in `demo-generation/ios/raw/` (gitignored); finished assets in
`demo-generation/screenshots/appstore/<device>/` at exact App Store Connect
sizes (iPhone 6.9" 1320×2868, iPad 13" 2064×2752). The app must be built in
Debug — seeding is compiled out of Release builds.
