# iOS Data-Safety Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the four silent data-loss paths found in the August 2026 iOS review: remote deletions destroying unsynced edits, conflict resolution no-oping on missing rows, "Keep Both" dropping local attachments, `markSynced` clobbering concurrent edits, and `lock()` discarding the pending edit on write failure.

**Architecture:** All fixes live in the sync/persistence layer (`SyncService.swift`, `NoteRepository.swift`, `ConflictInfo.swift`, `AppState.swift`), each gated by a repo-level or service-level unit test in `JotteryTests` (Swift Testing, app-hosted, real GRDB on a temp path — follow the harness in `AppStateListRefreshTests.makeUnlockedState()`).

**Tech Stack:** Swift 6 strict concurrency, GRDB, Swift Testing.

## Global Constraints

- Beads issues: jottery-xayz (Tasks 1–2), jottery-uocc (Task 3), jottery-nlbc (Task 4), jottery-efqh (Task 5).
- TDD per task: failing test first, then the fix. Test command (from `ios-native/Jottery`):
  `xcodebuild -project Jottery.xcodeproj -scheme Jottery -destination 'platform=iOS Simulator,name=iPhone 17' test`
- Do NOT touch `project.pbxproj`. New test files inside `JotteryTests/` auto-join the target (file-system-synchronized groups).
- Semantics rule for every fix: **when in doubt, keep the user's data** — a note that should not exist is recoverable from the recycle bin/server; a destroyed edit is not.
- `SyncService` is constructible without network activity for `resolveConflict` tests: `SyncClient(endpoint:)` performs no I/O at init (verify; if it does, add a test-only convenience init — smallest possible change).
- British English in comments/docs.

---

### Task 1: Remote deletions must not destroy unsynced local edits

**Files:**
- Modify: `ios-native/Jottery/Jottery/Database/NoteRepository.swift` (add `hardDeleteIfSynced`)
- Modify: `ios-native/Jottery/Jottery/Services/SyncService.swift:396-404` (deletions loop)
- Test: `ios-native/Jottery/JotteryTests/SyncDeletionSafetyTests.swift`

**Interfaces:**
- Produces: `NoteRepository.hardDeleteIfSynced(id: String) throws -> Bool` — deletes the row only when `needs_sync = 0`; returns whether it deleted. Task 2 does not depend on it, but the SyncService deletions loop switches to it.

- [ ] **Step 1: Write the failing tests**

`SyncDeletionSafetyTests.swift` (mirror `AppStateListRefreshTests`' temp-DB harness, but repo-level — no AppState needed):

```swift
import CryptoKit
import Foundation
import Testing

@testable import Jottery

struct SyncDeletionSafetyTests {

    private func makeRepo() throws -> NoteRepository {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-deletion-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        return NoteRepository(db: db)
    }

    @Test func deletesNoteWithNoPendingChanges() throws {
        let repo = try makeRepo()
        let key = SymmetricKey(data: Data(repeating: 7, count: 32))
        let note = try repo.create(content: "synced note", key: key)
        try repo.markSynced(id: note.id, syncedAt: Date().iso8601)

        let deleted = try repo.hardDeleteIfSynced(id: note.id)

        #expect(deleted == true)
        #expect(try repo.getRaw(id: note.id) == nil)
    }

    @Test func keepsNoteWithUnsyncedEdits() throws {
        let repo = try makeRepo()
        let key = SymmetricKey(data: Data(repeating: 7, count: 32))
        var note = try repo.create(content: "original", key: key)
        try repo.markSynced(id: note.id, syncedAt: Date().iso8601)
        note.content = "offline edit the user must not lose"
        try repo.update(note, key: key)   // sets needs_sync = 1

        let deleted = try repo.hardDeleteIfSynced(id: note.id)

        #expect(deleted == false)
        let survivor = try #require(try repo.getRaw(id: note.id))
        #expect(survivor.needsSync == true)
    }
}
```

Adapt constructor calls to the actual `DatabaseManager`/`NoteRepository` initialisers (check how `AppState.initialise` builds them) — the test intent is fixed, the plumbing may need adjusting.

- [ ] **Step 2: Run to verify failure** — expect "no member 'hardDeleteIfSynced'".
- [ ] **Step 3: Implement**

In `NoteRepository.swift`, next to `hardDelete` (line 262):

```swift
/// Hard-delete only when the note has no unsynced local changes.
/// A remote deletion must never destroy an edit the server has not seen —
/// the surviving note re-pushes and is resurrected server-side.
/// Returns true if the row was deleted.
func hardDeleteIfSynced(id: String) throws -> Bool {
    try db.dbPool.write { db in
        try db.execute(
            sql: "DELETE FROM notes WHERE id = ? AND needs_sync = 0",
            arguments: [id]
        )
        return db.changesCount > 0
    }
}
```

In `SyncService.swift` deletions loop (replace the `hardDelete` call):

```swift
let deleted = try noteRepo.hardDeleteIfSynced(id: deletion.id)
if !deleted {
    Log.debug("[Sync] pull: kept note \(deletion.id) — local unsynced edit outranks remote deletion")
}
```

- [ ] **Step 4: Tests pass; run full suite.**
- [ ] **Step 5: Commit** — `fix(ios): remote deletions no longer destroy unsynced local edits`

---

### Task 2: resolveConflict must honour the user's choice when the local row is missing

**Files:**
- Modify: `ios-native/Jottery/Jottery/Services/SyncService.swift:620-707` (`resolveConflict`)
- Test: `ios-native/Jottery/JotteryTests/ConflictResolutionSafetyTests.swift`

**Interfaces:**
- Consumes: `ConflictInfo` (has decrypted `localContent`/`localTags`), `CryptoService.encryptText/encryptStringArray/serializeEncryptedJSON`, `NoteRecord.new(encryptedContent:encryptedTags:)`, `noteRepo.insertOrReplace`.
- Behaviour contract: `.keepLocal` with a missing row **recreates** the note from the conflict's local snapshot (`needsSync = true`); `.keepServer` with a missing row recreates it from the server payload (`needsSync = false`, `syncedAt` stamped); `.keepBoth` recreates both. In every case the conflict is removed only after the store reflects the choice.

- [ ] **Step 1: Write failing tests** — construct a `SyncService` with real repos on a temp DB and a `SyncClient` pointing at an unused endpoint (no calls are made by `resolveConflict`); append a hand-built `ConflictInfo` to `pendingConflicts` for a note id that has **no row**; resolve with `.keepLocal`; assert a row now exists whose decrypted content equals `localContent` and `needsSync == true`, and `pendingConflicts` is empty. Second test: `.keepServer` on missing row → row exists with `content == serverEncryptedContent`, `needsSync == false`. Third: `.keepBoth` on missing row → two rows (server-content original id + duplicate with local content).

- [ ] **Step 2: Verify failure** — currently the `guard … else { break }` paths mean no row is created; the asserts on row existence fail.

- [ ] **Step 3: Implement** — in each case, replace `guard let record = try noteRepo.getRaw(id: noteId) else { break }` with a fallback that synthesises the starting record. For `.keepLocal`:

```swift
let record: NoteRecord
if let existing = try noteRepo.getRaw(id: noteId) {
    record = existing
} else {
    // Row vanished (e.g. a remote deletion raced the conflict) — the user
    // chose to keep their local version, so recreate it from the snapshot.
    let encContent = try CryptoService.encryptText(conflict.localContent, key: key)
    let encTags = try CryptoService.encryptStringArray(conflict.localTags, key: key)
    var recreated = NoteRecord.new(
        encryptedContent: try CryptoService.serializeEncryptedJSON(encContent),
        encryptedTags: try CryptoService.serializeEncryptedJSON(encTags)
    )
    recreated.id = noteId
    recreated.modifiedAt = conflict.localModifiedAt
    record = recreated
}
```

(`NoteRecord.id` is `let` in some layouts — if so, build the record with an explicit memberwise init instead; keep the change minimal.) Then the existing mutation path continues unchanged. Mirror for `.keepServer` (start from a recreated record and apply the server fields exactly as the existing code does) and `.keepBoth` (server side as `.keepServer`, duplicate as today). Keep the final `pendingConflicts.remove(at:)` unconditional — after this change every path genuinely applied the choice.

- [ ] **Step 4: Tests pass; full suite.**
- [ ] **Step 5: Commit** — `fix(ios): conflict resolution recreates the note when the local row is missing`

---

### Task 3: markSynced must not clobber edits made during the push round-trip

**Files:**
- Modify: `ios-native/Jottery/Jottery/Database/NoteRepository.swift:323-340`
- Modify: `ios-native/Jottery/Jottery/Services/SyncService.swift` (push loop's `markSynced` call — pass the pushed snapshot's `modifiedAt`)
- Test: `ios-native/Jottery/JotteryTests/MarkSyncedGuardTests.swift`

**Interfaces:**
- Produces: `markSynced(id:syncedAt:serverVersion:ifModifiedAt:)` where `ifModifiedAt` is the `modified_at` of the record as it was pushed. `synced_at`/`version` always update; `needs_sync` is cleared **only** when `modified_at` still equals `ifModifiedAt`. Existing 3-arg callers (if any beyond push) keep working via a default `ifModifiedAt: String? = nil` meaning "unconditional" — but the push loop must pass it.

- [ ] **Step 1: Failing test** — create note (repo-level harness as Task 1); capture `t1 = record.modifiedAt`; simulate a user edit (`repo.update` → `needs_sync = 1`, new `modified_at`); call `markSynced(id:, syncedAt:, serverVersion: 2, ifModifiedAt: t1)`; assert `needsSync == true` still (the edit survives) and `version == 2`. Second test: no intervening edit → `needsSync == false`.
- [ ] **Step 2: Verify failure.**
- [ ] **Step 3: Implement** — two-statement write inside one GRDB transaction:

```swift
func markSynced(id: String, syncedAt: String, serverVersion: Int? = nil, ifModifiedAt: String? = nil) throws {
    try db.dbPool.write { db in
        if let serverVersion {
            try db.execute(sql: "UPDATE notes SET synced_at = ?, version = ? WHERE id = ?",
                           arguments: [syncedAt, serverVersion, id])
        } else {
            try db.execute(sql: "UPDATE notes SET synced_at = ? WHERE id = ?",
                           arguments: [syncedAt, id])
        }
        if let ifModifiedAt {
            try db.execute(sql: "UPDATE notes SET needs_sync = 0 WHERE id = ? AND modified_at = ?",
                           arguments: [id, ifModifiedAt])
        } else {
            try db.execute(sql: "UPDATE notes SET needs_sync = 0 WHERE id = ?", arguments: [id])
        }
    }
}
```

In `SyncService.push`, the accepted-notes loop already iterates the pushed `records` snapshot — look up the pushed record for `accepted.id` and pass its `modifiedAt` as `ifModifiedAt`.

- [ ] **Step 4: Tests pass; full suite.**
- [ ] **Step 5: Commit** — `fix(ios): edits made during a push round-trip are no longer stranded as synced`

---

### Task 4: lock() must not discard the pending edit on write failure

**Files:**
- Modify: `ios-native/Jottery/Jottery/ViewModels/AppState.swift:443-447` (lock flush) and the unlock success path (re-flush)
- Test: `ios-native/Jottery/JotteryTests/PendingEditorFlushTests.swift`

**Interfaces:**
- Behaviour contract: on lock, attempt the flush; clear `pendingEditorNote` **only on success**. On failure, retain it in memory (plaintext survives lock — acceptable: it is the user's own unsaved edit, and losing it is worse; note this trade-off in a comment). On the next successful unlock, if `pendingEditorNote` is non-nil, retry the flush before `loadNotes()` and clear it on success.

- [ ] **Step 1: Failing test** — using the AppState temp-DB harness (`AppStateListRefreshTests.makeUnlockedState` pattern): create a note, set `appState.pendingEditorNote` to an edited copy, call `appState.lock()`, then unlock (`keyManager.unlockWithKeyData` + whatever re-init the harness needs) and assert the DB row contains the edited content — i.e. the pending flush persisted (this passes today via the lock-time flush; it pins the behaviour). Second test (the actual regression case): make the lock-time write fail by pointing the pending note at content that cannot be written — if no clean failure injection exists, restructure minimally: extract the flush into `internal func flushPendingEditorNote() -> Bool` and test that a failure (e.g. repo deallocated/nil) leaves `pendingEditorNote` non-nil.
- [ ] **Step 2: Verify the second test fails** (today the pending note is nilled unconditionally).
- [ ] **Step 3: Implement:**

```swift
/// Flush the in-flight editor state. Returns true when persisted.
/// Kept in memory on failure — the plaintext of the user's own unsaved
/// edit is a smaller risk than silently destroying it at lock time; the
/// retry happens on the next successful unlock.
@discardableResult
internal func flushPendingEditorNote() -> Bool {
    guard let pending = pendingEditorNote else { return true }
    guard let noteRepo, let key = keyManager.masterKey else { return false }
    do {
        try noteRepo.update(pending, key: key)
        pendingEditorNote = nil
        return true
    } catch {
        Log.debug("[Lock] failed to flush pending edit: \(error)")
        return false
    }
}
```

Call it from `lock()` in place of the `try?` block, and from the unlock success path (after the key is set, before `loadNotes()`).

- [ ] **Step 4: Tests pass; full suite.**
- [ ] **Step 5: Commit** — `fix(ios): retain unsaved edit when the lock-time flush fails, retry on unlock`

---

### Task 5: Keep Both must preserve attachments on both notes; Keep Server must apply server attachments

**Files:**
- Modify: `ios-native/Jottery/Jottery/Models/ConflictInfo.swift` (add `localAttachments: [AttachmentRef]`)
- Modify: `ios-native/Jottery/Jottery/Services/SyncService.swift` — conflict creation site (`processNote`, ~:543-615) captures local refs; `resolveConflict` applies attachments per strategy
- Test: `ios-native/Jottery/JotteryTests/ConflictAttachmentTests.swift`

**Interfaces:**
- Consumes: how `NoteRecord.attachments` encodes refs (JSON string of `[AttachmentRef]` — verify the exact encode/decode helpers used by pull; reuse them, do not hand-roll).
- Behaviour contract: `.keepServer` → note's attachment refs become `conflict.serverAttachments`. `.keepLocal` → refs stay local (already the case — verify and pin with a test). `.keepBoth` → original note gets `serverAttachments`; the duplicate gets the local refs.
- **Blob-sharing decision (investigate before coding):** if `AttachmentRepository` deletes blobs when a note is deleted (cascade by ref), two notes sharing blob ids is unsafe → the duplicate must copy each blob under a new id and rewrite its refs. If blobs are only ever deleted by explicit id with no cascade, sharing ids is acceptable — but check what sync push does with a duplicate note referencing already-known blob ids. State the finding in the task report; implement whichever the evidence supports, preferring blob copies if uncertain.

- [ ] **Step 1: Failing tests** — build a conflict for a note that has one local attachment ref (and a stored blob); resolve `.keepBoth`; assert the duplicate row's decoded attachment refs are non-empty and its blob data is retrievable; assert the original row's refs equal `serverAttachments`. Second test: `.keepServer` applies `serverAttachments`.
- [ ] **Step 2: Verify failure** (duplicate currently has `[]`).
- [ ] **Step 3: Implement** per the contract above, including the `ConflictInfo` field and its population at the conflict-creation site.
- [ ] **Step 4: Tests pass; full suite.**
- [ ] **Step 5: Commit** — `fix(ios): conflict resolution preserves attachments on both notes`
