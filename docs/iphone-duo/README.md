# Jottery on iPhone Duo

The goal at this stage is that Jottery works on iPhone Duo, not that it is
optimised for it.

## The device, in short

iPhone Duo folds. Closed, it has an outer display (about 466 × 678 pt, status bar
down the right-hand edge). Open, it has an inner display (about 669 × 951 pt) with
a fold down the middle.

- **Size classes.** The outer display behaves like an iPhone: compact width. The
  inner display is regular in both directions, so split views show their sidebars.
  The inner display ignores supported interface orientations.
- **iOS 27.1 SDK.** Apps built with it fill both displays edge to edge, with
  toolbars and tab bars laid out vertically under the status bar. Older SDKs get a
  letterboxed window.
- **Reserved regions.** The fold and the cameras are reserved regions:
  `.division` and `.occlusion`. They're queried with
  `GeometryProxy.reservedRegions(kind:)` in SwiftUI or `UIView.reservedRegions(kind:)`
  in UIKit, on iOS 27.1 and later. When the device is partly folded, like a book,
  content across the fold is hard to read and controls on it are hard to reach.
- **Apple's checklist:**
  - Avoid `UIScreen.main`, the interface idiom and interface orientation for layout.
  - Use size classes, container geometry and standard navigation.

Sources: Apple's [Prepare for iPhone Duo](https://developer.apple.com/iphone-duo/prepare/)
page and its tech talks, and the iOS 27.2 SDK headers.

## Plan

1. **Build with the latest SDK (27.2).** This gives full-screen layout on both
   displays and vertical toolbars. ✅ `FoldLayout` uses iOS 27.1 APIs, so the project
   needs Xcode 27.1 or later to build.
2. **Audit for screen assumptions.** I checked for `UIScreen.main`, idiom and
   orientation checks, fixed widths and orientation locks. ✅ None found. The only
   fixed widths are `maxWidth` caps on the setup and unlock forms, which is correct.
3. **Outer display (closed).** Every main screen should lay out correctly. ✅ Verified
   in the simulator (screenshots below).
4. **Inner display (open).** Use the space, and keep content off the fold. ✅ Done in
   code (`FoldLayout.swift`, `MainView`). When the display has a fold running top to
   bottom:
   - both split-view columns are shown
   - the divider sits on the fold's centre line, with the note list on the left page
     and the open note on the right
   - nothing spans the fold

   Elsewhere (iPad, other iPhones, a sideways fold) the split view is unchanged. The
   fold geometry is unit-tested. ⏳ It still needs seeing in the open pose (see below).
5. **Fold and unfold keep their state.** The selected note lives in `AppState`, so it
   survives the switch between displays. ⏳ This needs checking with an unsaved edit in
   progress, in the open pose.

## Screenshots

`screenshots/closed/` holds the outer display. `screenshots/open/` holds the inner
display, once captured. To recapture both:

```bash
./demo-generation/ios/capture-duo.sh --pose both
```

The open pose can't be set from the command line. The script waits until you open
the iPhone Duo in Device Hub (`DeviceHub.app` inside Xcode), then captures the inner
display automatically. Turning a display's power off with `simctl io screenConfig`
does *not* open the device. It leaves that display asleep until the simulator is
rebooted.

## Not done yet

- **Landscape on the outer display.** `devicectl device orientation set` didn't
  rotate the app in the simulator, so this is unverified.
- **Arrangement views** (`ArrangementView`, split and overlay). These are for richer
  layouts across the fold, for example a note's preview on one page and its source on
  the other. They're unnecessary for "works".
- **Reacting to the occlusion region** (the inner camera), only relevant while it's
  active.
