# iOS Unlock & Sync Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the full-vault decrypt (and unlock-time PBKDF2) from blocking the main thread (jottery-bzar): unlock shows a live spinner instead of freezing, and routine sync cycles refresh only the notes that changed.

**Architecture:** Three stages. (1) `loadNotes` becomes `async`, with the decrypt work in `Task.detached` and array publication on the MainActor — all call sites converted. (2) The unlock path moves PBKDF2 + verification off-main the same way, keeping `UnlockScreen`'s spinner rendering. (3) Pull returns the set of changed/deleted note ids and `AppState` refreshes only those rows in `notes`/`archivedNotes` (falling back to a full reload above a threshold or on force-resync).

**Tech Stack:** Swift 6 strict concurrency (`@MainActor` `@Observable` AppState, `Sendable` repos/keys), GRDB, Swift Testing.

## Global Constraints

- Beads issue: jottery-bzar.
- Branch: `fix/ios-unlock-performance` off main (after PR #99 merges).
- Test command (from `ios-native/Jottery`): `xcodebuild -project Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' test`
- No `project.pbxproj` changes. British English comments.
- Behavioural invariants that MUST hold (and stay covered by the existing suite): `filteredNotes` reflects saves immediately (AppStateListRefreshTests); pending-editor flush ordering on unlock (PendingEditorFlushTests); demo seeding still works (`DemoSeedService.run` calls `loadNotes` synchronously — it may keep a synchronous path or become async, but the capture pipeline must still produce correct screenshots).
- Sendability facts (verify, don't assume): `DecryptedNote` and `NoteRecord` are value types; `NoteRepository` is `Sendable`; `SymmetricKey` is `Sendable`. If any fails, prefer passing plain data across the boundary over sprinkling `@unchecked`.
- Measure before/after in Task 2 using the seeded simulator (3,000-note synthetic pack — generate by duplicating the demo pack notes in a temp JSON) and report wall-clock unlock→list times from log timestamps. Honest numbers, not estimates.

---

### Task 1: `loadNotes` goes async and off-main

**Files:**
- Modify: `ios-native/Jottery/Jottery/ViewModels/AppState.swift` (`loadNotes` + every caller inside AppState)
- Modify: callers outside AppState (inventory first — expect: `UnlockScreen.swift`, `SetupScreen.swift`, `DemoSeedService.swift`, possibly `SyncService` completion hooks via closures)
- Test: existing suites must pass with `await` added; add `ios-native/Jottery/JotteryTests/AsyncLoadNotesTests.swift` pinning that `loadNotes()` publishes both arrays and triggers `scheduleSearch`

**Interfaces:**
- Produces: `func loadNotes() async throws` — decrypt via `Task.detached(priority: .userInitiated)` calling `noteRepo.listActive/listArchived/savedSearchRepo.listAll` with the key captured as a local; results assigned to `notes`/`archivedNotes`/`savedSearches` back on the MainActor; then `scheduleSearch()`.
- Step 0 (scout): `grep -rn "loadNotes()" ios-native` — list every call site in the report with how each was converted (plain `await`, wrapped in `Task { }`, or left on a synchronous bridge and why).

- [ ] **Step 1:** Write `AsyncLoadNotesTests` (async `@Test` with `await state.loadNotes()`; seed two notes via repo, assert both arrays populate and `filteredNotes` catches up after `scheduleSearch` — poll with a bounded loop, no arbitrary sleeps beyond ~2s cap).
- [ ] **Step 2:** Convert `loadNotes`:

```swift
func loadNotes() async throws {
    guard let noteRepo, let key = keyManager.masterKey else { return }
    let savedSearchRepo = self.savedSearchRepo
    // Decrypting the whole vault is O(notes) — never on the main thread (jottery-bzar).
    let (active, archived, searches) = try await Task.detached(priority: .userInitiated) {
        (try noteRepo.listActive(key: key),
         try noteRepo.listArchived(key: key),
         (try? savedSearchRepo?.listAll(key: key)) ?? [])
    }.value
    notes = active
    archivedNotes = archived
    savedSearches = searches
    scheduleSearch()
}
```

- [ ] **Step 3:** Convert every call site (scout list). Sync-path closures already in `Task`/async contexts gain `await`; truly synchronous callers (if any remain) get a `Task { try? await loadNotes() }` wrapper ONLY where the caller cannot be made async without distortion — justify each in the report. `DemoSeedService.run` may become `async` (its caller `runIfRequested` already can wrap in a Task; the capture pipeline's 6s settle absorbs the change — note this explicitly).
- [ ] **Step 4:** Full suite green (existing tests updated with `await` where the compiler requires). Verify no test now races (`filteredNotes` assertions may need the bounded poll).
- [ ] **Step 5: Commit** — `perf(ios): decrypt the vault off the main thread`

---

### Task 2: Unlock path off-main with a live spinner

**Files:**
- Modify: `ios-native/Jottery/Jottery/ViewModels/AppState.swift` (`unlock(password:)` → async; PBKDF2/derivation off-main)
- Modify: `ios-native/Jottery/Jottery/Views/UnlockScreen.swift` (both password and biometric routes await; spinner state)
- Possibly: `SetupScreen.swift` (`createVault` route if it shares derivation)
- Test: existing `PendingEditorFlushTests` (flush-on-unlock ordering) must still pass; extend with an async-unlock variant if the signature changes them.

**Interfaces:**
- `func unlock(password: String) async throws -> Bool` — key derivation + verification inside `Task.detached`; all `@Observable` state mutations (isLocked, notes via `await loadNotes()`) on the MainActor afterwards. The pending-editor retry (from PR #98) keeps its position: after key set, before `loadNotes()`.
- KeyManager interactions: `keyManager` is MainActor-bound — capture the derived key data out of the detached task and hand it to `keyManager` on the MainActor; do not touch keyManager off-main.

- [ ] **Step 1:** Trace the current unlock flow and write the exact before/after ordering in the report (derive → verify → set key → flush pending → loadNotes → isLocked=false), preserving PR #98's flush position.
- [ ] **Step 2:** Implement; `UnlockScreen` sets `loading = true`, awaits, clears. Verify the spinner actually animates now (it previously froze): measure with the synthetic 3,000-note vault — log timestamps around unlock stages, report before/after wall-clock and where time is spent (PBKDF2 vs decrypt).
- [ ] **Step 3:** Full suite; demo-seed capture smoke (one screen) to prove the pipeline still works.
- [ ] **Step 4: Commit** — `perf(ios): unlock derives keys and decrypts off-main; spinner stays live`

---

### Task 3: Incremental post-sync refresh

**Files:**
- Modify: `ios-native/Jottery/Jottery/Services/SyncService.swift` (pull/push return changed + deleted ids)
- Modify: `ios-native/Jottery/Jottery/ViewModels/AppState.swift` (apply incremental refresh; full reload fallback)
- Test: `ios-native/Jottery/JotteryTests/IncrementalRefreshTests.swift`

**Interfaces:**
- Produces: `struct SyncChanges: Sendable { var updatedIds: Set<String>; var deletedIds: Set<String>; var fullReloadRequired: Bool }` returned from `pull()` (and `sync()`); `AppState.applySyncChanges(_:) async` refreshes only affected rows: decrypt just those records (off-main, batched in one detached task), replace/insert/remove in `notes`/`archivedNotes` (a note can move between them via archived flag), then `scheduleSearch()`.
- `fullReloadRequired = true` when: force-resync, conflicts resolved, >100 changed ids, or anything ambiguous — falling back to `await loadNotes()`. Honest simplicity beats cleverness here; the fallback is always correct.
- Collection points: every place pull currently writes a note record or deletion (`applyRemoteNote`, deletions loops) — collect ids there. If a path can change a record without being collected (verify: conflict auto-accepts, attachment healing), route it through `fullReloadRequired` rather than risking a stale row.

- [ ] **Step 1:** Failing test: seed 3 notes; simulate "server changed note 2" by mutating its record directly via repo + constructing `SyncChanges(updatedIds: [id2], deletedIds: [id3], fullReloadRequired: false)`; `await state.applySyncChanges(...)`; assert note 2's content refreshed in `notes`, note 3 removed, note 1's object untouched (compare identity by value: unchanged content), `filteredNotes` consistent.
- [ ] **Step 2:** Implement `applySyncChanges`; wire `AppState`'s post-sync handlers (`setupSync` completion, `triggerSync`) to use it instead of unconditional `loadNotes()`.
- [ ] **Step 3:** Full suite; verify with a log-timestamp measurement that a single-note sync no longer decrypts the whole vault (log the count of decrypted records per refresh).
- [ ] **Step 4: Commit** — `perf(ios): sync refreshes only changed notes`
