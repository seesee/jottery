# App Store Screenshot Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One-command generation of marketing-style App Store screenshots (iPhone 6.9" + iPad 13", en-GB, dark with one light frame) seeded from the existing demo pack.

**Architecture:** Three stages under `demo-generation/ios/`: a DEBUG-only launch-argument seeder inside the app (no pbxproj changes — the `Jottery/` group is file-system-synchronized), a `simctl`-driven capture script producing raw PNGs, and a Playwright compositor that renders an HTML marketing frame at exact App Store pixel sizes. `screens.json` is the single source of truth for screens, launch args, and captions.

**Tech Stack:** Swift (Swift 6 strict concurrency, Swift Testing for unit tests), bash + `xcrun simctl` + `jq`, Playwright (already a dev dependency), HTML/CSS.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-14-appstore-screenshots-design.md`
- All seeder code strictly inside `#if DEBUG` — Release builds must compile without it.
- No edits to `ios-native/Jottery/Jottery.xcodeproj/project.pbxproj` (it carries uncommitted user WIP; synchronized groups make edits unnecessary).
- Demo pack path: `demo-generation/jottery-demo-notes-en-GB.json` — read via env var at runtime, never bundled into the app.
- Fixed demo password: `demo-pass-2026`.
- Output sizes (exact, validated): iPhone `1320×2868`, iPad `2064×2752`.
- Finished assets committed to `demo-generation/screenshots/appstore/<device>/`; raw captures gitignored.
- British English in all captions and docs. Captions verbatim from the spec table.
- iOS build verification command (from project dir `ios-native/Jottery`):
  `xcodebuild -project Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' build`
- Unit test command: same but `test` instead of `build`.

---

### Task 1: DemoSeedConfig — launch-argument parsing (TDD)

**Files:**
- Create: `ios-native/Jottery/Jottery/Services/DemoSeedService.swift` (config struct only in this task)
- Test: `ios-native/Jottery/JotteryTests/DemoSeedConfigTests.swift`

**Interfaces:**
- Produces: `DemoSeedConfig` with `notesPath: String`, `theme: String`, `screen: DemoSeedConfig.Screen` (`.list`, `.note(titleContains: String)`, `.search(query: String)`, `.sync`, `.lock`) and `static func parse(arguments: [String], environment: [String: String]) -> DemoSeedConfig?`. Task 2 consumes this exactly.

- [ ] **Step 1: Write the failing test**

`ios-native/Jottery/JotteryTests/DemoSeedConfigTests.swift`:

```swift
import Testing
@testable import Jottery

struct DemoSeedConfigTests {

    @Test func returnsNilWithoutSeedFlag() {
        let config = DemoSeedConfig.parse(
            arguments: ["-demo-theme", "dark"],
            environment: ["DEMO_NOTES_PATH": "/tmp/notes.json"]
        )
        #expect(config == nil)
    }

    @Test func returnsNilWithoutNotesPath() {
        let config = DemoSeedConfig.parse(arguments: ["-demo-seed"], environment: [:])
        #expect(config == nil)
    }

    @Test func defaultsToDarkListScreen() {
        let config = DemoSeedConfig.parse(
            arguments: ["-demo-seed"],
            environment: ["DEMO_NOTES_PATH": "/tmp/notes.json"]
        )
        #expect(config == DemoSeedConfig(
            notesPath: "/tmp/notes.json", theme: "dark", screen: .list
        ))
    }

    @Test func parsesThemeAndScreenVariants() {
        let base = ["DEMO_NOTES_PATH": "/tmp/n.json"]
        #expect(DemoSeedConfig.parse(
            arguments: ["-demo-seed", "-demo-theme", "light", "-demo-screen", "list"],
            environment: base
        )?.theme == "light")
        #expect(DemoSeedConfig.parse(
            arguments: ["-demo-seed", "-demo-screen", "note:Welcome"], environment: base
        )?.screen == .note(titleContains: "Welcome"))
        #expect(DemoSeedConfig.parse(
            arguments: ["-demo-seed", "-demo-screen", "search:#recipe"], environment: base
        )?.screen == .search(query: "#recipe"))
        #expect(DemoSeedConfig.parse(
            arguments: ["-demo-seed", "-demo-screen", "sync"], environment: base
        )?.screen == .sync)
        #expect(DemoSeedConfig.parse(
            arguments: ["-demo-seed", "-demo-screen", "lock"], environment: base
        )?.screen == .lock)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run (from `ios-native/Jottery`):
`xcodebuild -project Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' test 2>&1 | tail -20`
Expected: build FAILS with "cannot find 'DemoSeedConfig' in scope".

- [ ] **Step 3: Write minimal implementation**

`ios-native/Jottery/Jottery/Services/DemoSeedService.swift`:

```swift
import Foundation

#if DEBUG
/// Configuration parsed from launch arguments for demo screenshot seeding.
/// See docs/superpowers/specs/2026-07-14-appstore-screenshots-design.md.
struct DemoSeedConfig: Equatable {
    enum Screen: Equatable {
        case list
        case note(titleContains: String)
        case search(query: String)
        case sync
        case lock
    }

    var notesPath: String
    var theme: String
    var screen: Screen

    static func parse(arguments: [String], environment: [String: String]) -> DemoSeedConfig? {
        guard arguments.contains("-demo-seed"),
              let path = environment["DEMO_NOTES_PATH"], !path.isEmpty else {
            return nil
        }

        var theme = "dark"
        if let i = arguments.firstIndex(of: "-demo-theme"), i + 1 < arguments.count {
            theme = arguments[i + 1]
        }

        var screen: Screen = .list
        if let i = arguments.firstIndex(of: "-demo-screen"), i + 1 < arguments.count {
            let raw = arguments[i + 1]
            switch raw {
            case "list": screen = .list
            case "sync": screen = .sync
            case "lock": screen = .lock
            default:
                if raw.hasPrefix("note:") {
                    screen = .note(titleContains: String(raw.dropFirst("note:".count)))
                } else if raw.hasPrefix("search:") {
                    screen = .search(query: String(raw.dropFirst("search:".count)))
                }
            }
        }

        return DemoSeedConfig(notesPath: path, theme: theme, screen: screen)
    }
}
#endif
```

- [ ] **Step 4: Run tests to verify they pass**

Same command as Step 2. Expected: `** TEST SUCCEEDED **` and the four `DemoSeedConfigTests` listed as passed.

- [ ] **Step 5: Commit**

```bash
git add ios-native/Jottery/Jottery/Services/DemoSeedService.swift ios-native/Jottery/JotteryTests/DemoSeedConfigTests.swift
git commit -m "feat(ios): parse demo-seed launch arguments (DEBUG only)"
```

---

### Task 2: DemoSeedService execution + app hook

**Files:**
- Modify: `ios-native/Jottery/Jottery/Services/DemoSeedService.swift` (append service enum)
- Modify: `ios-native/Jottery/Jottery/ViewModels/AppState.swift` (one DEBUG property)
- Modify: `ios-native/Jottery/Jottery/App/JotteryApp.swift` (`ContentView.onAppear` hook + DEBUG settings sheet)

**Interfaces:**
- Consumes: `DemoSeedConfig.parse` from Task 1; existing app APIs verified against current source: `AppState.wipeAllData()`, `createVault(password:)`, `loadNotes()`, `updateSettings(_:)`, `lock()`, `notes: [DecryptedNote]` (`.title`), `selectedNoteId: String?`, `searchQuery: String`, `syncEnabled: Bool`, `lastSyncAt: Date?`, `noteRepo`, `attachmentRepo`, `keyManager.masterKey`; `ImportService.parse(_:)` and `ImportService.importNotes(_:strategy:noteRepo:attachmentRepo:key:progress:)`; `AppState.createNote()` / `saveNote(_:)` for the calc note.
- Produces: `DemoSeedService.runIfRequested(appState:)` called from `ContentView.onAppear`; `AppState.demoShowSettings: Bool` (DEBUG) driving a `SettingsView` sheet — Task 3's `sync` screen relies on it.

- [ ] **Step 1: Append the service to DemoSeedService.swift**

Inside the existing `#if DEBUG` block (before `#endif`):

```swift
enum DemoSeedError: Error {
    case appNotReady
}

/// Seeds the app with demo data for App Store screenshot capture.
/// Runs only in DEBUG builds when launched with `-demo-seed`.
@MainActor
enum DemoSeedService {
    static let demoPassword = "demo-pass-2026"

    static func runIfRequested(appState: AppState) {
        guard let config = DemoSeedConfig.parse(
            arguments: ProcessInfo.processInfo.arguments,
            environment: ProcessInfo.processInfo.environment
        ) else { return }

        do {
            try run(config: config, appState: appState)
            print("[DemoSeed] ✓ Seeded — screen: \(config.screen), theme: \(config.theme)")
        } catch {
            print("[DemoSeed] ✗ FAILED: \(error)")
        }
    }

    static func run(config: DemoSeedConfig, appState: AppState) throws {
        // 1. Fresh vault with a known password
        appState.wipeAllData()
        try appState.createVault(password: demoPassword)

        // 2. Import the demo pack from the host filesystem
        guard let noteRepo = appState.noteRepo,
              let attachmentRepo = appState.attachmentRepo,
              let key = appState.keyManager.masterKey else {
            throw DemoSeedError.appNotReady
        }
        let data = try Data(contentsOf: URL(fileURLWithPath: config.notesPath))
        let export = try ImportService.parse(data)
        _ = try ImportService.importNotes(
            export, strategy: .replace,
            noteRepo: noteRepo, attachmentRepo: attachmentRepo, key: key
        )

        // 3. One calc note — the demo pack has none, and frame 7 needs one
        var calc = try appState.createNote()
        calc.content = """
        # Holiday budget
        flights = 420
        hotel = 380 * 4
        car_hire = 190
        total = flights + hotel + car_hire
        """
        calc.syntaxLanguage = "calc"
        calc.tags = ["budget", "travel"]
        try appState.saveNote(calc)

        try appState.loadNotes()

        // 4. Theme
        var settings = appState.settings
        settings.theme = config.theme
        try appState.updateSettings(settings)

        // 5. Target screen
        switch config.screen {
        case .list:
            appState.selectedNoteId = nil
        case .note(let fragment):
            appState.selectedNoteId = appState.notes.first {
                $0.title.localizedCaseInsensitiveContains(fragment)
            }?.id
        case .search(let query):
            appState.searchQuery = query
        case .sync:
            settings.syncEnabled = true
            settings.syncEndpoint = "https://notes.example.org"
            try appState.updateSettings(settings)
            appState.syncEnabled = true
            appState.lastSyncAt = Date()
            appState.demoShowSettings = true
        case .lock:
            appState.lock()
        }
    }
}
```

Note: if `AppState.syncEnabled` is `private(set)`, set it via whatever internal setter exists or relax to `internal` for the property — check the declaration at `AppState.swift:30` and keep the change minimal.

- [ ] **Step 2: Add the DEBUG property to AppState**

In `AppState.swift`, near the other `var` state declarations (around line 30):

```swift
#if DEBUG
    /// Demo screenshot mode: presents the settings sheet (see DemoSeedService).
    var demoShowSettings = false
#endif
```

- [ ] **Step 3: Hook ContentView**

In `JotteryApp.swift`, `ContentView.body`, extend the existing `.onAppear`:

```swift
.onAppear {
    appState.initialise()
    pickUpPendingShortcut()
    #if DEBUG
    DemoSeedService.runIfRequested(appState: appState)
    #endif
}
```

and add after the `.onChange(of: appState.isLocked)` modifier:

```swift
#if DEBUG
.sheet(isPresented: Binding(
    get: { appState.demoShowSettings },
    set: { appState.demoShowSettings = $0 }
)) {
    SettingsView()
}
#endif
```

- [ ] **Step 4: Build Debug and Release**

```bash
cd ios-native/Jottery
xcodebuild -project Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' build 2>&1 | tail -3
xcodebuild -project Jottery.xcodeproj -scheme Jottery -configuration Release -destination 'platform=iOS Simulator,name=iPhone 17' build 2>&1 | tail -3
```
Expected: `** BUILD SUCCEEDED **` twice (Release proves the DEBUG code compiles away).

- [ ] **Step 5: Smoke-test seeding manually**

```bash
UDID=$(xcrun simctl list devices available | grep "iPhone 17 Pro Max" | grep -oE '[0-9A-F-]{36}')
xcrun simctl boot "$UDID" || true
APP=$(find ~/Library/Developer/Xcode/DerivedData -path "*Debug-iphonesimulator/Jottery.app" | head -1)
xcrun simctl install "$UDID" "$APP"
SIMCTL_CHILD_DEMO_NOTES_PATH="$PWD/../../demo-generation/jottery-demo-notes-en-GB.json" \
  xcrun simctl launch "$UDID" com.jottery.ios -demo-seed -demo-screen list
sleep 5
xcrun simctl io "$UDID" screenshot /tmp/demo-smoke.png
```
Then Read `/tmp/demo-smoke.png`: expected a dark note list with ~11 notes ("Welcome to Jottery!" pinned at top, "Holiday budget" present), unlocked, no error alerts.

- [ ] **Step 6: Run unit tests, commit**

```bash
xcodebuild -project Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' test 2>&1 | tail -5
git add ios-native/Jottery/Jottery/Services/DemoSeedService.swift ios-native/Jottery/Jottery/ViewModels/AppState.swift ios-native/Jottery/Jottery/App/JotteryApp.swift
git commit -m "feat(ios): DEBUG demo seeding for App Store screenshot capture"
```

---

### Task 3: screens.json + capture.sh

**Files:**
- Create: `demo-generation/ios/screens.json`
- Create: `demo-generation/ios/capture.sh` (chmod +x)
- Modify: `.gitignore` (add `demo-generation/ios/raw/`)

**Interfaces:**
- Consumes: launch args/env from Task 2.
- Produces: raw PNGs at `demo-generation/ios/raw/<deviceKey>/<name>.png`; `screens.json` schema consumed verbatim by Task 4: top-level `devices` (array of `{key, simulator, width, height}`) and `screens` (array of `{name, args, theme, headline, devices}`).

- [ ] **Step 1: Write screens.json**

```json
{
  "devices": [
    { "key": "iphone-69", "simulator": "iPhone 17 Pro Max", "width": 1320, "height": 2868 },
    { "key": "ipad-13", "simulator": "iPad Pro 13-inch (M5)", "width": 2064, "height": 2752 }
  ],
  "screens": [
    { "name": "01-list", "args": "-demo-screen list", "theme": "dark",
      "headline": "Your notes. Nobody else's.", "devices": ["iphone-69", "ipad-13"] },
    { "name": "02-editor", "args": "-demo-screen note:Welcome", "theme": "dark",
      "headline": "Markdown, checklists and code", "devices": ["iphone-69", "ipad-13"] },
    { "name": "03-search", "args": "-demo-screen search:#recipe", "theme": "dark",
      "headline": "Search by tags, dates and words", "devices": ["iphone-69", "ipad-13"] },
    { "name": "04-lock", "args": "-demo-screen lock", "theme": "dark",
      "headline": "Secure notes. Unlocked with a glance.", "devices": ["iphone-69", "ipad-13"] },
    { "name": "05-sync", "args": "-demo-screen sync", "theme": "dark",
      "headline": "Sync to a server you control", "devices": ["iphone-69", "ipad-13"] },
    { "name": "06-list-light", "args": "-demo-screen list", "theme": "light",
      "headline": "Light, when you want it", "devices": ["iphone-69", "ipad-13"] },
    { "name": "07-calc", "args": "-demo-screen note:Holiday", "theme": "dark",
      "headline": "Notes that can count", "devices": ["iphone-69", "ipad-13"] },
    { "name": "08-split", "args": "-demo-screen note:Lake District", "theme": "dark",
      "headline": "Made for iPad", "devices": ["ipad-13"] }
  ]
}
```

- [ ] **Step 2: Write capture.sh**

```bash
#!/bin/bash
# Capture raw App Store screenshots from iOS simulators.
# Usage: ./capture.sh [--device iphone-69|ipad-13] [--screen NAME]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="$REPO_ROOT/ios-native/Jottery"
SCREENS_JSON="$SCRIPT_DIR/screens.json"
RAW_DIR="$SCRIPT_DIR/raw"
NOTES_PATH="$REPO_ROOT/demo-generation/jottery-demo-notes-en-GB.json"
BUNDLE_ID="com.jottery.ios"
DERIVED="$SCRIPT_DIR/.derived"
SETTLE_SECONDS=6

ONLY_DEVICE=""; ONLY_SCREEN=""
while [ $# -gt 0 ]; do
  case "$1" in
    --device) ONLY_DEVICE="$2"; shift 2 ;;
    --screen) ONLY_SCREEN="$2"; shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }
[ -f "$NOTES_PATH" ] || { echo "Demo pack not found: $NOTES_PATH" >&2; exit 1; }

echo "→ Building Jottery (Debug, generic simulator)..."
xcodebuild -project "$PROJECT_DIR/Jottery.xcodeproj" -scheme Jottery \
  -configuration Debug -destination 'generic/platform=iOS Simulator' \
  -derivedDataPath "$DERIVED" build | tail -2
APP_PATH="$DERIVED/Build/Products/Debug-iphonesimulator/Jottery.app"
[ -d "$APP_PATH" ] || { echo "App not found at $APP_PATH" >&2; exit 1; }

jq -c '.devices[]' "$SCREENS_JSON" | while read -r device; do
  KEY=$(jq -r '.key' <<<"$device")
  SIM_NAME=$(jq -r '.simulator' <<<"$device")
  [ -n "$ONLY_DEVICE" ] && [ "$KEY" != "$ONLY_DEVICE" ] && continue

  UDID=$(xcrun simctl list devices available | grep -F "$SIM_NAME (" | grep -oE '[0-9A-F-]{36}' | head -1)
  [ -n "$UDID" ] || { echo "Simulator not found: $SIM_NAME" >&2; exit 1; }

  echo "→ [$KEY] booting $SIM_NAME ($UDID)"
  xcrun simctl boot "$UDID" 2>/dev/null || true
  xcrun simctl bootstatus "$UDID" -b

  # Clean status bar (9:41, full signal/wifi/battery)
  xcrun simctl status_bar "$UDID" override \
    --time "09:41" --batteryState charged --batteryLevel 100 \
    --cellularMode active --cellularBars 4 --wifiBars 3 --operatorName ""

  # Best-effort Face ID enrolment so the lock screen shows biometric unlock
  xcrun simctl spawn "$UDID" notifyutil -s com.apple.BiometricKit.enrollmentChanged 1 2>/dev/null || true
  xcrun simctl spawn "$UDID" notifyutil -p com.apple.BiometricKit.enrollmentChanged 2>/dev/null || true

  xcrun simctl install "$UDID" "$APP_PATH"
  mkdir -p "$RAW_DIR/$KEY"

  jq -c '.screens[]' "$SCREENS_JSON" | while read -r screen; do
    NAME=$(jq -r '.name' <<<"$screen")
    [ -n "$ONLY_SCREEN" ] && [ "$NAME" != "$ONLY_SCREEN" ] && continue
    jq -e --arg k "$KEY" '.devices | index($k)' <<<"$screen" >/dev/null || continue

    ARGS=$(jq -r '.args' <<<"$screen")
    THEME=$(jq -r '.theme' <<<"$screen")

    echo "  • $NAME (theme: $THEME)"
    xcrun simctl ui "$UDID" appearance "$THEME"
    xcrun simctl terminate "$UDID" "$BUNDLE_ID" 2>/dev/null || true
    # shellcheck disable=SC2086
    SIMCTL_CHILD_DEMO_NOTES_PATH="$NOTES_PATH" \
      xcrun simctl launch "$UDID" "$BUNDLE_ID" -demo-seed -demo-theme "$THEME" $ARGS >/dev/null
    sleep "$SETTLE_SECONDS"
    xcrun simctl io "$UDID" screenshot "$RAW_DIR/$KEY/$NAME.png" >/dev/null
  done

  xcrun simctl terminate "$UDID" "$BUNDLE_ID" 2>/dev/null || true
  xcrun simctl shutdown "$UDID"
done

echo "✓ Raw screenshots in $RAW_DIR"
```

Note: `-demo-screen` values containing spaces (`note:Holiday budget`) must survive word-splitting — `$ARGS` is intentionally unquoted for flag splitting, so use space-free fragments in `screens.json` (`note:Holiday` matches via case-insensitive contains; adjust the JSON if ambiguous).

- [ ] **Step 3: Verify note fragments are space-free and unique**

Every `note:` fragment in screens.json must be space-free (unquoted `$ARGS` word-splits) and uniquely match one demo note title via case-insensitive contains: `Welcome` → the welcome note, `Holiday` → the calc note (no other title contains "holiday" — the Lake District note says "Trip"), `Lake` → the Lake District note. Cross-check against the pack with:
`python3 -c "import json; [print(n['content'].splitlines()[0]) for n in json.load(open('demo-generation/jottery-demo-notes-en-GB.json'))['notes']]"`

- [ ] **Step 4: Add gitignore entry**

Append to `.gitignore`:
```
demo-generation/ios/raw/
demo-generation/ios/.derived/
```

- [ ] **Step 5: Run capture for one screen, then all**

```bash
chmod +x demo-generation/ios/capture.sh
./demo-generation/ios/capture.sh --device iphone-69 --screen 01-list
```
Read `demo-generation/ios/raw/iphone-69/01-list.png`: dark note list, 9:41 status bar. Then run the full `./demo-generation/ios/capture.sh` and Read each PNG (15 files: 7 for iphone-69 + 8 for ipad-13) checking each matches its intent (editor shows markdown, search shows `#recipe` filtered to the recipe note, lock shows the unlock screen, sync shows settings sheet with server row, calc shows computed values).

- [ ] **Step 6: Commit**

```bash
git add demo-generation/ios/screens.json demo-generation/ios/capture.sh .gitignore
git commit -m "feat(demo): simulator capture pipeline for App Store screenshots"
```

---

### Task 4: Marketing compositor (template + Playwright)

**Files:**
- Create: `demo-generation/ios/compose/template.html`
- Create: `demo-generation/ios/compose/compose.spec.ts`
- Create: `demo-generation/ios/compose/playwright.config.ts`

**Interfaces:**
- Consumes: `screens.json` schema from Task 3; raw PNGs at `raw/<deviceKey>/<name>.png`.
- Produces: finished PNGs at `demo-generation/screenshots/appstore/<deviceKey>/<name>.png`, exact device pixel sizes.

- [ ] **Step 1: Write template.html**

```html
<!doctype html>
<html lang="en-GB">
<head>
<meta charset="utf-8">
<style>
  :root {
    --bg-top: #1b1626;
    --bg-bottom: #131318;
    --accent: #8b5cf6;
    --text: #f2f0f7;
    --bezel: #26262f;
  }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    width: 100vw; height: 100vh; overflow: hidden;
    background: linear-gradient(180deg, var(--bg-top), var(--bg-bottom) 55%);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    display: flex; flex-direction: column; align-items: center;
  }
  .headline {
    color: var(--text);
    font-weight: 800;
    letter-spacing: -0.02em;
    text-align: center;
    line-height: 1.15;
    margin: 7vh 6vw 4vh;
    font-size: 5.2vw;
  }
  .headline .accent { color: var(--accent); }
  .shell {
    flex: 1; width: 74vw;
    border: 0.9vw solid var(--bezel);
    border-bottom: none;
    border-radius: 5.5vw 5.5vw 0 0;
    overflow: hidden;
    box-shadow: 0 -1vh 8vh rgba(139, 92, 246, 0.25);
    background: #000;
  }
  .shell img { width: 100%; display: block; }
  body.ipad .headline { font-size: 4vw; margin-top: 5vh; }
  body.ipad .shell { width: 80vw; border-radius: 3vw 3vw 0 0; border-width: 0.6vw; }
</style>
</head>
<body>
  <h1 class="headline" id="headline"></h1>
  <div class="shell"><img id="shot" alt=""></div>
  <script>
    const params = new URLSearchParams(location.search);
    document.getElementById('headline').textContent = params.get('headline') || '';
    document.getElementById('shot').src = params.get('img') || '';
    if (params.get('device') === 'ipad-13') document.body.classList.add('ipad');
  </script>
</body>
</html>
```

- [ ] **Step 2: Write compose.spec.ts**

```typescript
import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

const iosDir = path.resolve(__dirname, '..');
const repoRoot = path.resolve(iosDir, '../..');
const config = JSON.parse(fs.readFileSync(path.join(iosDir, 'screens.json'), 'utf8'));
const outRoot = path.join(repoRoot, 'demo-generation/screenshots/appstore');

for (const device of config.devices) {
  for (const screen of config.screens) {
    if (!screen.devices.includes(device.key)) continue;

    test(`${device.key}/${screen.name}`, async ({ page }) => {
      const rawPath = path.join(iosDir, 'raw', device.key, `${screen.name}.png`);
      expect(fs.existsSync(rawPath), `missing raw capture: ${rawPath}`).toBe(true);

      // Render at half size with deviceScaleFactor 2 → exact device pixels.
      await page.setViewportSize({ width: device.width / 2, height: device.height / 2 });

      const url = new URL(`file://${path.join(iosDir, 'compose/template.html')}`);
      url.searchParams.set('headline', screen.headline);
      url.searchParams.set('img', `file://${rawPath}`);
      url.searchParams.set('device', device.key);
      await page.goto(url.toString());
      await page.waitForLoadState('networkidle');

      const outDir = path.join(outRoot, device.key);
      fs.mkdirSync(outDir, { recursive: true });
      const outPath = path.join(outDir, `${screen.name}.png`);
      await page.screenshot({ path: outPath });

      // Validate exact pixel dimensions from the PNG header (IHDR).
      const buf = fs.readFileSync(outPath);
      expect(buf.readUInt32BE(16)).toBe(device.width);
      expect(buf.readUInt32BE(20)).toBe(device.height);
    });
  }
}
```

- [ ] **Step 3: Write playwright.config.ts**

```typescript
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  timeout: 30_000,
  use: {
    deviceScaleFactor: 2,
    // viewport is set per-test from screens.json
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
```

- [ ] **Step 4: Run and inspect**

```bash
npx playwright test --config demo-generation/ios/compose/playwright.config.ts 2>&1 | tail -5
```
Expected: 15 passed (7 iphone + 8 ipad). Read at least `demo-generation/screenshots/appstore/iphone-69/01-list.png` and `.../ipad-13/08-split.png` to eyeball composition (headline legible, shell corners clean, screenshot not distorted).

- [ ] **Step 5: Commit**

```bash
git add demo-generation/ios/compose demo-generation/screenshots/appstore
git commit -m "feat(demo): marketing compositor for App Store screenshots"
```

---

### Task 5: generate.sh wrapper + documentation

**Files:**
- Create: `demo-generation/ios/generate.sh` (chmod +x)
- Modify: `demo-generation/README.md` (new section)

**Interfaces:**
- Consumes: `capture.sh` (Task 3), compose config (Task 4).
- Produces: single entry point `./demo-generation/ios/generate.sh [--skip-capture]`.

- [ ] **Step 1: Write generate.sh**

```bash
#!/bin/bash
# Generate App Store screenshot assets: capture from simulators, then compose.
# Usage: ./generate.sh [--skip-capture]
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [ "${1:-}" != "--skip-capture" ]; then
  "$SCRIPT_DIR/capture.sh"
fi

cd "$REPO_ROOT"
npx playwright test --config demo-generation/ios/compose/playwright.config.ts
echo "✓ Finished assets in demo-generation/screenshots/appstore/"
```

- [ ] **Step 2: Document in demo-generation/README.md**

Append a section:

```markdown
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
```

- [ ] **Step 3: Full pipeline run + commit**

```bash
chmod +x demo-generation/ios/generate.sh
./demo-generation/ios/generate.sh
git add demo-generation/ios/generate.sh demo-generation/README.md demo-generation/screenshots/appstore
git commit -m "feat(demo): one-command App Store screenshot generation"
```

---

### Task 6: Final review of assets

**Files:**
- Possibly modify: `demo-generation/ios/screens.json` (caption/screen tweaks), `demo-generation/ios/compose/template.html` (visual polish)

- [ ] **Step 1: Read every finished asset** (15 PNGs) and check against the spec table: correct screen content, correct headline, correct theme, no truncated text, no simulator artefacts (keyboard unexpectedly up, alerts, empty states).
- [ ] **Step 2: Fix and re-run** — content issues → adjust `screens.json` or the seeder and re-capture that screen (`capture.sh --screen NAME`); visual issues → adjust `template.html` and recompose (`generate.sh --skip-capture`).
- [ ] **Step 3: Verify Release build one last time** (`xcodebuild -configuration Release ... build`) and run the iOS unit tests.
- [ ] **Step 4: Commit final assets**

```bash
git add -A demo-generation
git commit -m "feat(demo): final App Store screenshot assets (en-GB, iPhone 6.9\" + iPad 13\")"
```
