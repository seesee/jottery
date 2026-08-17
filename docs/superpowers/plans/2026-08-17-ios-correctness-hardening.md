# iOS Correctness Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining correctness issues from the August review: the SyncSetupView envelope-password bug (jottery-md6b), sync-orchestration races (jottery-dggw, jottery-t94v, jottery-vn6f), the import decoder rejecting documented exports (jottery-sirv), shared-inbox duplicate notes (jottery-y8s7), and unchunked pushes (jottery-l9dt).

**Architecture:** Each task is independent and self-contained. Branch `fix/ios-correctness-hardening` off `fix/ios-unlock-performance` (stacked; PR after #100 merges). TDD with the established Swift Testing harnesses.

## Global Constraints

- Test command (from `ios-native/Jottery`): `xcodebuild -project Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' test` — suite currently 97 tests; must stay green throughout.
- No `project.pbxproj` changes. British English. "When in doubt, keep the user's data."
- Existing harnesses: repo-level (`SyncDeletionSafetyTests`), AppState-level (`AppStateListRefreshTests.makeUnlockedState` pattern), SyncService-level (`ConflictResolutionSafetyTests` — dummy `SyncClient`, `#if DEBUG addPendingConflictForTesting`).

---

### Task 1 (jottery-sirv): ImportService accepts the documented export format

**Files:** Modify `ios-native/Jottery/Jottery/Services/ExportService.swift` (ExportNote decoding); Test `ios-native/Jottery/JotteryTests/ImportFormatTests.swift`.

**Contract:** `archived`, `locked` (and audit every other field the documented cross-platform format marks optional — compare against `demo-generation/jottery-demo-notes-en-GB.json` and CLAUDE.md's export format: `pinned`, `color`, `syntaxLanguage`, `wordWrap`, `showPreview`, `attachments` may also be absent) decode with sensible defaults via `init(from:)` or optional-with-default properties. Export output format must NOT change (still writes all fields). The DEBUG-only `patchedForImport` workaround in `DemoSeedService.swift` becomes unnecessary — remove it and verify demo seeding still works via the seed-path test or a capture smoke.

**TDD:** failing test imports a JSON note with only `id/createdAt/modifiedAt/content/tags/attachments/pinned` (copy a real demo-pack note); assert parse succeeds and `importNotes` creates it. Second test: full-featured note round-trips export→import unchanged.

- [ ] Test red → implement → suite green → remove `patchedForImport` → suite green → commit `fix(ios): import accepts the documented export format (jottery-sirv)`

---

### Task 2 (jottery-dggw + t94v + vn6f): sync orchestration — single entry, epoch guard, no double reload

**Files:** Modify `ios-native/Jottery/Jottery/ViewModels/AppState.swift` (`triggerSync`, `forceFullSync`, `loadNotes`, `applySyncChanges`, `unlock` guard); `ios-native/Jottery/Jottery/Services/SyncService.swift` (only if routing needs a variant of `sync()`); Test `ios-native/Jottery/JotteryTests/SyncOrchestrationTests.swift`.

**Contract:**
1. `triggerSync()` routes through `syncService.sync()` (which holds the actor's `isSyncing` re-entrancy guard) instead of calling `push()`/`pull()` directly. Preserve `triggerSync`'s current observable behaviour (status flags, error surface, applySyncChanges usage) — read it first and document the mapping.
2. Load-epoch guard: `AppState` gains a monotonically increasing `loadEpoch`; `loadNotes()` and `applySyncChanges()` capture the epoch before their detached work and skip publication if a newer load has started since (`guard epoch == loadEpoch else { return }` on the MainActor after the await). Full reloads bump the epoch; incremental applies do not bump but do check (an incremental result older than a newer full reload must be dropped).
3. `AppState.unlock` gains an internal re-entrancy guard (`isUnlocking`-style state on AppState, checked-and-set at entry, cleared on exit/throw).
4. `forceFullSync` performs exactly one reload (drop the redundant `loadNotes` — keep whichever path reports errors better; justify).

**TDD:** epoch test — start `loadNotes`, then (before it publishes) bump epoch via a second `loadNotes`, assert final arrays reflect the second call's DB state and the count of publications is 1 (instrument with an internal counter or assert on content). A deterministic version: seed 2 notes, call `async let a = loadNotes()` and mutate DB + `async let b = loadNotes()` — timing-dependent tests are flaky; PREFER testing the guard mechanism directly (expose `internal func publishIfCurrent(epoch:...)` or equivalent seam) over racing real tasks. Re-entrancy: call `unlock` twice concurrently with the demo password; assert single unlock effect (second returns early).

- [ ] Tests red → implement → suite green → commit `fix(ios): sync orchestration — single entry point, load-epoch guard, single reload`

---

### Task 3 (jottery-y8s7): shared-inbox import is idempotent

**Files:** Modify `ios-native/Jottery/Jottery/ViewModels/AppState.swift` (`importSharedInboxItems`) and/or `ios-native/Jottery/Shared/SharedInboxStore.swift`; Test `ios-native/Jottery/JotteryTests/SharedInboxIdempotenceTests.swift` (extend existing `AppStateSharedImportTests` harness if present — read it first).

**Contract:** a staged item that fails mid-import (note created, attachment throws) must not produce a duplicate note on retry. Mechanism: write the created note id into the staged item's manifest (or a sidecar marker file in the item directory) immediately after `createNote` succeeds; on retry, if the marker exists and the note exists, resume attachments for that note instead of creating a new one. Keep the "leave item staged for retry" semantics for genuine failures.

**TDD:** simulate the partial failure (stage an item whose attachment file is missing/unreadable so `addAttachment` throws), run import twice, assert exactly one note exists with the item's content.

- [ ] Test red → implement → suite green → commit `fix(ios): shared-inbox import survives partial failure without duplicating notes`

---

### Task 4 (jottery-md6b): SyncSetupView wraps the envelope key with the vault password

**Files:** Modify `ios-native/Jottery/Jottery/Views/SyncSetupView.swift`; possibly `Services/EnvelopeService.swift` (only if a parameter rename clarifies); Test: none automatable for the sheet flow — model-level test only if a seam is touched; the report documents the manual flow.

**Contract:** `tryEnvelopeSetup` must receive the VAULT (notes) password, never the server login password. The view currently has only the server password. Resolution (match the web client's fix in PR #75 and the iOS SetupScreen precedent, which has both fields): add a "Notes password" `SecureField` to SyncSetupView's form, validated non-empty before registration proceeds, passed to `tryEnvelopeSetup`. Verify the entered vault password is CORRECT before using it to wrap: derive + unwrap the local envelope (`AppState`/`KeyManager` has the machinery — find the same verification `unlock` uses) and show an inline error if wrong. Localisation: add the needed `L.` strings + `Localizable.xcstrings` entries (follow the `settings.privacyPolicy` insertion pattern — alphabetical, minimal diff). UI copy should explain: "the password that unlocks your notes — used to protect your encryption key on the server".

**Failure surfacing (part of md6b's blast radius):** `tryEnvelopeSetup`'s catch currently logs only. Make envelope-setup failure during this flow visible in the sheet (inline error text) instead of silent — the user must not believe sync is fully set up when the envelope upload failed. Keep the non-fatal semantics for background/migration call sites.

- [ ] Implement → build + full suite → demo-capture smoke NOT required (sheet not in screenshot set) → commit `fix(ios): sync setup wraps the master key with the vault password (jottery-md6b)`

---

### Task 5 (jottery-l9dt): chunked pushes

**Files:** Modify `ios-native/Jottery/Jottery/Services/SyncService.swift` (`push()`); Test `ios-native/Jottery/JotteryTests/PushChunkingTests.swift` (chunking arithmetic seam) — full network path stays covered by existing behaviour.

**Contract:** `push()` splits `listNeedingSync()` into chunks (default 50 notes per request, constant with a comment; attachments ride with their note as today), sending sequentially; each chunk's acceptances are processed (markSynced etc.) BEFORE the next chunk is sent, so a mid-batch failure preserves the progress of completed chunks (incremental progress on flaky networks — the issue's point). A failed chunk aborts the loop with the error; already-accepted chunks stay marked. `SyncChanges` accumulates across chunks. Extract the chunking decision as a pure testable function (`static func chunked(_ records: [NoteRecord], size: Int) -> [[NoteRecord]]` or reuse Swift's `chunks` equivalent manually — no new dependencies).

**TDD:** pure-function tests (empty, exact multiple, remainder). Service-level behaviour (progress preserved across chunk failure) — only if achievable with the dummy-client harness without new mock infra; otherwise document the manual reasoning per the established pattern and say so.

- [ ] Tests red → implement → suite green → commit `feat(ios): chunked sync pushes preserve progress on flaky networks (jottery-l9dt)`
