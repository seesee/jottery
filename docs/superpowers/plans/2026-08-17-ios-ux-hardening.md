# iOS UX Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining UX/robustness items: privacy snapshot screen (jottery-jm02), error feedback for user actions (jottery-jqx8), attachment decrypt off-main (jottery-29y9), allTags memoisation (jottery-fxmq), read-only toggle gating + editor residuals (jottery-a3sb), setup-time PBKDF2 off-main (jottery-d9se).

**Architecture:** Branch `fix/ios-ux-hardening` off `fix/ios-correctness-hardening` (stacked; PR after #101 merges). Small independent tasks; TDD where a model seam exists, build+reasoning for pure view work.

## Global Constraints

- Test command (from `ios-native/Jottery`): `xcodebuild -project Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' test` — 124 tests currently; stays green.
- No `project.pbxproj` changes. British English. New user-facing strings via `L.` + `Localizable.xcstrings` (alphabetical insertion).
- CLAUDE.md UX rules: no native alert() analogues for routine feedback — inline/toast patterns.

---

### Task 1 (jm02): privacy screen over backgrounded content

**Files:** Modify `App/JotteryApp.swift` (ContentView); possibly a new `Views/PrivacyCoverView.swift`.
**Contract:** while `scenePhase != .active` AND the app is unlocked with content visible, an opaque cover (app icon or lock glyph on the app's background colour — check existing asset/colour usage; keep it simple and brand-consistent) overlays the UI so the iOS app-switcher snapshot never contains decrypted note content. Cover must NOT appear over UnlockScreen/SetupScreen (nothing sensitive; avoids Face-ID-prompt flicker interplay). Use `.overlay` + a short opacity transition; no state machine beyond reading `scenePhase` and `appState.isLocked/isFirstLaunch`. Verify the Face ID unlock flow (which makes the app inactive) doesn't flash the cover over UnlockScreen.
**Verify:** build + suite; simulator: launch demo-seeded, `xcrun simctl` cannot switch apps programmatically — document reasoning + capture a screenshot after `xcrun simctl ui <udid> appearance` change? Not equivalent. State honestly what remains manual (backgrounding hand-test).
- [ ] Implement → suite green → commit `feat(ios): privacy cover hides content in the app switcher (jottery-jm02)`

---

### Task 2 (jqx8): error feedback for note actions

**Files:** New `Views/ToastView.swift` (or check for an existing toast/banner — grep first: vault-health work may have added one); Modify `ViewModels/AppState.swift` (a small `userError: String?` published surface with auto-clear), `Views/NoteListView.swift`, `Views/NoteEditorView.swift`, `Views/RecycleBinView.swift`.
**Contract:** state-changing user actions that currently `try?` (delete, pin, archive, unarchive, restore, duplicate, lock/unlock note, add/remove attachment, bulk ops) route failures to a visible, auto-dismissing toast ("Couldn't delete note" style, localised). Pattern: a tiny `AppState.reportError(_ key:)`/`userErrorMessage` + one overlay in MainView. Do NOT convert the world to throwing UI — wrap at the call sites (`do/catch` replacing bare `try?`). Actions that succeed stay silent.
**TDD:** model-level test that `reportError` publishes and auto-clears (clock-free: assert set, then after the auto-clear task, nil — use a controllable duration parameter defaulting to 4s, test passes 0.1s).
- [ ] Test red → implement infra → wire call sites → suite green → commit `feat(ios): surface note-action failures as toasts (jottery-jqx8)`

---

### Task 3 (29y9): attachment decrypt off the main thread

**Files:** Modify `Views/AttachmentListView.swift` (`decryptToTempFile`, `decryptAndPreview`, `decryptAndShare`), `Views/MarkdownPreviewView.swift` (`resolveAttachment`).
**Contract:** blob fetch + decrypt + file write run in `Task.detached`; UI state (preview presentation, share sheet) updates on MainActor after. A brief per-tap progress state if one exists already — do not build new spinner UI. Guard: temp files land where they did before (same cleanup semantics — read the existing lifecycle first).
- [ ] Implement → suite green → commit `perf(ios): decrypt attachments off the main thread (jottery-29y9)`

---

### Task 4 (fxmq): memoised allTags

**Files:** Modify `ViewModels/AppState.swift` (`allTags` becomes stored, recomputed when notes/archivedNotes are published — hook the publication points: loadNotes, applySyncChanges, saveNote, createNote, delete/archive paths — enumerate them; simplest correct approach: a `private func rebuildAllTags()` called from the few places arrays are replaced/mutated, or compute lazily with an invalidation flag set at those sites. Choose the one with fewest touch points and justify).
**TDD:** test that allTags reflects a saveNote tag change and an applySyncChanges refresh without recomputing per access (assert correctness; the perf property — no per-keystroke O(n) — is by construction, document it).
- [ ] Test red → implement → suite green → commit `perf(ios): memoise allTags (jottery-fxmq)`

---

### Task 5 (a3sb): read-only gating for editor toggles + residuals

**Files:** Modify `Views/NoteEditorView.swift`.
**Contract:** toolbar toggles (wordWrap, preview, syntax language, colour, and any other note-mutating toolbar control) are disabled or no-op when `isReadOnly`; the debounce path cannot save a locked/archived note (add the guard in `scheduleSave`/`saveImmediately` too — defence in depth). Fix the two residuals: `flushOutgoingNote` early-returns clear `appState.pendingEditorNote` when appropriate (trace what state makes clearing correct vs harmful — the goal: `lock()` must never flush a stale pending note for a READ-ONLY or unchanged outgoing note); remove the dead `didSaveDuringSession` assignment.
- [ ] Implement → suite green → commit `fix(ios): read-only notes cannot be modified via toolbar toggles (jottery-a3sb)`

---

### Task 6 (d9se): setup-time PBKDF2 off-main

**Files:** Modify `ViewModels/AppState.swift` (`createVault`), `Views/SetupScreen.swift` (`unlockAndSync` + any synchronous derive path), following the exact pattern `unlock` now uses (Task.detached, plain-value captures, keyManager on MainActor).
**Contract:** first-run vault creation and setup-screen unlock don't block the main thread during derivation; SetupScreen's progress UI stays live. `createVault` signature may become async — convert callers (SetupScreen, DemoSeedService already async).
**TDD:** existing createVault-touching tests updated; one new test asserting createVault still produces an unlockable vault (create → lock → unlock with same password succeeds).
- [ ] Test red/adapted → implement → suite green → commit `perf(ios): vault creation derives keys off the main thread (jottery-d9se)`
