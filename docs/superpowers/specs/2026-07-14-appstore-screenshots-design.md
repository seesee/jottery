# App Store Screenshot Generation — Design

**Date:** 2026-07-14
**Status:** Approved pending spec review

## Goal

A repeatable, one-command pipeline that produces marketing-style App Store
screenshot assets for the Jottery iOS client, seeded with the existing demo
pack (`demo-generation/jottery-demo-notes-en-GB.json`), for every future
release.

## Decisions (agreed)

- **Devices:** iPhone 6.9" (iPhone 17 Pro Max simulator, 1320×2868) and
  iPad 13" (iPad Pro 13-inch simulator, 2064×2752) — the two sizes App Store
  Connect requires for an app offered on both device families.
- **Appearance:** dark mode throughout, with one light-mode frame to show the
  app supports it.
- **Seeding:** DEBUG-only launch-argument code path in the app (no XCUITest
  target, no `project.pbxproj` changes).
- **Locales:** en-GB only for this first set. Localised sets wait until the
  app UI itself is localised.

## Architecture

Three stages, all under `demo-generation/ios/`:

```
demo-generation/ios/
├── generate.sh          # one command: capture + compose
├── capture.sh           # simulator capture → raw PNGs
├── raw/<device>/        # raw simulator screenshots (gitignored)
├── compose/
│   ├── template.html    # marketing frame (brand background, headline, device shell)
│   └── compose.spec.ts  # Playwright: renders template at exact ASC pixel sizes
└── screens.json         # single source of truth: screen list, args, captions
```

Finished assets: `demo-generation/screenshots/appstore/<device>/NN-<name>.png`
(committed, like the existing web screenshots).

### Stage 1 — DEBUG seeding in the app

`ios-native/Jottery/Jottery/Services/DemoSeedService.swift`, entirely inside
`#if DEBUG`. When launched with `-demo-seed` the app:

1. wipes local data and creates a vault with a fixed password (`demo-pass-2026`);
2. imports notes from the JSON file at the `DEMO_NOTES_PATH` environment
   variable (an absolute host path — simulator apps can read host files, so
   the demo pack is never bundled into the app);
3. auto-unlocks;
4. applies `-demo-theme dark|light`;
5. navigates per `-demo-screen`:
   - `list` — note list
   - `note:<title-prefix>` — opens the first note whose title matches
   - `search:<query>` — note list with search active and query entered
   - `sync` — settings sheet, sync section visible
   - `lock` — locked state (after seeding, so Face ID unlock row shows)

Hook point: early in app startup (`JotteryApp`/`AppState`), guarded so release
builds compile none of it. The seed path must be deterministic — same input,
same visual state.

### Stage 2 — capture script

`capture.sh`:

1. `xcodebuild build` once per destination (iPhone 17 Pro Max, iPad Pro 13-inch).
2. Per device: boot simulator, `simctl status_bar override` (9:41, full
   signal/wifi/battery), set appearance to match theme, install app.
3. Per screen in `screens.json`: `simctl launch` with the screen's args +
   `DEMO_NOTES_PATH`, wait for settle (fixed delay + retry), `simctl io
   screenshot` to `raw/<device>/<name>.png`, terminate app.
4. Shutdown simulator.

### Stage 3 — marketing composition

Playwright renders `template.html` per (screen × device) at the exact target
pixel size and screenshots the page — same pattern as the existing web
screenshot generation. The template:

- brand background matching the app/privacy-page identity (near-black
  `#131318`, violet `#8b5cf6` accent);
- headline set large in the system font stack, top-aligned;
- raw screenshot inset in a CSS-drawn rounded device shell (own artwork —
  avoids Apple bezel licensing);
- no external assets; deterministic output.

## Screen set and captions

| # | Screen | Theme | Headline | iPhone | iPad |
|---|--------|-------|----------|--------|------|
| 1 | Note list | dark | Your notes. Nobody else's. | ✓ | ✓ |
| 2 | Editor (markdown note) | dark | Markdown, checklists and code | ✓ | ✓ |
| 3 | Search (`#recipe`) | dark | Search by tags, dates and words | ✓ | ✓ |
| 4 | Lock screen | dark | Secure notes. Unlocked with a glance. | ✓ | ✓ |
| 5 | Settings → sync | dark | Sync to a server you control | ✓ | ✓ |
| 6 | Note list | light | Light, when you want it | ✓ | ✓ |
| 7 | Editor (calc note) | dark | Notes that can count | ✓ | ✓ |
| 8 | Split view (list + editor) | dark | Made for iPad | — | ✓ |

Captions are plain-English statements of what the app does; British English;
no superlatives.

## Error handling

- `capture.sh` fails fast with the failing screen named; raw PNGs are
  idempotent (re-run overwrites).
- Compose validates each output's pixel dimensions and fails if a raw input
  is missing.
- Seeding failures surface as a visible error state in the app (existing
  error UI), which the screenshot makes obvious at review time.

## Testing / verification

- Run the full pipeline; eyeball each finished asset (dimensions script-checked).
- `xcodebuild build` in Release configuration to prove the DEBUG code
  compiles away.
- Existing unit tests still pass (seeder touches no release code paths).

## Out of scope

- Localised screenshot sets (blocked on app UI localisation).
- App Store Connect upload automation (fastlane deliver) — assets are
  produced for manual upload.
- App Preview videos.
