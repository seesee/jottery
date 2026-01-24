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
└── README.md                # This file
```

## Quick Start

From the project root, run:

```bash
chmod +x demo-generation/generate.sh
./demo-generation/generate.sh
```

This will:
1. Generate all web screenshots using Playwright
2. Generate all TUI demos using VHS
3. Verify all files are present

## Prerequisites

- **Node.js** - For running Playwright tests
- **Playwright** - Installed via `npm install`
- **VHS** - Terminal recorder by Charm
  ```bash
  brew install vhs
  ```

## Generated Assets

### Web Screenshots (Playwright)

- **Main Interface Carousel** (8 screenshots)
  - Light: Welcome, Welcome preview, Japan preview, Japan edit
  - Dark: Welcome, Welcome preview, Japan preview, Japan edit

- **Feature Screenshots** (8 screenshots)
  - Python Syntax (light + dark)
  - Multi-select (light + dark)
  - Version History (light + dark)
  - Calculator (light + dark)

### TUI Demos (VHS)

- **tui-cli.gif** - CLI commands help screen
- **tui-piping.gif** - Piping content examples
- **tui-interface.gif** - Interactive TUI with vim integration
- **tui-sync.gif** - Cross-platform sync commands

All generated files are saved to: `public/screenshots/`

## Modifying Demos

### Web Screenshots

Edit `demo-generation/playwright/screenshots.spec.ts`:
- Change which notes to screenshot
- Update tag colors in test setup
- Modify editor content

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

See [VHS documentation](https://github.com/charmbracelet/vhs) for more.

## Regenerating Individual Assets

### Just Web Screenshots
```bash
npx playwright test demo-generation/playwright/screenshots.spec.ts --project=chromium
```

### Just TUI Demos
```bash
vhs demo-generation/vhs/tui-cli.tape
vhs demo-generation/vhs/tui-interface.tape
vhs demo-generation/vhs/tui-piping.tape
vhs demo-generation/vhs/tui-sync.tape
```

### Specific Screenshot
```bash
npx playwright test demo-generation/playwright/screenshots.spec.ts -g "Version History"
```

### Specific TUI Demo
```bash
vhs demo-generation/vhs/tui-interface.tape
```

## CI/CD Integration

The generation script can be integrated into CI/CD pipelines to ensure screenshots stay up-to-date:

```yaml
# Example GitHub Actions
- name: Generate demos
  run: |
    npm install
    ./demo-generation/generate.sh
```

## Troubleshooting

### VHS shows "cd tui/target/release" in output

This is expected. VHS records from the project root and must navigate to the TUI binary location. The `cd` command is part of the demo.

### Playwright tests fail

Ensure the dev server is running or that the webServer config in `playwright.config.ts` is correct.

### TUI demos show wrong content

The TUI demos use the default database in `tui/target/release/jottery.db`. Ensure this database has appropriate demo content.

### Missing VHS dependency

Install VHS:
```bash
brew install vhs  # macOS
# or
go install github.com/charmbracelet/vhs@latest
```

## Output Files

All generated files go to `public/screenshots/`:

```
public/screenshots/
├── 01-main-interface-light.png
├── 01-main-interface-light-preview.png
├── 01-main-interface-light-japan-preview.png
├── 01-main-interface-light-japan.png
├── 01-main-interface-dark.png
├── 01-main-interface-dark-preview.png
├── 01-main-interface-dark-japan-preview.png
├── 01-main-interface-dark-japan.png
├── 02-python-syntax-light.png
├── 02-python-syntax-dark.png
├── 03-multi-select-light.png
├── 03-multi-select-dark.png
├── 04-version-history-light.png
├── 04-version-history-dark.png
├── 05-calculator-light.png
├── 05-calculator-dark.png
├── tui-cli.gif
├── tui-piping.gif
├── tui-interface.gif
└── tui-sync.gif
```

These files are referenced by `src/lib/components/LandingPage.svelte`.
